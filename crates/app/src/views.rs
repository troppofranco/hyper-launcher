//! Dati inviati all'interfaccia (serializzati in camelCase per TypeScript).

use hyper_auth::Account;
use hyper_content::library::InstalledContent;
use hyper_content::{InstallPlan, Project, SearchResults};
use hyper_installer::LoaderKind;
use hyper_instances::{ContentFile, Instance, World};
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
    pub sync_settings: bool,
    /// Immagine scelta dall'utente (`data:` URL), se c'è.
    pub icon: Option<String>,
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
            sync_settings: c.sync_settings,
            icon: instance
                .icon()
                .map(|png| crate::custom::data_url("image/png", &png)),
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitView {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    /// Immagine di copertina (galleria del progetto), se c'è.
    pub image: Option<String>,
    pub color: Option<u32>,
    pub downloads: u64,
    pub updated: String,
    pub categories: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchView {
    pub hits: Vec<SearchHitView>,
    pub total: u32,
}

impl From<SearchResults> for SearchView {
    fn from(r: SearchResults) -> Self {
        Self {
            total: r.total_hits,
            hits: r
                .hits
                .into_iter()
                .map(|h| SearchHitView {
                    id: h.project_id,
                    slug: h.slug,
                    title: h.title,
                    description: h.description,
                    author: h.author,
                    icon_url: h.icon_url,
                    image: h.featured_gallery.or_else(|| h.gallery.into_iter().next()),
                    color: h.color,
                    downloads: h.downloads,
                    updated: h.date_modified,
                    categories: h.display_categories,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageView {
    pub url: String,
    pub title: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub updated: String,
    pub categories: Vec<String>,
    pub gallery: Vec<ImageView>,
    pub license: Option<String>,
    pub source_url: Option<String>,
    pub page_url: String,
}

impl From<Project> for ProjectView {
    fn from(p: Project) -> Self {
        Self {
            page_url: format!("https://modrinth.com/{}/{}", p.project_type, p.slug),
            id: p.id,
            slug: p.slug,
            title: p.title,
            description: p.description,
            project_type: p.project_type,
            downloads: p.downloads,
            icon_url: p.icon_url,
            updated: p.updated,
            categories: p.categories,
            gallery: p
                .gallery
                .into_iter()
                .map(|g| ImageView {
                    url: g.url,
                    title: g.title,
                })
                .collect(),
            license: p
                .license
                .map(|l| if l.name.is_empty() { l.id } else { l.name }),
            source_url: p.source_url,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedView {
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub file: String,
    pub size_bytes: u64,
    pub folder: String,
    pub replaces: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub files: Vec<PlannedView>,
    pub total_bytes: u64,
}

impl From<&InstallPlan> for PlanView {
    fn from(plan: &InstallPlan) -> Self {
        Self {
            total_bytes: plan.total_bytes(),
            files: plan
                .files
                .iter()
                .map(|f| PlannedView {
                    project_id: f.version.project_id.clone(),
                    name: f.version.name.clone(),
                    version_number: f.version.version_number.clone(),
                    file: f.file.filename.clone(),
                    size_bytes: f.file.size,
                    folder: f
                        .dest
                        .parent()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    replaces: f.replaces.is_some(),
                })
                .collect(),
        }
    }
}

/// Un file installato, con i dati Modrinth se riconosciuto.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentView {
    pub file: String,
    pub name: String,
    pub enabled: bool,
    pub size_bytes: u64,
    pub project_id: Option<String>,
    pub slug: Option<String>,
    pub title: Option<String>,
    pub icon_url: Option<String>,
    pub version_number: Option<String>,
    pub update: Option<String>,
}

impl From<ContentFile> for ContentView {
    fn from(f: ContentFile) -> Self {
        Self {
            file: f.file,
            name: f.name,
            enabled: f.enabled,
            size_bytes: f.size_bytes,
            project_id: None,
            slug: None,
            title: None,
            icon_url: None,
            version_number: None,
            update: None,
        }
    }
}

impl From<InstalledContent> for ContentView {
    fn from(c: InstalledContent) -> Self {
        let project = c.project;
        Self {
            project_id: c.version.as_ref().map(|v| v.project_id.clone()),
            version_number: c.version.map(|v| v.version_number),
            update: c.update.map(|v| v.version_number),
            slug: project.as_ref().map(|p| p.slug.clone()),
            title: project.as_ref().map(|p| p.title.clone()),
            icon_url: project.and_then(|p| p.icon_url),
            ..Self::from(c.file)
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub instance: String,
    pub start: u64,
    pub end: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportView {
    pub path: String,
    pub linked: usize,
    pub bundled: usize,
}
