//! Dati inviati all'interfaccia (serializzati in camelCase per TypeScript).

use hyper_auth::Account;
use hyper_installer::LoaderKind;
use hyper_instances::{Instance, ModFile, World};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderView {
    pub kind: LoaderKind,
    pub name: &'static str,
    pub version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    pub name: String,
    pub version: String,
    pub loader: Option<LoaderView>,
    pub created: u64,
    pub last_played: Option<u64>,
    pub playtime_secs: u64,
    pub memory_mb: Option<u32>,
    pub java_path: Option<String>,
    pub jvm_args: Vec<String>,
}

impl InstanceView {
    pub fn new(instance: &Instance) -> Self {
        let c = &instance.config;
        Self {
            name: c.name.clone(),
            version: c.version.clone(),
            loader: c.loader.as_ref().map(|l| LoaderView {
                kind: l.kind,
                name: l.kind.display_name(),
                version: l.version.clone(),
            }),
            created: c.created,
            last_played: c.last_played,
            playtime_secs: c.playtime_secs,
            memory_mb: c.memory_mb,
            java_path: c.java_path.as_ref().map(|p| p.display().to_string()),
            jvm_args: c.jvm_args.clone(),
        }
    }
}

impl From<ModFile> for ModView {
    fn from(m: ModFile) -> Self {
        Self {
            file: m.file,
            name: m.name,
            enabled: m.enabled,
            size_bytes: m.size_bytes,
        }
    }
}

impl From<World> for WorldView {
    fn from(w: World) -> Self {
        Self {
            name: w.name,
            last_played: w.last_played,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub name: String,
    pub uuid: String,
    pub kind: &'static str,
    pub default: bool,
}

impl AccountView {
    pub fn new(account: &Account, default: bool) -> Self {
        Self {
            name: account.name().to_owned(),
            uuid: account.uuid().to_string(),
            kind: account.kind(),
            default,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionView {
    pub id: String,
    pub kind: &'static str,
    pub release_time: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionsView {
    pub latest_release: String,
    pub latest_snapshot: String,
    pub versions: Vec<VersionView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModView {
    pub file: String,
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldView {
    pub name: String,
    pub last_played: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsView {
    pub title: String,
    pub date: String,
    pub text: String,
    pub image: Option<String>,
    pub url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: &'static str,
    pub build: Option<u64>,
    pub data_dir: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub current: Option<u64>,
    pub available: Option<u64>,
}
