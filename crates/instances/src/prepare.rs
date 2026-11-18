//! Pipeline completa di avvio, con l'ordine dei passi garantito dai tipi:
//! lock dell'istanza → installazione → scelta di Java → native → comando →
//! processo. Il lock resta vivo finché il gioco non termina.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ExitStatus};

use hyper_auth::Session;
use hyper_core::{Config, SyncConfig};
use hyper_installer::{InstalledVersion, Installer, InstallerError, StageProgressFn};

use crate::console::{GameOutput, Log4jParser};
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
    let installed = match &instance.config.loader {
        Some(loader) => {
            installer
                .install_with_loader(entry, loader, progress)
                .await?
        }
        None => installer.install_version(entry, progress).await?,
    };
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
    instance: Instance,
    lock: File,
    sync: Option<SettingsSync>,
}

/// Dove e cosa sincronizzare a fine partita.
#[derive(Debug, Clone)]
struct SettingsSync {
    shared: PathBuf,
    config: SyncConfig,
    /// `options.txt` dell'istanza all'avvio: a fine partita si condivide
    /// solo ciò che è cambiato rispetto a questo.
    baseline: Option<hyper_sync::OptionsFile>,
}

/// Gioco in esecuzione; il lock dell'istanza vive quanto questo valore.
#[derive(Debug)]
pub struct RunningGame {
    pub child: Child,
    program: PathBuf,
    instance: Instance,
    started: u64,
    sync: Option<SettingsSync>,
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
    // Impostazioni condivise (tasti, video…) copiate nell'istanza; un errore
    // qui non deve impedire di giocare.
    let enabled = config.sync.enabled
        && instance.config.sync_settings
        && hyper_sync::supports_game(&instance.config.version);
    let sync = enabled.then(|| {
        let shared = installer.paths().shared_options_file();
        let baseline = match hyper_sync::pull(&shared, &instance.options_file(), &config.sync) {
            Ok(baseline) => baseline,
            Err(e) => {
                tracing::warn!(error = %e, "cannot apply shared settings");
                None
            }
        };
        SettingsSync {
            shared,
            config: config.sync,
            baseline,
        }
    });
    tracing::debug!(natives, "native libraries extracted");
    // Skin per gli account offline (con Microsoft vale quella del profilo).
    let skin = session
        .access_token
        .is_none()
        .then(|| {
            let library = crate::skins::SkinLibrary::new(installer.paths());
            let skin = library.selected(&session.uuid.to_string())?;
            library.png(&skin.id).ok()
        })
        .flatten();
    if let Err(e) = crate::skins::apply_to_instance(instance, skin.as_deref()) {
        tracing::warn!(error = %e, "cannot apply the skin");
    }

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
        instance: instance.clone(),
        lock,
        sync,
    })
}

impl PreparedLaunch {
    /// Avvia il gioco trasferendo il lock. Lo stdout è catturato e va letto
    /// con [`RunningGame::wait`]; lo stderr resta quello del launcher.
    pub fn spawn(self) -> Result<RunningGame> {
        let child = self.command.spawn()?;
        Ok(RunningGame {
            child,
            program: self.command.program,
            instance: self.instance,
            started: hyper_core::unix_now(),
            sync: self.sync,
            _lock: self.lock,
        })
    }
}

impl RunningGame {
    /// Legge l'output del gioco fino alla sua uscita, passando ogni riga
    /// (eventi log4j già decodificati) a `on_output`; poi registra il tempo
    /// di gioco nell'istanza e rilascia il lock.
    /// Bloccante: in un contesto async va eseguita con `spawn_blocking`.
    pub fn wait(mut self, mut on_output: impl FnMut(GameOutput)) -> Result<ExitStatus> {
        let process_error = |source| InstanceError::Process {
            program: self.program.clone(),
            source,
        };
        if let Some(stdout) = self.child.stdout.take() {
            let mut parser = Log4jParser::new();
            for line in BufReader::new(stdout).lines() {
                // Righe non UTF-8 o pipe chiusa: si smette di leggere, ma si
                // attende comunque l'uscita del processo.
                let Ok(line) = line else { break };
                if let Some(output) = parser.push_line(&line) {
                    on_output(output);
                }
            }
            if let Some(output) = parser.finish() {
                on_output(output);
            }
        }
        let status = self.child.wait().map_err(process_error)?;
        // Ricarica l'istanza dal disco: le impostazioni potrebbero essere
        // cambiate mentre si giocava.
        let mut instance = Instance::reload(&self.instance).unwrap_or(self.instance);
        if let Err(e) = instance.record_session(self.started, hyper_core::unix_now()) {
            tracing::warn!(error = %e, "cannot record playtime");
        }
        if let Some(sync) = &self.sync
            && let Err(e) = hyper_sync::push(
                &instance.options_file(),
                &sync.shared,
                &sync.config,
                sync.baseline.as_ref(),
            )
        {
            tracing::warn!(error = %e, "cannot save shared settings");
        }
        Ok(status)
    }
}
