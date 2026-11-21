//! Pacchetti, mondi e modelli di un'installazione: elenco, importazione dei
//! file scaricati (`.mcpack`, `.mcaddon`, `.mcworld`, `.mctemplate`, `.zip`)
//! ed eliminazione.

use std::collections::BTreeSet;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use hyper_core::{CoreError, fs, safe_component};
use hyper_installer::natives::extract_zip_mapped;
use serde::Serialize;

use crate::{BedrockError, Location, Result, Scope};

/// Tipo di contenuto, cioè la scheda dell'installazione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackKind {
    /// Pacchetti risorse (texture, suoni).
    Resource,
    /// Pacchetti comportamento: gli add-on.
    Behavior,
    Skin,
    World,
    /// Modelli di mondo.
    Template,
}

impl PackKind {
    fn folder(self) -> &'static str {
        match self {
            Self::Resource => "resource_packs",
            Self::Behavior => "behavior_packs",
            Self::Skin => "skin_packs",
            Self::World => "minecraftWorlds",
            Self::Template => "world_templates",
        }
    }

    fn scopes(self) -> &'static [Scope] {
        match self {
            Self::Resource | Self::Behavior | Self::Skin => &[Scope::Shared, Scope::User],
            Self::World => &[Scope::User, Scope::Shared],
            Self::Template => &[Scope::User],
        }
    }

    /// Dove finiscono i file importati.
    fn import_scope(self) -> Scope {
        self.scopes()[0]
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub kind: PackKind,
    pub scope: Scope,
    /// Nome della cartella (identifica il pacchetto per l'eliminazione).
    pub folder: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub uuid: Option<String>,
    /// `pack_icon.png` o `world_icon.jpeg`, se non troppo grande.
    #[serde(skip)]
    pub icon: Option<Vec<u8>>,
    pub modified: u64,
}

/// Esito di un'importazione, per il messaggio all'utente.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub kind: PackKind,
    pub name: String,
    /// `true` se ha sostituito una versione già presente.
    pub replaced: bool,
}

const MAX_ICON: u64 = 512 * 1024;
/// Limite al contenuto estratto da un file (contro gli zip "bomba").
const MAX_UNPACKED: u64 = 4 * 1024 * 1024 * 1024;
/// Limite a un archivio annidato in un `.mcaddon`.
const MAX_NESTED: u64 = 1024 * 1024 * 1024;

// --- Elenco ---

/// Contenuti di un tipo, ordinati per nome.
pub fn list(location: &Location, kind: PackKind) -> Vec<Pack> {
    let mut packs = Vec::new();
    for &scope in kind.scopes() {
        let Some(dir) = location.folder(scope, kind.folder()) else {
            continue;
        };
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let folder = entry.file_name().to_string_lossy().into_owned();
            if folder.starts_with(".hyper-") {
                continue;
            }
            if let Some(pack) = read_pack(&path, kind, scope, folder) {
                packs.push(pack);
            }
        }
    }
    packs.sort_by_key(|p| p.name.to_lowercase());
    packs
}

fn read_pack(path: &Path, kind: PackKind, scope: Scope, folder: String) -> Option<Pack> {
    let modified_of = |p: &Path| {
        p.metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs())
    };
    let icon = |name: &str| {
        let p = path.join(name);
        p.metadata()
            .is_ok_and(|m| m.len() <= MAX_ICON)
            .then(|| std::fs::read(p).ok())
            .flatten()
    };
    if kind == PackKind::World {
        if !path.join("level.dat").is_file() {
            return None;
        }
        let name = std::fs::read_to_string(path.join("levelname.txt"))
            .map(|n| clean_text(n.trim()))
            .ok()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| folder.clone());
        return Some(Pack {
            kind,
            scope,
            name,
            description: String::new(),
            version: None,
            uuid: None,
            icon: icon("world_icon.jpeg"),
            modified: modified_of(&path.join("level.dat")),
            folder,
        });
    }
    let manifest = Manifest::read(&std::fs::read(path.join("manifest.json")).ok()?)?;
    let lang = |key: &str| lookup_lang(path, key);
    Some(Pack {
        kind,
        scope,
        name: manifest
            .display(&manifest.name, &lang)
            .unwrap_or_else(|| folder.clone()),
        description: manifest
            .display(&manifest.description, &lang)
            .unwrap_or_default(),
        version: manifest.version,
        uuid: manifest.uuid,
        icon: icon("pack_icon.png"),
        modified: modified_of(&path.join("manifest.json")),
        folder,
    })
}

/// Nomi tradotti: `pack.name` → `texts/<lingua>.lang` (`pack.name=...`).
fn lookup_lang(dir: &Path, key: &str) -> Option<String> {
    ["it_IT", "en_US"].iter().find_map(|lang| {
        let text = std::fs::read_to_string(dir.join("texts").join(format!("{lang}.lang"))).ok()?;
        lang_value(&text, key)
    })
}

/// Toglie i codici colore (`§a`) dei nomi di Minecraft.
fn clean_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out.trim().to_owned()
}

// --- Manifest ---

#[derive(Debug, Default)]
struct Manifest {
    name: String,
    description: String,
    uuid: Option<String>,
    version: Option<String>,
    /// Tipi dei moduli: `resources`, `data`, `script`, `skin_pack`,
    /// `world_template`…
    modules: Vec<String>,
}

impl Manifest {
    fn read(bytes: &[u8]) -> Option<Self> {
        let text = String::from_utf8_lossy(bytes);
        let text = text.trim_start_matches('\u{feff}');
        // Molti pacchetti hanno commenti nel JSON (il gioco li accetta).
        let value: serde_json::Value = serde_json::from_str(text)
            .or_else(|_| serde_json::from_str(&strip_comments(text)))
            .ok()?;
        let header = value.get("header")?;
        let text_of = |v: Option<&serde_json::Value>| {
            v.and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let version = header.get("version").and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Array(parts) => Some(
                parts
                    .iter()
                    .map(|p| p.as_u64().map_or_else(|| "0".into(), |n| n.to_string()))
                    .collect::<Vec<_>>()
                    .join("."),
            ),
            _ => None,
        });
        let modules = value
            .get("modules")
            .and_then(serde_json::Value::as_array)
            .map(|m| m.iter().map(|m| text_of(m.get("type"))).collect())
            .unwrap_or_default();
        Some(Self {
            name: text_of(header.get("name")),
            description: text_of(header.get("description")),
            uuid: header
                .get("uuid")
                .and_then(|u| u.as_str())
                .map(str::to_lowercase),
            version,
            modules,
        })
    }

    /// Tipo di pacchetto dai moduli.
    fn kind(&self) -> Option<PackKind> {
        let has = |t: &str| self.modules.iter().any(|m| m == t);
        if has("world_template") {
            Some(PackKind::Template)
        } else if has("skin_pack") {
            Some(PackKind::Skin)
        } else if has("data") || has("script") || has("client_data") {
            Some(PackKind::Behavior)
        } else if has("resources") {
            Some(PackKind::Resource)
        } else {
            None
        }
    }

    /// Testo da mostrare: le chiavi `pack.name`/`pack.description` si
    /// traducono con i file `.lang` del pacchetto.
    fn display(&self, text: &str, lang: &dyn Fn(&str) -> Option<String>) -> Option<String> {
        let text = if text.starts_with("pack.") && !text.contains(' ') {
            lang(text)?
        } else {
            text.to_owned()
        };
        Some(clean_text(&text)).filter(|t| !t.is_empty())
    }
}

/// Toglie i commenti `//` e `/* */` fuori dalle stringhe.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

// --- Importazione ---

/// Importa un file scaricato nell'installazione: ogni pacchetto o mondo che
/// contiene va nella sua cartella. Un pacchetto con lo stesso uuid di uno
/// già presente lo sostituisce (aggiornamento).
pub fn import(location: &Location, file: &Path) -> Result<Vec<Imported>> {
    let reader = std::fs::File::open(file).map_err(|e| CoreError::io(file, e))?;
    let mut imported = Vec::new();
    import_archive(location, reader, file, 0, &mut imported)?;
    if imported.is_empty() {
        return Err(BedrockError::Invalid(
            "il file non contiene pacchetti né mondi di Minecraft Bedrock".into(),
        ));
    }
    Ok(imported)
}

fn zip_err(file: &Path, e: zip::result::ZipError) -> BedrockError {
    BedrockError::Invalid(format!("{} non è un archivio valido: {e}", file.display()))
}

/// Una radice dell'archivio da estrarre.
struct Job {
    root: String,
    kind: PackKind,
    name: String,
    uuid: Option<String>,
}

fn import_archive<R: Read + Seek>(
    location: &Location,
    mut reader: R,
    file: &Path,
    depth: u8,
    imported: &mut Vec<Imported>,
) -> Result<()> {
    // Prima si legge l'archivio (nomi, manifest, archivi annidati), poi si
    // estrae ogni radice.
    let (jobs, nested) = {
        let mut archive = zip::ZipArchive::new(&mut reader).map_err(|e| zip_err(file, e))?;
        scan(&mut archive, file, depth)?
    };
    for job in jobs {
        let replaced = place(location, &mut reader, file, &job)?;
        imported.push(Imported {
            kind: job.kind,
            name: job.name,
            replaced,
        });
    }
    for (name, bytes) in nested {
        import_archive(
            location,
            std::io::Cursor::new(bytes),
            &file.join(name),
            depth + 1,
            imported,
        )?;
    }
    Ok(())
}

type Nested = Vec<(String, Vec<u8>)>;

fn scan<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    file: &Path,
    depth: u8,
) -> Result<(Vec<Job>, Nested)> {
    let mut names = Vec::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let (size, name) = {
            let entry = archive.by_index(i).map_err(|e| zip_err(file, e))?;
            (entry.size(), entry.name().map(|n| n.into_owned()))
        };
        total = total.saturating_add(size);
        names.push(name.map_err(|e| zip_err(file, e))?);
    }
    if total > MAX_UNPACKED {
        return Err(BedrockError::Invalid("il file è troppo grande".into()));
    }
    // Nomi con `/` (alcuni zip di Windows usano `\`), con l'originale per
    // leggere l'entry.
    let normal: Vec<String> = names.iter().map(|n| n.replace('\\', "/")).collect();
    let original = |wanted: &str| {
        normal
            .iter()
            .position(|n| n == wanted)
            .map(|i| names[i].clone())
    };

    // Radici: cartelle con `level.dat` (mondi) o `manifest.json` (pacchetti),
    // al massimo due livelli sotto la cima; una radice dentro un'altra fa
    // parte di quella (es. i "subpacks").
    let mut found: BTreeSet<String> = BTreeSet::new();
    for name in &normal {
        let (dir, base) = match name.rsplit_once('/') {
            Some((dir, base)) => (format!("{dir}/"), base),
            None => (String::new(), name.as_str()),
        };
        if matches!(base, "level.dat" | "manifest.json") && dir.matches('/').count() <= 2 {
            found.insert(dir);
        }
    }
    let roots: Vec<String> = found
        .iter()
        .filter(|r| !found.iter().any(|o| o != *r && r.starts_with(o.as_str())))
        .cloned()
        .collect();

    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut entry = archive.by_name(&original(name)?).ok()?;
        let mut bytes = Vec::new();
        (&mut entry)
            .take(1024 * 1024)
            .read_to_end(&mut bytes)
            .ok()?;
        Some(bytes)
    };
    let mut jobs = Vec::new();
    for root in &roots {
        let manifest = read(&format!("{root}manifest.json")).and_then(|b| Manifest::read(&b));
        let has_level = normal.iter().any(|n| *n == format!("{root}level.dat"));
        let kind = match (&manifest, has_level) {
            (Some(m), true) if m.kind() == Some(PackKind::Template) => PackKind::Template,
            (_, true) => PackKind::World,
            (Some(m), false) => match m.kind() {
                Some(kind) if kind != PackKind::Template => kind,
                _ => continue,
            },
            (None, false) => continue,
        };
        let name = match (&manifest, kind) {
            (Some(m), k) if k != PackKind::World => {
                if m.name.starts_with("pack.") && !m.name.contains(' ') {
                    ["it_IT", "en_US"].iter().find_map(|l| {
                        let text = read(&format!("{root}texts/{l}.lang"))?;
                        lang_value(&String::from_utf8_lossy(&text), &m.name)
                    })
                } else {
                    Some(m.name.clone())
                }
            }
            _ => read(&format!("{root}levelname.txt"))
                .map(|b| String::from_utf8_lossy(&b).into_owned()),
        }
        .map(|n| clean_text(n.trim()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            if kind == PackKind::World {
                "Mondo"
            } else {
                "Pacchetto"
            }
            .into()
        });
        jobs.push(Job {
            root: root.clone(),
            kind,
            name,
            uuid: manifest.and_then(|m| m.uuid),
        });
    }

    // Archivi annidati (un `.mcaddon` spesso è fatto di `.mcpack`).
    let mut nested = Vec::new();
    if depth == 0 {
        for (i, name) in normal.iter().enumerate() {
            let lower = name.to_lowercase();
            let is_archive = [".mcpack", ".mcworld", ".mctemplate", ".zip"]
                .iter()
                .any(|ext| lower.ends_with(ext));
            // Un archivio dentro un pacchetto (anche quello alla radice,
            // radice "") fa parte del pacchetto, non è da importare.
            if !is_archive || roots.iter().any(|r| name.starts_with(r.as_str())) {
                continue;
            }
            let mut entry = archive.by_name(&names[i]).map_err(|e| zip_err(file, e))?;
            if entry.size() > MAX_NESTED {
                return Err(BedrockError::Invalid("il file è troppo grande".into()));
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(MAX_NESTED)
                .read_to_end(&mut bytes)
                .map_err(|e| CoreError::io(file, e))?;
            nested.push((name.clone(), bytes));
        }
    }
    Ok((jobs, nested))
}

fn lang_value(text: &str, key: &str) -> Option<String> {
    // Molti file .lang di Windows iniziano con il BOM UTF-8.
    text.trim_start_matches('\u{feff}')
        .lines()
        .find_map(|line| {
            let (k, v) = line.split_once('=')?;
            (k.trim() == key).then(|| v.split('#').next().unwrap_or(v).trim().to_owned())
        })
}

/// Estrae una radice dell'archivio nella cartella del suo tipo, passando da
/// una cartella temporanea accanto (stesso disco: il rename finale è
/// atomico). Restituisce `true` se ha sostituito un pacchetto esistente.
fn place<R: Read + Seek>(
    location: &Location,
    reader: &mut R,
    file: &Path,
    job: &Job,
) -> Result<bool> {
    let kind = job.kind;
    let parent = location
        .folder(kind.import_scope(), kind.folder())
        .ok_or(BedrockError::NoProfile)?;
    fs::create_dir_all(&parent)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let tmp = parent.join(format!(".hyper-import-{id}"));
    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|e| CoreError::io(file, e))?;
    // `extract_zip_mapped` controlla ogni percorso con `safe_join`: niente
    // path traversal anche da nomi d'entry costruiti apposta.
    let root = job.root.as_str();
    if let Err(e) = extract_zip_mapped(&mut *reader, file, &tmp, |name| name.strip_prefix(root)) {
        let _ = fs::remove_dir_all_if_exists(&tmp);
        return Err(e.into());
    }

    // Stesso uuid già presente (in qualunque ambito): è un aggiornamento.
    let existing = job.uuid.as_deref().and_then(|uuid| {
        list(location, kind)
            .into_iter()
            .find(|p| p.uuid.as_deref() == Some(uuid))
            .and_then(|p| {
                location
                    .folder(p.scope, kind.folder())
                    .map(|d| d.join(p.folder))
            })
    });
    let dest = match &existing {
        Some(old) => old.clone(),
        None => unique_dir(&parent, &folder_name(kind, &job.name, &id)),
    };
    if let Some(old) = &existing {
        let trash = parent.join(format!(".hyper-old-{id}"));
        std::fs::rename(old, &trash).map_err(|e| CoreError::io(old, e))?;
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::rename(&trash, old);
            let _ = fs::remove_dir_all_if_exists(&tmp);
            return Err(CoreError::io(&dest, e).into());
        }
        fs::remove_dir_all_if_exists(&trash)?;
    } else {
        std::fs::rename(&tmp, &dest).map_err(|e| {
            let _ = fs::remove_dir_all_if_exists(&tmp);
            CoreError::io(&dest, e)
        })?;
    }
    Ok(existing.is_some())
}

/// Nome di cartella leggibile e sicuro: i mondi usano un id come fa il gioco.
fn folder_name(kind: PackKind, display: &str, id: &str) -> String {
    if matches!(kind, PackKind::World) {
        return id[..12].to_owned();
    }
    let mut name: String = display
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect::<String>()
        .trim()
        .to_owned();
    if safe_component(&name).is_err() || name.is_empty() {
        name = id[..12].to_owned();
    }
    name
}

fn unique_dir(parent: &Path, name: &str) -> PathBuf {
    let mut dest = parent.join(name);
    let mut n = 2;
    while dest.exists() {
        dest = parent.join(format!("{name} ({n})"));
        n += 1;
    }
    dest
}

// --- Eliminazione ---

/// Elimina un pacchetto o un mondo (cartella `folder` del tipo e ambito dati).
pub fn delete_pack(location: &Location, kind: PackKind, scope: Scope, folder: &str) -> Result<()> {
    safe_component(folder)?;
    if !kind.scopes().contains(&scope) {
        return Err(BedrockError::Invalid("contenuto non trovato".into()));
    }
    let dir = location
        .folder(scope, kind.folder())
        .ok_or(BedrockError::NoProfile)?
        .join(folder);
    if !dir.is_dir() {
        return Err(BedrockError::Invalid("contenuto non trovato".into()));
    }
    fs::remove_dir_all_if_exists(&dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::DataDirs;

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut out);
        for (name, bytes) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        out.into_inner()
    }

    fn manifest(name: &str, uuid: &str, module: &str) -> Vec<u8> {
        format!(
            r#"{{
              // commento come in molti pacchetti
              "format_version": 2,
              "header": {{ "name": "{name}", "description": "§aProva", "uuid": "{uuid}", "version": [1, 2, 0] }},
              "modules": [{{ "type": "{module}", "uuid": "x", "version": [1, 0, 0] }}]
            }}"#
        )
        .into_bytes()
    }

    fn location(tmp: &Path) -> Location {
        let dirs = DataDirs {
            shared: tmp.join("Shared/com.mojang"),
            user: Some(tmp.join("123/com.mojang")),
        };
        Location::new(dirs, tmp.join("storage"), true)
    }

    fn import_bytes(
        loc: &Location,
        dir: &Path,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<Vec<Imported>> {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        import(loc, &path)
    }

    #[test]
    fn imports_packs_worlds_and_addons() {
        let tmp = tempfile::tempdir().unwrap();
        let loc = location(tmp.path());

        let rp = zip_of(&[
            ("manifest.json", &manifest("pack.name", "AAAA", "resources")),
            (
                "texts/en_US.lang",
                "\u{feff}pack.name=Texture belle #commento\n".as_bytes(),
            ),
            ("textures/a.png", b"png"),
        ]);
        let done = import_bytes(&loc, tmp.path(), "rp.mcpack", rp).unwrap();
        assert_eq!(done[0].kind, PackKind::Resource);
        assert_eq!(done[0].name, "Texture belle");
        let list_rp = list(&loc, PackKind::Resource);
        assert_eq!(list_rp.len(), 1);
        assert_eq!(list_rp[0].version.as_deref(), Some("1.2.0"));
        assert_eq!(list_rp[0].description, "Prova");
        assert_eq!(list_rp[0].uuid.as_deref(), Some("aaaa"));

        // Un .mcaddon con un pacchetto comportamento in cartella e un .mcpack annidato.
        let nested = zip_of(&[("manifest.json", &manifest("Mobili RP", "CCCC", "resources"))]);
        let addon = zip_of(&[
            (
                "Mobili BP/manifest.json",
                &manifest("Mobili", "BBBB", "data"),
            ),
            ("Mobili BP/entities/sedia.json", b"{}"),
            ("Mobili RP.mcpack", &nested),
        ]);
        let done = import_bytes(&loc, tmp.path(), "mobili.mcaddon", addon).unwrap();
        assert_eq!(done.len(), 2);
        assert_eq!(list(&loc, PackKind::Behavior)[0].name, "Mobili");
        assert_eq!(list(&loc, PackKind::Resource).len(), 2);
        let bp_dir = tmp.path().join("Shared/com.mojang/behavior_packs/Mobili");
        assert!(bp_dir.join("entities/sedia.json").is_file());

        // Mondo.
        let world = zip_of(&[
            ("level.dat", b"nbt"),
            ("levelname.txt", b"Isola"),
            ("db/CURRENT", b"x"),
        ]);
        let done = import_bytes(&loc, tmp.path(), "isola.mcworld", world).unwrap();
        assert_eq!(done[0].kind, PackKind::World);
        let worlds = list(&loc, PackKind::World);
        assert_eq!(worlds[0].name, "Isola");
        assert_eq!(worlds[0].scope, Scope::User);

        // Stesso uuid: sostituisce invece di duplicare.
        let rp2 = zip_of(&[(
            "manifest.json",
            &manifest("Texture belle 2", "AAAA", "resources"),
        )]);
        let done = import_bytes(&loc, tmp.path(), "rp2.mcpack", rp2).unwrap();
        assert!(done[0].replaced);
        let names: Vec<_> = list(&loc, PackKind::Resource)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Mobili RP", "Texture belle 2"]);

        // Elimina.
        let folder = list(&loc, PackKind::World)[0].folder.clone();
        delete_pack(&loc, PackKind::World, Scope::User, &folder).unwrap();
        assert!(list(&loc, PackKind::World).is_empty());
        assert!(delete_pack(&loc, PackKind::World, Scope::User, "../x").is_err());
    }

    #[test]
    fn archives_inside_a_pack_stay_in_the_pack() {
        let tmp = tempfile::tempdir().unwrap();
        let loc = location(tmp.path());
        let inner = zip_of(&[("manifest.json", &manifest("Interno", "EEEE", "resources"))]);
        let pack = zip_of(&[
            ("manifest.json", &manifest("Esterno", "FFFF", "resources")),
            ("extras/vecchio.mcpack", &inner),
        ]);
        let done = import_bytes(&loc, tmp.path(), "p.mcpack", pack).unwrap();
        assert_eq!(done.len(), 1);
        let names: Vec<_> = list(&loc, PackKind::Resource)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, ["Esterno"]);
    }

    #[test]
    fn rejects_bad_files() {
        let tmp = tempfile::tempdir().unwrap();
        let loc = location(tmp.path());
        let evil = zip_of(&[
            ("manifest.json", &manifest("Cattivo", "DDDD", "resources")),
            ("../../fuori.txt", b"x"),
        ]);
        assert!(import_bytes(&loc, tmp.path(), "evil.mcpack", evil).is_err());
        assert!(!tmp.path().join("fuori.txt").exists());
        assert!(!tmp.path().join("Shared/com.mojang/fuori.txt").exists());
        // Niente cartelle temporanee rimaste.
        let rp = tmp.path().join("Shared/com.mojang/resource_packs");
        assert_eq!(std::fs::read_dir(&rp).map(|d| d.count()).unwrap_or(0), 0);

        let junk = zip_of(&[("readme.txt", b"ciao")]);
        assert!(import_bytes(&loc, tmp.path(), "junk.zip", junk).is_err());
        std::fs::write(tmp.path().join("no.mcpack"), b"not a zip").unwrap();
        assert!(import(&loc, &tmp.path().join("no.mcpack")).is_err());
    }

    #[test]
    fn worlds_need_a_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = DataDirs {
            shared: tmp.path().join("Shared/com.mojang"),
            user: None,
        };
        let loc = Location::new(dirs, tmp.path().join("storage"), true);
        let world = zip_of(&[("level.dat", b"nbt")]);
        let err = import_bytes(&loc, tmp.path(), "w.mcworld", world).unwrap_err();
        assert!(matches!(err, BedrockError::NoProfile), "{err}");
    }
}
