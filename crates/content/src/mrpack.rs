//! Modpack nel formato Modrinth (`.mrpack`): uno zip con
//! `modrinth.index.json` (file da scaricare con hash e URL) e le cartelle
//! `overrides/` e `client-overrides/` da copiare nell'istanza.

use std::collections::BTreeMap;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use hyper_core::{CoreError, fs, safe_join};
use hyper_installer::download::ProgressFn;
use hyper_installer::natives::extract_zip_mapped;
use hyper_installer::{DownloadTask, Downloader, LoaderKind, LoaderSpec, Sha1Hex};
use hyper_instances::{ContentDir, Instance, InstanceManager};
use serde::{Deserialize, Serialize};

use crate::install::preferred;
use crate::library::installed;
use crate::modrinth::{checked_id, is_trusted_download};
use crate::{ContentError, ContentKind, Modrinth, Result};

const INDEX: &str = "modrinth.index.json";
const OVERRIDES: [&str; 2] = ["overrides/", "client-overrides/"];
/// Override per un server (gli ultimi hanno la precedenza).
const SERVER_OVERRIDES: [&str; 2] = ["overrides/", "server-overrides/"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub files: Vec<PackFile>,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub hashes: PackHashes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<PackEnv>,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackHashes {
    pub sha1: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackEnv {
    pub client: String,
    pub server: String,
}

pub(crate) fn zip_error(path: &Path) -> impl Fn(zip::result::ZipError) -> ContentError + '_ {
    move |source| ContentError::Zip {
        path: path.to_owned(),
        source,
    }
}

/// Legge l'indice di un `.mrpack`.
pub fn read_index(pack: &Path) -> Result<PackIndex> {
    let file = std::fs::File::open(pack).map_err(|e| CoreError::io(pack, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error(pack))?;
    let mut entry = archive.by_name(INDEX).map_err(|_| {
        ContentError::InvalidPack(format!("{INDEX} missing: not a Modrinth modpack"))
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

/// Versione di Minecraft e loader richiesti dal pack.
pub fn requirements(index: &PackIndex) -> Result<(String, Option<LoaderSpec>)> {
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(ContentError::InvalidPack(format!(
            "unsupported format {} for {}",
            index.format_version, index.game
        )));
    }
    let mut game = None;
    let mut loader = None;
    for (key, version) in &index.dependencies {
        match key.as_str() {
            "minecraft" => game = Some(version.clone()),
            other => {
                let kind = LoaderKind::from_pack_dependency(other)
                    .ok_or_else(|| ContentError::UnsupportedLoader(other.to_owned()))?;
                loader = Some(LoaderSpec {
                    kind,
                    version: version.clone(),
                });
            }
        }
    }
    let game = game.ok_or_else(|| ContentError::InvalidPack("no Minecraft version".into()))?;
    // Stessa regola della creazione di istanze (es. niente Forge 1.16).
    if let Some(l) = loader.as_ref().filter(|l| !l.kind.supports_game(&game)) {
        return Err(ContentError::UnsupportedLoader(format!(
            "{} {game}",
            l.kind.display_name()
        )));
    }
    Ok((game, loader))
}

/// Un file del pack da scaricare, già validato (percorso relativo sicuro,
/// URL ammesso, SHA1 ben formato).
struct PackDownload {
    path: String,
    url: String,
    sha1: Sha1Hex,
    size: u64,
}

/// File da scaricare per un client o (`server`) per un server: si saltano
/// quelli che il pack dichiara non supportati da quel lato.
fn validated_downloads(index: &PackIndex, server: bool) -> Result<Vec<PackDownload>> {
    index
        .files
        .iter()
        .filter(|f| {
            f.env.as_ref().is_none_or(|e| {
                let side = if server { &e.server } else { &e.client };
                side != "unsupported"
            })
        })
        .map(|f| {
            // Verifica il percorso prima di creare qualsiasi cosa su disco.
            safe_join(Path::new("."), &f.path)?;
            let url = f
                .downloads
                .iter()
                .find(|u| is_trusted_download(u))
                .ok_or_else(|| {
                    ContentError::UntrustedUrl(f.downloads.first().cloned().unwrap_or_default())
                })?;
            Ok(PackDownload {
                path: f.path.clone(),
                url: url.clone(),
                sha1: f.hashes.sha1.parse()?,
                size: f.file_size,
            })
        })
        .collect()
}

/// Nome di cartella valido ricavato dal nome del pack.
pub(crate) fn folder_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.()+'&!".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned: String = cleaned.chars().take(48).collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_owned();
    if cleaned.is_empty() {
        "Modpack".to_owned()
    } else {
        cleaned
    }
}

/// Crea una nuova istanza da un `.mrpack`: scarica i file (verificati con
/// SHA1, solo da host ammessi) e copia gli override. Se qualcosa va storto
/// l'istanza appena creata viene eliminata.
pub async fn import(
    manager: &InstanceManager,
    downloader: &Downloader,
    pack: &Path,
    progress: Option<ProgressFn<'_>>,
) -> Result<Instance> {
    let index = read_index(pack)?;
    let (game, loader) = requirements(&index)?;
    let downloads = validated_downloads(&index, false)?;

    let name = manager.unique_name(&folder_name(&index.name))?;
    let instance = manager.create(&name, &game, loader)?;
    let result = async {
        let game_dir = instance.game_dir();
        let tasks = downloads
            .into_iter()
            .map(|d| {
                Ok(DownloadTask::new(
                    d.url,
                    safe_join(&game_dir, &d.path)?,
                    d.sha1,
                    Some(d.size),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        downloader.run(tasks, progress).await?;
        let pack = pack.to_owned();
        tokio::task::spawn_blocking(move || extract_overrides(&pack, &game_dir)).await?
    }
    .await;
    match result {
        Ok(()) => Ok(instance),
        Err(e) => {
            if let Err(cleanup) = fs::remove_dir_all_if_exists(&instance.dir) {
                tracing::warn!(error = %cleanup, "cannot remove the half-imported instance");
            }
            Err(e)
        }
    }
}

/// Copia `overrides/` e poi `client-overrides/` (che hanno la precedenza)
/// in `dest`. Ogni nome è non fidato e passa da [`safe_join`].
fn extract_overrides(pack: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(pack).map_err(|e| CoreError::io(pack, e))?;
    extract_overrides_from(file, pack, dest, &OVERRIDES)
}

fn extract_overrides_from<R: Read + Seek>(
    mut reader: R,
    origin: &Path,
    dest: &Path,
    prefixes: &[&str],
) -> Result<()> {
    for prefix in prefixes {
        reader.rewind().map_err(|e| CoreError::io(origin, e))?;
        extract_zip_mapped(&mut reader, origin, dest, |name| name.strip_prefix(prefix))?;
    }
    Ok(())
}

/// Scarica l'ultima versione Fabric di un modpack Modrinth in `cache_dir` e
/// la importa come nuova istanza.
pub async fn install_from_modrinth(
    api: &Modrinth,
    manager: &InstanceManager,
    downloader: &Downloader,
    cache_dir: &Path,
    project: &str,
    progress: Option<ProgressFn<'_>>,
) -> Result<Instance> {
    let path = download_from_modrinth(api, downloader, cache_dir, project).await?;
    let result = import(manager, downloader, &path, progress).await;
    let _ = std::fs::remove_file(&path);
    let instance = result?;
    project_icon(api, downloader, project, &instance).await;
    Ok(instance)
}

/// Scarica in `cache_dir` il `.mrpack` dell'ultima versione di un modpack
/// Modrinth (verificato con SHA1, solo da host ammessi); chi lo usa poi lo
/// cancella.
pub async fn download_from_modrinth(
    api: &Modrinth,
    downloader: &Downloader,
    cache_dir: &Path,
    project: &str,
) -> Result<PathBuf> {
    let loaders = ContentKind::Modpack.loaders(None)?;
    let version = preferred(api.versions(project, loaders, &[]).await?).ok_or_else(|| {
        ContentError::NoCompatibleVersion {
            project: project.to_owned(),
            game: "Fabric".to_owned(),
        }
    })?;
    let file = version
        .files
        .iter()
        .find(|f| f.filename.ends_with(".mrpack"))
        .ok_or_else(|| ContentError::InvalidPack("no .mrpack file in this version".into()))?;
    if !is_trusted_download(&file.url) {
        return Err(ContentError::UntrustedUrl(file.url.clone()));
    }
    // L'id arriva dalla risposta dell'API: validato prima di diventare un nome di file.
    let path = cache_dir.join(format!("{}.mrpack", checked_id(&version.id)?));
    downloader
        .fetch(DownloadTask::new(
            &file.url,
            path.clone(),
            file.hashes.sha1.clone(),
            Some(file.size),
        ))
        .await?;
    Ok(path)
}

/// Prepara un server da un `.mrpack`: scarica in `dest` i file che servono al
/// server (stesse verifiche dell'import: percorsi sicuri, host ammessi,
/// SHA1) e copia `overrides/` e `server-overrides/`. Il loader lo installa
/// chi chiama, da [`requirements`].
pub async fn install_server_files(
    downloader: &Downloader,
    pack: &Path,
    dest: &Path,
    progress: Option<ProgressFn<'_>>,
) -> Result<()> {
    let index = read_index(pack)?;
    let tasks = validated_downloads(&index, true)?
        .into_iter()
        .map(|d| {
            Ok(DownloadTask::new(
                d.url,
                safe_join(dest, &d.path)?,
                d.sha1,
                Some(d.size),
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    downloader.run(tasks, progress).await?;
    let (pack, dest) = (pack.to_owned(), dest.to_owned());
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&pack).map_err(|e| CoreError::io(&pack, e))?;
        extract_overrides_from(file, &pack, &dest, &SERVER_OVERRIDES)
    })
    .await?
}

/// Usa l'icona del progetto Modrinth come immagine della nuova istanza.
async fn project_icon(api: &Modrinth, downloader: &Downloader, project: &str, instance: &Instance) {
    if let Some(url) = api.project(project).await.ok().and_then(|p| p.icon_url) {
        apply_icon(downloader, &url, instance).await;
    }
}

/// Host da cui si accettano le icone dei progetti (CDN di Modrinth e
/// CurseForge).
const ICON_HOSTS: &[&str] = &["cdn.modrinth.com", "media.forgecdn.net"];

/// Scarica un'icona e la usa come immagine dell'istanza. Facoltativa: se
/// non si scarica o è in un formato non gestito (es. SVG), l'istanza resta
/// senza immagine. Non ha un hash da verificare: viene solo mostrata, dopo
/// il controllo del formato.
pub(crate) async fn apply_icon(downloader: &Downloader, url: &str, instance: &Instance) {
    let trusted = reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https" && u.host_str().is_some_and(|h| ICON_HOSTS.contains(&h))
    });
    if !trusted {
        return;
    }
    let result = match downloader.get_bytes(url).await {
        Ok(bytes) => instance.set_icon(Some(&bytes)).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "cannot use the modpack icon");
    }
}

/// Esito di un export.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExportSummary {
    /// File scaricabili da Modrinth, elencati nell'indice.
    pub linked: usize,
    /// File copiati dentro il pack (sconosciuti a Modrinth, config…).
    pub bundled: usize,
}

/// Esporta un'istanza in un `.mrpack`. Le mod, i resource pack e gli shader
/// attivi che Modrinth conosce finiscono nell'indice; gli altri file, la
/// cartella `config` e `options.txt` vanno negli override.
pub async fn export(api: &Modrinth, instance: &Instance, out: &Path) -> Result<ExportSummary> {
    let mut files = Vec::new();
    let mut bundled: Vec<(String, PathBuf)> = Vec::new();
    for dir in ContentDir::ALL {
        for item in installed(api, None, instance, dir, false).await? {
            if !item.file.enabled {
                continue;
            }
            let relative = format!("{}/{}", dir.dir_name(), item.file.file);
            let linked = item
                .version
                .as_ref()
                .and_then(|v| v.file_with_sha1(&item.sha1))
                .filter(|f| f.hashes.sha512.is_some() && is_trusted_download(&f.url));
            match linked {
                Some(f) => files.push(PackFile {
                    path: relative,
                    hashes: PackHashes {
                        sha1: item.sha1.to_string(),
                        sha512: f.hashes.sha512.clone(),
                    },
                    env: None,
                    downloads: vec![f.url.clone()],
                    file_size: f.size,
                }),
                None => bundled.push((relative, item.path)),
            }
        }
    }

    let mut dependencies =
        BTreeMap::from([("minecraft".to_owned(), instance.config.version.clone())]);
    if let Some(loader) = &instance.config.loader {
        dependencies.insert(
            loader.kind.pack_dependency().to_owned(),
            loader.version.clone(),
        );
    }
    let index = PackIndex {
        format_version: 1,
        game: "minecraft".into(),
        version_id: "1.0.0".into(),
        name: instance.config.name.clone(),
        summary: None,
        files,
        dependencies,
    };
    let summary = ExportSummary {
        linked: index.files.len(),
        bundled: bundled.len(),
    };

    let game_dir = instance.game_dir();
    let out = out.to_owned();
    tokio::task::spawn_blocking(move || write_pack(&out, &index, &game_dir, &bundled)).await??;
    Ok(summary)
}

fn write_pack(
    out: &Path,
    index: &PackIndex,
    game_dir: &Path,
    bundled: &[(String, PathBuf)],
) -> Result<()> {
    let tmp = fs::sibling_with_suffix(out, &format!("{}.part", std::process::id()));
    let result = (|| {
        let file = std::fs::File::create(&tmp).map_err(|e| CoreError::io(&tmp, e))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        let zip_err = zip_error(out);
        let io_err = |e| CoreError::io(out, e);

        zip.start_file(INDEX, options).map_err(&zip_err)?;
        let json = serde_json::to_vec_pretty(index).expect("index serializes");
        zip.write_all(&json).map_err(io_err)?;

        let mut extra: Vec<(String, PathBuf)> = bundled.to_vec();
        let options_txt = game_dir.join("options.txt");
        if options_txt.is_file() {
            extra.push(("options.txt".into(), options_txt));
        }
        collect_files(&game_dir.join("config"), "config", &mut extra)?;
        for (relative, path) in extra {
            zip.start_file(format!("overrides/{relative}"), options)
                .map_err(&zip_err)?;
            let mut source = std::fs::File::open(&path).map_err(|e| CoreError::io(&path, e))?;
            std::io::copy(&mut source, &mut zip).map_err(|e| CoreError::io(&path, e))?;
        }
        zip.finish().map_err(&zip_err)?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(fs::rename_replacing(&tmp, out)?),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// File sotto `dir` (ricorsivo, senza seguire i link), con percorso
/// relativo che inizia con `prefix`.
fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|e| CoreError::io(dir, e))?;
        let kind = entry.file_type().map_err(|e| CoreError::io(dir, e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("{prefix}/{name}");
        if kind.is_dir() {
            collect_files(&entry.path(), &relative, out)?;
        } else if kind.is_file() {
            out.push((relative, entry.path()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use hyper_core::Paths;
    use zip::write::SimpleFileOptions;

    use super::*;

    fn index_json(files: serde_json::Value, deps: serde_json::Value) -> String {
        serde_json::json!({
            "formatVersion": 1, "game": "minecraft", "versionId": "1.0", "name": "Test: Pack/1",
            "files": files, "dependencies": deps
        })
        .to_string()
    }

    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, data) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    fn file(path: &str, url: &str) -> serde_json::Value {
        serde_json::json!({
            "path": path, "hashes": {"sha1": "a".repeat(40), "sha512": "b"},
            "downloads": [url], "fileSize": 1
        })
    }

    #[test]
    fn reads_requirements_and_rejects_other_loaders() {
        let ok: PackIndex = serde_json::from_str(&index_json(
            serde_json::json!([]),
            serde_json::json!({"minecraft": "26.3", "fabric-loader": "0.20.1"}),
        ))
        .unwrap();
        let (game, loader) = requirements(&ok).unwrap();
        assert_eq!(game, "26.3");
        assert_eq!(loader.unwrap().version, "0.20.1");

        let forge: PackIndex = serde_json::from_str(&index_json(
            serde_json::json!([]),
            serde_json::json!({"minecraft": "1.20.1", "forge": "47.2.0"}),
        ))
        .unwrap();
        assert_eq!(
            requirements(&forge).unwrap().1.unwrap().kind,
            LoaderKind::Forge
        );
        let liteloader: PackIndex = serde_json::from_str(&index_json(
            serde_json::json!([]),
            serde_json::json!({"minecraft": "1.12.2", "liteloader": "1.0"}),
        ))
        .unwrap();
        assert!(matches!(
            requirements(&liteloader),
            Err(ContentError::UnsupportedLoader(l)) if l == "liteloader"
        ));
    }

    #[test]
    fn validates_paths_urls_and_hashes() {
        let index = |f| -> PackIndex {
            serde_json::from_str(&index_json(
                serde_json::json!([f]),
                serde_json::json!({"minecraft": "26.3"}),
            ))
            .unwrap()
        };
        let good = "https://cdn.modrinth.com/data/x/versions/y/a.jar";
        assert_eq!(
            validated_downloads(&index(file("mods/a.jar", good)), false)
                .unwrap()
                .len(),
            1
        );
        for evil in [
            "../a.jar",
            "mods/../../a.jar",
            "C:/a.jar",
            "/a.jar",
            "\\\\srv\\a.jar",
        ] {
            assert!(
                validated_downloads(&index(file(evil, good)), false).is_err(),
                "{evil}"
            );
        }
        assert!(matches!(
            validated_downloads(
                &index(file("mods/a.jar", "https://evil.example/a.jar")),
                false
            ),
            Err(ContentError::UntrustedUrl(_))
        ));
        let mut bad_hash = file("mods/a.jar", good);
        bad_hash["hashes"]["sha1"] = "nope".into();
        assert!(validated_downloads(&index(bad_hash), false).is_err());
        // I file solo-server vengono saltati.
        let mut server = file("mods/s.jar", "https://evil.example/s.jar");
        server["env"] = serde_json::json!({"client": "unsupported", "server": "required"});
        assert!(
            validated_downloads(&index(server.clone()), false)
                .unwrap()
                .is_empty()
        );
        // Per un server invece servono, e l'URL non ammesso viene rifiutato.
        assert!(validated_downloads(&index(server), true).is_err());
    }

    #[test]
    fn overrides_cannot_escape_the_instance() {
        for evil in [
            "overrides/../evil.txt",
            "client-overrides/../../evil.txt",
            "overrides/C:/evil.txt",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let dest = dir.path().join("game");
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            writer
                .start_file(evil, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"pwned").unwrap();
            let mut cursor = writer.finish().unwrap();
            cursor.set_position(0);
            let result = extract_overrides_from(cursor, Path::new("p.mrpack"), &dest, &OVERRIDES);
            assert!(result.is_err(), "{evil} accepted");
            assert!(!dir.path().join("evil.txt").exists());
        }
    }

    #[tokio::test]
    async fn imports_overrides_with_client_precedence() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("p.mrpack");
        let index = index_json(
            serde_json::json!([]),
            serde_json::json!({"minecraft": "26.3", "fabric-loader": "0.20.1"}),
        );
        write_zip(
            &pack,
            &[
                (INDEX, index.as_bytes()),
                ("overrides/config/a.toml", b"common"),
                ("overrides/options.txt", b"fov:70"),
                ("client-overrides/config/a.toml", b"client"),
            ],
        );
        let manager = InstanceManager::new(Paths::new(dir.path().join("root")));
        let downloader = Downloader::new(2).unwrap();
        let instance = import(&manager, &downloader, &pack, None).await.unwrap();
        assert_eq!(instance.config.name, "Test Pack 1");
        assert_eq!(instance.config.loader.unwrap().version, "0.20.1");
        let game = instance.dir.join("minecraft");
        assert_eq!(
            std::fs::read(game.join("config/a.toml")).unwrap(),
            b"client"
        );
        assert_eq!(std::fs::read(game.join("options.txt")).unwrap(), b"fov:70");

        // Lo stesso pack di nuovo: nome libero successivo.
        let again = import(&manager, &downloader, &pack, None).await.unwrap();
        assert_eq!(again.config.name, "Test Pack 1 (2)");
    }

    #[tokio::test]
    async fn failed_import_leaves_no_instance() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("p.mrpack");
        let index = index_json(
            serde_json::json!([]),
            serde_json::json!({"minecraft": "26.3"}),
        );
        write_zip(
            &pack,
            &[(INDEX, index.as_bytes()), ("overrides/../evil.txt", b"x")],
        );
        let manager = InstanceManager::new(Paths::new(dir.path().join("root")));
        let downloader = Downloader::new(2).unwrap();
        assert!(import(&manager, &downloader, &pack, None).await.is_err());
        assert!(manager.list().unwrap().is_empty());
    }

    #[test]
    fn folder_names_are_safe() {
        assert_eq!(folder_name("Fabulously Optimized"), "Fabulously Optimized");
        assert_eq!(folder_name("A: B/C"), "A B C");
        assert_eq!(folder_name("..."), "Modpack");
        assert_eq!(folder_name("con."), "con");
    }
}
