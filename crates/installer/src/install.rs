//! Orchestrazione: dal manifest a una versione pronta da avviare.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use hyper_core::{Paths, fs};

use crate::assets::AssetIndex;
use crate::download::Progress;
use crate::java::{self, RuntimeIndex, RuntimeManifest};
use crate::loader::LoaderSpec;
use crate::manifest::{ManifestVersion, VERSION_MANIFEST_URL, VersionManifest};
use crate::version::{RuleContext, VersionJson};
use crate::{DownloadTask, Downloader, InstallerError, Result, Sha1Hex};

/// Fase di installazione, per i report di avanzamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Client jar, librerie, configurazione di logging.
    Libraries,
    Assets,
    Java,
}

impl Stage {
    /// Nome stabile della fase (`libraries`, `assets`, `java`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Libraries => "libraries",
            Self::Assets => "assets",
            Self::Java => "java",
        }
    }
}

pub type StageProgressFn<'a> = &'a (dyn Fn(Stage, Progress) + Sync);

/// Jar di native da estrarre all'avvio.
#[derive(Debug, Clone)]
pub struct NativeJar {
    pub path: PathBuf,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Argomento JVM con il segnaposto `${path}`.
    pub argument: String,
    pub path: PathBuf,
}

/// Versione con tutti i file presenti e verificati.
#[derive(Debug, Clone)]
pub struct InstalledVersion {
    pub version: VersionJson,
    /// Librerie nell'ordine della version JSON, seguite dal client jar.
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<NativeJar>,
    pub asset_index_id: String,
    pub logging: Option<LoggingConfig>,
    /// Rules con cui sono state scelte le librerie; da riusare per gli argomenti.
    pub rules: RuleContext,
}

impl InstalledVersion {
    /// Componente del runtime Java Mojang richiesto da questa versione.
    pub fn java_component(&self) -> &str {
        self.version.java_component()
    }
}

#[derive(Debug, Clone)]
pub struct Installer {
    pub(crate) paths: Paths,
    pub(crate) downloader: Downloader,
    pub(crate) ctx: RuleContext,
}

impl Installer {
    pub fn new(paths: Paths, concurrency: usize) -> Result<Self> {
        Ok(Self {
            paths,
            downloader: Downloader::new(concurrency)?,
            ctx: RuleContext::current(),
        })
    }

    pub fn downloader(&self) -> &Downloader {
        &self.downloader
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn rule_context(&self) -> &RuleContext {
        &self.ctx
    }

    /// Scarica il version manifest; se la rete non è disponibile usa
    /// l'ultima copia salvata.
    pub async fn fetch_manifest(&self) -> Result<VersionManifest> {
        self.fetch_json_cached(VERSION_MANIFEST_URL, &self.paths.version_manifest_cache())
            .await
    }

    /// Scarica un JSON e ne salva una copia; se la rete non risponde usa
    /// l'ultima copia salvata. Solo gli errori di rete ricadono sulla cache:
    /// un JSON malformato deve emergere.
    pub(crate) async fn fetch_json_cached<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        cache: &Path,
    ) -> Result<T> {
        match self.downloader.get_bytes(url).await {
            Ok(bytes) => {
                let value = parse_json(&bytes, url)?;
                fs::write_atomic(cache, &bytes)?;
                Ok(value)
            }
            Err(e @ InstallerError::Http { .. }) if cache.is_file() => {
                tracing::warn!(error = %e, url, "network unavailable, using cached copy");
                read_json(cache)
            }
            Err(e) => Err(e),
        }
    }

    /// Scarica e verifica version JSON, client, librerie, asset e log config.
    pub async fn install_version(
        &self,
        entry: &ManifestVersion,
        progress: Option<StageProgressFn<'_>>,
    ) -> Result<InstalledVersion> {
        let version = self.vanilla_version(entry).await?;
        self.install_resolved(version, progress).await
    }

    /// Come [`Self::install_version`], con un mod loader sopra la versione
    /// vanilla: il profilo del loader viene fuso sulla version JSON di Mojang.
    pub async fn install_with_loader(
        &self,
        entry: &ManifestVersion,
        loader: &LoaderSpec,
        progress: Option<StageProgressFn<'_>>,
    ) -> Result<InstalledVersion> {
        let vanilla = self.vanilla_version(entry).await?;
        let profile = self.loader_profile(loader, &entry.id, &vanilla).await?;
        self.install_resolved(profile.inherit(vanilla), progress)
            .await
    }

    /// Version JSON di Mojang, scaricata e verificata con lo SHA1 del manifest.
    pub async fn vanilla_version(&self, entry: &ManifestVersion) -> Result<VersionJson> {
        let json_path = self
            .downloader
            .fetch(DownloadTask::new(
                &entry.url,
                self.paths.version_json(&entry.id)?,
                entry.sha1.clone(),
                None,
            ))
            .await?;
        let version: VersionJson = read_json(&json_path)?;
        if version.id != entry.id {
            return Err(InstallerError::UnsupportedVersion {
                version: version.id,
                reason: "version JSON id differs from manifest id".into(),
            });
        }
        Ok(version)
    }

    async fn install_resolved(
        &self,
        version: VersionJson,
        progress: Option<StageProgressFn<'_>>,
    ) -> Result<InstalledVersion> {
        let unsupported = |reason: &str| InstallerError::UnsupportedVersion {
            version: version.id.clone(),
            reason: reason.to_owned(),
        };
        if version.inherits_from.is_some() {
            return Err(unsupported("inheritsFrom must be resolved first"));
        }
        let client = &version
            .downloads
            .as_ref()
            .ok_or_else(|| unsupported("missing client download"))?
            .client;
        let asset_ref = version
            .asset_index
            .as_ref()
            .ok_or_else(|| unsupported("missing asset index"))?;

        // L'asset index è piccolo: lo prendiamo subito, così librerie e asset
        // possono scaricarsi in parallelo.
        let index_path = self
            .downloader
            .fetch(
                asset_ref
                    .download
                    .task(self.paths.asset_index(&asset_ref.id)?),
            )
            .await?;
        let index: AssetIndex = read_json(&index_path)?;
        if index.is_virtual || index.map_to_resources {
            tracing::warn!(
                "asset index {} uses the legacy virtual layout, which is not supported yet: sounds and textures may be missing",
                asset_ref.id
            );
        }

        let mut tasks = Vec::new();
        let (mut classpath, natives) = self.library_tasks(&version, &mut tasks).await?;
        let client_jar = self.paths.version_jar(version.client_jar_id())?;
        tasks.push(client.task(client_jar.clone()));
        classpath.push(client_jar);

        let logging = match version.logging.as_ref().and_then(|l| l.client.as_ref()) {
            Some(client) => {
                let path = self.paths.log_config(&client.file.id)?;
                tasks.push(client.file.download.task(path.clone()));
                Some(LoggingConfig {
                    argument: client.argument.clone(),
                    path,
                })
            }
            None => None,
        };

        let libraries_report = stage_reporter(progress, Stage::Libraries);
        let assets_report = stage_reporter(progress, Stage::Assets);
        let (libs, assets) = tokio::try_join!(
            self.downloader.run(tasks, Some(&libraries_report)),
            self.downloader.run(
                index.object_tasks(&self.paths.asset_objects_dir()),
                Some(&assets_report)
            ),
        )?;
        tracing::debug!(
            libraries = libs.total,
            assets = assets.total,
            downloaded = libs.downloaded + assets.downloaded,
            "version files ready"
        );

        Ok(InstalledVersion {
            asset_index_id: asset_ref.id.clone(),
            classpath,
            natives,
            logging,
            version,
            rules: self.ctx.clone(),
        })
    }

    /// Aggiunge a `tasks` le librerie consentite su questa piattaforma e
    /// restituisce classpath (senza duplicati) e jar di native.
    async fn library_tasks(
        &self,
        version: &VersionJson,
        tasks: &mut Vec<DownloadTask>,
    ) -> Result<(Vec<PathBuf>, Vec<NativeJar>)> {
        let libraries_dir = self.paths.libraries_dir();
        let mut classpath = Vec::new();
        let mut seen = HashSet::new();
        let mut natives = Vec::new();

        for library in version.libraries.iter().filter(|l| l.allowed(&self.ctx)) {
            if library.downloads.is_none() && library.url.is_some() {
                let task = self.maven_library_task(library, &libraries_dir).await?;
                if seen.insert(task.path.clone()) {
                    classpath.push(task.path.clone());
                }
                tasks.push(task);
                continue;
            }
            let artifact = library.downloads.as_ref().and_then(|d| d.artifact.as_ref());
            let native = library.native_artifact(&self.ctx);
            if artifact.is_none() && native.is_none() {
                if library.downloads.is_none() {
                    return Err(InstallerError::UnsupportedLibrary(library.name.clone()));
                }
                continue; // solo classifier per altri OS
            }

            if let Some(artifact) = artifact {
                let task = artifact.download.task_in(&libraries_dir, &artifact.path)?;
                if seen.insert(task.path.clone()) {
                    classpath.push(task.path.clone());
                }
                tasks.push(task);
            }
            if let Some(native) = native {
                let task = native.download.task_in(&libraries_dir, &native.path)?;
                natives.push(NativeJar {
                    path: task.path.clone(),
                    exclude: library.extract_excludes().to_vec(),
                });
                tasks.push(task);
            }
        }
        Ok((classpath, natives))
    }

    /// Libreria nel formato dei mod loader (`name` + `url` di un repository
    /// Maven). L'hash viene dal profilo; se manca, dal file `<jar>.sha1`
    /// pubblicato dallo stesso repository. Mai un file senza hash.
    pub(crate) async fn maven_library_task(
        &self,
        library: &crate::version::Library,
        libraries_dir: &Path,
    ) -> Result<DownloadTask> {
        let unsupported = || InstallerError::UnsupportedLibrary(library.name.clone());
        let repo = library.url.as_deref().ok_or_else(unsupported)?;
        // Solo HTTPS: l'hash del sidecar viene dallo stesso server.
        if !repo.starts_with("https://") {
            return Err(unsupported());
        }
        let relative = crate::version::maven_path(&library.name).ok_or_else(unsupported)?;
        let path = hyper_core::safe_join(libraries_dir, &relative)?;
        let url = format!("{}/{relative}", repo.trim_end_matches('/'));
        let sha1 = match &library.sha1 {
            Some(sha1) => sha1.clone(),
            None => {
                let text = self.downloader.get_bytes(&format!("{url}.sha1")).await?;
                parse_sha1_sidecar(&text)?
            }
        };
        Ok(DownloadTask::new(url, path, sha1, library.size))
    }

    /// Installa (o ri-verifica) il runtime Java Mojang `component` e
    /// restituisce il percorso dell'eseguibile `java`.
    pub async fn install_java(
        &self,
        component: &str,
        progress: Option<StageProgressFn<'_>>,
    ) -> Result<PathBuf> {
        let runtime_dir = self.paths.runtime_dir(component)?;
        let manifest_path = self.paths.runtime_manifest(component)?;

        match self.fetch_runtime_manifest(component, &manifest_path).await {
            Ok(()) => {}
            // Solo gli errori di rete ricadono sulla copia locale: un hash
            // sbagliato o un indice malformato devono emergere.
            Err(e @ InstallerError::Http { .. }) if manifest_path.is_file() => {
                tracing::warn!(error = %e, "cannot refresh Java runtime manifest, using cached copy");
            }
            Err(e) => return Err(e),
        }
        let manifest: RuntimeManifest = read_json(&manifest_path)?;
        let files = manifest.files(&runtime_dir)?;

        let stats = self
            .downloader
            .run(files.tasks, Some(&stage_reporter(progress, Stage::Java)))
            .await?;
        tracing::debug!(
            component,
            files = stats.total,
            downloaded = stats.downloaded,
            "Java runtime ready"
        );

        for path in &files.executables {
            java::mark_executable(path)?;
        }
        let java = java::java_executable(&runtime_dir);
        if !java.is_file() {
            return Err(InstallerError::JavaRuntimeIncomplete(java));
        }
        Ok(java)
    }

    async fn fetch_runtime_manifest(&self, component: &str, dest: &Path) -> Result<()> {
        let platform = java::current_platform().ok_or(InstallerError::UnsupportedPlatform)?;
        let bytes = self
            .downloader
            .get_bytes(java::JAVA_RUNTIME_INDEX_URL)
            .await?;
        let index: RuntimeIndex = parse_json(&bytes, java::JAVA_RUNTIME_INDEX_URL)?;
        let release = java::find_release(&index, platform, component)?;
        tracing::debug!(component, version = %release.version.name, "Java runtime release");
        self.downloader
            .fetch(release.manifest.task(dest.to_owned()))
            .await?;
        Ok(())
    }
}

fn stage_reporter(
    progress: Option<StageProgressFn<'_>>,
    stage: Stage,
) -> impl Fn(Progress) + Sync + '_ {
    move |p| {
        if let Some(report) = progress {
            report(stage, p);
        }
    }
}

pub(crate) fn parse_json<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
    origin: impl std::fmt::Display,
) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|source| InstallerError::Json {
        origin: origin.to_string(),
        source,
    })
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    parse_json(&fs::read(path)?, path.display())
}

/// Primo campo di un file `.sha1` di Maven (`<hash>` o `<hash>  <nome>`).
pub(crate) fn parse_sha1_sidecar(bytes: &[u8]) -> Result<Sha1Hex> {
    String::from_utf8_lossy(bytes)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maven_sha1_sidecars() {
        let hash = "7c7f9831e9f27b303d5d08decb5f4258a9f03296";
        assert_eq!(parse_sha1_sidecar(hash.as_bytes()).unwrap().as_str(), hash);
        assert_eq!(
            parse_sha1_sidecar(format!("{hash}  fabric-loader.jar\n").as_bytes())
                .unwrap()
                .as_str(),
            hash
        );
        assert!(parse_sha1_sidecar(b"<html>302 Found</html>").is_err());
        assert!(parse_sha1_sidecar(b"").is_err());
    }
}
