//! Plugin (Paper) e mod (Fabric, NeoForge, Forge) dei server da Modrinth: ricerca della versione compatibile,
//! dipendenze obbligatorie, download verificati con SHA1 nella cartella
//! `plugins/`, riconoscimento dei jar installati tramite il loro hash.
//!
//! Un plugin disattivato resta nella cartella come `<nome>.jar.disabled`:
//! Paper carica solo i file `.jar`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use hyper_content::modrinth::{DependencyType, is_trusted_download};
use hyper_content::{ContentKind, Modrinth, Version};
use hyper_core::safe_component;
use hyper_installer::hash::sha1_file;
use hyper_installer::{DownloadTask, Downloader, Sha1Hex};
use serde::Serialize;

use crate::{Result, ServerError};

/// Cartella e loader Modrinth dei contenuti di un server: i plugin per
/// Paper, le mod per i server con loader (niente per Vanilla).
pub fn target(
    server_dir: &Path,
    kind: crate::ServerKind,
) -> Option<(PathBuf, &'static [&'static str])> {
    use crate::ServerKind;
    let (folder, loaders): (&str, &'static [&'static str]) = match kind {
        ServerKind::Vanilla => return None,
        ServerKind::Paper => (
            "plugins",
            ContentKind::Plugin.loaders(None).unwrap_or_default(),
        ),
        ServerKind::Fabric => ("mods", &["fabric"]),
        ServerKind::NeoForge => ("mods", &["neoforge"]),
        ServerKind::Forge => ("mods", &["forge"]),
    };
    Some((server_dir.join(folder), loaders))
}
const DISABLED: &str = ".disabled";
/// Limite di sicurezza sulla catena di dipendenze.
const MAX_FILES: usize = 32;

fn content_err(e: hyper_content::ContentError) -> ServerError {
    ServerError::Content(Box::new(e))
}

/// Un plugin nella cartella del server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    /// Nome del file così com'è (anche `.jar.disabled`).
    pub file: String,
    pub enabled: bool,
    pub size_bytes: u64,
    /// Dati di Modrinth, se il file è riconosciuto.
    pub project_id: Option<String>,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub version_number: Option<String>,
}

/// Jar presenti (attivi e disattivati) con il loro SHA1.
fn jars(dir: &Path) -> Vec<(String, PathBuf, Sha1Hex, u64)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let lower = file.to_ascii_lowercase();
            if !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
                return None;
            }
            let path = e.path();
            let (sha1, size) = sha1_file(&path).ok()?;
            Some((file, path, sha1, size))
        })
        .collect()
}

/// Plugin installati, riconosciuti su Modrinth quando possibile (senza rete
/// restano comunque elencati, solo con il nome del file).
pub async fn installed(api: &Modrinth, dir: &Path) -> Result<Vec<InstalledPlugin>> {
    let dir = dir.to_owned();
    let files = tokio::task::spawn_blocking(move || jars(&dir))
        .await
        .map_err(|e| ServerError::Invalid(e.to_string()))?;
    let hashes: Vec<Sha1Hex> = files.iter().map(|(_, _, h, _)| h.clone()).collect();
    let versions = api.versions_by_hash(&hashes).await.unwrap_or_default();
    let ids: Vec<String> = versions
        .values()
        .map(|v| v.project_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<String, hyper_content::Project> = if ids.is_empty() {
        HashMap::new()
    } else {
        api.projects(&ids)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect()
    };
    let mut list: Vec<InstalledPlugin> = files
        .into_iter()
        .map(|(file, _, sha1, size)| {
            let version = versions.get(sha1.as_str());
            let project = version.and_then(|v| projects.get(&v.project_id));
            InstalledPlugin {
                enabled: !file.to_ascii_lowercase().ends_with(DISABLED),
                size_bytes: size,
                project_id: version.map(|v| v.project_id.clone()),
                title: project.map(|p| p.title.clone()),
                icon_url: project.and_then(|p| p.icon_url.clone()),
                version_number: version.map(|v| v.version_number.clone()),
                file,
            }
        })
        .collect();
    list.sort_by_key(|p| {
        p.title
            .clone()
            .unwrap_or_else(|| p.file.clone())
            .to_lowercase()
    });
    Ok(list)
}

/// Ultima versione per questa versione di Minecraft (le release prima).
async fn pick(api: &Modrinth, project: &str, game: &str, loaders: &[&str]) -> Result<Version> {
    let versions = api
        .versions(project, loaders, &[game])
        .await
        .map_err(content_err)?;
    let best = versions
        .iter()
        .position(|v| v.version_type == "release")
        .unwrap_or(0);
    versions.into_iter().nth(best).ok_or_else(|| {
        ServerError::Invalid(format!("no version of {project} for Minecraft {game}"))
    })
}

/// Installa `project` (id o slug) e le sue dipendenze obbligatorie non
/// ancora presenti; restituisce i nomi dei file scaricati.
pub async fn install(
    api: &Modrinth,
    downloader: &Downloader,
    dir: &Path,
    loaders: &[&str],
    project: &str,
    game: &str,
) -> Result<Vec<String>> {
    let plugins = dir.to_owned();
    // Progetti già presenti (anche disattivati): non si riscaricano.
    let present: HashSet<String> = installed(api, dir)
        .await?
        .into_iter()
        .filter_map(|p| p.project_id)
        .collect();
    let mut seen: HashSet<String> = present.clone();
    let mut queue = VecDeque::new();
    let root = pick(api, project, game, loaders).await?;
    if present.contains(&root.project_id) {
        return Err(ServerError::Invalid(format!(
            "{} is already installed",
            root.name
        )));
    }
    seen.insert(root.project_id.clone());
    queue.push_back(root);

    let mut tasks = Vec::new();
    let mut files = Vec::new();
    while let Some(version) = queue.pop_front() {
        if tasks.len() >= MAX_FILES {
            return Err(ServerError::Invalid("too many dependencies".into()));
        }
        let file = version
            .primary_file()
            .ok_or_else(|| ServerError::Invalid(format!("{} has no files", version.name)))?;
        if !is_trusted_download(&file.url) {
            return Err(ServerError::UntrustedUrl(file.url.clone()));
        }
        let name = safe_component(&file.filename)?.to_owned();
        if !name.to_ascii_lowercase().ends_with(".jar") {
            return Err(ServerError::Invalid(format!("{name} is not a plugin jar")));
        }
        tasks.push(DownloadTask::new(
            &file.url,
            plugins.join(&name),
            file.hashes.sha1.clone(),
            Some(file.size),
        ));
        files.push(name);
        for dep in &version.dependencies {
            if dep.dependency_type != DependencyType::Required {
                continue;
            }
            let Some(id) = &dep.project_id else { continue };
            if seen.insert(id.clone()) {
                queue.push_back(pick(api, id, game, loaders).await?);
            }
        }
    }
    std::fs::create_dir_all(&plugins)?;
    // Il downloader verifica dimensione e SHA1 prima di mettere ogni file al
    // suo posto.
    downloader.run(tasks, None).await?;
    Ok(files)
}

/// Il file di un plugin dentro `plugins/` (solo jar, niente percorsi).
fn plugin_file(dir: &Path, file: &str) -> Result<PathBuf> {
    let name = safe_component(file)?;
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
        return Err(ServerError::Invalid(format!("{name} is not a plugin")));
    }
    let path = dir.join(name);
    if !path.is_file() {
        return Err(ServerError::NotFound(name.to_owned()));
    }
    Ok(path)
}

/// Attiva o disattiva un plugin (rinominandolo); restituisce il nuovo nome.
pub fn set_enabled(dir: &Path, file: &str, enabled: bool) -> Result<String> {
    let path = plugin_file(dir, file)?;
    let is_disabled = file.to_ascii_lowercase().ends_with(DISABLED);
    let new_name = match (enabled, is_disabled) {
        (true, true) => file[..file.len() - DISABLED.len()].to_owned(),
        (false, false) => format!("{file}{DISABLED}"),
        _ => return Ok(file.to_owned()),
    };
    let target = dir.join(&new_name);
    if target.exists() {
        return Err(ServerError::Invalid(format!(
            "{new_name} already exists in the plugins folder"
        )));
    }
    std::fs::rename(path, target)?;
    Ok(new_name)
}

/// Elimina il jar di un plugin (la sua cartella di configurazione resta).
pub fn remove(dir: &Path, file: &str) -> Result<()> {
    std::fs::remove_file(plugin_file(dir, file)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enables_disables_and_refuses_paths() {
        let server = tempfile::tempdir().unwrap();
        std::fs::write(server.path().join("server.jar"), b"x").unwrap();
        let (folder, loaders) = target(server.path(), crate::ServerKind::Paper).unwrap();
        assert!(loaders.contains(&"paper"));
        assert!(target(server.path(), crate::ServerKind::Vanilla).is_none());
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("Essentials.jar"), b"x").unwrap();
        let dir = folder.as_path();

        let off = set_enabled(dir, "Essentials.jar", false).unwrap();
        assert_eq!(off, "Essentials.jar.disabled");
        let found = jars(dir);
        assert_eq!(found.len(), 1);
        let on = set_enabled(dir, &off, true).unwrap();
        assert_eq!(on, "Essentials.jar");

        for bad in ["../server.jar", "..\\server.jar", "config.yml", "C:x.jar"] {
            assert!(remove(dir, bad).is_err(), "{bad}");
            assert!(set_enabled(dir, bad, false).is_err(), "{bad}");
        }
        assert!(server.path().join("server.jar").exists());
        remove(dir, "Essentials.jar").unwrap();
        assert!(jars(dir).is_empty());
    }
}
