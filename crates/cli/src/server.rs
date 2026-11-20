//! Comandi `hyper server …`: server Minecraft ospitati (la logica sta in
//! `hyper-server`; i server accesi li possiede l'host in background).

use std::io::Write;

use anyhow::{Result, bail};
use clap::Subcommand;
use hyper_server::install::{self, NewServer};
use hyper_server::{HostClient, ServerKind, ServerManager};

use crate::App;

#[derive(Subcommand)]
pub enum ServerCommand {
    /// Elenca i server e quelli accesi.
    List,
    /// Crea un server (scarica il jar e Java). Serve accettare l'EULA di
    /// Minecraft: https://aka.ms/MinecraftEULA
    Create {
        name: String,
        /// vanilla, paper, fabric, neoforge o forge.
        #[arg(long, default_value = "paper")]
        kind: String,
        /// Versione di Minecraft (non serve con --modpack).
        #[arg(long, default_value = "")]
        version: String,
        /// Versione del loader (predefinita: l'ultima).
        #[arg(long)]
        loader: Option<String>,
        /// Crea il server da un modpack: progetto Modrinth o file .mrpack.
        #[arg(long)]
        modpack: Option<String>,
        #[arg(long, default_value_t = 4096)]
        memory: u32,
        #[arg(long, default_value_t = 25565)]
        port: u16,
        /// Accetto l'EULA di Minecraft.
        #[arg(long)]
        accept_eula: bool,
    },
    /// Avvia un server (in background).
    Start { name: String },
    /// Ferma un server.
    Stop { name: String },
    /// Invia un comando alla console del server.
    Cmd { name: String, line: Vec<String> },
    /// Mostra la console del server.
    Console { name: String },
    /// Esegue l'host dei server (di solito lo avvia il launcher da solo).
    Host {
        #[arg(long)]
        autostart: bool,
    },
    /// Accesso da internet con playit.gg.
    Playit {
        #[command(subcommand)]
        command: PlayitCommand,
    },
}

#[derive(Subcommand)]
pub enum PlayitCommand {
    /// Collega playit.gg: apri il link stampato e approva nel browser.
    Connect,
    /// Agent collegato e tunnel.
    Status,
    /// Crea il tunnel per un server (Minecraft Java verso la sua porta).
    Tunnel { name: String },
    /// Elimina il tunnel di un server.
    Untunnel { name: String },
    /// Scollega playit.gg da questo PC.
    Forget,
    /// Esegue l'agent in primo piano (per le prove; di solito lo avvia l'host).
    Agent,
}

async fn playit_cmd(app: &App, manager: &ServerManager, cmd: PlayitCommand) -> Result<()> {
    use hyper_server::playit;
    let client = playit::Client::new()?;
    let key = || -> Result<_> {
        playit::secret()?
            .ok_or_else(|| anyhow::anyhow!("playit.gg non collegato: hyper server playit connect"))
    };
    match cmd {
        PlayitCommand::Connect => {
            let code = playit::claim_code();
            println!(
                "Apri questo link e approva Hyper Launcher:\n  {}",
                playit::claim_url(&code)
            );
            client.claim(&code, |s| eprintln!("  stato: {s:?}")).await?;
            println!("playit.gg collegato.");
        }
        PlayitCommand::Status => {
            let data = client.rundata(&key()?).await?;
            println!("agent {}", data.agent_id);
            for t in data.tunnels {
                println!(
                    "  tunnel {} {:?} → {}{}",
                    t.id,
                    t.name,
                    t.display_address,
                    t.disabled_reason
                        .map(|r| format!(" (disattivato: {r})"))
                        .unwrap_or_default()
                );
            }
            for p in data.pending {
                println!("  in preparazione {}: {}", p.id, p.status_msg);
            }
        }
        PlayitCommand::Tunnel { name } => {
            let address = playit::enable(&app.installer()?, &app.paths, manager, &name, |s| {
                eprintln!("  {s:?}…")
            })
            .await?;
            println!("Raggiungibile da internet: {address}");
        }
        PlayitCommand::Untunnel { name } => {
            playit::disable(manager, &name).await?;
            println!("Accesso da internet disattivato.");
        }
        PlayitCommand::Agent => {
            playit::ensure_agent(&app.installer()?, &app.paths).await?;
            let mut child = playit::spawn_agent(&app.paths)?;
            println!("agent playit avviato (Ctrl+C per fermarlo)");
            child.wait().await?;
        }
        PlayitCommand::Forget => {
            playit::disconnect(manager).await?;
            println!("playit.gg scollegato da questo PC.");
        }
    }
    Ok(())
}

pub async fn server(app: &App, cmd: ServerCommand) -> Result<()> {
    let manager = ServerManager::new(app.paths.clone());
    let client = HostClient::new(&app.paths);
    match cmd {
        ServerCommand::List => {
            let running = client.status().await;
            for config in manager.list() {
                let state = running
                    .iter()
                    .find(|r| r.name == config.name)
                    .map_or("spento".to_owned(), |r| {
                        format!("acceso, {}/{} giocatori", r.players, r.max_players)
                    });
                println!(
                    "{:<24} {} {:<8} {} MB  porta {}  {state}",
                    config.name,
                    config.kind.as_str(),
                    config.version,
                    config.memory_mb,
                    config.port
                );
            }
        }
        ServerCommand::Create {
            name,
            kind,
            version,
            loader,
            modpack,
            memory,
            port,
            accept_eula,
        } => {
            if !accept_eula {
                bail!(
                    "per creare un server accetta l'EULA di Minecraft (https://aka.ms/MinecraftEULA) con --accept-eula"
                );
            }
            let kind = match kind.as_str() {
                "vanilla" => ServerKind::Vanilla,
                "paper" => ServerKind::Paper,
                "fabric" => ServerKind::Fabric,
                "neoforge" => ServerKind::NeoForge,
                "forge" => ServerKind::Forge,
                other => bail!("tipo di server sconosciuto: {other}"),
            };
            let installer = app.installer()?;
            let report = |stage: install::CreateStage, done: usize, total: usize| {
                eprint!("\r  {} {done}/{total}   ", stage.as_str());
                let _ = std::io::stderr().flush();
            };
            let request = NewServer {
                name,
                kind,
                version,
                memory_mb: memory,
                port,
                keep_running: false,
                autostart: false,
                eula: true,
                loader_version: loader,
                world: None,
            };
            let config = match modpack {
                None => install::create(&installer, &manager, &request, Some(&report)).await?,
                Some(pack) => {
                    let file = std::path::Path::new(&pack);
                    let (path, project) = if file.is_file() {
                        (file.to_owned(), None)
                    } else {
                        let api = hyper_content::Modrinth::new()?;
                        let path = hyper_content::mrpack::download_from_modrinth(
                            &api,
                            installer.downloader(),
                            &app.paths.cache_dir(),
                            &pack,
                        )
                        .await?;
                        (path, Some(pack.clone()))
                    };
                    let name = hyper_content::mrpack::read_index(&path)?.name;
                    let source = hyper_server::config::ModpackSource {
                        name,
                        project: project.clone(),
                    };
                    let result = install::create_from_modpack(
                        &installer,
                        &manager,
                        &request,
                        &path,
                        source,
                        Some(&report),
                    )
                    .await;
                    if project.is_some() {
                        let _ = std::fs::remove_file(&path);
                    }
                    result?
                }
            };
            eprintln!();
            println!(
                "Server {:?} creato in {}",
                config.name,
                manager.dir(&config.name)?.display()
            );
        }
        ServerCommand::Start { name } => {
            manager.load(&name)?;
            client
                .ensure(|| hyper_server::client::spawn_host(&["server", "host"]))
                .await?;
            client.start(&name).await?;
            println!("Server {name:?} in avvio. Console: hyper server console {name:?}");
        }
        ServerCommand::Stop { name } => {
            client.stop(&name).await?;
            println!("Arresto di {name:?} richiesto.");
        }
        ServerCommand::Cmd { name, line } => {
            client.command(&name, &line.join(" ")).await?;
        }
        ServerCommand::Console { name } => {
            for line in client.console(&name, 0).await?.lines {
                println!("{line}");
            }
        }
        ServerCommand::Host { autostart } => {
            hyper_server::host::run(app.paths.clone(), autostart).await?;
        }
        ServerCommand::Playit { command } => playit_cmd(app, &manager, command).await?,
    }
    Ok(())
}
