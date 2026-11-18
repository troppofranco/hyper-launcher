//! Aggiornamento automatico del launcher dalle release GitHub.
//!
//! Ogni push su `main` pubblica una release con il setup e il suo SHA1,
//! taggata con la versione (`v0.1.<N>`; le prime usavano `build-<N>`). Il
//! launcher conosce il proprio numero di release N (incorporato in
//! compilazione); se l'ultima release ha un numero più alto, scarica il
//! setup, ne verifica l'hash e lo esegue in modo silenzioso, che aggiorna
//! l'installazione sul posto.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::{DownloadTask, Downloader, InstallerError, Result, Sha1Hex};

const SETUP_ASSET: &str = "HyperLauncher-Setup.exe";
const SHA1_ASSET: &str = "HyperLauncher-Setup.exe.sha1";
/// Numero di release N dal tag: `v<major>.<minor>.<N>` oppure `build-<N>`
/// (il formato delle prime release, che i launcher più vecchi cercano ancora).
fn release_number(tag: &str) -> Option<u64> {
    if let Some(n) = tag.strip_prefix("build-") {
        return n.parse().ok();
    }
    let mut parts = tag.strip_prefix('v')?.split('.');
    let (major, minor, n) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || major.parse::<u64>().is_err() || minor.parse::<u64>().is_err() {
        return None;
    }
    n.parse().ok()
}

/// Una versione più nuova disponibile.
#[derive(Debug, Clone)]
pub struct AvailableUpdate {
    pub build: u64,
    /// Pagina della release su GitHub.
    pub page_url: String,
    setup_url: String,
    setup_size: u64,
    sha1_url: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Numero di build incorporato in compilazione dalla GitHub Action (variabile
/// `HYPER_BUILD`, uguale per tutti i binari); `None` nelle build locali, che
/// non si aggiornano mai da sole.
pub fn current_build() -> Option<u64> {
    option_env!("HYPER_BUILD").and_then(|b| b.parse().ok())
}

/// Versione mostrata all'utente per un numero di release: `<major>.<minor>.<N>`
/// (es. 0.1.12, dalla versione del workspace). `None` = compilazione locale,
/// che diventa `<versione>-dev`. Il tag delle release resta `build-<N>`: lo
/// cercano anche i launcher già installati.
pub fn version_name(build: Option<u64>) -> String {
    match build {
        Some(n) => format!(
            "{}.{}.{n}",
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR")
        ),
        None => format!("{}-dev", env!("CARGO_PKG_VERSION")),
    }
}

/// Versione di questo binario (vedi [`version_name`]).
pub fn current_version() -> String {
    version_name(current_build())
}

/// Repository GitHub (`owner/nome`) da cui arrivano gli aggiornamenti.
pub fn update_repo() -> Option<&'static str> {
    option_env!("HYPER_UPDATE_REPO").filter(|r| !r.is_empty())
}

/// Aggiornamento disponibile per questa build; `None` se aggiornati o in
/// una build di sviluppo.
pub async fn available() -> Result<Option<AvailableUpdate>> {
    match (current_build(), update_repo()) {
        (Some(build), Some(repo)) => check(&latest_release_url(repo), build).await,
        _ => Ok(None),
    }
}

/// Flusso completo: se c'è una build più nuova ne scarica e verifica il
/// setup in `dir`, lo avvia in modo silenzioso e restituisce la build;
/// il chiamante deve poi uscire. Pulisce comunque i setup vecchi.
pub async fn update_to_latest(
    downloader: &Downloader,
    dir: &Path,
    relaunch: bool,
) -> Result<Option<u64>> {
    let Some(update) = available().await? else {
        clean_old_setups(dir, None);
        return Ok(None);
    };
    let setup = download(downloader, &update, dir).await?;
    clean_old_setups(dir, Some(&setup));
    run_installer(&setup, relaunch)?;
    Ok(Some(update.build))
}

/// URL dell'API GitHub per l'ultima release di `repo` (`owner/nome`).
pub fn latest_release_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

/// Chiede a GitHub l'ultima release; `None` se non è più nuova di
/// `current_build`. Usa un timeout breve: il controllo avviene a ogni avvio
/// e non deve rallentarlo quando la rete è lenta o assente.
pub async fn check(api_url: &str, current_build: u64) -> Result<Option<AvailableUpdate>> {
    let http = |source| InstallerError::Http {
        url: api_url.to_owned(),
        source,
    };
    let client = reqwest::Client::builder()
        .user_agent(hyper_core::USER_AGENT)
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(InstallerError::HttpClient)?;
    let release: Release = client
        .get(api_url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(http)?
        .json()
        .await
        .map_err(http)?;
    Ok(newer_than(&release, current_build))
}

fn newer_than(release: &Release, current_build: u64) -> Option<AvailableUpdate> {
    let build = release_number(&release.tag_name)?;
    if build <= current_build {
        return None;
    }
    let asset = |name: &str| release.assets.iter().find(|a| a.name == name);
    let setup = asset(SETUP_ASSET)?;
    let sha1 = asset(SHA1_ASSET)?;
    Some(AvailableUpdate {
        build,
        page_url: release.html_url.clone(),
        setup_url: setup.browser_download_url.clone(),
        setup_size: setup.size,
        sha1_url: sha1.browser_download_url.clone(),
    })
}

/// Scarica il setup in `dir` verificandone SHA1 e dimensione; restituisce
/// il percorso del file pronto da eseguire.
pub async fn download(
    downloader: &Downloader,
    update: &AvailableUpdate,
    dir: &Path,
) -> Result<PathBuf> {
    let text = downloader.get_bytes(&update.sha1_url).await?;
    let sha1: Sha1Hex = String::from_utf8_lossy(&text)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .parse()?;
    let path = dir.join(format!("HyperLauncher-Setup-build-{}.exe", update.build));
    downloader
        .fetch(DownloadTask::new(
            &update.setup_url,
            path,
            sha1,
            Some(update.setup_size),
        ))
        .await
}

/// Avvia il setup in modo silenzioso e non attende: il chiamante deve
/// uscire subito, così il setup può sostituire l'eseguibile in uso.
/// Con `relaunch` l'installer riapre l'app grafica a fine aggiornamento.
pub fn run_installer(setup: &Path, relaunch: bool) -> Result<()> {
    let mut command = std::process::Command::new(setup);
    command.args([
        "/VERYSILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/CLOSEAPPLICATIONS",
    ]);
    if relaunch {
        command.arg("/relaunch=1");
    }
    command
        .spawn()
        .map_err(|e| hyper_core::CoreError::io(setup, e))?;
    Ok(())
}

/// Elimina i setup di aggiornamenti precedenti (tranne `keep`).
pub fn clean_old_setups(dir: &Path, keep: Option<&Path>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_setup = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("HyperLauncher-Setup-build-"));
        if is_setup && Some(path.as_path()) != keep {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_numbers_from_both_tag_styles() {
        assert_eq!(release_number("build-12"), Some(12));
        assert_eq!(release_number("v0.1.14"), Some(14));
        assert_eq!(release_number("v1.2.30"), Some(30));
        assert_eq!(release_number("v0.1"), None);
        assert_eq!(release_number("v0.1.2.3"), None);
        assert_eq!(release_number("0.1.14"), None);
        assert_eq!(release_number("build-x"), None);
        let update = newer_than(&release("v0.1.14", true), 13).unwrap();
        assert_eq!(update.build, 14);
    }

    #[test]
    fn version_names() {
        let major = env!("CARGO_PKG_VERSION_MAJOR");
        let minor = env!("CARGO_PKG_VERSION_MINOR");
        assert_eq!(version_name(Some(12)), format!("{major}.{minor}.12"));
        assert!(version_name(None).ends_with("-dev"));
    }

    fn release(tag: &str, with_sha1: bool) -> Release {
        let mut json = serde_json::json!({
            "tag_name": tag,
            "assets": [
                {"name": SETUP_ASSET, "size": 4_500_000,
                 "browser_download_url": "https://github.com/o/r/releases/download/x/HyperLauncher-Setup.exe"},
            ]
        });
        if with_sha1 {
            json["assets"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"name": SHA1_ASSET, "size": 41,
                "browser_download_url": "https://github.com/o/r/releases/download/x/HyperLauncher-Setup.exe.sha1"}));
        }
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn detects_only_newer_complete_builds() {
        let update = newer_than(&release("build-12", true), 11).unwrap();
        assert_eq!(update.build, 12);
        assert_eq!(update.setup_size, 4_500_000);
        assert!(update.sha1_url.ends_with(".sha1"));

        assert!(newer_than(&release("build-12", true), 12).is_none());
        assert!(newer_than(&release("build-12", true), 30).is_none());
        // Release vecchio stile, malformata o senza hash: ignorata.
        assert!(newer_than(&release("v0.1.0", true), 0).is_none());
        assert!(newer_than(&release("build-x", true), 0).is_none());
        assert!(newer_than(&release("build-12", false), 0).is_none());
    }

    #[test]
    fn cleans_only_old_update_setups() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("HyperLauncher-Setup-build-3.exe");
        let new = dir.path().join("HyperLauncher-Setup-build-4.exe");
        let other = dir.path().join("keep.txt");
        for f in [&old, &new, &other] {
            std::fs::write(f, b"x").unwrap();
        }
        clean_old_setups(dir.path(), Some(&new));
        assert!(!old.exists());
        assert!(new.exists());
        assert!(other.exists());
    }
}
