//! Backup dei mondi di un server, in `<dati>/backups/servers/<nome>/`. A
//! server acceso li fa l'host (che può sospendere il salvataggio del gioco),
//! a server spento chiunque. Il ripristino si fa solo a server spento.

use std::path::{Path, PathBuf};

use hyper_core::{Paths, safe_component};
use hyper_installer::backups::{self, BackupInfo};

use crate::properties::Properties;
use crate::{Result, ServerError, ServerManager};

/// Cartella dei backup di un server.
pub fn dir(paths: &Paths, name: &str) -> Result<PathBuf> {
    Ok(paths
        .root()
        .join("backups")
        .join("servers")
        .join(safe_component(name)?))
}

/// Cartelle del mondo: `level-name` di `server.properties` (predefinito
/// `world`) più le dimensioni separate che crea Paper.
pub fn worlds(server_dir: &Path) -> Vec<String> {
    let level = Properties::load(server_dir)
        .ok()
        .and_then(|p| p.get("level-name").map(str::to_owned))
        .filter(|l| safe_component(l).is_ok())
        .unwrap_or_else(|| "world".to_owned());
    let nether = format!("{level}_nether");
    let end = format!("{level}_the_end");
    vec![level, nether, end]
}

pub fn list(paths: &Paths, name: &str) -> Result<Vec<BackupInfo>> {
    Ok(backups::list(&dir(paths, name)?))
}

/// Zippa i mondi così come sono su disco (a server acceso va fatto
/// dall'host, che prima sospende il salvataggio) e tiene solo gli ultimi
/// `schedule.keep_backups`.
pub fn create_now(
    manager: &ServerManager,
    paths: &Paths,
    name: &str,
    auto: bool,
) -> Result<BackupInfo> {
    let config = manager.load(name)?;
    let server_dir = manager.dir(name)?;
    let sources: Vec<(PathBuf, String)> = worlds(&server_dir)
        .into_iter()
        .map(|w| (server_dir.join(&w), w))
        .collect();
    if !sources.iter().any(|(p, _)| p.is_dir()) {
        return Err(ServerError::Invalid(
            "il server non ha ancora un mondo: avvialo una volta".into(),
        ));
    }
    let backups_dir = dir(paths, name)?;
    let info = backups::create(&backups_dir, &sources, auto)?;
    backups::prune(&backups_dir, config.schedule.keep_backups as usize)?;
    Ok(info)
}

/// Ripristina un backup (server spento: lo controlla chi chiama).
pub fn restore(
    manager: &ServerManager,
    paths: &Paths,
    name: &str,
    file: &str,
) -> Result<Vec<String>> {
    let server_dir = manager.dir(name)?;
    Ok(backups::restore(
        &dir(paths, name)?,
        file,
        &server_dir,
        &worlds(&server_dir),
    )?)
}

pub fn delete(paths: &Paths, name: &str, file: &str) -> Result<()> {
    Ok(backups::delete(&dir(paths, name)?, file)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worlds_follow_level_name() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            worlds(dir.path()),
            ["world", "world_nether", "world_the_end"]
        );
        std::fs::write(dir.path().join("server.properties"), "level-name=amici\n").unwrap();
        assert_eq!(worlds(dir.path())[0], "amici");
        std::fs::write(dir.path().join("server.properties"), "level-name=../x\n").unwrap();
        assert_eq!(worlds(dir.path())[0], "world", "unsafe names fall back");
    }
}
