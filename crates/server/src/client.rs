//! Client dell'host dei server, usato dal launcher e dalla CLI.

use std::path::PathBuf;
use std::time::Duration;

use hyper_core::Paths;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::protocol::{
    ConsoleChunk, HostInfo, MAX_LINE, Op, Request, Response, RunningStatus, Stats,
};
use crate::{Result, ServerError};

fn process_alive(pid: u32) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    system.process(pid).is_some()
}

/// Avvia l'host in background con `args` (stesso eseguibile), senza finestra
/// e staccato da chi lo avvia: chiudere il launcher non lo ferma.
pub fn spawn_host(args: &[&str]) -> std::io::Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
        command.creation_flags(0x0800_0000 | 0x0000_0008 | 0x0000_0200);
    }
    command.spawn().map(drop)
}

#[derive(Debug, Clone)]
pub struct HostClient {
    file: PathBuf,
}

impl HostClient {
    pub fn new(paths: &Paths) -> Self {
        Self {
            file: paths.server_host_file(),
        }
    }

    fn info(&self) -> Result<HostInfo> {
        let bytes = std::fs::read(&self.file).map_err(|_| ServerError::HostUnavailable)?;
        let info: HostInfo =
            serde_json::from_slice(&bytes).map_err(|_| ServerError::HostUnavailable)?;
        // File rimasto da un host che non c'è più (es. PC riavviato): su
        // Windows collegarsi a una porta chiusa costa ~2 s, quindi lo si
        // riconosce dal processo e lo si toglie.
        if !process_alive(info.pid) {
            let _ = std::fs::remove_file(&self.file);
            return Err(ServerError::HostUnavailable);
        }
        Ok(info)
    }

    async fn request(&self, op: Op) -> Result<serde_json::Value> {
        let info = self.info()?;
        let exchange = async {
            let mut stream = TcpStream::connect(("127.0.0.1", info.port)).await?;
            let mut line = serde_json::to_string(&Request {
                token: info.token,
                op,
            })?;
            line.push('\n');
            stream.write_all(line.as_bytes()).await?;
            let mut reply = String::new();
            BufReader::new(stream.take(MAX_LINE))
                .read_line(&mut reply)
                .await?;
            Ok::<_, ServerError>(serde_json::from_str::<Response>(&reply)?)
        };
        let response = tokio::time::timeout(Duration::from_secs(10), exchange)
            .await
            .map_err(|_| ServerError::HostUnavailable)?
            .map_err(|e| match e {
                ServerError::Io(_) => ServerError::HostUnavailable,
                other => other,
            })?;
        if response.ok {
            Ok(response.data)
        } else {
            Err(ServerError::Host(response.error.unwrap_or_default()))
        }
    }

    async fn typed<T: DeserializeOwned>(&self, op: Op) -> Result<T> {
        Ok(serde_json::from_value(self.request(op).await?)?)
    }

    /// L'host risponde (al ping, con il token giusto).
    pub async fn is_running(&self) -> bool {
        self.request(Op::Ping).await.is_ok()
    }

    /// Avvia l'host con `spawn` se non risponde, e aspetta che sia pronto.
    pub async fn ensure(&self, spawn: impl FnOnce() -> std::io::Result<()>) -> Result<()> {
        if self.is_running().await {
            return Ok(());
        }
        spawn()?;
        for _ in 0..150 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if self.is_running().await {
                return Ok(());
            }
        }
        Err(ServerError::HostUnavailable)
    }

    /// Server accesi; vuoto se l'host non è attivo.
    pub async fn status(&self) -> Vec<RunningStatus> {
        self.typed(Op::Status).await.unwrap_or_default()
    }

    pub async fn start(&self, name: &str) -> Result<()> {
        self.request(Op::Start { name: name.into() })
            .await
            .map(drop)
    }

    pub async fn stop(&self, name: &str) -> Result<()> {
        self.request(Op::Stop { name: name.into() }).await.map(drop)
    }

    pub async fn kill(&self, name: &str) -> Result<()> {
        self.request(Op::Kill { name: name.into() }).await.map(drop)
    }

    pub async fn command(&self, name: &str, line: &str) -> Result<()> {
        self.request(Op::Command {
            name: name.into(),
            line: line.into(),
        })
        .await
        .map(drop)
    }

    pub async fn console(&self, name: &str, since: u64) -> Result<ConsoleChunk> {
        self.typed(Op::Console {
            name: name.into(),
            since,
        })
        .await
    }

    pub async fn stats(&self, name: &str) -> Result<Stats> {
        self.typed(Op::Stats { name: name.into() }).await
    }

    /// Ferma i server senza "resta acceso" (alla chiusura del launcher).
    pub async fn stop_unpinned(&self) -> Result<()> {
        self.request(Op::StopUnpinned).await.map(drop)
    }
}
