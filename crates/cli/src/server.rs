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
        /// vanilla o paper.
        #[arg(long, default_value = "paper")]
        kind: String,
        #[arg(long)]
        version: String,
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
                other => bail!("tipo di server sconosciuto: {other} (vanilla o paper)"),
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
            };
            let config = install::create(&installer, &manager, &request, Some(&report)).await?;
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
    }
    Ok(())
}
