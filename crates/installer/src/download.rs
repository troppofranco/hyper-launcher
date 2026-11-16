//! Download paralleli con verifica SHA1 prima dell'uso.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use futures::{StreamExt, TryStreamExt, stream};
use hyper_core::{CoreError, fs};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use crate::hash::{check_hash, check_size, commit, file_is_valid};
use crate::{InstallerError, Result, Sha1Hex};

const MAX_ATTEMPTS: u32 = 3;
const USER_AGENT: &str = concat!("HyperLauncher/", env!("CARGO_PKG_VERSION"));

/// File da garantire su disco con un certo hash.
#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Sha1Hex,
    pub size: Option<u64>,
}

impl DownloadTask {
    pub fn new(url: impl Into<String>, path: PathBuf, sha1: Sha1Hex, size: Option<u64>) -> Self {
        Self {
            url: url.into(),
            path,
            sha1,
            size,
        }
    }
}

/// Avanzamento di un batch di download.
#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub downloaded_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DownloadStats {
    pub total: usize,
    pub downloaded: usize,
    pub downloaded_bytes: u64,
}

/// Callback di avanzamento; può essere invocata da più download concorrenti.
pub type ProgressFn<'a> = &'a (dyn Fn(Progress) + Sync);

#[derive(Debug, Clone)]
pub struct Downloader {
    client: reqwest::Client,
    concurrency: usize,
}

impl Downloader {
    pub fn new(concurrency: usize) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()
            .map_err(InstallerError::HttpClient)?;
        Ok(Self {
            client,
            concurrency: concurrency.max(1),
        })
    }

    /// Scarica in memoria una risorsa senza hash noto: solo le radici della
    /// catena di fiducia (version manifest, indice dei runtime Java), servite
    /// in HTTPS da Mojang.
    pub async fn get_bytes(&self, url: &str) -> Result<Vec<u8>> {
        with_retry(url, || async {
            let response = self.send(url).await?;
            let bytes = response.bytes().await.map_err(|e| http_error(url, e))?;
            Ok(bytes.to_vec())
        })
        .await
    }

    /// Garantisce un singolo file (scaricandolo se manca o è corrotto).
    pub async fn fetch(&self, task: DownloadTask) -> Result<PathBuf> {
        let path = task.path.clone();
        self.run(vec![task], None).await?;
        Ok(path)
    }

    /// Garantisce tutti i file in parallelo. I file già presenti vengono
    /// ri-verificati; quelli mancanti o corrotti riscaricati. Si ferma al
    /// primo errore definitivo (dopo i retry).
    pub async fn run(
        &self,
        tasks: Vec<DownloadTask>,
        progress: Option<ProgressFn<'_>>,
    ) -> Result<DownloadStats> {
        let mut seen = HashSet::new();
        let tasks: Vec<_> = tasks
            .into_iter()
            .filter(|t| seen.insert(t.path.clone()))
            .collect();

        let total = tasks.len();
        let done = &AtomicUsize::new(0);
        let downloaded = &AtomicUsize::new(0);
        let bytes = &AtomicU64::new(0);

        stream::iter(tasks)
            .map(|task| async move {
                if let Some(n) = self.ensure_with_retry(&task).await? {
                    downloaded.fetch_add(1, Ordering::Relaxed);
                    bytes.fetch_add(n, Ordering::Relaxed);
                }
                let done = done.fetch_add(1, Ordering::Relaxed) + 1;
                if let Some(report) = progress {
                    report(Progress {
                        done,
                        total,
                        downloaded_bytes: bytes.load(Ordering::Relaxed),
                    });
                }
                Ok::<_, InstallerError>(())
            })
            .buffer_unordered(self.concurrency)
            .try_collect::<()>()
            .await?;

        Ok(DownloadStats {
            total,
            downloaded: downloaded.load(Ordering::Relaxed),
            downloaded_bytes: bytes.load(Ordering::Relaxed),
        })
    }

    /// `Some(bytes)` se il file è stato scaricato, `None` se era già valido.
    async fn ensure_with_retry(&self, task: &DownloadTask) -> Result<Option<u64>> {
        let check = task.clone();
        let valid = tokio::task::spawn_blocking(move || {
            file_is_valid(&check.path, &check.sha1, check.size)
        })
        .await??;
        if valid {
            return Ok(None);
        }
        with_retry(&task.url, || self.download(task))
            .await
            .map(Some)
    }

    async fn send(&self, url: &str) -> Result<reqwest::Response> {
        self.client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| http_error(url, e))
    }

    /// Scarica in `<file>.<pid>.part` calcolando SHA1 e dimensione durante lo
    /// streaming; solo se combaciano il file viene rinominato su `task.path`.
    async fn download(&self, task: &DownloadTask) -> Result<u64> {
        if let Some(parent) = task.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| CoreError::io(parent, e))?;
        }
        // Il PID nel nome evita che due processi del launcher che scaricano lo
        // stesso file scrivano nello stesso temporaneo; dentro un processo
        // `run` deduplica già per percorso.
        let part = fs::sibling_with_suffix(&task.path, &format!("{}.part", std::process::id()));
        let io_err = |e| CoreError::io(&part, e);

        let response = self.send(&task.url).await?;
        let mut file = tokio::fs::File::create(&part).await.map_err(io_err)?;
        let mut body = response.bytes_stream();
        let mut hasher = Sha1::new();
        let mut written = 0u64;
        let streamed: Result<()> = async {
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(|e| http_error(&task.url, e))?;
                written += chunk.len() as u64;
                // Interrompe subito un corpo più grande del previsto invece di
                // riempire il disco e scoprirlo solo alla verifica finale.
                if task.size.is_some_and(|expected| written > expected) {
                    return check_size(&task.path, task.size, written);
                }
                hasher.update(&chunk);
                file.write_all(&chunk).await.map_err(io_err)?;
            }
            file.flush().await.map_err(io_err)?;
            Ok(())
        }
        .await;
        drop(file);

        let check = streamed.and_then(|()| {
            check_size(&task.path, task.size, written)?;
            let actual = Sha1Hex::try_from(hex::encode(hasher.finalize()))?;
            check_hash(&task.path, &task.sha1, &actual)
        });
        commit(&part, &task.path, check)?;
        Ok(written)
    }
}

fn http_error(url: &str, source: reqwest::Error) -> InstallerError {
    InstallerError::Http {
        url: url.to_owned(),
        source,
    }
}

/// Ripete `op` fino a `MAX_ATTEMPTS` volte sugli errori transitori.
async fn with_retry<T, F, Fut>(url: &str, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 1;
    loop {
        match op().await {
            Err(e) if attempt < MAX_ATTEMPTS && is_retryable(&e) => {
                tracing::warn!(url, attempt, error = %e, "request failed, retrying");
                tokio::time::sleep(Duration::from_millis(500 * u64::from(attempt))).await;
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn is_retryable(error: &InstallerError) -> bool {
    match error {
        InstallerError::Http { source, .. } => source
            .status()
            .is_none_or(|status| status.is_server_error() || status.as_u16() == 429),
        // Un file corrotto in transito può riuscire al tentativo successivo.
        InstallerError::HashMismatch { .. } | InstallerError::SizeMismatch { .. } => true,
        _ => false,
    }
}
