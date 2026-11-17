//! Contenuto di un'istanza: mod (`mods/*.jar`), resource pack
//! (`resourcepacks/*.zip`) e shader (`shaderpacks/*.zip`), disattivabili
//! con il suffisso `.disabled`, più i mondi (`saves/*/level.dat`).

use std::cmp::Reverse;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use hyper_core::{CoreError, safe_component};
use serde::Deserialize;

use crate::{Instance, InstanceError, Result};

/// Suffisso dei file disattivati (Minecraft e Fabric li ignorano).
pub const DISABLED: &str = ".disabled";

/// Cartella di contenuti dentro il `.minecraft` dell'istanza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentDir {
    Mods,
    ResourcePacks,
    ShaderPacks,
}

impl ContentDir {
    pub const ALL: [Self; 3] = [Self::Mods, Self::ResourcePacks, Self::ShaderPacks];

    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::ResourcePacks => "resourcepacks",
            Self::ShaderPacks => "shaderpacks",
        }
    }

    /// Estensione dei file riconosciuti in questa cartella.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mods => ".jar",
            Self::ResourcePacks | Self::ShaderPacks => ".zip",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFile {
    /// Nome del file nella cartella.
    pub file: String,
    /// Nome senza estensione né `.disabled`.
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    pub name: String,
    /// Ultima modifica di `level.dat` (secondi dall'epoch Unix).
    pub last_played: Option<u64>,
}

impl Instance {
    pub fn content_dir(&self, dir: ContentDir) -> PathBuf {
        self.game_dir().join(dir.dir_name())
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.content_dir(ContentDir::Mods)
    }

    /// Impostazioni di gioco dell'istanza.
    pub fn options_file(&self) -> PathBuf {
        self.game_dir().join("options.txt")
    }

    pub fn saves_dir(&self) -> PathBuf {
        self.game_dir().join("saves")
    }

    /// File di contenuto installati, in ordine alfabetico.
    pub fn content_files(&self, dir: ContentDir) -> Vec<ContentFile> {
        let Ok(entries) = std::fs::read_dir(self.content_dir(dir)) else {
            return Vec::new();
        };
        let ext = dir.extension();
        let mut files: Vec<ContentFile> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|e| {
                let file = e.file_name().to_string_lossy().into_owned();
                let (name, enabled) = match file.strip_suffix(DISABLED) {
                    Some(base) => (base.strip_suffix(ext)?, false),
                    None => (file.strip_suffix(ext)?, true),
                };
                Some(ContentFile {
                    name: name.to_owned(),
                    enabled,
                    size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                    file: file.clone(),
                })
            })
            .collect();
        files.sort_by_key(|f| f.name.to_lowercase());
        files
    }

    /// Percorso di un file di contenuto indicato dall'esterno: solo un nome
    /// semplice con l'estensione della cartella (o la stessa + `.disabled`).
    pub fn content_path(&self, dir: ContentDir, file: &str) -> Result<PathBuf> {
        let file = safe_component(file)?;
        let base = file.strip_suffix(DISABLED).unwrap_or(file);
        if !base.ends_with(dir.extension()) {
            return Err(InstanceError::InvalidContentFile(file.to_owned()));
        }
        Ok(self.content_dir(dir).join(file))
    }

    /// Attiva o disattiva un file rinominandolo. Non sovrascrive mai: se
    /// esiste già l'altra variante (es. due versioni della stessa mod) è
    /// un errore.
    pub fn set_content_enabled(&self, dir: ContentDir, file: &str, enabled: bool) -> Result<()> {
        let from = self.content_path(dir, file)?;
        let base = file.strip_suffix(DISABLED).unwrap_or(file);
        let target = if enabled {
            base.to_owned()
        } else {
            format!("{base}{DISABLED}")
        };
        let to = self.content_path(dir, &target)?;
        let _lock = self.try_lock()?;
        if from == to {
            return Ok(());
        }
        if to.exists() {
            return Err(InstanceError::ContentConflict(target));
        }
        std::fs::rename(&from, &to).map_err(|e| CoreError::io(&from, e))?;
        Ok(())
    }

    pub fn delete_content(&self, dir: ContentDir, file: &str) -> Result<()> {
        let path = self.content_path(dir, file)?;
        let _lock = self.try_lock()?;
        std::fs::remove_file(&path).map_err(|e| CoreError::io(&path, e))?;
        Ok(())
    }

    /// Mondi salvati, dal più recente.
    pub fn worlds(&self) -> Vec<World> {
        let Ok(entries) = std::fs::read_dir(self.saves_dir()) else {
            return Vec::new();
        };
        let mut worlds: Vec<World> = entries
            .flatten()
            .filter_map(|e| {
                let level = e.path().join("level.dat");
                let modified = std::fs::metadata(&level).ok()?.modified().ok();
                Some(World {
                    name: e.file_name().to_string_lossy().into_owned(),
                    last_played: modified
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs()),
                })
            })
            .collect();
        worlds.sort_by_key(|w| Reverse(w.last_played));
        worlds
    }
}

#[cfg(test)]
mod tests {
    use hyper_core::Paths;

    use crate::InstanceManager;

    use super::*;

    #[test]
    fn mod_toggle_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let instance = InstanceManager::new(Paths::new(dir.path()))
            .create("m", "26.3", None)
            .unwrap();
        std::fs::create_dir_all(instance.mods_dir()).unwrap();
        std::fs::write(instance.mods_dir().join("sodium.jar"), b"new").unwrap();
        std::fs::write(instance.mods_dir().join("lithium.jar"), b"x").unwrap();
        std::fs::write(instance.mods_dir().join("readme.txt"), b"x").unwrap();

        let names: Vec<_> = instance
            .content_files(ContentDir::Mods)
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["lithium", "sodium"]);

        instance
            .set_content_enabled(ContentDir::Mods, "lithium.jar", false)
            .unwrap();
        assert!(instance.mods_dir().join("lithium.jar.disabled").is_file());
        assert!(
            !instance
                .content_files(ContentDir::Mods)
                .iter()
                .find(|m| m.name == "lithium")
                .unwrap()
                .enabled
        );
        instance
            .set_content_enabled(ContentDir::Mods, "lithium.jar.disabled", true)
            .unwrap();
        assert!(instance.mods_dir().join("lithium.jar").is_file());

        // Due copie della stessa mod: disattivare non deve cancellarne una.
        std::fs::write(instance.mods_dir().join("sodium.jar.disabled"), b"old").unwrap();
        assert!(matches!(
            instance.set_content_enabled(ContentDir::Mods, "sodium.jar", false),
            Err(InstanceError::ContentConflict(_))
        ));
        assert_eq!(
            std::fs::read(instance.mods_dir().join("sodium.jar.disabled")).unwrap(),
            b"old"
        );

        for bad in ["../x.jar", "readme.txt", "a/b.jar"] {
            assert!(
                instance.delete_content(ContentDir::Mods, bad).is_err(),
                "{bad}"
            );
        }
        instance
            .delete_content(ContentDir::Mods, "lithium.jar")
            .unwrap();
        assert!(!instance.mods_dir().join("lithium.jar").exists());
    }

    #[test]
    fn resource_packs_use_zip_and_skip_folders() {
        let dir = tempfile::tempdir().unwrap();
        let instance = InstanceManager::new(Paths::new(dir.path()))
            .create("r", "26.3", None)
            .unwrap();
        let packs = instance.content_dir(ContentDir::ResourcePacks);
        std::fs::create_dir_all(packs.join("cartella.zip")).unwrap();
        std::fs::write(packs.join("Faithful.zip"), b"x").unwrap();
        std::fs::write(packs.join("mod.jar"), b"x").unwrap();
        let files = instance.content_files(ContentDir::ResourcePacks);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "Faithful");
        assert!(
            instance
                .content_path(ContentDir::ResourcePacks, "mod.jar")
                .is_err()
        );
    }

    #[test]
    fn lists_only_real_worlds() {
        let dir = tempfile::tempdir().unwrap();
        let instance = InstanceManager::new(Paths::new(dir.path()))
            .create("w", "26.3", None)
            .unwrap();
        std::fs::create_dir_all(instance.saves_dir().join("Mondo")).unwrap();
        std::fs::write(instance.saves_dir().join("Mondo").join("level.dat"), b"x").unwrap();
        std::fs::create_dir_all(instance.saves_dir().join("vuota")).unwrap();
        let worlds = instance.worlds();
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].name, "Mondo");
        assert!(worlds[0].last_played.is_some());
    }
}
