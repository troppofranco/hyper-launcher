//! Contenuti già installati in un'istanza: riconosciuti su Modrinth dal
//! loro SHA1 (anche se copiati a mano), con eventuali aggiornamenti.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use hyper_core::{CoreError, safe_component};
use hyper_installer::download::ProgressFn;
use hyper_installer::hash::sha1_file;
use hyper_installer::{DownloadTask, Downloader, Sha1Hex};
use hyper_instances::{ContentDir, ContentFile, DISABLED, Instance};

use crate::curseforge::{self, CfFile, CfMod, CurseForge, RELEASE, fingerprint, loader_type};
use crate::install::download_replacing;
use crate::modrinth::is_trusted_download;
use crate::{ContentError, ContentKind, Modrinth, Project, Result, Version};

/// Un file installato e, se Modrinth lo conosce, il suo progetto.
#[derive(Debug, Clone)]
pub struct InstalledContent {
    pub dir: ContentDir,
    pub file: ContentFile,
    pub path: PathBuf,
    pub sha1: Sha1Hex,
    /// Versione Modrinth a cui appartiene il file.
    pub version: Option<Version>,
    pub project: Option<Project>,
    /// Versione più nuova compatibile con l'istanza.
    pub update: Option<Version>,
    /// Se Modrinth non lo conosce ma CurseForge sì: progetto, file e
    /// aggiornamento.
    pub curseforge: Option<CfInstalled>,
}

/// Un file riconosciuto da CurseForge.
#[derive(Debug, Clone)]
pub struct CfInstalled {
    pub project: CfMod,
    pub file: CfFile,
    /// File più nuovo per la versione del gioco e il loader dell'istanza.
    pub update: Option<CfFile>,
}

/// Impronte CurseForge dei file (lettura dal disco: fuori dal runtime async).
async fn fingerprints(paths: Vec<PathBuf>) -> Result<Vec<u32>> {
    let prints = tokio::task::spawn_blocking(move || {
        paths
            .iter()
            .map(|p| {
                std::fs::read(p)
                    .map(|bytes| fingerprint(&bytes))
                    .map_err(|e| CoreError::io(p, e))
            })
            .collect::<Result<Vec<_>, CoreError>>()
    })
    .await??;
    Ok(prints)
}

/// File della cartella con il loro SHA1 (lettura dal disco: va fuori dal
/// runtime async).
async fn hashed_files(
    instance: &Instance,
    dir: ContentDir,
) -> Result<Vec<(ContentFile, PathBuf, Sha1Hex)>> {
    let instance = instance.clone();
    let files = tokio::task::spawn_blocking(move || {
        instance
            .content_files(dir)
            .into_iter()
            .map(|file| {
                let path = instance.content_dir(dir).join(&file.file);
                let (sha1, _) = sha1_file(&path)?;
                Ok((file, path, sha1))
            })
            .collect::<Result<Vec<_>, CoreError>>()
    })
    .await??;
    Ok(files)
}

/// Contenuti di una cartella dell'istanza, riconosciuti su Modrinth. Con
/// `check_updates` cerca anche le versioni più nuove compatibili.
pub async fn installed(
    api: &Modrinth,
    cf: Option<&CurseForge>,
    instance: &Instance,
    dir: ContentDir,
    check_updates: bool,
) -> Result<Vec<InstalledContent>> {
    let files = hashed_files(instance, dir).await?;
    let hashes: Vec<Sha1Hex> = files.iter().map(|(_, _, h)| h.clone()).collect();
    let mut versions = api.versions_by_hash(&hashes).await?;

    let mut ids: Vec<String> = versions.values().map(|v| v.project_id.clone()).collect();
    ids.sort();
    ids.dedup();
    let projects: HashMap<String, Project> = api
        .projects(&ids)
        .await?
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();

    let kind = ContentKind::from(dir);
    // Senza un loader adatto (es. mod in un'istanza vanilla) i file si
    // elencano lo stesso, senza cercare aggiornamenti.
    let loaders = kind
        .loaders(instance.config.loader.as_ref().map(|l| l.kind))
        .ok();
    let mut updates = match loaders {
        Some(loaders) if check_updates && !versions.is_empty() => {
            let known: Vec<Sha1Hex> = hashes
                .iter()
                .filter(|h| versions.contains_key(h.as_str()))
                .cloned()
                .collect();
            // Gli shader non sono legati a una versione precisa del gioco.
            let game = instance.config.version.as_str();
            let games: &[&str] = if kind == ContentKind::Shader {
                &[]
            } else {
                &[game]
            };
            let mut latest = api.latest_by_hash(&known, loaders, games).await?;
            // Chi usa una release riceve solo release: se l'ultima è una beta
            // si cerca la release più recente, se è più nuova di quella
            // installata. Le richieste partono tutte insieme.
            let fallbacks = latest.iter().filter_map(|(hash, newest)| {
                let current = versions.get(hash)?;
                if current.version_type != "release" || newest.version_type == "release" {
                    return None;
                }
                Some(async move {
                    let all = api.versions(&current.project_id, loaders, games).await?;
                    let release = all.iter().position(|v| v.version_type == "release");
                    let installed_at = all.iter().position(|v| v.id == current.id);
                    let best = match (release, installed_at) {
                        (Some(r), Some(i)) if r < i => all[r].clone(),
                        (Some(r), None) => all[r].clone(),
                        _ => current.clone(),
                    };
                    Ok::<_, ContentError>((hash.clone(), best))
                })
            });
            let fallbacks = futures::future::try_join_all(fallbacks).await?;
            latest.extend(fallbacks);
            latest
        }
        _ => HashMap::new(),
    };

    // I file che Modrinth non conosce si cercano su CurseForge.
    let unknown: Vec<(usize, PathBuf)> = files
        .iter()
        .enumerate()
        .filter(|(_, (_, _, sha1))| !versions.contains_key(sha1.as_str()))
        .map(|(i, (_, path, _))| (i, path.clone()))
        .collect();
    let mut on_curseforge = match cf {
        Some(cf) if !unknown.is_empty() => {
            recognize_on_curseforge(cf, instance, kind, unknown, check_updates).await
        }
        _ => HashMap::new(),
    };

    Ok(files
        .into_iter()
        .enumerate()
        .map(|(i, (file, path, sha1))| {
            let version = versions.remove(sha1.as_str());
            let update = updates
                .remove(sha1.as_str())
                .filter(|latest| version.as_ref().is_some_and(|v| v.id != latest.id));
            InstalledContent {
                dir,
                project: version
                    .as_ref()
                    .and_then(|v| projects.get(&v.project_id).cloned()),
                file,
                path,
                sha1,
                version,
                update,
                curseforge: on_curseforge.remove(&i),
            }
        })
        .collect())
}

/// Riconosce su CurseForge i file indicati (indice → percorso) e, con
/// `check_updates`, cerca i loro aggiornamenti. CurseForge è solo un
/// complemento: se non risponde i file restano semplicemente sconosciuti.
async fn recognize_on_curseforge(
    cf: &CurseForge,
    instance: &Instance,
    kind: ContentKind,
    unknown: Vec<(usize, PathBuf)>,
    check_updates: bool,
) -> HashMap<usize, CfInstalled> {
    let result = async {
        let (indexes, paths): (Vec<usize>, Vec<PathBuf>) = unknown.into_iter().unzip();
        let prints = fingerprints(paths).await?;
        let matches: HashMap<u32, curseforge::FingerprintMatch> = cf
            .by_fingerprint(&prints)
            .await?
            .into_iter()
            .map(|m| (m.file.file_fingerprint, m))
            .collect();
        let mut ids: Vec<u32> = matches.values().map(|m| m.id).collect();
        ids.sort_unstable();
        ids.dedup();
        let projects: HashMap<u32, CfMod> = cf
            .projects(&ids)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect();

        // Ultimo file per la versione del gioco e il loader dell'istanza; chi
        // usa una release riceve solo release.
        let game = instance.config.version.as_str();
        let loader = match kind {
            ContentKind::Mod => instance.config.loader.as_ref().map(|l| loader_type(l.kind)),
            _ => None,
        };
        let newer = |m: &curseforge::FingerprintMatch| {
            let project = projects.get(&m.id)?;
            let latest = project.latest_file(game, loader, m.file.release_type == RELEASE)?;
            (latest.file_id > m.file.id).then_some(latest.file_id)
        };
        let update_ids: Vec<u32> = if check_updates {
            matches.values().filter_map(newer).collect()
        } else {
            Vec::new()
        };
        let mut updates: HashMap<u32, CfFile> = cf
            .files(&update_ids)
            .await?
            .into_iter()
            .map(|f| (f.mod_id, f))
            .collect();

        let mut found = HashMap::new();
        for (index, print) in indexes.into_iter().zip(prints) {
            let Some(m) = matches.get(&print) else {
                continue;
            };
            let Some(project) = projects.get(&m.id) else {
                continue;
            };
            found.insert(
                index,
                CfInstalled {
                    project: project.clone(),
                    file: m.file.clone(),
                    update: updates.remove(&m.id),
                },
            );
        }
        Ok::<_, ContentError>(found)
    }
    .await;
    result.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "cannot look up files on CurseForge");
        HashMap::new()
    })
}

/// Progetti CurseForge già presenti tra le mod (attive o no) → file.
pub async fn installed_cf_projects(
    cf: &CurseForge,
    instance: &Instance,
) -> Result<HashMap<u32, PathBuf>> {
    let dir = instance.content_dir(ContentDir::Mods);
    let paths: Vec<PathBuf> = instance
        .content_files(ContentDir::Mods)
        .into_iter()
        .map(|f| dir.join(f.file))
        .collect();
    let prints = fingerprints(paths.clone()).await?;
    let by_print: HashMap<u32, u32> = cf
        .by_fingerprint(&prints)
        .await?
        .into_iter()
        .map(|m| (m.file.file_fingerprint, m.id))
        .collect();
    Ok(paths
        .into_iter()
        .zip(prints)
        .filter_map(|(path, print)| by_print.get(&print).map(|id| (*id, path)))
        .collect())
}

/// Progetti Modrinth già presenti tra le mod (attive o no) → file.
pub async fn installed_mod_projects(
    api: &Modrinth,
    instance: &Instance,
) -> Result<HashMap<String, PathBuf>> {
    let files = hashed_files(instance, ContentDir::Mods).await?;
    let hashes: Vec<Sha1Hex> = files.iter().map(|(_, _, h)| h.clone()).collect();
    let versions = api.versions_by_hash(&hashes).await?;
    Ok(files
        .into_iter()
        .filter_map(|(_, path, sha1)| {
            versions
                .get(sha1.as_str())
                .map(|v| (v.project_id.clone(), path))
        })
        .collect())
}

/// Aggiorna i file indicati (vuoto = tutti quelli con un aggiornamento).
/// Il file nuovo viene scaricato e verificato prima di cancellare il
/// vecchio; un file disattivato resta disattivato. Restituisce quanti file
/// sono stati aggiornati.
pub async fn update(
    api: &Modrinth,
    cf: Option<&CurseForge>,
    downloader: &Downloader,
    instance: &Instance,
    dir: ContentDir,
    only: &[String],
    progress: Option<ProgressFn<'_>>,
) -> Result<usize> {
    let _lock = instance.try_lock()?;
    let wanted: HashSet<&str> = only.iter().map(String::as_str).collect();
    let mut tasks = Vec::new();
    let mut replaced = Vec::new();
    for item in installed(api, cf, instance, dir, true).await? {
        if !wanted.is_empty() && !wanted.contains(item.file.file.as_str()) {
            continue;
        }
        // URL, SHA1, dimensione e nome del file nuovo, da Modrinth o CurseForge.
        let (url, sha1, size, filename) =
            if let Some(file) = item.update.as_ref().and_then(Version::primary_file) {
                if !is_trusted_download(&file.url) {
                    return Err(ContentError::UntrustedUrl(file.url.clone()));
                }
                let sha1 = file.hashes.sha1.clone();
                (file.url.clone(), sha1, file.size, file.filename.clone())
            } else if let Some(file) = item.curseforge.and_then(|c| c.update) {
                match file.download() {
                    Ok((url, sha1)) => (url, sha1, file.file_length, file.file_name),
                    // Da aggiornare a mano dal sito: si salta.
                    Err(ContentError::DistributionBlocked { .. }) => continue,
                    Err(e) => return Err(e),
                }
            } else {
                continue;
            };
        let mut name = safe_component(&filename)?.to_owned();
        if !item.file.enabled {
            name.push_str(DISABLED);
        }
        // Stesso controllo di nome dei file aggiunti a mano.
        let dest = instance.content_path(dir, &name)?;
        tasks.push(DownloadTask::new(url, dest.clone(), sha1, Some(size)));
        replaced.push((item.path, dest));
    }
    let count = tasks.len();
    download_replacing(downloader, tasks, replaced, progress).await?;
    Ok(count)
}
