//! Pipeline completa di avvio, con l'ordine dei passi garantito dai tipi:
//! lock dell'istanza → installazione → scelta di Java → native → comando →
//! processo. Il lock resta vivo finché il gioco non termina.

use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, ExitStatus};

use hyper_auth::Session;
use hyper_core::Config;
use hyper_installer::{InstalledVersion, Installer, InstallerError, StageProgressFn};

use crate::launch::{LaunchCommand, LaunchOptions, build_command, prepare_natives};
use crate::{Instance, InstanceError, Result};

/// Scelta di Java, in ordine di precedenza: `override` (es. `--java`),
/// `java_path` dell'istanza, `java_path` globale. `None` = usare il runtime
/// Mojang. Un nome senza cartella (es. `java`) è accettato e cercato nel PATH.
pub fn resolve_java(
    java_override: Option<PathBuf>,
    instance: &Instance,
    config: &Config,
) -> Result<Option<PathBuf>> {
    let Some(java) = java_override
        .or_else(|| instance.config.java_path.clone())
        .or_else(|| config.java_path.clone())
    else {
        return Ok(None);
    };
    let bare_command = java.parent().is_none_or(|p| p.as_os_str().is_empty());
    if bare_command || java.is_file() {
        Ok(Some(java))
    } else {
        Err(InstanceError::JavaNotFound(java))
    }
}

/// Garantisce versione e Java dell'istanza; restituisce la versione
/// installata e l'eseguibile Java da usare.
pub async fn install(
    installer: &Installer,
    instance: &Instance,
    config: &Config,
    java_override: Option<PathBuf>,
    progress: Option<StageProgressFn<'_>>,
) -> Result<(InstalledVersion, PathBuf)> {
    let custom_java = resolve_java(java_override, instance, config)?;
    let manifest = installer.fetch_manifest().await?;
    let entry = manifest
        .find(&instance.config.version)
        .ok_or_else(|| InstallerError::UnknownVersion(instance.config.version.clone()))?;
    let installed = installer.install_version(entry, progress).await?;
    let java = match custom_java {
        Some(java) => java,
        None => {
            installer
                .install_java(installed.java_component(), progress)
                .await?
        }
    };
    Ok((installed, java))
}

/// Avvio pronto: comando costruito e istanza bloccata.
#[derive(Debug)]
pub struct PreparedLaunch {
    pub command: LaunchCommand,
    pub installed: InstalledVersion,
    lock: File,
}

/// Gioco in esecuzione; il lock dell'istanza vive quanto questo valore.
#[derive(Debug)]
pub struct RunningGame {
    pub child: Child,
    program: PathBuf,
    _lock: File,
}

pub async fn prepare_launch(
    installer: &Installer,
    instance: &Instance,
    session: &Session,
    config: &Config,
    java_override: Option<PathBuf>,
    progress: Option<StageProgressFn<'_>>,
) -> Result<PreparedLaunch> {
    let memory_mb = instance.config.effective_memory_mb(config)?;
    // Prima di tutto: un'istanza già in uso non deve rifare il lavoro di
    // installazione né toccare le native della sessione in corso.
    let lock = instance.try_lock()?;
    let (installed, java) = install(installer, instance, config, java_override, progress).await?;
    let natives = prepare_natives(instance, &installed)?;
    tracing::debug!(natives, "native libraries extracted");

    let command = build_command(
        instance,
        &installed,
        LaunchOptions {
            java: &java,
            memory_mb,
            session,
            paths: installer.paths(),
        },
    );
    Ok(PreparedLaunch {
        command,
        installed,
        lock,
    })
}

impl PreparedLaunch {
    /// Avvia il gioco (stdout/stderr ereditati) trasferendo il lock.
    pub fn spawn(self) -> Result<RunningGame> {
        let child = self.command.spawn()?;
        Ok(RunningGame {
            child,
            program: self.command.program,
            _lock: self.lock,
        })
    }
}

impl RunningGame {
    /// Attende la fine del gioco (bloccante) e rilascia il lock.
    pub fn wait(mut self) -> Result<ExitStatus> {
        self.child.wait().map_err(|source| InstanceError::Process {
            program: self.program,
            source,
        })
    }
}
