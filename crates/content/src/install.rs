//! Installazione di un progetto Modrinth in un'istanza: scelta della
//! versione compatibile, dipendenze obbligatorie, download verificati.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use hyper_core::{CoreError, safe_component};
use hyper_installer::download::ProgressFn;
use hyper_installer::{DownloadTask, Downloader, Sha1Hex};
use hyper_instances::Instance;

use crate::library::installed_mod_projects;
use crate::modrinth::{DependencyType, is_trusted_download};
use crate::{ContentError, ContentKind, Modrinth, Result, Version};

/// Iris: serve per caricare gli shader su Fabric.
pub const IRIS_PROJECT_ID: &str = "YL57xq9U";

/// Limite di sicurezza sulla catena di dipendenze.
const MAX_FILES: usize = 64;

#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub kind: ContentKind,
    /// Progetto: id Modrinth, oppure `cf:<id>` per CurseForge.
    pub project_id: String,
    /// Nome della versione (es. "Sodium 0.6.0").
    pub name: String,
    pub version_number: String,
    pub filename: String,
    pub url: String,
    pub sha1: Sha1Hex,
    pub size: u64,
    pub dest: PathBuf,
    /// File della stessa mod già installato, da togliere dopo il download.
    pub replaces: Option<PathBuf>,
}

/// Cosa verrà installato: il progetto scelto per primo, poi le dipendenze.
#[derive(Debug, Clone, Default)]
pub struct InstallPlan {
    pub files: Vec<PlannedFile>,
}

impl InstallPlan {
    pub fn total_bytes(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// Scarica e verifica tutti i file; solo dopo toglie le vecchie versioni
    /// delle mod sostituite.
    pub async fn execute(
        &self,
        instance: &Instance,
        downloader: &Downloader,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<()> {
        // Niente partita né altre installazioni sull'istanza intanto.
        let _lock = instance.try_lock()?;
        let tasks = self
            .files
            .iter()
            .map(|f| DownloadTask::new(&f.url, f.dest.clone(), f.sha1.clone(), Some(f.size)))
            .collect();
        let replaced = self
            .files
            .iter()
            .filter_map(|f| f.replaces.clone().map(|old| (old, f.dest.clone())))
            .collect();
        download_replacing(downloader, tasks, replaced, progress).await
    }
}

/// Scarica tutti i file e solo dopo toglie le versioni sostituite
/// (`(vecchio, nuovo)`). Se un download fallisce, i file nuovi già scritti
/// vengono tolti: non restano mai due versioni della stessa mod insieme.
pub(crate) async fn download_replacing(
    downloader: &Downloader,
    tasks: Vec<DownloadTask>,
    replaced: Vec<(PathBuf, PathBuf)>,
    progress: Option<ProgressFn<'_>>,
) -> Result<()> {
    let fresh: Vec<PathBuf> = tasks
        .iter()
        .filter(|t| !t.path.exists())
        .map(|t| t.path.clone())
        .collect();
    if let Err(e) = downloader.run(tasks, progress).await {
        for path in &fresh {
            let _ = std::fs::remove_file(path);
        }
        return Err(e.into());
    }
    for (old, new) in replaced {
        if old != new {
            std::fs::remove_file(&old).map_err(|e| CoreError::io(&old, e))?;
        }
    }
    Ok(())
}

/// Cartella in cui installare un contenuto di questo tipo.
pub(crate) fn target_dir(
    instance: &Instance,
    kind: ContentKind,
    world: Option<&str>,
) -> Result<PathBuf> {
    if let Some(dir) = kind.content_dir() {
        return Ok(instance.content_dir(dir));
    }
    match kind {
        ContentKind::DataPack => {
            let world = safe_component(world.ok_or(ContentError::WorldRequired)?)?;
            let dir = instance.saves_dir().join(world);
            if !dir.join("level.dat").is_file() {
                return Err(ContentError::WorldNotFound(world.to_owned()));
            }
            Ok(dir.join("datapacks"))
        }
        _ => Err(ContentError::ModpackIsNotContent),
    }
}

/// Ultima versione compatibile con l'istanza (le release prima delle beta).
pub async fn pick_version(
    api: &Modrinth,
    instance: &Instance,
    project: &str,
    kind: ContentKind,
) -> Result<Version> {
    let loaders = kind.loaders(instance.config.loader.as_ref().map(|l| l.kind))?;
    let game = instance.config.version.as_str();
    let mut versions = api.versions(project, loaders, &[game]).await?;
    // Gli shader raramente dichiarano le versioni più recenti del gioco, ma
    // funzionano lo stesso: si ripiega sull'ultima pubblicata.
    if versions.is_empty() && kind == ContentKind::Shader {
        versions = api.versions(project, loaders, &[]).await?;
    }
    preferred(versions).ok_or_else(|| ContentError::NoCompatibleVersion {
        project: project.to_owned(),
        game: game.to_owned(),
    })
}

/// La versione da installare tra quelle compatibili (dalla più recente):
/// la release più nuova, altrimenti l'ultima beta o alpha.
pub(crate) fn preferred(versions: Vec<Version>) -> Option<Version> {
    let best = versions
        .iter()
        .position(|v| v.version_type == "release")
        .unwrap_or(0);
    versions.into_iter().nth(best)
}

/// Prepara l'installazione di `project` (id o slug) e delle sue dipendenze
/// obbligatorie non ancora presenti. Per uno shader aggiunge Iris se manca.
pub async fn plan(
    api: &Modrinth,
    instance: &Instance,
    project: &str,
    kind: ContentKind,
    world: Option<&str>,
) -> Result<InstallPlan> {
    let dir = target_dir(instance, kind, world)?;
    // Le mod presenti servono solo per dipendenze, Iris e sostituzioni.
    let installed = if matches!(kind, ContentKind::Mod | ContentKind::Shader) {
        installed_mod_projects(api, instance).await?
    } else {
        HashMap::new()
    };
    let mut planner = Planner {
        instance,
        installed: &installed,
        seen: HashSet::new(),
        plan: InstallPlan::default(),
    };

    let root = pick_version(api, instance, project, kind).await?;
    let mut mods = VecDeque::new();
    planner.add(root.clone(), kind, &dir)?;
    if kind == ContentKind::Mod {
        mods.push_back(root);
    }
    if kind == ContentKind::Shader && !installed.contains_key(IRIS_PROJECT_ID) {
        let iris = pick_version(api, instance, IRIS_PROJECT_ID, ContentKind::Mod).await?;
        planner.add(iris.clone(), ContentKind::Mod, &instance.mods_dir())?;
        mods.push_back(iris);
    }

    while let Some(version) = mods.pop_front() {
        for dep in &version.dependencies {
            if dep.dependency_type != DependencyType::Required {
                continue;
            }
            if dep.project_id.as_ref().is_some_and(|p| planner.knows(p)) {
                continue;
            }
            // Meglio l'ultima versione compatibile del progetto che quella
            // fissata dalla dipendenza, spesso vecchia.
            let dep_version = match (&dep.project_id, &dep.version_id) {
                (Some(project), _) => {
                    pick_version(api, instance, project, ContentKind::Mod).await?
                }
                (None, Some(id)) => api.version(id).await?,
                (None, None) => continue,
            };
            if planner.knows(&dep_version.project_id) {
                continue;
            }
            planner.add(dep_version.clone(), ContentKind::Mod, &instance.mods_dir())?;
            mods.push_back(dep_version);
        }
    }
    Ok(planner.plan)
}

struct Planner<'a> {
    instance: &'a Instance,
    installed: &'a HashMap<String, PathBuf>,
    seen: HashSet<String>,
    plan: InstallPlan,
}

impl Planner<'_> {
    fn knows(&self, project: &str) -> bool {
        self.seen.contains(project) || self.installed.contains_key(project)
    }

    fn add(&mut self, version: Version, kind: ContentKind, dir: &Path) -> Result<()> {
        if self.plan.files.len() >= MAX_FILES {
            return Err(ContentError::InvalidPack(format!(
                "more than {MAX_FILES} dependencies"
            )));
        }
        let file =
            version
                .primary_file()
                .cloned()
                .ok_or_else(|| ContentError::NoCompatibleVersion {
                    project: version.project_id.clone(),
                    game: self.instance.config.version.clone(),
                })?;
        if !is_trusted_download(&file.url) {
            return Err(ContentError::UntrustedUrl(file.url));
        }
        let dest = match kind.content_dir() {
            // Stesse regole sul nome dei file gestiti dall'istanza.
            Some(content) => self.instance.content_path(content, &file.filename)?,
            None => {
                let name = safe_component(&file.filename)?;
                if !name.ends_with(".zip") {
                    return Err(ContentError::InvalidPack(format!(
                        "{name} is not a data pack"
                    )));
                }
                dir.join(name)
            }
        };
        self.seen.insert(version.project_id.clone());
        self.plan.files.push(PlannedFile {
            replaces: self.installed.get(&version.project_id).cloned(),
            kind,
            project_id: version.project_id,
            name: version.name,
            version_number: version.version_number,
            filename: file.filename,
            url: file.url,
            sha1: file.hashes.sha1,
            size: file.size,
            dest,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use hyper_core::Paths;
    use hyper_installer::{LoaderKind, LoaderSpec};
    use hyper_instances::InstanceManager;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::modrinth::tests::version_json;

    fn fabric_instance(root: &Path) -> Instance {
        InstanceManager::new(Paths::new(root))
            .create(
                "f",
                "26.3",
                Some(LoaderSpec {
                    kind: LoaderKind::Fabric,
                    version: "0.20.1".into(),
                }),
            )
            .unwrap()
    }

    async fn mock_versions(server: &MockServer, project: &str, versions: serde_json::Value) {
        Mock::given(method("GET"))
            .and(path(format!("/project/{project}/version")))
            .respond_with(ResponseTemplate::new(200).set_body_json(versions))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn plans_required_dependencies_once() {
        let server = MockServer::start().await;
        let sha = |c: char| c.to_string().repeat(40);
        let mut iris = version_json("vi", "iris-id", "iris.jar", &sha('1'));
        iris["dependencies"] = serde_json::json!([
            {"project_id": "sodium-id", "version_id": "old", "dependency_type": "required"},
            {"project_id": "modmenu-id", "dependency_type": "optional"}
        ]);
        let mut sodium = version_json("vs", "sodium-id", "sodium.jar", &sha('2'));
        // Dipendenza circolare: non deve ripetere Iris.
        sodium["dependencies"] =
            serde_json::json!([{"project_id": "iris-id", "dependency_type": "required"}]);
        mock_versions(&server, "iris", serde_json::json!([iris])).await;
        mock_versions(&server, "sodium-id", serde_json::json!([sodium])).await;

        let dir = tempfile::tempdir().unwrap();
        let instance = fabric_instance(dir.path());
        let api = Modrinth::with_base(server.uri()).unwrap();
        let plan = plan(&api, &instance, "iris", ContentKind::Mod, None)
            .await
            .unwrap();
        let names: Vec<_> = plan.files.iter().map(|f| f.filename.as_str()).collect();
        assert_eq!(names, ["iris.jar", "sodium.jar"]);
        assert_eq!(plan.files[1].dest, instance.mods_dir().join("sodium.jar"));
        assert_eq!(plan.total_bytes(), 6);
    }

    #[tokio::test]
    async fn refuses_untrusted_or_unsafe_files() {
        let server = MockServer::start().await;
        let mut evil = version_json("v", "p", "x.jar", &"3".repeat(40));
        evil["files"][0]["url"] = "https://evil.example/x.jar".into();
        mock_versions(&server, "evil", serde_json::json!([evil])).await;
        let mut traversal = version_json("v2", "p2", "x.jar", &"4".repeat(40));
        traversal["files"][0]["filename"] = "../../x.jar".into();
        mock_versions(&server, "traversal", serde_json::json!([traversal])).await;

        let dir = tempfile::tempdir().unwrap();
        let instance = fabric_instance(dir.path());
        let api = Modrinth::with_base(server.uri()).unwrap();
        assert!(matches!(
            plan(&api, &instance, "evil", ContentKind::Mod, None).await,
            Err(ContentError::UntrustedUrl(_))
        ));
        assert!(
            plan(&api, &instance, "traversal", ContentKind::Mod, None)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn failed_batch_keeps_the_old_version_only() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/new.jar"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"abc".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.jar");
        let new = dir.path().join("new.jar");
        std::fs::write(&old, b"old").unwrap();
        let tasks = vec![
            DownloadTask::new(
                format!("{}/new.jar", server.uri()),
                new.clone(),
                "a9993e364706816aba3e25717850c26c9cd0d89d".parse().unwrap(),
                Some(3),
            ),
            DownloadTask::new(
                format!("{}/missing.jar", server.uri()),
                dir.path().join("other.jar"),
                "a9993e364706816aba3e25717850c26c9cd0d89d".parse().unwrap(),
                Some(3),
            ),
        ];
        let downloader = Downloader::new(1).unwrap();
        let result =
            download_replacing(&downloader, tasks, vec![(old.clone(), new.clone())], None).await;
        assert!(result.is_err());
        assert!(old.is_file(), "the old version must survive");
        assert!(!new.exists(), "the new version must be rolled back");
    }

    #[tokio::test]
    async fn data_packs_need_an_existing_world() {
        let dir = tempfile::tempdir().unwrap();
        let instance = fabric_instance(dir.path());
        let api = Modrinth::with_base("http://127.0.0.1:9").unwrap();
        assert!(matches!(
            plan(&api, &instance, "x", ContentKind::DataPack, None).await,
            Err(ContentError::WorldRequired)
        ));
        assert!(matches!(
            plan(&api, &instance, "x", ContentKind::DataPack, Some("Mondo")).await,
            Err(ContentError::WorldNotFound(_))
        ));
        assert!(
            plan(&api, &instance, "x", ContentKind::DataPack, Some("../x"))
                .await
                .is_err()
        );
    }
}
