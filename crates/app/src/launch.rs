//! Avvio del gioco dall'interfaccia: installa, prepara e avvia in
//! background, comunicando alla UI avanzamento, righe di console e stato.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use hyper_auth::{AccountStore, KeyringStore, MsaClient};
use hyper_installer::{Progress, Stage};
use hyper_instances::console::GameOutput;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::restore_window;
use crate::state::{AppState, CmdResult};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent<'a> {
    instance: &'a str,
    stage: &'static str,
    done: usize,
    total: usize,
    downloaded_bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEvent<'a> {
    instance: &'a str,
    line: String,
}

/// `installing` → `running` → `stopped` (con `error` o `exitCode`).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateEvent<'a> {
    instance: &'a str,
    state: &'static str,
    error: Option<String>,
    exit_code: Option<i32>,
    /// Analisi del crash, se il gioco si è chiuso con un errore.
    crash: Option<hyper_instances::crash::CrashInfo>,
}

fn emit_state(app: &AppHandle, instance: &str, state: &'static str) {
    let _ = app.emit(
        "instance-state",
        StateEvent {
            instance,
            state,
            error: None,
            exit_code: None,
            crash: None,
        },
    );
}

/// Avvia l'istanza in background e ritorna subito; l'esito arriva con gli
/// eventi `instance-state`.
/// Con `join` ("host:porta") il gioco entra subito in quel server.
#[tauri::command]
pub fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    join: Option<String>,
) -> CmdResult<()> {
    if !state.try_mark_busy(&name) {
        return Err("L'istanza è già in avvio o in gioco.".into());
    }
    emit_state(&app, &name, "installing");
    tauri::async_runtime::spawn(async move {
        let result = run(&app, &name, join.as_deref()).await;
        let (error, exit_code, crash) = match result {
            Ok((code, crash)) => (None, code, crash),
            Err(e) => (Some(format!("{e:#}")), None, None),
        };
        app.state::<AppState>().clear_busy(&name);
        let _ = app.emit(
            "instance-state",
            StateEvent {
                instance: &name,
                state: "stopped",
                error,
                exit_code,
                crash,
            },
        );
        restore_window(&app);
    });
    Ok(())
}

/// Codice di uscita del gioco e, se si è chiuso con un errore, l'analisi
/// del crash.
async fn run(
    app: &AppHandle,
    name: &str,
    join: Option<&str>,
) -> anyhow::Result<(Option<i32>, Option<hyper_instances::crash::CrashInfo>)> {
    let state = app.state::<AppState>();
    let config = state.config();

    // Bedrock: niente account del launcher né Java, il gioco è quello del
    // Microsoft Store.
    if state.instances().load(name)?.is_bedrock() {
        crate::bedrock::run(app, name, || {
            emit_state(app, name, "running");
            if config.minimize_on_launch
                && let Some(window) = app.get_webview_window("main")
            {
                let _ = window.minimize();
            }
        })
        .await?;
        return Ok((None, None));
    }

    let accounts = AccountStore::load(&state.paths.accounts_file())?;
    let account = accounts
        .select(None)
        .map_err(|_| anyhow::anyhow!("Aggiungi un account prima di giocare."))?;
    let msa = MsaClient::from_config(&config)?;
    let session = hyper_auth::session(account, &KeyringStore::default(), msa.as_ref()).await?;
    let instance = state.instances().load(name)?;
    let installer = state.installer();

    // Un evento per file sarebbero migliaia al secondo: la UI ne riceve al
    // massimo uno ogni 100 ms, più quello finale di ogni fase.
    let last_emit = Mutex::new(None::<Instant>);
    let report = |stage: Stage, p: Progress| {
        {
            let mut last = last_emit.lock().unwrap_or_else(PoisonError::into_inner);
            let due = last.is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
            if !due && p.done < p.total {
                return;
            }
            *last = Some(Instant::now());
        }
        let stage = stage.as_str();
        let _ = app.emit(
            "launch-progress",
            ProgressEvent {
                instance: name,
                stage,
                done: p.done,
                total: p.total,
                downloaded_bytes: p.downloaded_bytes,
            },
        );
    };
    let mut prepared = hyper_instances::prepare_launch(
        &installer,
        &instance,
        &session,
        &config,
        None,
        Some(&report),
    )
    .await?;

    prepared.command.hide_console = !config.show_console;
    if let Some((host, port)) = join.and_then(|j| j.rsplit_once(':')) {
        // La schermata di benvenuto del primo avvio bloccherebbe l'ingresso.
        if let Err(e) = instance.skip_onboarding() {
            tracing::warn!(error = %e, "cannot skip the onboarding screen");
        }
        let port: u16 = port
            .parse()
            .map_err(|_| anyhow::anyhow!("Porta non valida: {port}"))?;
        prepared.command.args.extend(hyper_instances::join_args(
            &instance.config.version,
            host,
            port,
        ));
    }
    let started = hyper_core::unix_now();
    let game = prepared.spawn()?;
    emit_state(app, name, "running");
    if config.minimize_on_launch
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.minimize();
    }

    let (log_app, log_name) = (app.clone(), name.to_owned());
    let status = tokio::task::spawn_blocking(move || {
        game.wait(|output: GameOutput| {
            let _ = log_app.emit(
                "game-log",
                LogEvent {
                    instance: &log_name,
                    line: output.to_string(),
                },
            );
        })
    })
    .await??;
    let code = status.code();
    // Chiusura con errore: si cerca la causa nel crash report o nel log.
    let crash = (!status.success())
        .then(|| hyper_instances::crash::analyze_game(&instance.game_dir(), started));
    Ok((code, crash))
}
