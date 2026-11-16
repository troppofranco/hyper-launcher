//! `hyper`: CLI headless per provare il core di Hyper Launcher. Contiene solo
//! parsing degli argomenti e presentazione: la logica di avvio sta in
//! `hyper-instances`, condivisa con la futura UI.

mod progress;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use hyper_auth::{Account, AccountStore, AuthError, KeyringStore, MsaClient};
use hyper_core::{Config, Paths};
use hyper_installer::{Installer, VersionKind};
use hyper_instances::InstanceManager;

use crate::progress::ProgressView;

#[derive(Parser)]
#[command(name = "hyper", version, about = "Hyper Launcher — CLI headless")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Elenca le versioni di Minecraft disponibili.
    Versions {
        /// Includi gli snapshot.
        #[arg(long)]
        snapshots: bool,
        /// Includi anche beta e alpha storiche.
        #[arg(long)]
        all: bool,
        /// Numero massimo di righe (0 = tutte).
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Gestione account.
    #[command(subcommand)]
    Account(AccountCommand),
    /// Gestione istanze.
    #[command(subcommand)]
    Instance(InstanceCommand),
    /// Scarica e verifica tutto il necessario per un'istanza.
    Install {
        instance: String,
        /// Java da usare invece del runtime Mojang.
        #[arg(long)]
        java: Option<PathBuf>,
    },
    /// Installa (se serve) e avvia un'istanza.
    Launch {
        instance: String,
        /// Account da usare (default: quello predefinito).
        #[arg(long)]
        account: Option<String>,
        /// Java da usare invece del runtime Mojang.
        #[arg(long)]
        java: Option<PathBuf>,
        /// Stampa il comando senza avviare il gioco.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum AccountCommand {
    /// Aggiunge un account offline.
    AddOffline { name: String },
    /// Aggiunge un account Microsoft (login nel browser con un codice).
    AddMicrosoft,
    /// Rimuove un account e cancella i suoi token dal keyring.
    Remove { name: String },
    /// Imposta l'account predefinito.
    Default { name: String },
    /// Elenca gli account.
    List,
}

#[derive(Subcommand)]
enum InstanceCommand {
    /// Crea un'istanza isolata.
    Create {
        name: String,
        /// Id versione, `latest` o `latest-snapshot`.
        #[arg(long, default_value = "latest")]
        version: String,
    },
    /// Elenca le istanze.
    List,
}

struct App {
    paths: Paths,
    config: Config,
}

impl App {
    fn installer(&self) -> Result<Installer> {
        Ok(Installer::new(
            self.paths.clone(),
            self.config.download_concurrency,
        )?)
    }

    fn instances(&self) -> InstanceManager {
        InstanceManager::new(self.paths.clone())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let paths = Paths::from_env()?;
    let config = Config::load(&paths)?;
    let app = App { paths, config };

    match cli.command {
        Command::Versions {
            snapshots,
            all,
            limit,
        } => versions(&app, snapshots, all, limit).await,
        Command::Account(cmd) => account(&app, cmd).await,
        Command::Instance(cmd) => instance(&app, cmd).await,
        Command::Install { instance, java } => install(&app, &instance, java).await,
        Command::Launch {
            instance,
            account,
            java,
            dry_run,
        } => launch(&app, &instance, account.as_deref(), java, dry_run).await,
    }
}

async fn versions(app: &App, snapshots: bool, all: bool, limit: usize) -> Result<()> {
    let manifest = app.installer()?.fetch_manifest().await?;
    println!(
        "Ultima release: {}   Ultimo snapshot: {}\n",
        manifest.latest.release, manifest.latest.snapshot
    );
    let shown = manifest.versions.iter().filter(|v| match v.kind {
        VersionKind::Release => true,
        VersionKind::Snapshot => snapshots || all,
        _ => all,
    });
    let limit = if limit == 0 { usize::MAX } else { limit };
    for v in shown.take(limit) {
        let date = v.release_time.get(..10).unwrap_or(&v.release_time);
        println!("{:<24} {:<10} {date}", v.id, v.kind.as_str());
    }
    Ok(())
}

async fn account(app: &App, cmd: AccountCommand) -> Result<()> {
    let path = app.paths.accounts_file();
    let mut store = AccountStore::load(&path)?;
    match cmd {
        AccountCommand::AddOffline { name } => {
            let account = Account::offline(&name)?;
            println!("Account offline {} ({})", account.name(), account.uuid());
            store.add_and_save(&path, account, &KeyringStore::default())?;
        }
        AccountCommand::AddMicrosoft => {
            let msa = MsaClient::from_config(&app.config)?.ok_or(AuthError::MissingClientId)?;
            let secrets = KeyringStore::default();
            let code = msa.start_device_login().await?;
            println!(
                "Apri {} e inserisci il codice {}\n(valido {} minuti, in attesa della conferma…)",
                code.verification_uri,
                code.user_code,
                code.expires_in / 60
            );
            let account = hyper_auth::login_microsoft(&msa, &secrets, &code).await?;
            let (name, uuid) = (account.name().to_owned(), account.uuid());
            store.add_and_save(&path, account, &secrets)?;
            println!(
                "Account Microsoft {name} ({uuid}) aggiunto. Token salvati nel keyring di sistema."
            );
        }
        AccountCommand::Remove { name } => {
            let account = store.remove_and_save(&path, &name, &KeyringStore::default())?;
            println!("Account {} rimosso.", account.name());
        }
        AccountCommand::Default { name } => {
            println!("Account predefinito: {}", store.set_default(&name)?.name());
            store.save(&path)?;
        }
        AccountCommand::List => {
            if store.accounts().is_empty() {
                println!("Nessun account. Aggiungine uno con `hyper account add-offline <nome>`.");
            }
            for a in store.accounts() {
                let marker = if store.is_default(a) { "*" } else { " " };
                println!("{marker} {:<16} {:<8} {}", a.name(), a.kind(), a.uuid());
            }
        }
    }
    Ok(())
}

async fn instance(app: &App, cmd: InstanceCommand) -> Result<()> {
    match cmd {
        InstanceCommand::Create { name, version } => {
            let manifest = app.installer()?.fetch_manifest().await?;
            let entry = manifest
                .resolve(&version)
                .with_context(|| format!("versione {version:?} non trovata"))?;
            let instance = app.instances().create(&name, &entry.id)?;
            println!(
                "Istanza {:?} creata (Minecraft {}) in {}",
                instance.config.name,
                instance.config.version,
                instance.dir.display()
            );
        }
        InstanceCommand::List => {
            let instances = app.instances().list()?;
            if instances.is_empty() {
                println!(
                    "Nessuna istanza. Creane una con `hyper instance create <nome> --version latest`."
                );
            }
            for i in instances {
                println!("{:<24} {}", i.config.name, i.config.version);
            }
        }
    }
    Ok(())
}

async fn install(app: &App, name: &str, java: Option<PathBuf>) -> Result<()> {
    let instance = app.instances().load(name)?;
    let view = ProgressView::new();
    let report = |stage, progress| view.update(stage, progress);
    let (installed, java) = hyper_instances::install(
        &app.installer()?,
        &instance,
        &app.config,
        java,
        Some(&report),
    )
    .await?;
    view.finish();
    println!(
        "{} pronta: {} file nel classpath, Java {}",
        installed.version.id,
        installed.classpath.len(),
        java.display()
    );
    Ok(())
}

async fn launch(
    app: &App,
    name: &str,
    account: Option<&str>,
    java: Option<PathBuf>,
    dry_run: bool,
) -> Result<()> {
    let accounts = AccountStore::load(&app.paths.accounts_file())?;
    let account = accounts.select(account).context(
        "serve un account: `hyper account add-offline <nome>` o `hyper account add-microsoft`",
    )?;
    let instance = app.instances().load(name)?;
    let msa = MsaClient::from_config(&app.config)?;
    let session = match hyper_auth::session(account, &KeyringStore::default(), msa.as_ref()).await {
        Err(e) if e.requires_login() => {
            bail!("{e}\nRifai il login con `hyper account add-microsoft`.")
        }
        other => other?,
    };

    let view = ProgressView::new();
    let report = |stage, progress| view.update(stage, progress);
    let prepared = hyper_instances::prepare_launch(
        &app.installer()?,
        &instance,
        &session,
        &app.config,
        java,
        Some(&report),
    )
    .await?;
    view.finish();

    if dry_run {
        let command = &prepared.command;
        println!("cd {}", command.cwd.display());
        println!("{}", command.program.display());
        for arg in command.redacted_args() {
            println!("  {arg}");
        }
        return Ok(());
    }

    println!(
        "Avvio Minecraft {} come {} ({})…",
        prepared.installed.version.id,
        session.name,
        account.kind()
    );
    let game = prepared.spawn()?;
    let status =
        tokio::task::spawn_blocking(move || game.wait(|output| println!("{output}"))).await??;
    if status.success() {
        println!("Minecraft terminato correttamente.");
        Ok(())
    } else {
        bail!("Minecraft è terminato con {status}")
    }
}
