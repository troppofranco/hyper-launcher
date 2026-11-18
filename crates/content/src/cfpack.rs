//! Modpack nel formato CurseForge: uno zip con `manifest.json` (versione
//! del gioco, loader, progetti e file da scaricare) e una cartella di
//! override da copiare nell'istanza.
//!
//! I file si scaricano dalla CDN di CurseForge con il loro SHA1. Quelli che
//! l'autore non permette di scaricare dai launcher non bloccano l'import:
//! vengono restituiti a parte, con la pagina da cui prenderli a mano.

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::Path;

use hyper_core::{CoreError, fs, safe_component};
use hyper_installer::download::ProgressFn;
use hyper_installer::natives::extract_zip_mapped;
use hyper_installer::{DownloadTask, Downloader, LoaderKind, LoaderSpec};
use hyper_instances::{Instance, InstanceManager};
use serde::Deserialize;

use crate::cfinstall::pack_dir;
use crate::curseforge::{CfMod, CurseForge, RELEASE};
use crate::mrpack::{self, apply_icon, folder_name, zip_error};
use crate::{ContentError, Result};

const MANIFEST: &str = "manifest.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub minecraft: ManifestMinecraft,
    #[serde(default)]
    pub manifest_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
    #[serde(default)]
    pub overrides: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMinecraft {
    pub version: String,
    #[serde(default)]
    pub mod_loaders: Vec<ManifestLoader>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestLoader {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u32,
    #[serde(rename = "fileID")]
    pub file_id: u32,
    #[serde(default = "yes")]
    pub required: bool,
}

fn yes() -> bool {
    true
}

/// Un file del modpack da scaricare a mano (l'autore non permette il
/// download dai launcher).
#[derive(Debug, Clone)]
pub struct BlockedFile {
    pub name: String,
    pub file_name: String,
    /// Pagina CurseForge del progetto.
    pub url: String,
}

/// Esito di un import: l'istanza creata e i file da scaricare a mano.
#[derive(Debug, Clone)]
pub struct PackImport {
    pub instance: Instance,
    pub blocked: Vec<BlockedFile>,
}

/// `true` se lo zip contiene un `manifest.json` (modpack CurseForge).
pub fn is_curseforge_pack(pack: &Path) -> bool {
    std::fs::File::open(pack)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .is_some_and(|mut z| z.by_name(MANIFEST).is_ok())
}

/// Legge il `manifest.json` di un modpack CurseForge.
pub fn read_manifest(pack: &Path) -> Result<Manifest> {
    let file = std::fs::File::open(pack).map_err(|e| CoreError::io(pack, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error(pack))?;
    let mut entry = archive.by_name(MANIFEST).map_err(|_| {
        ContentError::InvalidPack(format!("{MANIFEST} missing: not a CurseForge modpack"))
    })?;
    let mut json = Vec::new();
    entry
        .read_to_end(&mut json)
        .map_err(|e| CoreError::io(pack, e))?;
    serde_json::from_slice(&json).map_err(|source| ContentError::Json {
        url: pack.display().to_string(),
        source,
    })
}

/// Loader del modpack (`fabric-0.16.0`, `neoforge-21.1.77`, …).
fn loader(manifest: &Manifest) -> Result<Option<LoaderSpec>> {
    let Some(entry) = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
    else {
        return Ok(None);
    };
    let (name, version) = entry
        .id
        .split_once('-')
        .ok_or_else(|| ContentError::UnsupportedLoader(entry.id.clone()))?;
    let kind = match name {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        "neoforge" => LoaderKind::NeoForge,
        "forge" => LoaderKind::Forge,
        _ => return Err(ContentError::UnsupportedLoader(entry.id.clone())),
    };
    let game = &manifest.minecraft.version;
    if !kind.supports_game(game) {
        return Err(ContentError::UnsupportedLoader(format!(
            "{} {game}",
            kind.display_name()
        )));
    }
    // NeoForge 1.20.1 usava id del tipo `neoforge-1.20.1-47.1.79`.
    let version = version
        .strip_prefix(&format!("{game}-"))
        .unwrap_or(version)
        .to_owned();
    Ok(Some(LoaderSpec { kind, version }))
}

/// Prefisso degli override nello zip (di solito `overrides/`). Un nome
/// strano viene rifiutato: deve essere una sola cartella.
fn overrides_prefix(manifest: &Manifest) -> Result<String> {
    let name = manifest.overrides.as_deref().unwrap_or("overrides");
    Ok(format!("{}/", safe_component(name.trim_end_matches('/'))?))
}

/// Crea una nuova istanza da un modpack CurseForge: scarica i file
/// (verificati con SHA1, solo dalla CDN di CurseForge) e copia gli
/// override. Se qualcosa va storto l'istanza appena creata viene eliminata.
pub async fn import(
    cf: &CurseForge,
    manager: &InstanceManager,
    downloader: &Downloader,
    pack: &Path,
    progress: Option<ProgressFn<'_>>,
) -> Result<PackImport> {
    let manifest = read_manifest(pack)?;
    if !manifest.manifest_type.is_empty() && manifest.manifest_type != "minecraftModpack" {
        return Err(ContentError::InvalidPack(format!(
            "unsupported manifest type {}",
            manifest.manifest_type
        )));
    }
    let loader = loader(&manifest)?;
    let prefix = overrides_prefix(&manifest)?;
    let wanted: Vec<&ManifestFile> = manifest.files.iter().filter(|f| f.required).collect();

    // Progetti (per la cartella giusta e la pagina) e file (URL e hash).
    let mut project_ids: Vec<u32> = wanted.iter().map(|f| f.project_id).collect();
    project_ids.sort_unstable();
    project_ids.dedup();
    let projects: HashMap<u32, CfMod> = cf
        .projects(&project_ids)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let file_ids: Vec<u32> = wanted.iter().map(|f| f.file_id).collect();
    let files = cf.files(&file_ids).await?;
    if files.len() < file_ids.len() {
        return Err(ContentError::InvalidPack(format!(
            "{} of {} files are no longer on CurseForge",
            file_ids.len() - files.len(),
            file_ids.len()
        )));
    }

    let name = manager.unique_name(&folder_name(if manifest.name.is_empty() {
        "Modpack"
    } else {
        &manifest.name
    }))?;
    let instance = manager.create(&name, &manifest.minecraft.version, loader)?;
    let result = async {
        let mut tasks = Vec::new();
        let mut blocked = Vec::new();
        for file in &files {
            let project = projects.get(&file.mod_id);
            match file.download() {
                Ok((url, sha1)) => {
                    let dir = pack_dir(project.and_then(CfMod::kind));
                    // Stesse regole sul nome dei file gestiti dall'istanza.
                    let dest = instance.content_path(dir, &file.file_name)?;
                    tasks.push(DownloadTask::new(url, dest, sha1, Some(file.file_length)));
                }
                Err(ContentError::DistributionBlocked { .. }) => blocked.push(BlockedFile {
                    name: project.map_or_else(|| file.display_name.clone(), |p| p.name.clone()),
                    file_name: file.file_name.clone(),
                    url: project.map_or_else(
                        || format!("https://www.curseforge.com/projects/{}", file.mod_id),
                        CfMod::website,
                    ),
                }),
                Err(e) => return Err(e),
            }
        }
        downloader.run(tasks, progress).await?;
        let (pack, game_dir) = (pack.to_owned(), instance.game_dir());
        tokio::task::spawn_blocking(move || extract_overrides(&pack, &prefix, &game_dir)).await??;
        Ok(blocked)
    }
    .await;
    match result {
        Ok(blocked) => Ok(PackImport { instance, blocked }),
        Err(e) => {
            if let Err(cleanup) = fs::remove_dir_all_if_exists(&instance.dir) {
                tracing::warn!(error = %cleanup, "cannot remove the half-imported instance");
            }
            Err(e)
        }
    }
}

/// Copia gli override in `dest`. Ogni nome è non fidato e passa da
/// `safe_join` dentro [`extract_zip_mapped`].
fn extract_overrides(pack: &Path, prefix: &str, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(pack).map_err(|e| CoreError::io(pack, e))?;
    extract_overrides_from(file, pack, prefix, dest)
}

fn extract_overrides_from<R: Read + Seek>(
    mut reader: R,
    origin: &Path,
    prefix: &str,
    dest: &Path,
) -> Result<()> {
    extract_zip_mapped(&mut reader, origin, dest, |name| name.strip_prefix(prefix))?;
    Ok(())
}

/// Importa un modpack Modrinth (`.mrpack`) o CurseForge (zip con
/// `manifest.json`), riconosciuto dal contenuto.
pub async fn import_any(
    cf: Option<&CurseForge>,
    manager: &InstanceManager,
    downloader: &Downloader,
    pack: &Path,
    progress: Option<ProgressFn<'_>>,
) -> Result<PackImport> {
    if is_curseforge_pack(pack) {
        let cf = cf.ok_or(ContentError::CurseForgeUnavailable)?;
        return import(cf, manager, downloader, pack, progress).await;
    }
    let instance = mrpack::import(manager, downloader, pack, progress).await?;
    Ok(PackImport {
        instance,
        blocked: Vec::new(),
    })
}

/// Scarica l'ultima versione di un modpack CurseForge in `cache_dir`, la
/// importa come nuova istanza e ne usa il logo come immagine.
pub async fn install_from_curseforge(
    cf: &CurseForge,
    manager: &InstanceManager,
    downloader: &Downloader,
    cache_dir: &Path,
    id: u32,
    progress: Option<ProgressFn<'_>>,
) -> Result<PackImport> {
    let project = cf.project(id).await?;
    if project.kind() != Some(crate::ContentKind::Modpack) {
        return Err(ContentError::InvalidKind(project.name));
    }
    let files = cf.project_files(id, None, None).await?;
    let best = files
        .iter()
        .position(|f| f.release_type == RELEASE)
        .unwrap_or(0);
    let file = files
        .into_iter()
        .nth(best)
        .ok_or_else(|| ContentError::InvalidPack(format!("{} has no files", project.name)))?;
    let (url, sha1) = file.download()?;
    let path = cache_dir.join(format!("curseforge-{}.zip", file.id));
    downloader
        .fetch(DownloadTask::new(
            url,
            path.clone(),
            sha1,
            Some(file.file_length),
        ))
        .await?;
    let result = import(cf, manager, downloader, &path, progress).await;
    let _ = std::fs::remove_file(&path);
    let imported = result?;
    if let Some(icon) = project.icon_url() {
        apply_icon(downloader, icon, &imported.instance).await;
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    fn manifest(json: serde_json::Value) -> Manifest {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn reads_loaders() {
        let m = manifest(serde_json::json!({
            "minecraft": {"version": "1.21.1", "modLoaders": [
                {"id": "forge-52.0.1", "primary": false},
                {"id": "neoforge-21.1.77", "primary": true}
            ]},
            "files": []
        }));
        let l = loader(&m).unwrap().unwrap();
        assert_eq!(l.kind, LoaderKind::NeoForge);
        assert_eq!(l.version, "21.1.77");

        let m = manifest(serde_json::json!({
            "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "rift-1.0"}]}
        }));
        assert!(matches!(
            loader(&m),
            Err(ContentError::UnsupportedLoader(_))
        ));

        let m = manifest(serde_json::json!({"minecraft": {"version": "1.21.1"}}));
        assert!(loader(&m).unwrap().is_none());
    }

    #[test]
    fn overrides_must_be_a_single_folder() {
        let m =
            |o: &str| manifest(serde_json::json!({"minecraft": {"version": "1"}, "overrides": o}));
        assert_eq!(overrides_prefix(&m("overrides")).unwrap(), "overrides/");
        assert!(overrides_prefix(&m("../x")).is_err());
        assert!(overrides_prefix(&m("a/b")).is_err());
    }

    #[test]
    fn overrides_cannot_escape_the_instance() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("overrides/config/a.txt", options).unwrap();
        zip.write_all(b"ok").unwrap();
        zip.start_file("overrides/../../evil.txt", options).unwrap();
        zip.write_all(b"no").unwrap();
        let bytes = zip.finish().unwrap().into_inner();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("game");
        let result =
            extract_overrides_from(Cursor::new(bytes), Path::new("p.zip"), "overrides/", &dest);
        assert!(result.is_err());
        assert!(!dir.path().join("evil.txt").exists());
    }
}
