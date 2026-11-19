//! Client dell'API di CurseForge (v1). Serve una chiave: quella del
//! launcher viene incorporata in compilazione (`HYPER_CURSEFORGE_KEY`, da un
//! segreto della CI) oppure letta dalla variabile d'ambiente omonima, utile
//! in sviluppo. Senza chiave CurseForge non è disponibile.
//!
//! La chiave viaggia solo nell'header `x-api-key`: non compare in URL,
//! errori, log o `Debug`.

use std::sync::Arc;
use std::time::Duration;

use hyper_installer::{LoaderKind, Sha1Hex};
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{ContentError, ContentKind, Result};

pub const API_URL: &str = "https://api.curseforge.com/v1";
/// Id di Minecraft su CurseForge.
pub const MINECRAFT: u32 = 432;
/// Variabile (di compilazione o d'ambiente) con la chiave API.
pub const KEY_ENV: &str = "HYPER_CURSEFORGE_KEY";

/// Host da cui CurseForge serve i file: tutto il resto viene rifiutato.
const TRUSTED_HOSTS: &[&str] = &["edge.forgecdn.net", "mediafilez.forgecdn.net"];

/// `true` se `url` è HTTPS verso la CDN di CurseForge.
pub fn is_trusted_download(url: &str) -> bool {
    Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.host_str().is_some_and(|h| TRUSTED_HOSTS.contains(&h))
    })
}

/// Categoria principale ("class") di CurseForge per un tipo di contenuto.
pub fn class_id(kind: ContentKind) -> u32 {
    match kind {
        ContentKind::Mod => 6,
        ContentKind::ResourcePack => 12,
        ContentKind::Shader => 6552,
        ContentKind::DataPack => 6945,
        ContentKind::Modpack => 4471,
        ContentKind::Plugin => 5,
    }
}

/// Tipo di contenuto di una "class" di CurseForge (`None` per mondi,
/// plugin Bukkit e altro che il launcher non installa).
pub fn kind_of_class(class: u32) -> Option<ContentKind> {
    [
        ContentKind::Mod,
        ContentKind::ResourcePack,
        ContentKind::Shader,
        ContentKind::DataPack,
        ContentKind::Modpack,
    ]
    .into_iter()
    .find(|k| class_id(*k) == class)
}

/// `modLoaderType` di CurseForge.
pub fn loader_type(kind: LoaderKind) -> u32 {
    match kind {
        LoaderKind::Forge => 1,
        LoaderKind::Fabric => 4,
        LoaderKind::Quilt => 5,
        LoaderKind::NeoForge => 6,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMod {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: f64,
    #[serde(default)]
    pub class_id: Option<u32>,
    #[serde(default)]
    pub logo: Option<CfLogo>,
    #[serde(default)]
    pub links: CfLinks,
    #[serde(default)]
    pub authors: Vec<CfAuthor>,
    #[serde(default)]
    pub latest_files_indexes: Vec<CfFileIndex>,
    #[serde(default)]
    pub date_modified: String,
}

impl CfMod {
    pub fn icon_url(&self) -> Option<&str> {
        let logo = self.logo.as_ref()?;
        logo.thumbnail_url.as_deref().or(logo.url.as_deref())
    }

    pub fn kind(&self) -> Option<ContentKind> {
        self.class_id.and_then(kind_of_class)
    }

    /// Pagina del progetto su curseforge.com.
    pub fn website(&self) -> String {
        self.links
            .website_url
            .clone()
            .unwrap_or_else(|| format!("https://www.curseforge.com/minecraft/{}", self.slug))
    }

    pub fn author(&self) -> String {
        self.authors
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }

    /// File più recente per questa versione del gioco e questo loader
    /// (`None` = loader non richiesto, es. resource pack). Con `releases_only`
    /// si considerano solo le release.
    pub fn latest_file(
        &self,
        game: &str,
        loader: Option<u32>,
        releases_only: bool,
    ) -> Option<&CfFileIndex> {
        self.latest_files_indexes
            .iter()
            .filter(|i| i.game_version == game)
            .filter(|i| loader.is_none() || i.mod_loader == loader)
            .filter(|i| !releases_only || i.release_type == RELEASE)
            .max_by_key(|i| i.file_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLogo {
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLinks {
    #[serde(default)]
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfAuthor {
    pub name: String,
}

/// Ultimo file di un progetto per una coppia (versione del gioco, loader).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFileIndex {
    pub game_version: String,
    pub file_id: u32,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub release_type: u8,
    #[serde(default)]
    pub mod_loader: Option<u32>,
}

/// `releaseType` di una release (2 = beta, 3 = alpha).
pub const RELEASE: u8 = 1;
/// `relationType` di una dipendenza obbligatoria.
pub const REQUIRED_DEPENDENCY: u32 = 3;
/// `algo` di un hash SHA1.
const SHA1: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    pub id: u32,
    pub mod_id: u32,
    #[serde(default)]
    pub display_name: String,
    pub file_name: String,
    /// `None` quando l'autore non permette il download dai launcher.
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub release_type: u8,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<CfDependency>,
    #[serde(default)]
    pub file_fingerprint: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfHash {
    pub value: String,
    pub algo: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfDependency {
    pub mod_id: u32,
    pub relation_type: u32,
}

impl CfFile {
    /// URL (solo CDN di CurseForge) e SHA1 con cui scaricare il file.
    /// Errore se l'autore non permette il download, se manca l'hash o se
    /// l'URL non è ammesso.
    pub fn download(&self) -> Result<(String, Sha1Hex)> {
        let url = self
            .download_url
            .clone()
            .ok_or_else(|| ContentError::DistributionBlocked {
                name: self.display_name.clone(),
                mod_id: self.mod_id,
            })?;
        if !is_trusted_download(&url) {
            return Err(ContentError::UntrustedUrl(url));
        }
        let sha1 = self
            .hashes
            .iter()
            .find(|h| h.algo == SHA1)
            .and_then(|h| h.value.parse().ok())
            .ok_or_else(|| ContentError::InvalidPack(format!("{} has no SHA1", self.file_name)))?;
        Ok((url, sha1))
    }

    pub fn required_dependencies(&self) -> impl Iterator<Item = u32> + '_ {
        self.dependencies
            .iter()
            .filter(|d| d.relation_type == REQUIRED_DEPENDENCY)
            .map(|d| d.mod_id)
    }
}

/// Le tre liste della pagina "in evidenza" di CurseForge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Featured {
    #[serde(default)]
    pub featured: Vec<CfMod>,
    #[serde(default)]
    pub popular: Vec<CfMod>,
    #[serde(default)]
    pub recently_updated: Vec<CfMod>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintResponse {
    #[serde(default)]
    exact_matches: Vec<FingerprintMatch>,
}

/// File riconosciuto dalla sua impronta.
#[derive(Debug, Clone, Deserialize)]
pub struct FingerprintMatch {
    /// Id del progetto.
    pub id: u32,
    pub file: CfFile,
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

/// Client CurseForge. Clonarlo è economico (condivide connessioni e chiave).
#[derive(Clone)]
pub struct CurseForge {
    client: reqwest::Client,
    base: String,
    key: Arc<str>,
}

impl std::fmt::Debug for CurseForge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurseForge")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl CurseForge {
    /// Client con la chiave del launcher; `None` se non c'è (CurseForge
    /// allora non viene offerto).
    pub fn new() -> Option<Self> {
        let key = std::env::var(KEY_ENV)
            .ok()
            .or_else(|| option_env!("HYPER_CURSEFORGE_KEY").map(str::to_owned))
            .map(|k| k.trim().to_owned())
            .filter(|k| !k.is_empty())?;
        match Self::with_base(key, API_URL) {
            Ok(client) => Some(client),
            Err(e) => {
                tracing::warn!(error = %e, "cannot create the CurseForge client");
                None
            }
        }
    }

    /// Client verso un'altra base (per i test).
    pub fn with_base(key: impl Into<String>, base: impl Into<String>) -> Result<Self> {
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
            key: key.into().into(),
        })
    }

    async fn send<T: DeserializeOwned>(&self, request: reqwest::RequestBuilder) -> Result<T> {
        let request = request
            .header("x-api-key", &*self.key)
            .header("Accept", "application/json")
            .build()
            .map_err(|source| ContentError::Http {
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
        serde_json::from_slice::<Envelope<T>>(&bytes)
            .map(|e| e.data)
            .map_err(|source| ContentError::Json { url, source })
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.get(format!("{}{path}", self.base))
    }

    fn post(&self, path: &str, body: serde_json::Value) -> reqwest::RequestBuilder {
        self.client.post(format!("{}{path}", self.base)).json(&body)
    }

    /// Progetti in evidenza, popolari e aggiornati di recente.
    pub async fn featured(&self) -> Result<Featured> {
        let body = serde_json::json!({ "gameId": MINECRAFT, "excludedModIds": [] });
        self.send(self.post("/mods/featured", body)).await
    }

    pub async fn project(&self, id: u32) -> Result<CfMod> {
        self.send(self.get(&format!("/mods/{id}"))).await
    }

    /// Più progetti in una richiesta.
    pub async fn projects(&self, ids: &[u32]) -> Result<Vec<CfMod>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "modIds": ids });
        self.send(self.post("/mods", body)).await
    }

    /// Più file in una richiesta.
    pub async fn files(&self, ids: &[u32]) -> Result<Vec<CfFile>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "fileIds": ids });
        self.send(self.post("/mods/files", body)).await
    }

    /// File di un progetto (dal più recente), filtrati per versione del
    /// gioco e loader se indicati.
    pub async fn project_files(
        &self,
        id: u32,
        game: Option<&str>,
        loader: Option<u32>,
    ) -> Result<Vec<CfFile>> {
        let mut request = self
            .get(&format!("/mods/{id}/files"))
            .query(&[("pageSize", "50")]);
        if let Some(game) = game {
            request = request.query(&[("gameVersion", game)]);
        }
        if let Some(loader) = loader {
            request = request.query(&[("modLoaderType", loader)]);
        }
        let mut files: Vec<CfFile> = self.send(request).await?;
        files.sort_by_key(|f| std::cmp::Reverse(f.id));
        Ok(files)
    }

    /// File riconosciuti dalle loro impronte (quelli sconosciuti mancano).
    pub async fn by_fingerprint(&self, fingerprints: &[u32]) -> Result<Vec<FingerprintMatch>> {
        if fingerprints.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "fingerprints": fingerprints });
        let response: FingerprintResponse = self
            .send(self.post(&format!("/fingerprints/{MINECRAFT}"), body))
            .await?;
        Ok(response.exact_matches)
    }
}

/// Impronta di un file come la calcola CurseForge: MurmurHash2 (seme 1)
/// dei byte, tolti tab, a capo e spazi.
pub fn fingerprint(bytes: &[u8]) -> u32 {
    let data: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !matches!(b, 9 | 10 | 13 | 32))
        .collect();
    murmur2(&data, 1)
}

fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let (chunks, rest) = data.as_chunks::<4>();
    for chunk in chunks {
        let mut k = u32::from_le_bytes(*chunk);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    if !rest.is_empty() {
        for (i, &b) in rest.iter().enumerate() {
            h ^= u32::from(b) << (8 * i);
        }
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

#[cfg(test)]
pub(crate) mod tests {
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    pub(crate) fn file_json(id: u32, mod_id: u32, name: &str, sha1: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "modId": mod_id, "displayName": name, "fileName": name,
            "downloadUrl": format!("https://edge.forgecdn.net/files/{id}/{name}"),
            "hashes": [{"value": sha1, "algo": 1}, {"value": "ab", "algo": 2}],
            "fileLength": 3, "releaseType": 1, "gameVersions": ["26.3", "Fabric"],
            "dependencies": [], "fileFingerprint": 1
        })
    }

    #[test]
    fn fingerprint_ignores_whitespace() {
        assert_eq!(fingerprint(b"a b\tc\r\nd"), fingerprint(b"abcd"));
        assert_ne!(fingerprint(b"abcd"), fingerprint(b"abce"));
        // Code di 1, 2 e 3 byte dopo i blocchi da 4.
        assert_ne!(fingerprint(b"abcde"), fingerprint(b"abcdf"));
        assert_ne!(fingerprint(b"abcdef"), fingerprint(b"abcdeg"));
        assert_ne!(fingerprint(b"abcdefg"), fingerprint(b"abcdefh"));
    }

    #[test]
    fn downloads_only_from_the_cdn_with_sha1() {
        let mut file: CfFile =
            serde_json::from_value(file_json(5, 9, "a.jar", &"a".repeat(40))).unwrap();
        assert!(file.download().is_ok());
        file.download_url = Some("https://evil.example/a.jar".into());
        assert!(matches!(
            file.download(),
            Err(ContentError::UntrustedUrl(_))
        ));
        file.download_url = None;
        assert!(matches!(
            file.download(),
            Err(ContentError::DistributionBlocked { mod_id: 9, .. })
        ));
        file.download_url = Some("https://edge.forgecdn.net/files/5/a.jar".into());
        file.hashes.retain(|h| h.algo != 1);
        assert!(file.download().is_err());
    }

    #[test]
    fn picks_latest_file_for_game_and_loader() {
        let project: CfMod = serde_json::from_value(serde_json::json!({
            "id": 1, "name": "X",
            "latestFilesIndexes": [
                {"gameVersion": "26.3", "fileId": 10, "releaseType": 1, "modLoader": 4},
                {"gameVersion": "26.3", "fileId": 12, "releaseType": 2, "modLoader": 4},
                {"gameVersion": "26.3", "fileId": 13, "releaseType": 1, "modLoader": 6},
                {"gameVersion": "26.2", "fileId": 14, "releaseType": 1, "modLoader": 4}
            ]
        }))
        .unwrap();
        assert_eq!(
            project.latest_file("26.3", Some(4), false).unwrap().file_id,
            12
        );
        assert_eq!(
            project.latest_file("26.3", Some(4), true).unwrap().file_id,
            10
        );
        assert_eq!(
            project.latest_file("26.3", None, false).unwrap().file_id,
            13
        );
        assert!(project.latest_file("1.20.1", Some(4), false).is_none());
    }

    #[tokio::test]
    async fn sends_the_key_and_unwraps_data() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/fingerprints/432"))
            .and(header("x-api-key", "secret-key"))
            .and(body_json(serde_json::json!({ "fingerprints": [7] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"exactMatches": [{"id": 9, "file": file_json(5, 9, "a.jar", &"a".repeat(40))}]}
            })))
            .mount(&server)
            .await;
        let api = CurseForge::with_base("secret-key", server.uri()).unwrap();
        let matches = api.by_fingerprint(&[7]).await.unwrap();
        assert_eq!(matches[0].id, 9);
        assert!(!format!("{api:?}").contains("secret-key"));
    }
}
