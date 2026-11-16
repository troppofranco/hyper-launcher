//! Version JSON (`<id>.json`): tipi serde e valutazione delle `rules`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::manifest::VersionKind;
use crate::{DownloadTask, Result, Sha1Hex};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    pub main_class: String,
    pub asset_index: Option<AssetIndexRef>,
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    /// Formato moderno (1.13+).
    pub arguments: Option<Arguments>,
    /// Formato legacy (fino a 1.12.2): stringa unica di argomenti di gioco.
    pub minecraft_arguments: Option<String>,
    pub java_version: Option<JavaVersion>,
    pub logging: Option<Logging>,
    /// Presente nei profili dei mod loader; non supportato in questa fase.
    pub inherits_from: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Download {
    pub sha1: Sha1Hex,
    pub size: u64,
    pub url: String,
}

impl Download {
    /// Task di download verso un percorso costruito dal launcher.
    pub fn task(&self, path: PathBuf) -> DownloadTask {
        DownloadTask::new(&self.url, path, self.sha1.clone(), Some(self.size))
    }

    /// Task verso un percorso relativo non fidato (da metadata remoti),
    /// validato contro il path traversal rispetto a `base`.
    pub fn task_in(&self, base: &Path, relative: &str) -> Result<DownloadTask> {
        Ok(self.task(hyper_core::safe_join(base, relative)?))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndexRef {
    pub id: String,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionDownloads {
    pub client: Download,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Legacy: OS → classifier (es. `"windows": "natives-windows-${arch}"`).
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: HashMap<String, Artifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub path: String,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional { rules: Vec<Rule>, value: ArgValue },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingClient {
    /// Es. `-Dlog4j.configurationFile=${path}`.
    pub argument: String,
    pub file: LoggingFile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    #[serde(flatten)]
    pub download: Download,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    // `version` (regex sulla versione dell'OS) è ignorato: riguarda solo
    // vecchie eccezioni macOS e Windows 10 non rilevanti per l'avvio.
}

/// Ambiente rispetto a cui valutare le rules.
#[derive(Debug, Clone)]
pub struct RuleContext {
    pub os: String,
    pub arch: String,
    /// Valore di `${arch}` nei classifier delle native legacy (`64`/`32`).
    pub arch_bits: String,
    /// Feature attive (es. `has_custom_resolution`); le assenti valgono `false`.
    pub features: HashSet<String>,
}

impl RuleContext {
    pub fn current() -> Self {
        use hyper_core::platform;
        Self {
            os: platform::mojang_os().to_owned(),
            arch: platform::mojang_arch().to_owned(),
            arch_bits: platform::arch_bits().to_owned(),
            features: HashSet::new(),
        }
    }
}

impl Rule {
    fn matches(&self, ctx: &RuleContext) -> bool {
        if let Some(os) = &self.os {
            if os.name.as_ref().is_some_and(|name| *name != ctx.os) {
                return false;
            }
            if os.arch.as_ref().is_some_and(|arch| *arch != ctx.arch) {
                return false;
            }
        }
        self.features
            .iter()
            .all(|(name, wanted)| ctx.features.contains(name) == *wanted)
    }
}

/// Semantica Mojang: nessuna regola = consentito; altrimenti parte da
/// "negato" e vince l'ultima regola che corrisponde.
pub fn rules_allow(rules: &[Rule], ctx: &RuleContext) -> bool {
    if rules.is_empty() {
        return true;
    }
    rules
        .iter()
        .rfind(|rule| rule.matches(ctx))
        .is_some_and(|rule| rule.action == RuleAction::Allow)
}

impl Argument {
    /// Valori dell'argomento se le sue rules lo consentono.
    pub fn values(&self, ctx: &RuleContext) -> Vec<&str> {
        match self {
            Self::Plain(value) => vec![value],
            Self::Conditional { rules, value } if rules_allow(rules, ctx) => match value {
                ArgValue::One(v) => vec![v],
                ArgValue::Many(vs) => vs.iter().map(String::as_str).collect(),
            },
            Self::Conditional { .. } => Vec::new(),
        }
    }
}

impl Library {
    pub fn allowed(&self, ctx: &RuleContext) -> bool {
        rules_allow(&self.rules, ctx)
    }

    /// Artifact nativo da estrarre, se questa libreria ne ha uno per l'OS:
    /// - formato legacy: mappa `natives` + `downloads.classifiers`;
    /// - formato moderno (1.19+): libreria `gruppo:nome:versione:natives-<os>`.
    pub fn native_artifact(&self, ctx: &RuleContext) -> Option<&Artifact> {
        let downloads = self.downloads.as_ref()?;
        if let Some(natives) = &self.natives {
            let classifier = natives.get(&ctx.os)?.replace("${arch}", &ctx.arch_bits);
            return downloads.classifiers.get(&classifier);
        }
        let is_native = self
            .name
            .split(':')
            .nth(3)
            .is_some_and(|classifier| classifier.starts_with("natives-"));
        if is_native {
            downloads.artifact.as_ref()
        } else {
            None
        }
    }

    /// Prefissi da escludere durante l'estrazione delle native.
    pub fn extract_excludes(&self) -> &[String] {
        self.extract.as_ref().map_or(&[], |e| e.exclude.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(os: &str, arch: &str) -> RuleContext {
        RuleContext {
            os: os.into(),
            arch: arch.into(),
            arch_bits: "64".into(),
            features: HashSet::new(),
        }
    }

    fn rules(json: &str) -> Vec<Rule> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn rule_semantics() {
        let win = ctx("windows", "x86_64");
        let mac = ctx("osx", "arm64");

        assert!(rules_allow(&[], &win));

        let only_windows = rules(r#"[{"action":"allow","os":{"name":"windows"}}]"#);
        assert!(rules_allow(&only_windows, &win));
        assert!(!rules_allow(&only_windows, &mac));

        let all_but_osx =
            rules(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]"#);
        assert!(rules_allow(&all_but_osx, &win));
        assert!(!rules_allow(&all_but_osx, &mac));

        let x86_only = rules(r#"[{"action":"allow","os":{"arch":"x86"}}]"#);
        assert!(!rules_allow(&x86_only, &win));
        assert!(rules_allow(&x86_only, &ctx("windows", "x86")));

        let demo = rules(r#"[{"action":"allow","features":{"is_demo_user":true}}]"#);
        assert!(!rules_allow(&demo, &win));
        let mut demo_ctx = win.clone();
        demo_ctx.features.insert("is_demo_user".into());
        assert!(rules_allow(&demo, &demo_ctx));
    }

    const MODERN: &str = r#"{
        "id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "assetIndex": {"id": "19", "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 10, "totalSize": 100, "url": "https://example.invalid/19.json"},
        "assets": "19",
        "downloads": {"client": {"sha1": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "size": 5, "url": "https://example.invalid/client.jar"}},
        "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
        "libraries": [
            {"name": "com.mojang:brigadier:1.3.10",
             "downloads": {"artifact": {"path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar", "sha1": "cccccccccccccccccccccccccccccccccccccccc", "size": 1, "url": "https://example.invalid/b.jar"}}},
            {"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
             "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar", "sha1": "dddddddddddddddddddddddddddddddddddddddd", "size": 1, "url": "https://example.invalid/n.jar"}},
             "rules": [{"action": "allow", "os": {"name": "windows"}}]}
        ],
        "arguments": {
            "game": ["--username", "${auth_player_name}",
                     {"rules": [{"action": "allow", "features": {"is_demo_user": true}}], "value": "--demo"},
                     {"rules": [{"action": "allow", "features": {"has_custom_resolution": true}}], "value": ["--width", "${resolution_width}"]}],
            "jvm": [{"rules": [{"action": "allow", "os": {"name": "osx"}}], "value": ["-XstartOnFirstThread"]},
                    {"rules": [{"action": "allow", "os": {"name": "windows"}}], "value": "-XX:HeapDumpPath=x.dmp"},
                    "-cp", "${classpath}"]
        },
        "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}", "type": "log4j2-xml",
            "file": {"id": "client-1.12.xml", "sha1": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "size": 1, "url": "https://example.invalid/log.xml"}}}
    }"#;

    const LEGACY: &str = r#"{
        "id": "1.8.9", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "minecraftArguments": "--username ${auth_player_name} --version ${version_name}",
        "assetIndex": {"id": "1.8", "sha1": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 10, "url": "https://example.invalid/1.8.json"},
        "libraries": [
            {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
             "natives": {"linux": "natives-linux", "osx": "natives-osx", "windows": "natives-windows-${arch}"},
             "extract": {"exclude": ["META-INF/"]},
             "downloads": {"classifiers": {
                "natives-windows-64": {"path": "x/natives-windows-64.jar", "sha1": "1111111111111111111111111111111111111111", "size": 1, "url": "https://example.invalid/w64.jar"},
                "natives-osx": {"path": "x/natives-osx.jar", "sha1": "2222222222222222222222222222222222222222", "size": 1, "url": "https://example.invalid/osx.jar"}}}}
        ]
    }"#;

    #[test]
    fn parses_modern_version_json() {
        let v: VersionJson = serde_json::from_str(MODERN).unwrap();
        let win = ctx("windows", "x86_64");
        assert_eq!(v.java_version.as_ref().unwrap().major_version, 21);
        assert_eq!(v.asset_index.as_ref().unwrap().id, "19");

        let args = v.arguments.as_ref().unwrap();
        let game: Vec<&str> = args.game.iter().flat_map(|a| a.values(&win)).collect();
        assert_eq!(game, ["--username", "${auth_player_name}"]);
        let jvm: Vec<&str> = args.jvm.iter().flat_map(|a| a.values(&win)).collect();
        assert_eq!(jvm, ["-XX:HeapDumpPath=x.dmp", "-cp", "${classpath}"]);

        let natives = &v.libraries[1];
        assert!(natives.allowed(&win));
        assert!(!natives.allowed(&ctx("linux", "x86_64")));
        assert!(natives.native_artifact(&win).is_some());
        assert!(v.libraries[0].native_artifact(&win).is_none());
        assert_eq!(
            v.logging.unwrap().client.unwrap().file.id,
            "client-1.12.xml"
        );
    }

    #[test]
    fn parses_legacy_version_json() {
        let v: VersionJson = serde_json::from_str(LEGACY).unwrap();
        assert!(v.arguments.is_none());
        assert!(v.minecraft_arguments.unwrap().contains("${version_name}"));

        let lib = &v.libraries[0];
        let win = ctx("windows", "x86_64");
        let native = lib.native_artifact(&win).unwrap();
        assert_eq!(native.path, "x/natives-windows-64.jar");
        assert_eq!(
            lib.native_artifact(&ctx("osx", "x86_64")).unwrap().path,
            "x/natives-osx.jar"
        );
        assert!(lib.native_artifact(&ctx("linux", "x86_64")).is_none());
        assert_eq!(lib.extract_excludes(), ["META-INF/"]);
    }
}
