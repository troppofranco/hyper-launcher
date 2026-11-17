//! Contenuto di un'istanza: mod (`mods/*.jar`, disattivate come
//! `*.jar.disabled`) e mondi (`saves/*/level.dat`).

use std::cmp::Reverse;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use hyper_core::{CoreError, safe_component};

use crate::{Instance, InstanceError, Result};

const DISABLED: &str = ".disabled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModFile {
    /// Nome del file nella cartella mod.
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
    pub fn mods_dir(&self) -> PathBuf {
        self.game_dir().join("mods")
    }

    pub fn saves_dir(&self) -> PathBuf {
        self.game_dir().join("saves")
    }

    /// Mod installate, in ordine alfabetico.
    pub fn mods(&self) -> Vec<ModFile> {
        let Ok(entries) = std::fs::read_dir(self.mods_dir()) else {
            return Vec::new();
        };
        let mut mods: Vec<ModFile> = entries
            .flatten()
            .filter_map(|e| {
                let file = e.file_name().to_string_lossy().into_owned();
                let (name, enabled) = match file.strip_suffix(DISABLED) {
                    Some(base) => (base.strip_suffix(".jar")?, false),
                    None => (file.strip_suffix(".jar")?, true),
                };
                Some(ModFile {
                    name: name.to_owned(),
                    enabled,
                    size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
                    file: file.clone(),
                })
            })
            .collect();
        mods.sort_by_key(|m| m.name.to_lowercase());
        mods
    }

    /// Percorso di un file di mod indicato dall'esterno: solo un nome
    /// semplice che finisce in `.jar` o `.jar.disabled`.
    fn mod_path(&self, file: &str) -> Result<PathBuf> {
        let file = safe_component(file)?;
        let base = file.strip_suffix(DISABLED).unwrap_or(file);
        if !base.ends_with(".jar") {
            return Err(InstanceError::InvalidModFile(file.to_owned()));
        }
        Ok(self.mods_dir().join(file))
    }

    /// Attiva o disattiva una mod rinominandola. Non sovrascrive mai: se
    /// esiste già l'altra variante (es. due versioni della stessa mod) è
    /// un errore.
    pub fn set_mod_enabled(&self, file: &str, enabled: bool) -> Result<()> {
        let from = self.mod_path(file)?;
        let base = file.strip_suffix(DISABLED).unwrap_or(file);
        let target = if enabled {
            base.to_owned()
        } else {
            format!("{base}{DISABLED}")
        };
        let to = self.mod_path(&target)?;
        if from == to {
            return Ok(());
        }
        if to.exists() {
            return Err(InstanceError::ModConflict(target));
        }
        std::fs::rename(&from, &to).map_err(|e| CoreError::io(&from, e))?;
        Ok(())
    }

    pub fn delete_mod(&self, file: &str) -> Result<()> {
        let path = self.mod_path(file)?;
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

        let names: Vec<_> = instance.mods().into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["lithium", "sodium"]);

        instance.set_mod_enabled("lithium.jar", false).unwrap();
        assert!(instance.mods_dir().join("lithium.jar.disabled").is_file());
        assert!(
            !instance
                .mods()
                .iter()
                .find(|m| m.name == "lithium")
                .unwrap()
                .enabled
        );
        instance
            .set_mod_enabled("lithium.jar.disabled", true)
            .unwrap();
        assert!(instance.mods_dir().join("lithium.jar").is_file());

        // Due copie della stessa mod: disattivare non deve cancellarne una.
        std::fs::write(instance.mods_dir().join("sodium.jar.disabled"), b"old").unwrap();
        assert!(matches!(
            instance.set_mod_enabled("sodium.jar", false),
            Err(InstanceError::ModConflict(_))
        ));
        assert_eq!(
            std::fs::read(instance.mods_dir().join("sodium.jar.disabled")).unwrap(),
            b"old"
        );

        for bad in ["../x.jar", "readme.txt", "a/b.jar"] {
            assert!(instance.delete_mod(bad).is_err(), "{bad}");
        }
        instance.delete_mod("lithium.jar").unwrap();
        assert!(!instance.mods_dir().join("lithium.jar").exists());
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
