//! Mod loader. Fabric e Quilt pubblicano un profilo (version JSON con
//! `inheritsFrom`) da fondere sulla versione vanilla; NeoForge e Forge
//! distribuiscono un installer Java che genera quel profilo (e le librerie
//! patchate) nella cartella dati, con il layout di `.minecraft`.

use std::cmp::Ordering;
use std::io::Read;
use std::path::Path;

use hyper_core::{CoreError, fs, safe_component};
use serde::{Deserialize, Serialize};

use crate::install::{parse_json, parse_sha1_sidecar, read_json};
use crate::version::VersionJson;
use crate::{DownloadTask, Installer, InstallerError, Result};

pub const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";
pub const QUILT_META_URL: &str = "https://meta.quiltmc.org/v3";
pub const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";
const NEOFORGE_VERSIONS: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    NeoForge,
    Forge,
}

impl LoaderKind {
    pub const ALL: [Self; 4] = [Self::Fabric, Self::Quilt, Self::NeoForge, Self::Forge];

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
            Self::NeoForge => "NeoForge",
            Self::Forge => "Forge",
        }
    }

    /// Nome usato da Modrinth (loader delle versioni) e nei modpack.
    pub fn modrinth_name(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neoforge",
            Self::Forge => "forge",
        }
    }

    /// Chiave della dipendenza nel `modrinth.index.json` dei modpack.
    pub fn pack_dependency(self) -> &'static str {
        match self {
            Self::Fabric => "fabric-loader",
            Self::Quilt => "quilt-loader",
            Self::NeoForge => "neoforge",
            Self::Forge => "forge",
        }
    }

    pub fn from_pack_dependency(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.pack_dependency() == key)
    }

    /// Se il launcher sa installare questo loader per la versione `game`.
    /// NeoForge esiste dalla 1.20.2 (la 1.20.1 è pubblicata con un altro
    /// nome); di Forge supportiamo solo gli installer moderni (1.17+), che
    /// hanno `version.json` e l'opzione `--installClient`.
    pub fn supports_game(self, game: &str) -> bool {
        let mut parts = game.split('.').map(str::parse::<u32>);
        let (major, minor, patch) = match (parts.next(), parts.next(), parts.next()) {
            (Some(Ok(a)), Some(Ok(b)), None) => (a, b, 0),
            (Some(Ok(a)), Some(Ok(b)), Some(Ok(c))) => (a, b, c),
            // Snapshot e versioni non numeriche: lo dice l'API del loader.
            _ => return matches!(self, Self::Fabric | Self::Quilt),
        };
        let at_least = |b: u32, c: u32| major > 1 || (minor, patch) >= (b, c);
        match self {
            Self::Fabric | Self::Quilt => true,
            Self::NeoForge => at_least(20, 2),
            Self::Forge => at_least(17, 0),
        }
    }
}

/// Loader scelto per un'istanza (salvato in `instance.toml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoaderSpec {
    pub kind: LoaderKind,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderVersion {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

#[derive(Debug, Deserialize)]
struct MetaLoaderEntry {
    loader: LoaderVersion,
}

#[derive(Debug, Deserialize)]
struct NeoForgeVersions {
    versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ForgePromotions {
    promos: std::collections::HashMap<String, String>,
}

impl Installer {
    /// Versioni del loader disponibili per una versione di gioco, dalla più
    /// recente (vuoto se il loader non supporta quella versione).
    pub async fn loader_versions(
        &self,
        kind: LoaderKind,
        game: &str,
    ) -> Result<Vec<LoaderVersion>> {
        let game = safe_component(game)?;
        if !kind.supports_game(game) {
            return Ok(Vec::new());
        }
        let mut versions = match kind {
            LoaderKind::Fabric => self.meta_versions(FABRIC_META_URL, game).await?,
            LoaderKind::Quilt => {
                // Quilt non segnala le stabili: lo sono quelle senza "-beta".
                let mut list = self.meta_versions(QUILT_META_URL, game).await?;
                for v in &mut list {
                    v.stable = !v.version.contains('-');
                }
                list
            }
            LoaderKind::NeoForge => {
                let bytes = self.downloader.get_bytes(NEOFORGE_VERSIONS).await?;
                let all: NeoForgeVersions = parse_json(&bytes, NEOFORGE_VERSIONS)?;
                neoforge_for(game, &all.versions)
            }
            LoaderKind::Forge => {
                let url = format!("{FORGE_MAVEN}/net/minecraftforge/forge/maven-metadata.xml");
                let (xml, promos) = tokio::try_join!(
                    self.downloader.get_bytes(&url),
                    self.downloader.get_bytes(FORGE_PROMOTIONS)
                )?;
                let promos: ForgePromotions = parse_json(&promos, FORGE_PROMOTIONS)?;
                forge_for(game, &String::from_utf8_lossy(&xml), &promos.promos)
            }
        };
        versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
        Ok(versions)
    }

    async fn meta_versions(&self, base: &str, game: &str) -> Result<Vec<LoaderVersion>> {
        let url = format!("{base}/versions/loader/{game}");
        let bytes = self.downloader.get_bytes(&url).await?;
        let entries: Vec<MetaLoaderEntry> = parse_json(&bytes, &url)?;
        Ok(entries.into_iter().map(|e| e.loader).collect())
    }

    /// Ultima versione stabile del loader per `game`.
    pub async fn latest_loader(&self, kind: LoaderKind, game: &str) -> Result<LoaderSpec> {
        let versions = self.loader_versions(kind, game).await?;
        let version = versions
            .iter()
            .find(|v| v.stable)
            .or(versions.first())
            .ok_or_else(|| InstallerError::UnsupportedVersion {
                version: game.to_owned(),
                reason: format!("{} does not support it", kind.display_name()),
            })?;
        Ok(LoaderSpec {
            kind,
            version: version.version.clone(),
        })
    }

    /// Profilo del loader per `game`. Per Fabric e Quilt viene dalla loro
    /// API (salvato in `versions/` come cache); per NeoForge e Forge lo
    /// genera il loro installer, eseguito una volta sola.
    pub(crate) async fn loader_profile(
        &self,
        spec: &LoaderSpec,
        game: &str,
        vanilla: &VersionJson,
    ) -> Result<VersionJson> {
        let (game, loader) = checked_versions(spec, game)?;
        let profile = match spec.kind {
            LoaderKind::Fabric => {
                self.meta_profile(FABRIC_META_URL, "fabric", game, loader)
                    .await?
            }
            LoaderKind::Quilt => {
                self.meta_profile(QUILT_META_URL, "quilt", game, loader)
                    .await?
            }
            LoaderKind::NeoForge | LoaderKind::Forge => {
                self.installer_profile(spec.kind, game, loader, vanilla)
                    .await?
            }
        };
        if profile.inherits_from.as_deref() != Some(game) {
            return Err(InstallerError::UnsupportedVersion {
                version: profile.id,
                reason: format!("loader profile does not inherit from {game}"),
            });
        }
        Ok(profile)
    }

    /// Profilo pubblicato da un'API "meta" (Fabric, Quilt). Come il version
    /// manifest arriva in HTTPS senza hash noto; le librerie che elenca sono
    /// invece verificate una a una.
    async fn meta_profile(
        &self,
        base: &str,
        name: &str,
        game: &str,
        loader: &str,
    ) -> Result<VersionJson> {
        let url = format!("{base}/versions/loader/{game}/{loader}/profile/json");
        let cache = self
            .paths
            .version_json(&format!("{name}-loader-{loader}-{game}"))?;
        // Il profilo di una coppia loader/gioco fissata non cambia: la copia
        // salvata, se leggibile, evita una richiesta a ogni avvio.
        match cache
            .is_file()
            .then(|| read_json::<VersionJson>(&cache).ok())
        {
            Some(Some(profile)) => Ok(profile),
            _ => self.fetch_json_cached(&url, &cache).await,
        }
    }

    /// Esegue l'installer ufficiale di NeoForge o Forge nella cartella dati
    /// (stesso layout di `.minecraft`): scrive il profilo in `versions/` e le
    /// librerie, comprese quelle generate dai suoi "processor". L'installer
    /// è scaricato dal Maven ufficiale in HTTPS e verificato con lo SHA1
    /// pubblicato accanto; le librerie che scarica le verifica lui stesso, e
    /// il launcher le riverifica con gli hash del profilo a ogni avvio.
    async fn installer_profile(
        &self,
        kind: LoaderKind,
        game: &str,
        loader: &str,
        vanilla: &VersionJson,
    ) -> Result<VersionJson> {
        let (expected_id, url) = match kind {
            LoaderKind::NeoForge => (
                format!("neoforge-{loader}"),
                format!(
                    "{NEOFORGE_MAVEN}/net/neoforged/neoforge/{loader}/neoforge-{loader}-installer.jar"
                ),
            ),
            _ => (
                format!("{game}-forge-{loader}"),
                format!(
                    "{FORGE_MAVEN}/net/minecraftforge/forge/{game}-{loader}/forge-{game}-{loader}-installer.jar"
                ),
            ),
        };
        // Il profilo vale solo se l'installer è arrivato fino in fondo: lo
        // scrive per primo, prima di librerie e processor, quindi da solo non
        // basta a dire che l'installazione è completa.
        let cached = self.paths.version_json(&expected_id)?;
        let installed = || {
            cached
                .with_file_name(INSTALLED_MARKER)
                .is_file()
                .then(|| read_json::<VersionJson>(&cached).ok())
                .flatten()
        };
        if let Some(profile) = installed() {
            return Ok(profile);
        }

        // Un installer alla volta, anche tra app e CLI: lavorano sulle stesse
        // librerie. Chi aspetta ricontrolla poi se il lavoro è già fatto.
        let dir = self.paths.cache_dir().join("installers");
        fs::create_dir_all(&dir)?;
        let lock_path = dir.join(".lock");
        let _lock = tokio::task::spawn_blocking(move || {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&lock_path)
                .map_err(|e| CoreError::io(&lock_path, e))?;
            file.lock().map_err(|e| CoreError::io(&lock_path, e))?;
            Ok::<_, CoreError>(file)
        })
        .await??;
        if let Some(profile) = installed() {
            return Ok(profile);
        }

        let jar = dir.join(format!("{}-{loader}-installer.jar", kind.modrinth_name()));
        let sha1 = parse_sha1_sidecar(&self.downloader.get_bytes(&format!("{url}.sha1")).await?)?;
        self.downloader
            .fetch(DownloadTask::new(&url, jar.clone(), sha1, None))
            .await?;
        let id = installer_version_id(&jar)?;

        let java = self.install_java(vanilla.java_component(), None).await?;

        // L'installer rifiuta una cartella senza il file del launcher ufficiale.
        let profiles = self.paths.root().join("launcher_profiles.json");
        if !profiles.is_file() {
            fs::write_atomic(&profiles, br#"{"profiles":{}}"#)?;
        }

        let result = run_installer(&java, &jar, "--installClient", self.paths.root(), &dir).await;
        let _ = std::fs::remove_file(&jar);
        let _ = std::fs::remove_file(jar.with_extension("jar.log"));
        result.map_err(|message| InstallerError::LoaderInstaller {
            loader: kind.display_name().to_owned(),
            message,
        })?;
        let profile_path = self.paths.version_json(&id)?;
        let profile = read_json(&profile_path)?;
        fs::write_atomic(&profile_path.with_file_name(INSTALLED_MARKER), b"ok")?;
        Ok(profile)
    }
}

// ---------- Server dedicati con loader ----------

impl Installer {
    /// Installa il server di un loader nella cartella `dir` e restituisce gli
    /// argomenti di avvio da mettere dopo i flag di memoria di `java`.
    ///
    /// - **Fabric**: profilo server dall'API ufficiale e librerie scaricate
    ///   con il loro SHA1 (come per il client); il gioco è il `server.jar`
    ///   vanilla già presente in `dir`.
    /// - **NeoForge / Forge**: installer ufficiale verificato con lo SHA1
    ///   pubblicato, eseguito con `--installServer`; si avvia con i file di
    ///   argomenti che crea.
    pub async fn install_loader_server(
        &self,
        spec: &LoaderSpec,
        game: &str,
        dir: &Path,
        java: &Path,
    ) -> Result<Vec<String>> {
        let (game, loader) = checked_versions(spec, game)?;
        match spec.kind {
            LoaderKind::Fabric => self.fabric_server(game, loader).await,
            LoaderKind::NeoForge | LoaderKind::Forge => {
                self.forge_server(spec.kind, game, loader, dir, java).await
            }
            LoaderKind::Quilt => Err(InstallerError::UnsupportedVersion {
                version: loader.to_owned(),
                reason: "Quilt servers are not supported".into(),
            }),
        }
    }

    async fn fabric_server(&self, game: &str, loader: &str) -> Result<Vec<String>> {
        let url = format!("{FABRIC_META_URL}/versions/loader/{game}/{loader}/server/json");
        let profile: VersionJson = parse_json(&self.downloader.get_bytes(&url).await?, &url)?;
        let libraries_dir = self.paths.libraries_dir();
        let mut tasks = Vec::new();
        for library in &profile.libraries {
            tasks.push(self.maven_library_task(library, &libraries_dir).await?);
        }
        let classpath: Vec<String> = tasks.iter().map(|t| t.path.display().to_string()).collect();
        self.downloader.run(tasks, None).await?;
        let separator = if cfg!(windows) { ";" } else { ":" };
        Ok(vec![
            "-cp".into(),
            classpath.join(separator),
            "-Dfabric.gameJarPath=server.jar".into(),
            profile.main_class,
            "nogui".into(),
        ])
    }

    async fn forge_server(
        &self,
        kind: LoaderKind,
        game: &str,
        loader: &str,
        dir: &Path,
        java: &Path,
    ) -> Result<Vec<String>> {
        let (url, args_dir) = match kind {
            LoaderKind::NeoForge => (
                format!(
                    "{NEOFORGE_MAVEN}/net/neoforged/neoforge/{loader}/neoforge-{loader}-installer.jar"
                ),
                format!("libraries/net/neoforged/neoforge/{loader}"),
            ),
            _ => (
                format!(
                    "{FORGE_MAVEN}/net/minecraftforge/forge/{game}-{loader}/forge-{game}-{loader}-installer.jar"
                ),
                format!("libraries/net/minecraftforge/forge/{game}-{loader}"),
            ),
        };
        let jar = dir.join("loader-installer.jar");
        let sha1 = parse_sha1_sidecar(&self.downloader.get_bytes(&format!("{url}.sha1")).await?)?;
        self.downloader
            .fetch(DownloadTask::new(&url, jar.clone(), sha1, None))
            .await?;
        let result = run_installer(java, &jar, "--installServer", dir, dir).await;
        let _ = std::fs::remove_file(&jar);
        let _ = std::fs::remove_file(jar.with_extension("jar.log"));
        result.map_err(|message| InstallerError::LoaderInstaller {
            loader: kind.display_name().to_owned(),
            message,
        })?;
        let args_file = format!(
            "{args_dir}/{}",
            if cfg!(windows) {
                "win_args.txt"
            } else {
                "unix_args.txt"
            }
        );
        if !dir.join(&args_file).is_file() {
            return Err(InstallerError::LoaderInstaller {
                loader: kind.display_name().to_owned(),
                message: format!("{args_file} not created"),
            });
        }
        Ok(vec![
            "@user_jvm_args.txt".into(),
            format!("@{args_file}"),
            "nogui".into(),
        ])
    }
}

/// Versione del gioco e del loader, controllate: finiscono in URL e nomi di
/// file, quindi solo caratteri da numero di versione.
fn checked_versions<'a>(spec: &'a LoaderSpec, game: &'a str) -> Result<(&'a str, &'a str)> {
    let (game, loader) = (safe_component(game)?, safe_component(&spec.version)?);
    for v in [game, loader] {
        if !v
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
        {
            return Err(InstallerError::UnsupportedVersion {
                version: v.to_owned(),
                reason: "unexpected characters in the version".into(),
            });
        }
    }
    if !spec.kind.supports_game(game) {
        return Err(InstallerError::UnsupportedVersion {
            version: game.to_owned(),
            reason: format!("{} is not supported for it", spec.kind.display_name()),
        });
    }
    Ok((game, loader))
}

/// Scritto accanto al profilo NeoForge/Forge quando l'installer finisce bene.
const INSTALLED_MARKER: &str = ".hyper-installed";

/// Id della versione che l'installer creerà (dal suo `version.json`).
fn installer_version_id(jar: &Path) -> Result<String> {
    let zip_err = |source| InstallerError::Zip {
        path: jar.to_owned(),
        source,
    };
    let file = std::fs::File::open(jar).map_err(|e| CoreError::io(jar, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_err)?;
    let mut entry = archive.by_name("version.json").map_err(zip_err)?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| CoreError::io(jar, e))?;
    #[derive(Deserialize)]
    struct Id {
        id: String,
    }
    let id: Id = parse_json(&bytes, jar.display())?;
    Ok(safe_component(&id.id)?.to_owned())
}

/// `java -jar installer.jar --install-client <root>`, senza finestra di
/// console. In caso di errore restituisce le ultime righe di output.
async fn run_installer(
    java: &Path,
    jar: &Path,
    mode: &str,
    root: &Path,
    cwd: &Path,
) -> Result<(), String> {
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(jar)
        .arg(mode)
        .arg(root)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let output = command.output().await.map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let tail: Vec<&str> = text.lines().rev().take(6).collect();
    Err(format!(
        "{} ({})",
        tail.into_iter().rev().collect::<Vec<_>>().join(" | "),
        output.status
    ))
}

/// Versioni NeoForge per `game`. Dalla 1.20.2 il numero segue il gioco:
/// 1.21.1 → `21.1.x`, 1.21 → `21.0.x`; con la numerazione annuale
/// 26.3 → `26.3.0.x`, 26.1.2 → `26.1.2.x`.
fn neoforge_for(game: &str, all: &[String]) -> Vec<LoaderVersion> {
    let parts: Vec<&str> = game.split('.').collect();
    if parts.iter().any(|p| p.parse::<u32>().is_err()) {
        return Vec::new(); // snapshot o pre-release
    }
    let prefix = match parts.as_slice() {
        ["1", major, rest @ ..] => format!("{major}.{}.", rest.first().unwrap_or(&"0")),
        [year, drop, rest @ ..] => format!("{year}.{drop}.{}.", rest.first().unwrap_or(&"0")),
        _ => return Vec::new(),
    };
    all.iter()
        .filter(|v| v.starts_with(&prefix))
        .map(|v| LoaderVersion {
            stable: !v.contains("beta") && !v.contains("alpha"),
            version: v.clone(),
        })
        .collect()
}

/// Versioni Forge per `game` dal `maven-metadata.xml` (`<game>-<forge>`);
/// stabile è la "recommended" (o, se manca, la "latest") delle promozioni.
fn forge_for(
    game: &str,
    xml: &str,
    promos: &std::collections::HashMap<String, String>,
) -> Vec<LoaderVersion> {
    let preferred = promos
        .get(&format!("{game}-recommended"))
        .or_else(|| promos.get(&format!("{game}-latest")));
    let prefix = format!("{game}-");
    xml.split("<version>")
        .skip(1)
        .filter_map(|chunk| chunk.split("</version>").next())
        .filter_map(|full| full.strip_prefix(&prefix))
        // Le vecchie versioni hanno un suffisso (`-1.7.10`): non supportate.
        .filter(|v| !v.contains('-'))
        .map(|v| LoaderVersion {
            stable: preferred.is_some_and(|p| p == v),
            version: v.to_owned(),
        })
        .collect()
}

/// Confronta versioni tipo `0.30.0`, `0.30.0-beta.2`, `21.1.77`: numeri
/// confrontati come numeri, e una release viene dopo le sue pre-release.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn key(v: &str) -> (Vec<u64>, bool, Vec<u64>) {
        let (main, pre) = v.split_once('-').unwrap_or((v, ""));
        let nums = |s: &str| {
            s.split(|c: char| !c.is_ascii_digit())
                .filter(|p| !p.is_empty())
                .map(|p| p.parse().unwrap_or(u64::MAX))
                .collect::<Vec<u64>>()
        };
        (nums(main), pre.is_empty(), nums(pre))
    }
    key(a).cmp(&key(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_spec_toml_round_trip() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Holder {
            loader: LoaderSpec,
        }
        for kind in LoaderKind::ALL {
            let holder = Holder {
                loader: LoaderSpec {
                    kind,
                    version: "1.0".into(),
                },
            };
            let text = toml::to_string(&holder).unwrap();
            assert!(
                text.contains(&format!("kind = \"{}\"", kind.modrinth_name())),
                "{text}"
            );
            assert_eq!(toml::from_str::<Holder>(&text).unwrap(), holder);
        }
    }

    #[test]
    fn parses_meta_loader_lists() {
        let fabric = r#"[{"loader":{"separator":".","build":1,"maven":"net.fabricmc:fabric-loader:0.20.1","version":"0.20.1","stable":true},
                       "intermediary":{"maven":"net.fabricmc:intermediary:0.0.0","version":"0.0.0","stable":true}}]"#;
        let entries: Vec<MetaLoaderEntry> = serde_json::from_str(fabric).unwrap();
        assert!(entries[0].loader.stable);
        // Quilt non ha il campo "stable".
        let quilt = r#"[{"loader":{"maven":"org.quiltmc:quilt-loader:0.30.0","version":"0.30.0","build":1}}]"#;
        let entries: Vec<MetaLoaderEntry> = serde_json::from_str(quilt).unwrap();
        assert_eq!(entries[0].loader.version, "0.30.0");
    }

    #[test]
    fn orders_versions_numerically_with_prereleases_first() {
        let mut v = vec![
            "0.20.0-beta.9",
            "0.30.0",
            "0.29.2",
            "0.30.0-beta.10",
            "0.30.0-beta.2",
        ];
        v.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(
            v,
            [
                "0.30.0",
                "0.30.0-beta.10",
                "0.30.0-beta.2",
                "0.29.2",
                "0.20.0-beta.9"
            ]
        );
        assert_eq!(compare_versions("21.1.100", "21.1.99"), Ordering::Greater);
    }

    #[test]
    fn maps_game_versions_to_neoforge() {
        let all: Vec<String> = [
            "20.4.237",
            "21.0.167",
            "21.1.77",
            "21.1.80-beta",
            "26.3.0.110",
            "26.1.2.5",
        ]
        .map(String::from)
        .to_vec();
        let names = |game| {
            neoforge_for(game, &all)
                .into_iter()
                .map(|v| v.version)
                .collect::<Vec<_>>()
        };
        assert_eq!(names("1.21.1"), ["21.1.77", "21.1.80-beta"]);
        assert_eq!(names("1.21"), ["21.0.167"]);
        assert_eq!(names("26.3"), ["26.3.0.110"]);
        assert_eq!(names("26.1.2"), ["26.1.2.5"]);
        assert!(names("26.4-snapshot-8").is_empty());
        assert!(!neoforge_for("1.21.1", &all)[1].stable);
    }

    #[test]
    fn reads_forge_versions_and_promotions() {
        let xml = "<metadata><versioning><versions><version>26.3-66.1.8</version><version>26.3-66.1.7</version>\
                   <version>1.20.1-47.3.0</version><version>1.7.10-10.13.4.1614-1.7.10</version></versions></versioning></metadata>";
        let promos =
            std::collections::HashMap::from([("26.3-recommended".to_owned(), "66.1.8".to_owned())]);
        let list = forge_for("26.3", xml, &promos);
        assert_eq!(list.len(), 2);
        assert!(list[0].stable && !list[1].stable);
        assert!(forge_for("1.7.10", xml, &promos).is_empty());
    }

    #[test]
    fn knows_which_game_versions_each_loader_supports() {
        use LoaderKind::*;
        assert!(Forge.supports_game("1.20.1") && Forge.supports_game("26.3"));
        assert!(!Forge.supports_game("1.16.5") && !Forge.supports_game("1.12.2"));
        assert!(NeoForge.supports_game("1.20.2") && NeoForge.supports_game("1.21"));
        assert!(!NeoForge.supports_game("1.20.1"));
        assert!(Fabric.supports_game("26.4-snapshot-8") && !Forge.supports_game("26.4-snapshot-8"));
    }

    #[test]
    fn pack_dependency_names_round_trip() {
        for kind in LoaderKind::ALL {
            assert_eq!(
                LoaderKind::from_pack_dependency(kind.pack_dependency()),
                Some(kind)
            );
        }
        assert_eq!(LoaderKind::from_pack_dependency("minecraft"), None);
    }
}
