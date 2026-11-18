//! Installazione di un progetto CurseForge in un'istanza: file compatibile
//! più recente, dipendenze obbligatorie, download verificati con SHA1 dalla
//! sola CDN di CurseForge. Stesso [`InstallPlan`] dei progetti Modrinth.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use hyper_core::safe_component;
use hyper_instances::{ContentDir, Instance};

use crate::curseforge::{CfFile, CfMod, CurseForge, RELEASE, loader_type};
use crate::install::{InstallPlan, PlannedFile, target_dir};
use crate::library::installed_cf_projects;
use crate::{ContentError, ContentKind, Result};

/// Limite di sicurezza sulla catena di dipendenze (come per Modrinth).
const MAX_FILES: usize = 64;

/// `modLoaderType` con cui filtrare i file di questo tipo di contenuto
/// nell'istanza (solo le mod dipendono dal loader). Controlla anche che
/// l'istanza possa ospitare il contenuto.
fn loader_filter(instance: &Instance, kind: ContentKind) -> Result<Option<u32>> {
    let loader = instance.config.loader.as_ref().map(|l| l.kind);
    kind.loaders(loader)?;
    Ok(match kind {
        ContentKind::Mod => loader.map(loader_type),
        _ => None,
    })
}

/// File più recente compatibile con l'istanza (le release prima delle beta).
pub async fn pick_file(
    api: &CurseForge,
    instance: &Instance,
    project: &CfMod,
    kind: ContentKind,
) -> Result<CfFile> {
    let loader = loader_filter(instance, kind)?;
    let game = instance.config.version.as_str();
    let mut files = api.project_files(project.id, Some(game), loader).await?;
    // Shader e resource pack spesso non dichiarano le versioni più recenti
    // del gioco, ma funzionano lo stesso: si ripiega sull'ultimo file.
    if files.is_empty() && matches!(kind, ContentKind::Shader | ContentKind::ResourcePack) {
        files = api.project_files(project.id, None, None).await?;
    }
    let best = files
        .iter()
        .position(|f| f.release_type == RELEASE)
        .unwrap_or(0);
    files
        .into_iter()
        .nth(best)
        .ok_or_else(|| ContentError::NoCompatibleVersion {
            project: project.name.clone(),
            game: game.to_owned(),
        })
}

/// Prepara l'installazione del progetto CurseForge `id` e delle sue
/// dipendenze obbligatorie non ancora presenti.
pub async fn plan(
    api: &CurseForge,
    instance: &Instance,
    id: u32,
    world: Option<&str>,
) -> Result<InstallPlan> {
    let project = api.project(id).await?;
    let kind = project
        .kind()
        .ok_or_else(|| ContentError::InvalidKind(project.name.clone()))?;
    let dir = target_dir(instance, kind, world)?;
    let installed = if kind == ContentKind::Mod {
        installed_cf_projects(api, instance).await?
    } else {
        HashMap::new()
    };
    let mut planner = Planner {
        instance,
        installed: &installed,
        seen: HashSet::new(),
        plan: InstallPlan::default(),
    };

    let root = pick_file(api, instance, &project, kind).await?;
    planner.add(&project, root.clone(), kind, &dir)?;
    let mut queue = VecDeque::from([root]);
    while let Some(file) = queue.pop_front() {
        let missing: Vec<u32> = file
            .required_dependencies()
            .filter(|id| !planner.knows(*id))
            .collect();
        for dep in api.projects(&missing).await? {
            if planner.knows(dep.id) {
                continue;
            }
            let dep_file = pick_file(api, instance, &dep, ContentKind::Mod).await?;
            planner.add(
                &dep,
                dep_file.clone(),
                ContentKind::Mod,
                &instance.mods_dir(),
            )?;
            queue.push_back(dep_file);
        }
    }
    Ok(planner.plan)
}

struct Planner<'a> {
    instance: &'a Instance,
    installed: &'a HashMap<u32, PathBuf>,
    seen: HashSet<u32>,
    plan: InstallPlan,
}

impl Planner<'_> {
    fn knows(&self, project: u32) -> bool {
        self.seen.contains(&project) || self.installed.contains_key(&project)
    }

    fn add(&mut self, project: &CfMod, file: CfFile, kind: ContentKind, dir: &Path) -> Result<()> {
        if self.plan.files.len() >= MAX_FILES {
            return Err(ContentError::InvalidPack(format!(
                "more than {MAX_FILES} dependencies"
            )));
        }
        let (url, sha1) = file.download()?;
        let dest = match kind.content_dir() {
            // Stesse regole sul nome dei file gestiti dall'istanza.
            Some(content) => self.instance.content_path(content, &file.file_name)?,
            None => {
                let name = safe_component(&file.file_name)?;
                if !name.ends_with(".zip") {
                    return Err(ContentError::InvalidPack(format!(
                        "{name} is not a data pack"
                    )));
                }
                dir.join(name)
            }
        };
        self.seen.insert(project.id);
        self.plan.files.push(PlannedFile {
            kind,
            project_id: format!("cf:{}", project.id),
            name: project.name.clone(),
            version_number: file.display_name.clone(),
            filename: file.file_name.clone(),
            url,
            sha1,
            size: file.file_length,
            dest,
            replaces: self.installed.get(&project.id).cloned(),
        });
        Ok(())
    }
}

/// Cartella dell'istanza per un file di un modpack CurseForge, dalla
/// "class" del suo progetto (mod se sconosciuta).
pub(crate) fn pack_dir(kind: Option<ContentKind>) -> ContentDir {
    match kind {
        Some(ContentKind::ResourcePack) => ContentDir::ResourcePacks,
        Some(ContentKind::Shader) => ContentDir::ShaderPacks,
        _ => ContentDir::Mods,
    }
}

#[cfg(test)]
mod tests {
    use hyper_core::Paths;
    use hyper_installer::{LoaderKind, LoaderSpec};
    use hyper_instances::InstanceManager;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::curseforge::tests::file_json;

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

    fn mod_json(id: u32, name: &str) -> serde_json::Value {
        serde_json::json!({"id": id, "name": name, "slug": name, "classId": 6})
    }

    async fn mock(server: &MockServer, m: &str, p: &str, body: serde_json::Value) {
        Mock::given(method(m))
            .and(path(p))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": body})),
            )
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn plans_dependencies_with_the_instance_loader() {
        let server = MockServer::start().await;
        let sha = |c: char| c.to_string().repeat(40);
        let mut iris = file_json(20, 1, "iris.jar", &sha('1'));
        iris["dependencies"] = serde_json::json!([
            {"modId": 2, "relationType": 3},
            {"modId": 3, "relationType": 2}
        ]);
        let mut sodium = file_json(21, 2, "sodium.jar", &sha('2'));
        // Dipendenza circolare: non deve ripetere Iris.
        sodium["dependencies"] = serde_json::json!([{"modId": 1, "relationType": 3}]);
        mock(&server, "GET", "/mods/1", mod_json(1, "iris")).await;
        mock(
            &server,
            "POST",
            "/mods",
            serde_json::json!([mod_json(2, "sodium")]),
        )
        .await;
        // Solo i file per Fabric (modLoaderType 4) e 26.3.
        Mock::given(method("GET"))
            .and(path("/mods/1/files"))
            .and(query_param("modLoaderType", "4"))
            .and(query_param("gameVersion", "26.3"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": [iris]})),
            )
            .mount(&server)
            .await;
        mock(&server, "GET", "/mods/2/files", serde_json::json!([sodium])).await;

        let dir = tempfile::tempdir().unwrap();
        let instance = fabric_instance(dir.path());
        let api = CurseForge::with_base("k", server.uri()).unwrap();
        let plan = plan(&api, &instance, 1, None).await.unwrap();
        let names: Vec<_> = plan.files.iter().map(|f| f.filename.as_str()).collect();
        assert_eq!(names, ["iris.jar", "sodium.jar"]);
        assert_eq!(plan.files[1].dest, instance.mods_dir().join("sodium.jar"));
        assert_eq!(plan.files[0].project_id, "cf:1");
    }

    #[tokio::test]
    async fn blocked_files_are_reported() {
        let server = MockServer::start().await;
        let mut file = file_json(20, 1, "x.jar", &"1".repeat(40));
        file["downloadUrl"] = serde_json::Value::Null;
        mock(&server, "GET", "/mods/1", mod_json(1, "x")).await;
        mock(&server, "GET", "/mods/1/files", serde_json::json!([file])).await;
        let dir = tempfile::tempdir().unwrap();
        let instance = fabric_instance(dir.path());
        let api = CurseForge::with_base("k", server.uri()).unwrap();
        assert!(matches!(
            plan(&api, &instance, 1, None).await,
            Err(ContentError::DistributionBlocked { mod_id: 1, .. })
        ));
    }
}
