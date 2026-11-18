//! Skin: libreria locale (`skins/` nella cartella dati), skin scelta per
//! ogni account e applicazione in gioco per gli account offline.
//!
//! Con un account offline Minecraft non scarica skin: mostra una delle skin
//! predefinite, scelta dall'UUID. Per vedere la propria skin il launcher
//! crea un resource pack (`hyper-skin.zip`) che sostituisce le skin
//! predefinite con quella scelta e lo attiva nell'istanza. Con gli account
//! Microsoft la skin è quella del profilo Mojang e il pack viene tolto.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use hyper_core::{CoreError, Paths, fs};
use serde::{Deserialize, Serialize};

use crate::{ContentDir, Instance, InstanceError, Result};

/// Skin predefinite di Minecraft (dalla 1.19.3), presenti nel client jar.
pub const DEFAULT_SKINS: [&str; 9] = [
    "steve", "alex", "zuri", "sunny", "noor", "makena", "kai", "efe", "ari",
];

const PACK_FILE: &str = "hyper-skin.zip";
const MAX_SKIN_BYTES: usize = 256 * 1024;
const INDEX_FILE: &str = "skins.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skin {
    pub id: String,
    pub name: String,
    /// Modello con le braccia sottili ("Alex").
    pub slim: bool,
    pub added: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    skins: Vec<Skin>,
    /// UUID dell'account → id della skin scelta.
    selected: BTreeMap<String, String>,
}

/// Controlla che i byte siano una skin PNG valida (64×64, o 64×32 per il
/// vecchio formato) e restituisce l'altezza.
pub fn validate_skin(png: &[u8]) -> Result<u32> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let bad = |why: &str| InstanceError::InvalidImage(why.to_owned());
    if png.len() > MAX_SKIN_BYTES {
        return Err(bad("file too large for a skin"));
    }
    if png.len() < 24 || !png.starts_with(SIGNATURE) || &png[12..16] != b"IHDR" {
        return Err(bad("not a PNG image"));
    }
    let width = u32::from_be_bytes(png[16..20].try_into().expect("4 bytes"));
    let height = u32::from_be_bytes(png[20..24].try_into().expect("4 bytes"));
    if width != 64 || !(height == 64 || height == 32) {
        return Err(bad(&format!(
            "a skin must be 64×64 pixels (this one is {width}×{height})"
        )));
    }
    Ok(height)
}

/// Libreria delle skin salvate.
#[derive(Debug, Clone)]
pub struct SkinLibrary {
    dir: PathBuf,
}

impl SkinLibrary {
    pub fn new(paths: &Paths) -> Self {
        Self {
            dir: paths.root().join("skins"),
        }
    }

    fn index(&self) -> Index {
        std::fs::read(self.dir.join(INDEX_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save(&self, index: &Index) -> Result<()> {
        let json = serde_json::to_vec_pretty(index).expect("index serializes");
        Ok(fs::write_atomic(&self.dir.join(INDEX_FILE), &json)?)
    }

    fn png_path(&self, id: &str) -> Result<PathBuf> {
        Ok(self
            .dir
            .join(format!("{}.png", hyper_core::safe_component(id)?)))
    }

    pub fn list(&self) -> Vec<Skin> {
        self.index().skins
    }

    pub fn png(&self, id: &str) -> Result<Vec<u8>> {
        Ok(fs::read(&self.png_path(id)?)?)
    }

    /// Aggiunge una skin (PNG 64×64) e la restituisce.
    pub fn add(&self, name: &str, png: &[u8], slim: bool) -> Result<Skin> {
        validate_skin(png)?;
        let mut index = self.index();
        let now = hyper_core::unix_now();
        // Id stabile e senza caratteri strani: tempo + contatore.
        let mut n = 0;
        let id = loop {
            let id = format!("skin-{now}-{n}");
            if !index.skins.iter().any(|s| s.id == id) {
                break id;
            }
            n += 1;
        };
        let name = name.trim();
        let skin = Skin {
            name: if name.is_empty() {
                "Skin".to_owned()
            } else {
                name.chars().take(40).collect()
            },
            id,
            slim,
            added: now,
        };
        fs::write_atomic(&self.png_path(&skin.id)?, png)?;
        index.skins.push(skin.clone());
        self.save(&index)?;
        Ok(skin)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let mut index = self.index();
        index.skins.retain(|s| s.id != id);
        index.selected.retain(|_, s| s != id);
        self.save(&index)?;
        let path = self.png_path(id)?;
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(CoreError::io(&path, e).into())
            }
            _ => Ok(()),
        }
    }

    /// Sceglie la skin di un account (`None` = nessuna, si torna a quella
    /// predefinita del gioco).
    pub fn select(&self, account: &str, id: Option<&str>) -> Result<()> {
        let mut index = self.index();
        match id {
            Some(id) => {
                if !index.skins.iter().any(|s| s.id == id) {
                    return Err(InstanceError::NotFound(id.to_owned()));
                }
                index.selected.insert(account.to_owned(), id.to_owned());
            }
            None => {
                index.selected.remove(account);
            }
        }
        self.save(&index)
    }

    pub fn selected(&self, account: &str) -> Option<Skin> {
        let index = self.index();
        let id = index.selected.get(account)?;
        index.skins.into_iter().find(|s| &s.id == id)
    }
}

/// Skin predefinite lette da un client jar già scaricato (le texture sono
/// di Mojang: il launcher non le distribuisce, le prende dal gioco).
/// Restituisce `(nome, sottile, png)`; vuoto se nessuna versione recente è
/// installata.
pub fn default_skins(paths: &Paths) -> Vec<(String, bool, Vec<u8>)> {
    let Ok(entries) = std::fs::read_dir(paths.versions_dir()) else {
        return Vec::new();
    };
    let mut jars: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let jar = e.path().join(format!("{id}.jar"));
            let modified = std::fs::metadata(&jar).ok()?.modified().ok()?;
            Some((modified, jar))
        })
        .collect();
    jars.sort_by_key(|j| std::cmp::Reverse(j.0));
    jars.into_iter()
        .find_map(|(_, jar)| read_default_skins(&jar))
        .unwrap_or_default()
}

fn read_default_skins(jar: &Path) -> Option<Vec<(String, bool, Vec<u8>)>> {
    let file = std::fs::File::open(jar).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut skins = Vec::new();
    for name in DEFAULT_SKINS {
        // Ogni skin predefinita ha una variante per modello: si mostra
        // quella "classica" di ciascuna (Steve larga, Alex sottile…).
        let slim = matches!(name, "alex" | "makena" | "noor" | "sunny" | "zuri" | "efe");
        let model = if slim { "slim" } else { "wide" };
        let path = format!("assets/minecraft/textures/entity/player/{model}/{name}.png");
        let mut entry = archive.by_name(&path).ok()?;
        let mut png = Vec::new();
        entry.read_to_end(&mut png).ok()?;
        skins.push((name.to_owned(), slim, png));
    }
    Some(skins)
}

/// Mette (o toglie, con `None`) il resource pack della skin nell'istanza e
/// lo attiva in `options.txt`. Se l'istanza non ha ancora un `options.txt`
/// il pack viene solo copiato: si attiva dalla partita successiva.
pub fn apply_to_instance(instance: &Instance, skin_png: Option<&[u8]>) -> Result<()> {
    let pack = instance
        .content_dir(ContentDir::ResourcePacks)
        .join(PACK_FILE);
    // Prima della 1.13 i resource pack attivi si scrivono in un altro
    // formato (senza `file/`): lì la skin resta quella predefinita.
    let skin_png = skin_png.filter(|_| hyper_sync::supports_game(&instance.config.version));
    let Some(png) = skin_png else {
        if pack.exists() {
            std::fs::remove_file(&pack).map_err(|e| CoreError::io(&pack, e))?;
        }
        return Ok(());
    };
    write_pack(&pack, png)?;
    enable_pack(&instance.options_file())
}

fn write_pack(pack: &Path, png: &[u8]) -> Result<()> {
    let zip_err = |e: zip::result::ZipError| CoreError::io(pack, std::io::Error::other(e));
    let io_err = |e| CoreError::io(pack, e);
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    // Intervallo di formati amplissimo: il pack contiene solo texture, che
    // valgono per tutte le versioni.
    let meta = serde_json::json!({"pack": {
        "pack_format": 34,
        "supported_formats": [1, 999],
        "min_format": 1,
        "max_format": 999,
        "description": "Skin scelta in Hyper Launcher"
    }});
    zip.start_file("pack.mcmeta", options).map_err(zip_err)?;
    zip.write_all(meta.to_string().as_bytes()).map_err(io_err)?;
    let mut files: Vec<String> = Vec::new();
    for model in ["wide", "slim"] {
        for name in DEFAULT_SKINS {
            files.push(format!(
                "assets/minecraft/textures/entity/player/{model}/{name}.png"
            ));
        }
    }
    // Versioni prima della 1.19.3.
    files.push("assets/minecraft/textures/entity/steve.png".into());
    files.push("assets/minecraft/textures/entity/alex.png".into());
    for file in files {
        zip.start_file(file, options).map_err(zip_err)?;
        zip.write_all(png).map_err(io_err)?;
    }
    let bytes = zip.finish().map_err(zip_err)?.into_inner();
    Ok(fs::write_atomic(pack, &bytes)?)
}

/// Aggiunge `file/hyper-skin.zip` ai resource pack attivi (in cima, così
/// vince sugli altri).
fn enable_pack(options_path: &Path) -> Result<()> {
    let Ok(text) = std::fs::read_to_string(options_path) else {
        return Ok(());
    };
    let mut options = hyper_sync::OptionsFile::parse(&text);
    let entry = format!("file/{PACK_FILE}");
    let list = |options: &hyper_sync::OptionsFile, key: &str| -> Vec<String> {
        options
            .get(key)
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    };
    let mut changed = false;
    // L'ultimo di `resourcePacks` è quello con priorità più alta. Il pack va
    // anche tra gli "incompatibili accettati": per le versioni che non ne
    // riconoscono il formato il gioco altrimenti lo toglierebbe all'avvio.
    for key in ["resourcePacks", "incompatibleResourcePacks"] {
        let mut packs = list(&options, key);
        if !packs.contains(&entry) {
            packs.push(entry.clone());
            options.set(key, serde_json::to_string(&packs).expect("strings"));
            changed = true;
        }
    }
    if !changed {
        return Ok(());
    }
    Ok(fs::write_atomic(
        options_path,
        options.to_string().as_bytes(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InstanceManager;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn validates_skin_size_and_format() {
        assert_eq!(validate_skin(&png(64, 64)).unwrap(), 64);
        assert_eq!(validate_skin(&png(64, 32)).unwrap(), 32);
        assert!(validate_skin(&png(128, 128)).is_err());
        assert!(validate_skin(b"GIF89a....................").is_err());
    }

    #[test]
    fn library_add_select_remove() {
        let dir = tempfile::tempdir().unwrap();
        let lib = SkinLibrary::new(&Paths::new(dir.path()));
        let skin = lib.add("  Mia skin ", &png(64, 64), true).unwrap();
        assert_eq!(skin.name, "Mia skin");
        assert_eq!(lib.list().len(), 1);
        assert_eq!(lib.png(&skin.id).unwrap(), png(64, 64));
        lib.select("uuid-1", Some(&skin.id)).unwrap();
        assert_eq!(lib.selected("uuid-1").unwrap().id, skin.id);
        assert!(lib.select("uuid-1", Some("missing")).is_err());
        assert!(lib.png("../skins").is_err(), "ids are path components");
        lib.remove(&skin.id).unwrap();
        assert!(lib.list().is_empty());
        assert!(lib.selected("uuid-1").is_none());
    }

    #[test]
    fn applies_and_removes_the_skin_pack() {
        let dir = tempfile::tempdir().unwrap();
        let instance = InstanceManager::new(Paths::new(dir.path()))
            .create("s", "26.3", None)
            .unwrap();
        std::fs::write(
            instance.options_file(),
            "version:5023\nresourcePacks:[\"vanilla\"]\n",
        )
        .unwrap();
        apply_to_instance(&instance, Some(&png(64, 64))).unwrap();
        let pack = instance
            .content_dir(ContentDir::ResourcePacks)
            .join(PACK_FILE);
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&pack).unwrap()).unwrap();
        assert!(
            archive
                .by_name("assets/minecraft/textures/entity/player/slim/alex.png")
                .is_ok()
        );
        let options = std::fs::read_to_string(instance.options_file()).unwrap();
        assert!(
            options.contains(r#"resourcePacks:["vanilla","file/hyper-skin.zip"]"#),
            "{options}"
        );
        assert!(
            options.contains(r#"incompatibleResourcePacks:["file/hyper-skin.zip"]"#),
            "{options}"
        );
        // Una seconda volta non duplica la voce.
        apply_to_instance(&instance, Some(&png(64, 64))).unwrap();
        let again = std::fs::read_to_string(instance.options_file()).unwrap();
        assert_eq!(again.matches("hyper-skin").count(), 2, "one entry per list");
        apply_to_instance(&instance, None).unwrap();
        assert!(!pack.exists());
    }
}
