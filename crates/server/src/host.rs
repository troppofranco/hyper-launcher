//! L'host dei server: un processo in background, senza finestra, che avvia i
//! server Minecraft e ne tiene console e statistiche. Il launcher e la CLI
//! gli parlano via [`crate::client::HostClient`]; chiudere il launcher non
//! ferma i server con "resta acceso".
//!
//! Sicurezza: ascolta solo su 127.0.0.1, ogni richiesta deve portare il
//! token casuale scritto nel file `server-host.json` (cartella dati
//! dell'utente), e i comandi vanno solo allo stdin dei propri server.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use hyper_core::{Paths, fs};
use hyper_installer::java::java_executable;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{ChildStdin, Command};
use tokio::sync::Notify;

use crate::config::{JAR, ServerManager};
use crate::players::join_or_leave;
use crate::properties::Properties;
use crate::protocol::{
    ConsoleChunk, HostInfo, MAX_LINE, Op, Request, Response, RunningStatus, Stats, StatsPoint,
};
use crate::{Result, ServerError};

const CONSOLE_LINES: usize = 3000;
/// 5 minuti di statistiche, un punto ogni 2 secondi.
const HISTORY: usize = 150;
const SAMPLE_EVERY: Duration = Duration::from_secs(2);
/// Senza server accesi l'host si chiude dopo questo tempo.
const IDLE_EXIT: Duration = Duration::from_secs(60);
/// Dopo `stop`, il server viene terminato se non si chiude entro questo tempo.
const STOP_GRACE: Duration = Duration::from_secs(30);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ultime righe di console, numerate per leggerle a pezzi.
#[derive(Debug, Default)]
pub struct Console {
    first: u64,
    lines: VecDeque<String>,
}

impl Console {
    pub fn push(&mut self, line: String) {
        self.lines.push_back(line);
        if self.lines.len() > CONSOLE_LINES {
            self.lines.pop_front();
            self.first += 1;
        }
    }

    /// Righe dalla numero `since` (o dalla più vecchia ancora tenuta).
    pub fn since(&self, since: u64) -> ConsoleChunk {
        let skip = since.saturating_sub(self.first) as usize;
        ConsoleChunk {
            lines: self.lines.iter().skip(skip).cloned().collect(),
            next: self.first + self.lines.len() as u64,
        }
    }
}

struct Running {
    pid: u32,
    started: u64,
    port: u16,
    max_players: u32,
    stdin: Arc<tokio::sync::Mutex<Option<ChildStdin>>>,
    console: Arc<Mutex<Console>>,
    online: Arc<Mutex<HashSet<String>>>,
    history: VecDeque<StatsPoint>,
    exited: Arc<AtomicBool>,
    kill: Arc<Notify>,
}

struct Host {
    paths: Paths,
    manager: ServerManager,
    token: String,
    servers: Mutex<HashMap<String, Running>>,
    last_activity: Mutex<Instant>,
    /// Agent playit (uno per tutti i server con tunnel).
    agent: Mutex<Option<tokio::process::Child>>,
}

impl Host {
    fn active(&self) -> usize {
        lock(&self.servers)
            .values()
            .filter(|r| !r.exited.load(Ordering::Relaxed))
            .count()
    }

    fn touch(&self) {
        *lock(&self.last_activity) = Instant::now();
    }

    /// Avvia un server. Il controllo "già acceso" e l'inserimento avvengono
    /// sotto lo stesso lucchetto: due richieste insieme non lanciano due Java.
    fn start(&self, name: &str) -> Result<()> {
        let mut servers = lock(&self.servers);
        if servers
            .get(name)
            .is_some_and(|r| !r.exited.load(Ordering::Relaxed))
        {
            return Err(ServerError::AlreadyRunning(name.to_owned()));
        }
        let config = self.manager.load(name)?;
        let dir = self.manager.dir(name)?;
        let java = java_executable(&self.paths.runtime_dir(&config.java_component)?);
        let props = Properties::load(&dir)?.common();
        let mut command = Command::new(&java);
        command
            .arg(format!("-Xms{}M", config.memory_mb.min(1024)))
            .arg(format!("-Xmx{}M", config.memory_mb))
            .args(["-jar", JAR, "nogui"])
            .current_dir(&dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(false);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = command.spawn()?;
        let pid = child.id().unwrap_or_default();
        let stdin = Arc::new(tokio::sync::Mutex::new(child.stdin.take()));

        // Un riavvio continua la console precedente: la numerazione delle
        // righe resta crescente e chi legge non deve accorgersi di nulla.
        let console = servers
            .get(name)
            .map_or_else(Default::default, |r| r.console.clone());
        lock(&console).push(format!(
            "[Hyper Launcher] Avvio di {name} ({} {})…",
            config.kind.as_str(),
            config.version
        ));
        let online = Arc::new(Mutex::new(HashSet::new()));
        let exited = Arc::new(AtomicBool::new(false));
        let kill = Arc::new(Notify::new());

        // Output: righe in console, entrate e uscite dei giocatori.
        let reader = |stream: Box<dyn tokio::io::AsyncRead + Unpin + Send>| {
            let (console, online) = (console.clone(), online.clone());
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some((player, joined)) = join_or_leave(&line) {
                        let mut set = lock(&online);
                        if joined {
                            set.insert(player);
                        } else {
                            set.remove(&player);
                        }
                    }
                    lock(&console).push(line);
                }
            });
        };
        if let Some(out) = child.stdout.take() {
            reader(Box::new(out));
        }
        if let Some(err) = child.stderr.take() {
            reader(Box::new(err));
        }
        // Attesa della fine (o terminazione forzata).
        {
            let (console, online, exited, kill) = (
                console.clone(),
                online.clone(),
                exited.clone(),
                kill.clone(),
            );
            tokio::spawn(async move {
                let status = tokio::select! {
                    status = child.wait() => status,
                    () = kill.notified() => {
                        let _ = child.start_kill();
                        child.wait().await
                    }
                };
                let code = status.ok().and_then(|s| s.code());
                lock(&console).push(format!(
                    "[Hyper Launcher] Server fermato{}",
                    code.map(|c| format!(" (codice {c})")).unwrap_or_default()
                ));
                lock(&online).clear();
                exited.store(true, Ordering::Relaxed);
            });
        }
        let running = Running {
            pid,
            started: hyper_core::unix_now(),
            port: props.port,
            max_players: props.max_players,
            stdin,
            console,
            online,
            history: VecDeque::new(),
            exited,
            kill,
        };
        servers.insert(name.to_owned(), running);
        drop(servers);
        self.touch();
        self.sync_agent();
        Ok(())
    }

    fn running_field<T>(&self, name: &str, f: impl FnOnce(&Running) -> T) -> Result<T> {
        let servers = lock(&self.servers);
        let r = servers
            .get(name)
            .ok_or_else(|| ServerError::NotRunning(name.to_owned()))?;
        Ok(f(r))
    }

    async fn command(&self, name: &str, line: &str) -> Result<()> {
        let (stdin, exited) = self.running_field(name, |r| (r.stdin.clone(), r.exited.clone()))?;
        if exited.load(Ordering::Relaxed) {
            return Err(ServerError::NotRunning(name.to_owned()));
        }
        // Una sola riga: niente comandi nascosti dopo un a capo.
        let line: String = line
            .chars()
            .filter(|c| !c.is_control())
            .take(1000)
            .collect();
        let mut guard = stdin.lock().await;
        let stdin = guard
            .as_mut()
            .ok_or_else(|| ServerError::NotRunning(name.to_owned()))?;
        stdin.write_all(format!("{line}\n").as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// `stop` al server; se dopo 30 secondi è ancora acceso viene terminato.
    async fn stop(&self, name: &str) -> Result<()> {
        self.command(name, "stop").await?;
        let (exited, kill) = self.running_field(name, |r| (r.exited.clone(), r.kill.clone()))?;
        tokio::spawn(async move {
            tokio::time::sleep(STOP_GRACE).await;
            if !exited.load(Ordering::Relaxed) {
                kill.notify_one();
            }
        });
        Ok(())
    }

    fn status(&self) -> Vec<RunningStatus> {
        lock(&self.servers)
            .iter()
            .filter(|(_, r)| !r.exited.load(Ordering::Relaxed))
            .map(|(name, r)| RunningStatus {
                name: name.clone(),
                started: r.started,
                players: r
                    .history
                    .back()
                    .map_or_else(|| lock(&r.online).len() as u32, |p| p.players),
                max_players: r.max_players,
                pid: r.pid,
            })
            .collect()
    }

    async fn handle(&self, op: Op) -> Result<serde_json::Value> {
        match op {
            Op::Ping => Ok(serde_json::Value::Null),
            Op::Status => Ok(serde_json::to_value(self.status())?),
            Op::Start { name } => self.start(&name).map(|()| serde_json::Value::Null),
            Op::Stop { name } => self.stop(&name).await.map(|()| serde_json::Value::Null),
            Op::Kill { name } => {
                self.running_field(&name, |r| r.kill.notify_one())?;
                Ok(serde_json::Value::Null)
            }
            Op::Command { name, line } => self
                .command(&name, &line)
                .await
                .map(|()| serde_json::Value::Null),
            Op::Console { name, since } => {
                let chunk = self.running_field(&name, |r| lock(&r.console).since(since))?;
                Ok(serde_json::to_value(chunk)?)
            }
            Op::Stats { name } => {
                let stats = self.running_field(&name, |r| Stats {
                    points: r.history.iter().cloned().collect(),
                    online: {
                        let mut v: Vec<String> = lock(&r.online).iter().cloned().collect();
                        v.sort();
                        v
                    },
                })?;
                Ok(serde_json::to_value(stats)?)
            }
            Op::StopUnpinned => {
                let names: Vec<String> = self.status().into_iter().map(|s| s.name).collect();
                for name in names {
                    if self.manager.load(&name).is_ok_and(|c| !c.keep_running) {
                        let _ = self.stop(&name).await;
                    }
                }
                Ok(serde_json::Value::Null)
            }
        }
    }

    /// Campiona CPU, RAM, giocatori e latenza di ogni server acceso.
    /// Avvia l'agent playit se un server acceso ha il tunnel (e playit.gg è
    /// collegato), lo ferma quando nessuno ne ha più bisogno.
    fn sync_agent(&self) {
        let running: Vec<String> = lock(&self.servers)
            .iter()
            .filter(|(_, r)| !r.exited.load(Ordering::Relaxed))
            .map(|(n, _)| n.clone())
            .collect();
        // Si rilegge la configurazione: il tunnel si può attivare o togliere
        // anche a server acceso.
        let needed = running.iter().any(|n| {
            self.manager
                .load(n)
                .is_ok_and(|c| c.playit_tunnel.is_some())
        });
        let mut agent = lock(&self.agent);
        let alive = agent
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)));
        if needed && !alive {
            match crate::playit::spawn_agent(&self.paths) {
                Ok(child) => *agent = Some(child),
                Err(e) => tracing::warn!(error = %e, "playit agent not started"),
            }
        } else if !needed && let Some(mut child) = agent.take() {
            let _ = child.start_kill();
        }
    }

    async fn sample(&self, system: &mut System) {
        self.sync_agent();
        let targets: Vec<(String, u32, u16)> = lock(&self.servers)
            .iter()
            .filter(|(_, r)| !r.exited.load(Ordering::Relaxed))
            .map(|(n, r)| (n.clone(), r.pid, r.port))
            .collect();
        if targets.is_empty() {
            return;
        }
        let pids: Vec<Pid> = targets
            .iter()
            .map(|(_, pid, _)| Pid::from_u32(*pid))
            .collect();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let cpus = std::thread::available_parallelism().map_or(1, |n| n.get()) as f32;
        for (name, pid, port) in targets {
            let (cpu, mem_mb) = system.process(Pid::from_u32(pid)).map_or((0.0, 0), |p| {
                (p.cpu_usage() / cpus, (p.memory() / (1024 * 1024)) as u32)
            });
            let started = Instant::now();
            let ping = tokio::time::timeout(
                Duration::from_millis(1500),
                hyper_instances::servers::ping(&format!("127.0.0.1:{port}")),
            )
            .await;
            let (players, latency_ms) = match ping {
                Ok(Ok(status)) => (
                    Some(status.online),
                    Some(started.elapsed().as_millis().min(60_000) as u32),
                ),
                _ => (None, None),
            };
            let mut servers = lock(&self.servers);
            if let Some(r) = servers.get_mut(&name) {
                let players = players.unwrap_or_else(|| lock(&r.online).len() as u32);
                r.history.push_back(StatsPoint {
                    t: hyper_core::unix_now(),
                    cpu: (cpu * 10.0).round() / 10.0,
                    mem_mb,
                    players,
                    latency_ms,
                });
                if r.history.len() > HISTORY {
                    r.history.pop_front();
                }
            }
        }
    }
}

/// Confronto a tempo costante (il token non trapela dai tempi di risposta).
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

async fn serve(host: Arc<Host>, stream: TcpStream) {
    let (read, mut write) = stream.into_split();
    let mut line = String::new();
    let response = match BufReader::new(read.take(MAX_LINE))
        .read_line(&mut line)
        .await
    {
        Ok(_) => match serde_json::from_str::<Request>(&line) {
            Ok(req) if same_token(&req.token, &host.token) => match host.handle(req.op).await {
                Ok(data) => Response {
                    ok: true,
                    error: None,
                    data,
                },
                Err(e) => Response {
                    ok: false,
                    error: Some(e.to_string()),
                    data: serde_json::Value::Null,
                },
            },
            Ok(_) => Response {
                ok: false,
                error: Some("unauthorized".into()),
                data: serde_json::Value::Null,
            },
            Err(_) => Response {
                ok: false,
                error: Some("bad request".into()),
                data: serde_json::Value::Null,
            },
        },
        Err(_) => return,
    };
    if let Ok(mut text) = serde_json::to_string(&response) {
        text.push('\n');
        let _ = write.write_all(text.as_bytes()).await;
    }
}

/// Esegue l'host fino a quando non resta senza server per un minuto. Con
/// `autostart` avvia i server con "Avvia con il computer". Se un altro
/// host risponde già, esce subito.
pub async fn run(paths: Paths, autostart: bool) -> Result<()> {
    let client = crate::client::HostClient::new(&paths);
    if client.is_running().await {
        return Ok(());
    }
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let info = HostInfo {
        port,
        token: token.clone(),
        pid: std::process::id(),
    };
    fs::write_atomic(&paths.server_host_file(), &serde_json::to_vec(&info)?)?;
    tracing::info!(port, "server host listening");

    let host = Arc::new(Host {
        manager: ServerManager::new(paths.clone()),
        paths: paths.clone(),
        token,
        servers: Mutex::new(HashMap::new()),
        last_activity: Mutex::new(Instant::now()),
        agent: Mutex::new(None),
    });
    if autostart {
        for config in host
            .manager
            .list()
            .into_iter()
            .filter(|c| c.autostart && c.keep_running)
        {
            if let Err(e) = host.start(&config.name) {
                tracing::warn!(server = config.name, error = %e, "autostart failed");
            }
        }
    }
    {
        let host = host.clone();
        tokio::spawn(async move {
            let mut system = System::new();
            loop {
                tokio::time::sleep(SAMPLE_EVERY).await;
                host.sample(&mut system).await;
            }
        });
    }
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((stream, addr)) = accepted && addr.ip().is_loopback() {
                    // Una richiesta in arrivo rimanda la chiusura per
                    // inattività: l'host non esce mentre la sta servendo.
                    host.touch();
                    tokio::spawn(serve(host.clone(), stream));
                }
            }
            () = tokio::time::sleep(Duration::from_secs(5)) => {}
        }
        if host.active() > 0 {
            host.touch();
        } else if lock(&host.last_activity).elapsed() > IDLE_EXIT {
            break;
        }
    }
    // Il file punta a questo host: lo si toglie solo se è ancora suo.
    let path = paths.server_host_file();
    if std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice::<HostInfo>(&b).ok())
        .is_some_and(|i| i.pid == info.pid)
    {
        let _ = std::fs::remove_file(path);
    }
    tracing::info!("server host idle, exiting");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_keeps_the_last_lines_with_sequence_numbers() {
        let mut console = Console::default();
        for i in 0..(CONSOLE_LINES + 5) {
            console.push(i.to_string());
        }
        let all = console.since(0);
        assert_eq!(all.lines.len(), CONSOLE_LINES);
        assert_eq!(all.lines[0], "5");
        assert_eq!(all.next, (CONSOLE_LINES + 5) as u64);
        let tail = console.since(all.next - 2);
        assert_eq!(
            tail.lines,
            [
                (CONSOLE_LINES + 3).to_string(),
                (CONSOLE_LINES + 4).to_string()
            ]
        );
        assert!(console.since(all.next).lines.is_empty());
    }

    #[test]
    fn token_comparison() {
        assert!(same_token("abc", "abc"));
        assert!(!same_token("abc", "abd"));
        assert!(!same_token("abc", "abcd"));
    }
}
