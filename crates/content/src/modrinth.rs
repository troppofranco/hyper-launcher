//! Client dell'API pubblica di Modrinth (v2, senza chiave).

use std::collections::HashMap;
use std::time::Duration;

use hyper_installer::Sha1Hex;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{ContentError, ContentKind, Result};

pub const API_URL: &str = "https://api.modrinth.com/v2";

/// Host da cui si accettano download: quelli ammessi dalla specifica dei
/// modpack Modrinth. Tutto il resto viene rifiutato prima di scaricare.
const TRUSTED_HOSTS: &[&str] = &[
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

/// `true` se `url` è HTTPS verso uno degli host ammessi.
pub fn is_trusted_download(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.host_str().is_some_and(|h| TRUSTED_HOSTS.contains(&h))
    })
}

/// Valida un id o slug di Modrinth prima di metterlo in un URL.
pub(crate) fn checked_id(id: &str) -> Result<&str> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !id.starts_with('.');
    if ok {
        Ok(id)
    } else {
        Err(ContentError::InvalidId(id.to_owned()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    #[default]
    Relevance,
    Downloads,
    Updated,
}

impl SearchSort {
    fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Updated => "updated",
        }
    }
}

/// Parametri di una ricerca nel catalogo.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub kind: ContentKind,
    /// Solo progetti con una versione per questa versione di Minecraft.
    pub game_version: Option<String>,
    /// Categorie Modrinth (es. `optimization`), tutte richieste.
    pub categories: Vec<String>,
    /// Loader Modrinth ammessi (uno qualsiasi); vuoto = nessun filtro.
    pub loaders: Vec<String>,
    pub sort: SearchSort,
    pub offset: u32,
    pub limit: u32,
    /// Solo progetti che funzionano su un server (mod lato server).
    pub server_side: bool,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>, kind: ContentKind) -> Self {
        Self {
            text: text.into(),
            kind,
            game_version: None,
            categories: Vec::new(),
            loaders: Vec::new(),
            sort: SearchSort::Relevance,
            offset: 0,
            limit: 20,
            server_side: false,
        }
    }

    /// Facet di Modrinth: lista esterna in AND, liste interne in OR.
    fn facets(&self) -> String {
        let mut facets = vec![vec![format!("project_type:{}", self.kind.project_type())]];
        // Gli shader non dichiarano quasi mai le ultime versioni del gioco ma
        // funzionano lo stesso (l'installazione ripiega sull'ultima uscita).
        if let Some(game) = self
            .game_version
            .as_ref()
            .filter(|_| self.kind != ContentKind::Shader)
        {
            facets.push(vec![format!("versions:{game}")]);
        }
        // Filtro sul loader (in OR). "minecraft" e "datapack" sono impliciti
        // nel tipo di progetto e non sono categorie da filtrare.
        let loaders: Vec<String> = self
            .loaders
            .iter()
            .filter(|l| !matches!(l.as_str(), "minecraft" | "datapack"))
            .map(|l| format!("categories:{l}"))
            .collect();
        if !loaders.is_empty() {
            facets.push(loaders);
        } else if self.kind == ContentKind::Modpack {
            // Solo i modpack per loader che sappiamo installare.
            facets.push(
                ContentKind::Modpack
                    .loaders(None)
                    .unwrap_or_default()
                    .iter()
                    .map(|l| format!("categories:{l}"))
                    .collect(),
            );
        } else if self.kind == ContentKind::Shader {
            // Gli unici shader che sappiamo caricare (tramite Iris).
            facets.push(vec![
                "categories:iris".to_owned(),
                "categories:optifine".to_owned(),
            ]);
        }
        for category in &self.categories {
            facets.push(vec![format!("categories:{category}")]);
        }
        if self.server_side {
            facets.push(vec![
                "server_side:required".to_owned(),
                "server_side:optional".to_owned(),
            ]);
        }
        serde_json::to_string(&facets).expect("facets are plain strings")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub date_modified: String,
    /// Immagine in evidenza della galleria (per le schede del catalogo).
    pub featured_gallery: Option<String>,
    #[serde(default)]
    pub gallery: Vec<String>,
    /// Colore dominante dell'icona (RGB), usato se manca l'immagine.
    pub color: Option<u32>,
    #[serde(default)]
    pub display_categories: Vec<String>,
    pub project_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub project_type: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub updated: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    pub license: Option<License>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GalleryImage {
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct License {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

impl Version {
    /// Il file da installare: quello primario, altrimenti il primo.
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }

    /// Il file con questo SHA1, se fa parte della versione.
    pub fn file_with_sha1(&self, sha1: &Sha1Hex) -> Option<&VersionFile> {
        self.files.iter().find(|f| &f.hashes.sha1 == sha1)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub size: u64,
    pub hashes: FileHashes,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileHashes {
    /// Validato in deserializzazione: un hash malformato fa fallire la
    /// risposta invece di arrivare al downloader.
    pub sha1: Sha1Hex,
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
    #[serde(other)]
    Unknown,
}

/// Client Modrinth. Clonarlo è economico (condivide le connessioni).
#[derive(Debug, Clone)]
pub struct Modrinth {
    client: reqwest::Client,
    base: String,
}

impl Modrinth {
    pub fn new() -> Result<Self> {
        Self::with_base(API_URL)
    }

    /// Client verso un'altra base (per i test).
    pub fn with_base(base: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(hyper_core::USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|source| ContentError::Http {
                url: API_URL.to_owned(),
                source,
            })?;
        Ok(Self {
            client,
            base: base.into().trim_end_matches('/').to_owned(),
        })
    }

    async fn send<T: DeserializeOwned>(&self, request: reqwest::RequestBuilder) -> Result<T> {
        let request = request.build().map_err(|source| ContentError::Http {
            url: self.base.clone(),
            source,
        })?;
        let url = request.url().to_string();
        let http = |source| ContentError::Http {
            url: url.clone(),
            source,
        };
        let bytes = self
            .client
            .execute(request)
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(http)?
            .bytes()
            .await
            .map_err(http)?;
        serde_json::from_slice(&bytes).map_err(|source| ContentError::Json { url, source })
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.get(format!("{}{path}", self.base))
    }

    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResults> {
        let request = self.get("/search").query(&[
            ("query", query.text.as_str()),
            ("facets", &query.facets()),
            ("index", query.sort.as_str()),
            ("offset", &query.offset.to_string()),
            ("limit", &query.limit.clamp(1, 100).to_string()),
        ]);
        self.send(request).await
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<Project> {
        let path = format!("/project/{}", checked_id(id_or_slug)?);
        self.send(self.get(&path)).await
    }

    /// Più progetti in una richiesta (per nomi e icone dei file installati).
    pub async fn projects(&self, ids: &[String]) -> Result<Vec<Project>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        for id in ids {
            checked_id(id)?;
        }
        let ids = serde_json::to_string(ids).expect("ids are strings");
        self.send(self.get("/projects").query(&[("ids", ids)]))
            .await
    }

    /// Versioni di un progetto, dalla più recente, filtrate per loader e
    /// versione di Minecraft (filtri vuoti = nessun filtro).
    pub async fn versions(
        &self,
        id_or_slug: &str,
        loaders: &[&str],
        game_versions: &[&str],
    ) -> Result<Vec<Version>> {
        let path = format!("/project/{}/version", checked_id(id_or_slug)?);
        let mut request = self.get(&path);
        if !loaders.is_empty() {
            request = request.query(&[("loaders", serde_json::to_string(loaders).unwrap())]);
        }
        if !game_versions.is_empty() {
            request = request.query(&[(
                "game_versions",
                serde_json::to_string(game_versions).unwrap(),
            )]);
        }
        self.send(request).await
    }

    pub async fn version(&self, id: &str) -> Result<Version> {
        let path = format!("/version/{}", checked_id(id)?);
        self.send(self.get(&path)).await
    }

    /// Versioni a cui appartengono i file con questi SHA1 (chiave: hash).
    /// I file sconosciuti a Modrinth semplicemente mancano dalla mappa.
    pub async fn versions_by_hash(&self, hashes: &[Sha1Hex]) -> Result<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({ "hashes": hashes_json(hashes), "algorithm": "sha1" });
        let request = self
            .client
            .post(format!("{}/version_files", self.base))
            .json(&body);
        self.send(request).await
    }

    /// Ultima versione compatibile per ciascun file (chiave: hash attuale).
    pub async fn latest_by_hash(
        &self,
        hashes: &[Sha1Hex],
        loaders: &[&str],
        game_versions: &[&str],
    ) -> Result<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({
            "hashes": hashes_json(hashes),
            "algorithm": "sha1",
            "loaders": loaders,
            "game_versions": game_versions,
        });
        let request = self
            .client
            .post(format!("{}/version_files/update", self.base))
            .json(&body);
        self.send(request).await
    }
}

fn hashes_json(hashes: &[Sha1Hex]) -> Vec<&str> {
    hashes.iter().map(Sha1Hex::as_str).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use wiremock::matchers::{body_partial_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    pub(crate) fn version_json(
        id: &str,
        project: &str,
        file: &str,
        sha1: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "id": id, "project_id": project, "name": file, "version_number": "1.0",
            "version_type": "release", "date_published": "2026-11-01T00:00:00Z",
            "game_versions": ["26.3"], "loaders": ["fabric"],
            "files": [{
                "url": format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{file}"),
                "filename": file, "primary": true, "size": 3,
                "hashes": { "sha1": sha1, "sha512": "ab" }
            }],
            "dependencies": []
        })
    }

    #[test]
    fn trusts_only_https_on_allowed_hosts() {
        assert!(is_trusted_download(
            "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"
        ));
        assert!(is_trusted_download(
            "https://github.com/o/r/releases/download/v1/a.jar"
        ));
        for bad in [
            "http://cdn.modrinth.com/a.jar",
            "https://cdn.modrinth.com.evil.com/a.jar",
            "https://evil.com/cdn.modrinth.com/a.jar",
            "https://user@cdn.modrinth.com/a.jar",
            "https://cdn.modrinth.com:8443/a.jar",
            "file:///C:/Windows/a.jar",
            "not a url",
        ] {
            assert!(!is_trusted_download(bad), "{bad}");
        }
    }

    #[test]
    fn rejects_ids_that_could_change_the_url() {
        assert!(checked_id("AANobbMI").is_ok());
        assert!(checked_id("fabric-api").is_ok());
        for bad in ["", "../search", "a/b", "a?b", "a b", ".."] {
            assert!(checked_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn builds_facets_per_kind() {
        let mut q = SearchQuery::new("sodium", ContentKind::Mod);
        q.game_version = Some("26.3".into());
        q.loaders = vec!["fabric".into()];
        q.categories = vec!["optimization".into()];
        assert_eq!(
            q.facets(),
            r#"[["project_type:mod"],["versions:26.3"],["categories:fabric"],["categories:optimization"]]"#
        );
        let q = SearchQuery::new("", ContentKind::Shader);
        assert_eq!(
            q.facets(),
            r#"[["project_type:shader"],["categories:iris","categories:optifine"]]"#
        );
    }

    #[tokio::test]
    async fn searches_and_looks_up_hashes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("query", "sodium"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "hits": [{
                    "project_id": "AANobbMI", "slug": "sodium", "title": "Sodium",
                    "description": "Fast", "author": "jellysquid3", "icon_url": null,
                    "downloads": 10, "date_modified": "2026-11-12T00:00:00Z",
                    "display_categories": ["optimization"], "project_type": "mod"
                }],
                "offset": 0, "limit": 20, "total_hits": 1
            })))
            .mount(&server)
            .await;
        let sha1 = "a".repeat(40);
        Mock::given(method("POST"))
            .and(path("/version_files"))
            .and(body_partial_json(
                serde_json::json!({ "algorithm": "sha1" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                sha1.clone(): version_json("v1", "AANobbMI", "sodium.jar", &sha1)
            })))
            .mount(&server)
            .await;

        let api = Modrinth::with_base(server.uri()).unwrap();
        let results = api
            .search(&SearchQuery::new("sodium", ContentKind::Mod))
            .await
            .unwrap();
        assert_eq!(results.hits[0].slug, "sodium");

        let found = api
            .versions_by_hash(&[sha1.parse().unwrap()])
            .await
            .unwrap();
        assert_eq!(found[&sha1].project_id, "AANobbMI");
    }

    #[tokio::test]
    async fn rejects_malformed_hashes_in_responses() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/version/bad"))
            .respond_with(ResponseTemplate::new(200).set_body_json(version_json(
                "bad",
                "p",
                "x.jar",
                "../../evil",
            )))
            .mount(&server)
            .await;
        let api = Modrinth::with_base(server.uri()).unwrap();
        assert!(matches!(
            api.version("bad").await,
            Err(ContentError::Json { .. })
        ));
    }
}
