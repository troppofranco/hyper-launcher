//! Creazione di un server: versioni disponibili, download del server jar
//! (Vanilla da Mojang verificato con SHA1, Paper dall'API ufficiale di
//! PaperMC verificato con SHA256), Java e i file iniziali.

use hyper_core::fs;
use hyper_installer::loader::compare_versions;
use hyper_installer::{DownloadTask, Installer, Progress, Stage, VersionKind};
use reqwest::Url;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::{JAR, ServerConfig, ServerKind, ServerManager};
use crate::properties::Properties;
use crate::{Result, ServerError};

const PAPER_API: &str = "https://fill.papermc.io/v3/projects/paper";
/// Host da cui PaperMC serve i jar: tutto il resto viene rifiutato.
const PAPER_DOWNLOAD_HOST: &str = "fill-data.papermc.io";

/// Fasi della creazione, per l'avanzamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateStage {
    Download,
    Java,
    Setup,
}

impl CreateStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Download => "download",
            Self::Java => "java",
            Self::Setup => "setup",
        }
    }
}

pub type CreateProgress<'a> = &'a (dyn Fn(CreateStage, usize, usize) + Sync);

/// Richiesta di un nuovo server.
#[derive(Debug, Clone)]
pub struct NewServer {
    pub name: String,
    pub kind: ServerKind,
    pub version: String,
    pub memory_mb: u32,
    pub port: u16,
    pub keep_running: bool,
    pub autostart: bool,
    /// L'utente ha accettato l'EULA di Minecraft.
    pub eula: bool,
}

#[derive(Deserialize)]
struct PaperProject {
    versions: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct PaperBuild {
    id: u32,
    downloads: std::collections::HashMap<String, PaperDownload>,
}

#[derive(Deserialize)]
struct PaperDownload {
    name: String,
    url: String,
    size: u64,
    checksums: PaperChecksums,
}

#[derive(Deserialize)]
struct PaperChecksums {
    sha256: String,
}

async fn get_json<T: for<'de> Deserialize<'de>>(installer: &Installer, url: &str) -> Result<T> {
    let bytes = installer.downloader().get_bytes(url).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Versioni installabili, dalla più recente: le release di Minecraft per
/// Vanilla, quelle stabili supportate da Paper.
pub async fn versions(installer: &Installer, kind: ServerKind) -> Result<Vec<String>> {
    match kind {
        ServerKind::Vanilla => {
            let manifest = installer.fetch_manifest().await?;
            Ok(manifest
                .versions
                .iter()
                .filter(|v| v.kind == VersionKind::Release)
                .map(|v| v.id.clone())
                .collect())
        }
        ServerKind::Paper => {
            let project: PaperProject = get_json(installer, PAPER_API).await?;
            let mut list: Vec<String> = project
                .versions
                .into_values()
                .flatten()
                // Solo le versioni finali (niente -pre, -rc).
                .filter(|v| v.chars().all(|c| c.is_ascii_digit() || c == '.'))
                .collect();
            list.sort_by(|a, b| compare_versions(b, a));
            list.dedup();
            Ok(list)
        }
    }
}

fn check_sha256(bytes: &[u8], expected: &str, file: &str) -> Result<()> {
    let actual = hex::encode(Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(ServerError::HashMismatch {
            file: file.to_owned(),
        })
    }
}

/// Scarica il jar di Paper (ultima build della versione) verificandolo con
/// SHA256; restituisce il numero di build.
async fn download_paper(
    installer: &Installer,
    version: &str,
    dest: &std::path::Path,
) -> Result<u32> {
    if !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(ServerError::Invalid(format!("invalid version {version:?}")));
    }
    let build: PaperBuild = get_json(
        installer,
        &format!("{PAPER_API}/versions/{version}/builds/latest"),
    )
    .await?;
    let file = build
        .downloads
        .get("server:default")
        .ok_or_else(|| ServerError::Unsupported("Paper".into()))?;
    let url = Url::parse(&file.url).map_err(|_| ServerError::UntrustedUrl(file.url.clone()))?;
    if url.scheme() != "https" || url.host_str() != Some(PAPER_DOWNLOAD_HOST) {
        return Err(ServerError::UntrustedUrl(file.url.clone()));
    }
    let bytes = installer.downloader().get_bytes(url.as_str()).await?;
    if bytes.len() as u64 != file.size {
        return Err(ServerError::HashMismatch {
            file: file.name.clone(),
        });
    }
    check_sha256(&bytes, &file.checksums.sha256, &file.name)?;
    fs::write_atomic(dest, &bytes)?;
    Ok(build.id)
}

/// Crea un server: scarica il jar e Java, accetta l'EULA (solo se l'utente
/// l'ha fatto), prepara `server.properties` e salva la configurazione. Se
/// qualcosa va storto la cartella appena creata viene tolta.
pub async fn create(
    installer: &Installer,
    manager: &ServerManager,
    request: &NewServer,
    progress: Option<CreateProgress<'_>>,
) -> Result<ServerConfig> {
    if !request.eula {
        return Err(ServerError::EulaRequired);
    }
    // Nome, memoria e porta si controllano prima di scaricare qualunque cosa.
    let mut config = ServerConfig {
        name: request.name.clone(),
        kind: request.kind,
        version: request.version.clone(),
        build: None,
        memory_mb: request.memory_mb,
        port: request.port,
        keep_running: request.keep_running,
        autostart: request.keep_running && request.autostart,
        created: hyper_core::unix_now(),
        java_component: String::new(),
    };
    config.validate_settings()?;
    if manager.exists(&request.name) {
        return Err(ServerError::AlreadyExists(request.name.clone()));
    }
    let dir = manager.dir(&request.name)?;
    if dir.exists() {
        return Err(ServerError::AlreadyExists(request.name.clone()));
    }
    let report = |stage, done, total| {
        if let Some(p) = progress {
            p(stage, done, total);
        }
    };
    let manifest = installer.fetch_manifest().await?;
    let entry = manifest
        .find(&request.version)
        .ok_or_else(|| {
            ServerError::Invalid(format!("unknown Minecraft version {}", request.version))
        })?
        .clone();

    let result = async {
        std::fs::create_dir_all(&dir)?;
        report(CreateStage::Download, 0, 1);
        let version = installer.vanilla_version(&entry).await?;
        let jar = dir.join(JAR);
        config.build = match request.kind {
            ServerKind::Vanilla => {
                let server = version
                    .downloads
                    .as_ref()
                    .and_then(|d| d.server.as_ref())
                    .ok_or_else(|| ServerError::Unsupported("the dedicated server".into()))?;
                installer
                    .downloader()
                    .fetch(DownloadTask::new(
                        &server.url,
                        jar,
                        server.sha1.clone(),
                        Some(server.size),
                    ))
                    .await?;
                None
            }
            ServerKind::Paper => Some(download_paper(installer, &request.version, &jar).await?),
        };
        report(CreateStage::Download, 1, 1);

        let component = version.java_component().to_owned();
        let java_progress = |_: Stage, p: Progress| report(CreateStage::Java, p.done, p.total);
        installer
            .install_java(&component, Some(&java_progress))
            .await?;

        report(CreateStage::Setup, 0, 1);
        fs::write_atomic(
            &dir.join("eula.txt"),
            b"# Accettata dall'utente in Hyper Launcher: https://aka.ms/MinecraftEULA\neula=true\n",
        )?;
        let mut props = Properties::default();
        props.set("server-port", request.port.to_string());
        props.set("query.port", request.port.to_string());
        props.set("motd", request.name.clone());
        props.save(&dir)?;

        config.java_component = component;
        manager.save(&config)?;
        report(CreateStage::Setup, 1, 1);
        Ok(())
    }
    .await;
    match result {
        Ok(()) => Ok(config),
        Err(e) => {
            let _ = fs::remove_dir_all_if_exists(&dir);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_versions_numerically() {
        let mut v = vec![
            "1.9".to_owned(),
            "1.21.4".into(),
            "26.3".into(),
            "1.21.10".into(),
        ];
        v.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(v, ["26.3", "1.21.10", "1.21.4", "1.9"]);
    }

    #[test]
    fn checks_sha256() {
        // SHA256 di "abc".
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(check_sha256(b"abc", abc, "x").is_ok());
        assert!(check_sha256(b"abd", abc, "x").is_err());
    }
}
