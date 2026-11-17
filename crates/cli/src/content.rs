//! Comandi `hyper content …` (alias `hyper mod …`) e `hyper pack …`: catalogo
//! Modrinth da riga di comando. La logica sta in `hyper-content`.

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use hyper_content::library::{installed, update};
use hyper_content::{ContentKind, Modrinth, SearchQuery, install, mrpack};
use hyper_installer::Progress;
use hyper_instances::ContentDir;

use crate::App;

#[derive(Subcommand)]
pub enum ContentCommand {
    /// Cerca su Modrinth.
    Search {
        text: String,
        /// mod, resourcepack, shader, datapack, modpack.
        #[arg(long, default_value = "mod")]
        kind: ContentKind,
        /// Mostra solo ciò che è compatibile con questa istanza.
        #[arg(long)]
        instance: Option<String>,
    },
    /// Installa un progetto (id o slug) e le sue dipendenze in un'istanza.
    Add {
        instance: String,
        project: String,
        #[arg(long, default_value = "mod")]
        kind: ContentKind,
        /// Mondo in cui installare un data pack.
        #[arg(long)]
        world: Option<String>,
    },
    /// Elenca i contenuti installati e gli aggiornamenti disponibili.
    List {
        instance: String,
        #[arg(long, default_value = "mod")]
        kind: ContentKind,
    },
    /// Aggiorna tutti i contenuti di quel tipo.
    Update {
        instance: String,
        #[arg(long, default_value = "mod")]
        kind: ContentKind,
    },
}

#[derive(Subcommand)]
pub enum PackCommand {
    /// Crea un'istanza da un file `.mrpack`.
    Import { file: PathBuf },
    /// Crea un'istanza dall'ultima versione di un modpack Modrinth.
    Install { project: String },
    /// Esporta un'istanza in un file `.mrpack`.
    Export { instance: String, file: PathBuf },
}

fn report(p: Progress) {
    eprint!("\r  file {}/{}   ", p.done, p.total);
    let _ = std::io::stderr().flush();
}

fn dir_of(kind: ContentKind) -> Result<ContentDir> {
    kind.content_dir()
        .ok_or_else(|| anyhow::anyhow!("per i {kind:?} l'elenco non è disponibile"))
}

pub async fn content(app: &App, cmd: ContentCommand) -> Result<()> {
    let api = Modrinth::new()?;
    match cmd {
        ContentCommand::Search {
            text,
            kind,
            instance,
        } => {
            let mut query = SearchQuery::new(text, kind);
            if let Some(name) = instance {
                query.game_version = Some(app.instances().load(&name)?.config.version);
            }
            let results = api.search(&query).await?;
            println!("{} risultati", results.total_hits);
            for hit in results.hits {
                println!(
                    "{:<28} {:>12} download  {}",
                    hit.slug,
                    hit.downloads,
                    hit.description.chars().take(70).collect::<String>()
                );
            }
        }
        ContentCommand::Add {
            instance,
            project,
            kind,
            world,
        } => {
            let instance = app.instances().load(&instance)?;
            let plan = install::plan(&api, &instance, &project, kind, world.as_deref()).await?;
            for f in &plan.files {
                println!("  {} ({})", f.file.filename, f.version.version_number);
            }
            let installer = app.installer()?;
            plan.execute(&instance, installer.downloader(), Some(&report))
                .await?;
            eprintln!();
            println!(
                "Installati {} file ({:.1} MB) in {:?}.",
                plan.files.len(),
                plan.total_bytes() as f64 / 1e6,
                instance.config.name
            );
        }
        ContentCommand::List { instance, kind } => {
            let instance = app.instances().load(&instance)?;
            for item in installed(&api, &instance, dir_of(kind)?, true).await? {
                let title = item
                    .project
                    .as_ref()
                    .map_or("(sconosciuto su Modrinth)", |p| p.title.as_str());
                let update = item
                    .update
                    .as_ref()
                    .map(|v| format!("  → {}", v.version_number))
                    .unwrap_or_default();
                let off = if item.file.enabled {
                    ""
                } else {
                    " [disattivata]"
                };
                println!("{:<40} {title}{off}{update}", item.file.file);
            }
        }
        ContentCommand::Update { instance, kind } => {
            let instance = app.instances().load(&instance)?;
            let installer = app.installer()?;
            let n = update(
                &api,
                installer.downloader(),
                &instance,
                dir_of(kind)?,
                &[],
                Some(&report),
            )
            .await?;
            eprintln!();
            println!("Aggiornati {n} file.");
        }
    }
    Ok(())
}

pub async fn pack(app: &App, cmd: PackCommand) -> Result<()> {
    let installer = app.installer()?;
    let downloader = installer.downloader();
    let manager = app.instances();
    match cmd {
        PackCommand::Import { file } => {
            let instance = mrpack::import(&manager, downloader, &file, Some(&report)).await?;
            eprintln!();
            println!("Istanza {:?} creata.", instance.config.name);
        }
        PackCommand::Install { project } => {
            let api = Modrinth::new()?;
            let instance = mrpack::install_from_modrinth(
                &api,
                &manager,
                downloader,
                &app.paths.cache_dir(),
                &project,
                Some(&report),
            )
            .await?;
            eprintln!();
            println!("Istanza {:?} creata.", instance.config.name);
        }
        PackCommand::Export { instance, file } => {
            let api = Modrinth::new()?;
            let instance = manager.load(&instance)?;
            let summary = mrpack::export(&api, &instance, &file).await?;
            println!(
                "Esportato in {}: {} file da Modrinth, {} inclusi nel pack.",
                file.display(),
                summary.linked,
                summary.bundled
            );
        }
    }
    Ok(())
}
