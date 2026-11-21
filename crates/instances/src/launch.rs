//! Costruzione del comando JVM + argomenti di gioco per una versione vanilla.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use hyper_auth::Session;
use hyper_core::{LAUNCHER_NAME, LAUNCHER_VERSION, Paths, fs, platform};
use hyper_installer::version::Argument;
use hyper_installer::{InstalledVersion, natives};

use crate::{Instance, InstanceError, Result};

/// Parametri dell'avvio che non dipendono dalla versione.
#[derive(Debug, Clone, Copy)]
pub struct LaunchOptions<'a> {
    pub java: &'a Path,
    pub memory_mb: u32,
    pub session: &'a Session,
    pub paths: &'a Paths,
    /// Argomenti JVM per le istanze che non ne hanno di propri.
    pub default_jvm_args: &'a [String],
    pub window: GameWindow,
}

/// Finestra di gioco all'avvio (impostazione globale del launcher).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameWindow {
    /// Larghezza × altezza; `None` = quella decisa dal gioco.
    pub size: Option<(u32, u32)>,
    pub fullscreen: bool,
}

impl GameWindow {
    /// Argomenti di gioco corrispondenti (`--width`, `--height`, `--fullscreen`).
    fn args(self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some((w, h)) = self.size {
            args.extend([
                "--width".into(),
                w.to_string(),
                "--height".into(),
                h.to_string(),
            ]);
        }
        if self.fullscreen {
            args.push("--fullscreen".into());
        }
        args
    }
}

#[derive(Clone)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Avvia Java senza la sua finestra di console (Windows). Dall'app
    /// grafica la console si apre da sola, perché il launcher non ne ha una.
    pub hide_console: bool,
    /// Valori da non mostrare mai (token della sessione), ovunque compaiano.
    secrets: Vec<String>,
}

impl LaunchCommand {
    /// Avvia il gioco con lo stdout catturato (va letto, vedi
    /// [`crate::RunningGame::wait`]) e lo stderr ereditato.
    pub fn spawn(&self) -> Result<Child> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .stdout(std::process::Stdio::piped());
        #[cfg(windows)]
        if self.hide_console {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        command.spawn().map_err(|source| InstanceError::Process {
            program: self.program.clone(),
            source,
        })
    }

    /// Argomenti con i segreti sostituiti da `<redacted>`: l'unica forma
    /// del comando da mostrare o registrare.
    pub fn redacted_args(&self) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| {
                self.secrets
                    .iter()
                    .fold(arg.clone(), |acc, secret| acc.replace(secret, "<redacted>"))
            })
            .collect()
    }
}

impl fmt::Debug for LaunchCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchCommand")
            .field("program", &self.program)
            .field("args", &self.redacted_args())
            .field("cwd", &self.cwd)
            .finish_non_exhaustive()
    }
}

/// Ricrea la cartella delle native dell'istanza estraendo i jar della
/// versione (ogni entry è validata contro il path traversal). Va chiamata
/// tenendo il lock dell'istanza ([`Instance::try_lock`]), altrimenti
/// cancellerebbe le native di una sessione di gioco in corso.
pub fn prepare_natives(instance: &Instance, installed: &InstalledVersion) -> Result<usize> {
    let dir = instance.natives_dir();
    fs::remove_dir_all_if_exists(&dir)?;
    fs::create_dir_all(&dir)?;
    let mut total = 0;
    for jar in &installed.natives {
        total += natives::extract_natives(&jar.path, &dir, &jar.exclude)?;
    }
    Ok(total)
}

/// Argomenti JVM usati dalle versioni legacy, che non li dichiarano.
fn legacy_jvm_args() -> Vec<&'static str> {
    let mut args = Vec::new();
    if cfg!(windows) {
        args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump");
    }
    args.extend([
        "-Djava.library.path=${natives_directory}",
        "-Dminecraft.launcher.brand=${launcher_name}",
        "-Dminecraft.launcher.version=${launcher_version}",
        "-cp",
        "${classpath}",
    ]);
    args
}

/// Argomenti per entrare subito in un server all'avvio del gioco: Quick Play
/// (`--quickPlayMultiplayer host:porta`) dalla 1.20, prima `--server` e
/// `--port`. `host` arriva da un indirizzo scelto dall'utente o dal
/// launcher: niente spazi, così resta un solo argomento.
pub fn join_args(game_version: &str, host: &str, port: u16) -> Vec<String> {
    let host: String = host.chars().filter(|c| !c.is_whitespace()).collect();
    let quick_play =
        hyper_installer::loader::compare_versions(game_version, "1.20") != std::cmp::Ordering::Less;
    if quick_play {
        vec!["--quickPlayMultiplayer".into(), format!("{host}:{port}")]
    } else {
        vec!["--server".into(), host, "--port".into(), port.to_string()]
    }
}

pub fn build_command(
    instance: &Instance,
    installed: &InstalledVersion,
    opts: LaunchOptions<'_>,
) -> LaunchCommand {
    let version = &installed.version;
    let separator = platform::classpath_separator();
    let classpath = installed
        .classpath
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(separator);
    let path = |p: PathBuf| p.display().to_string();
    let assets_root = path(opts.paths.assets_dir());
    // Gli account offline passano il segnaposto `0`, che non è un segreto.
    let token = opts
        .session
        .access_token
        .as_ref()
        .map_or("0", |t| t.expose())
        .to_owned();

    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", opts.session.name.clone()),
        ("auth_uuid", opts.session.uuid.simple().to_string()),
        ("auth_access_token", token.clone()),
        ("auth_session", token),
        ("auth_xuid", String::new()),
        ("clientid", String::new()),
        // Le versioni recenti accettano solo `msa`/`legacy`/`mojang`;
        // `msa` funziona anche per gli account offline.
        ("user_type", "msa".to_owned()),
        ("user_properties", "{}".to_owned()),
        ("version_name", version.id.clone()),
        ("version_type", version.kind.as_str().to_owned()),
        ("game_directory", path(instance.game_dir())),
        ("assets_root", assets_root.clone()),
        ("game_assets", assets_root),
        ("assets_index_name", installed.asset_index_id.clone()),
        ("natives_directory", path(instance.natives_dir())),
        ("library_directory", path(opts.paths.libraries_dir())),
        ("launcher_name", LAUNCHER_NAME.to_owned()),
        ("launcher_version", LAUNCHER_VERSION.to_owned()),
        ("classpath", classpath),
        ("classpath_separator", separator.to_owned()),
    ]);
    let sub = |arg: &str| substitute(arg, &vars);
    let expand = |list: &[Argument]| -> Vec<String> {
        list.iter()
            .flat_map(|a| a.values(&installed.rules))
            .map(sub)
            .collect()
    };

    // Formato moderno: argomenti dichiarati; legacy: JVM di default e una
    // stringa unica di argomenti di gioco.
    let (jvm_args, game_args) = match &version.arguments {
        Some(arguments) => (expand(&arguments.jvm), expand(&arguments.game)),
        None => (
            legacy_jvm_args().into_iter().map(sub).collect(),
            version
                .minecraft_arguments
                .as_deref()
                .unwrap_or_default()
                .split_whitespace()
                .map(sub)
                .collect(),
        ),
    };

    let mut args = vec![format!("-Xmx{}M", opts.memory_mb)];
    // Gli argomenti dell'istanza, se ci sono, sostituiscono quelli predefiniti.
    let custom = if instance.config.jvm_args.is_empty() {
        opts.default_jvm_args
    } else {
        &instance.config.jvm_args
    };
    args.extend(custom.iter().cloned());
    args.extend(jvm_args);
    if let Some(logging) = &installed.logging {
        let path = HashMap::from([("path", logging.path.display().to_string())]);
        args.push(substitute(&logging.argument, &path));
    }
    args.push(version.main_class.clone());
    args.extend(game_args);
    args.extend(opts.window.args());

    let secrets: Vec<String> = opts
        .session
        .access_token
        .iter()
        .map(|t| t.expose().to_owned())
        .collect();
    LaunchCommand {
        program: opts.java.to_owned(),
        args,
        cwd: instance.game_dir(),
        hide_console: false,
        secrets,
    }
}

/// Sostituisce i segnaposto `${nome}` noti; quelli sconosciuti restano
/// invariati.
fn substitute(arg: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(start) = rest.find("${") {
        let Some(len) = rest[start + 2..].find('}') else {
            break;
        };
        let key = &rest[start + 2..start + 2 + len];
        out.push_str(&rest[..start]);
        match vars.get(key) {
            Some(value) => out.push_str(value),
            None => out.push_str(&rest[start..start + 3 + len]),
        }
        rest = &rest[start + 3 + len..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use hyper_auth::{Account, Secret};
    use hyper_installer::{RuleContext, VersionJson};

    use super::*;
    use crate::InstanceConfig;

    #[test]
    fn join_uses_quick_play_from_1_20() {
        assert_eq!(
            join_args("1.21.4", "localhost", 25565),
            ["--quickPlayMultiplayer", "localhost:25565"]
        );
        assert_eq!(
            join_args("26.3", "a.b", 1),
            ["--quickPlayMultiplayer", "a.b:1"]
        );
        assert_eq!(
            join_args("1.19.4", "local host", 25566),
            ["--server", "localhost", "--port", "25566"]
        );
    }

    fn installed(json: &str) -> InstalledVersion {
        let version: VersionJson = serde_json::from_str(json).unwrap();
        InstalledVersion {
            version,
            classpath: vec![PathBuf::from("a.jar"), PathBuf::from("client.jar")],
            natives: Vec::new(),
            asset_index_id: "19".into(),
            logging: None,
            rules: RuleContext {
                os: "windows".into(),
                arch: "x86_64".into(),
                arch_bits: "64".into(),
                features: Default::default(),
            },
        }
    }

    fn instance() -> Instance {
        Instance {
            config: InstanceConfig {
                name: "test".into(),
                version: "x".into(),
                memory_mb: None,
                java_path: None,
                jvm_args: vec!["-XX:+UseZGC".into()],
                loader: None,
                created: 0,
                last_played: None,
                playtime_secs: 0,
                sync_settings: true,
                edition: Default::default(),
            },
            dir: PathBuf::from("inst"),
        }
    }

    fn offline_session() -> Session {
        let account = Account::offline("Dev").unwrap();
        Session::offline(account.name(), account.uuid())
    }

    fn build(json: &str) -> Vec<String> {
        let session = offline_session();
        let paths = Paths::new("root");
        let opts = LaunchOptions {
            java: Path::new("java"),
            memory_mb: 2048,
            session: &session,
            paths: &paths,
            default_jvm_args: &[],
            window: GameWindow::default(),
        };
        build_command(&instance(), &installed(json), opts).args
    }

    #[test]
    fn microsoft_session_token_is_passed_but_redacted() {
        let mut session = offline_session();
        session.access_token = Some(Secret::new("eyJ.real.token"));
        let paths = Paths::new("root");
        let command = build_command(
            &instance(),
            &installed(
                r#"{"id": "1.21.4", "type": "release", "mainClass": "M",
                    "arguments": {"game": ["--accessToken", "${auth_access_token}"], "jvm": []}}"#,
            ),
            LaunchOptions {
                java: Path::new("java"),
                memory_mb: 2048,
                session: &session,
                paths: &paths,
                default_jvm_args: &[],
                window: GameWindow::default(),
            },
        );
        assert!(command.args.iter().any(|a| a == "eyJ.real.token"));
        let shown = format!("{:?} {:?}", command.redacted_args(), command);
        assert!(!shown.contains("eyJ.real.token"), "{shown}");
    }

    #[test]
    fn window_and_default_jvm_args() {
        let session = offline_session();
        let paths = Paths::new("root");
        // Senza argomenti propri l'istanza usa quelli predefiniti.
        let mut plain = instance();
        plain.config.jvm_args.clear();
        let defaults = vec!["-XX:+UseG1GC".to_owned()];
        let command = build_command(
            &plain,
            &installed(
                r#"{"id": "1.21.4", "type": "release", "mainClass": "M",
                    "arguments": {"game": [], "jvm": []}}"#,
            ),
            LaunchOptions {
                java: Path::new("java"),
                memory_mb: 2048,
                session: &session,
                paths: &paths,
                default_jvm_args: &defaults,
                window: GameWindow {
                    size: Some((1600, 900)),
                    fullscreen: true,
                },
            },
        );
        let args = command.args.join(" ");
        assert!(args.contains("-Xmx2048M -XX:+UseG1GC"), "{args}");
        assert!(
            args.ends_with("M --width 1600 --height 900 --fullscreen"),
            "{args}"
        );
    }

    #[test]
    fn redacts_secrets_everywhere() {
        let command = LaunchCommand {
            program: PathBuf::from("java"),
            args: vec![
                "--accessToken".into(),
                "s3cr3t".into(),
                "--session".into(),
                "token:s3cr3t:uuid".into(),
            ],
            cwd: PathBuf::from("."),
            hide_console: false,
            secrets: vec!["s3cr3t".into()],
        };
        let shown = command.redacted_args();
        assert_eq!(shown[1], "<redacted>");
        assert_eq!(shown[3], "token:<redacted>:uuid");
        assert!(!format!("{command:?}").contains("s3cr3t"));
    }

    #[test]
    fn substitution() {
        let vars = HashMap::from([("a", "1".to_owned()), ("bb", "22".to_owned())]);
        assert_eq!(substitute("x${a}y${bb}z", &vars), "x1y22z");
        assert_eq!(substitute("${unknown}-${a}", &vars), "${unknown}-1");
        assert_eq!(substitute("open ${a", &vars), "open ${a");
        assert_eq!(substitute("plain", &vars), "plain");
    }

    #[test]
    fn modern_arguments() {
        let args = build(
            r#"{"id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "arguments": {
                    "game": ["--username", "${auth_player_name}", "--uuid", "${auth_uuid}",
                             "--assetIndex", "${assets_index_name}",
                             {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"}],
                    "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": "-XstartOnFirstThread"},
                            "-Djava.library.path=${natives_directory}", "-cp", "${classpath}"]}}"#,
        );
        let sep = platform::classpath_separator();
        let main = args
            .iter()
            .position(|a| a == "net.minecraft.client.main.Main")
            .unwrap();
        let (jvm, game) = args.split_at(main);

        assert_eq!(jvm[0], "-Xmx2048M");
        assert_eq!(jvm[1], "-XX:+UseZGC");
        assert!(!jvm.iter().any(|a| a == "-XstartOnFirstThread"));
        assert!(jvm.contains(&format!("a.jar{sep}client.jar")));
        assert!(
            jvm.iter()
                .any(|a| a.starts_with("-Djava.library.path=") && a.ends_with("natives"))
        );

        let uuid = Account::offline("Dev").unwrap().uuid().simple().to_string();
        assert_eq!(
            &game[1..],
            [
                "--username",
                "Dev",
                "--uuid",
                uuid.as_str(),
                "--assetIndex",
                "19"
            ]
        );
        assert!(!args.iter().any(|a| a.contains("${")), "{args:?}");
    }

    #[test]
    fn legacy_arguments() {
        let args = build(
            r#"{"id": "1.8.9", "type": "release", "mainClass": "net.minecraft.client.main.Main",
                "minecraftArguments": "--username ${auth_player_name} --version ${version_name} --userProperties ${user_properties}"}"#,
        );
        assert!(args.iter().any(|a| a == "-cp"));
        assert!(args.ends_with(&[
            "net.minecraft.client.main.Main".to_owned(),
            "--username".into(),
            "Dev".into(),
            "--version".into(),
            "1.8.9".into(),
            "--userProperties".into(),
            "{}".into(),
        ]));
        assert!(!args.iter().any(|a| a.contains("${")), "{args:?}");
    }
}
