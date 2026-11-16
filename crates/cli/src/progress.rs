//! Riga di avanzamento unica per i download concorrenti delle varie fasi.

use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hyper_installer::{Progress, Stage};

const LABELS: [&str; 3] = ["librerie", "asset", "java"];

/// Posizione della fase in `LABELS`; un nuovo `Stage` rompe la build qui.
fn index(stage: Stage) -> usize {
    match stage {
        Stage::Libraries => 0,
        Stage::Assets => 1,
        Stage::Java => 2,
    }
}

#[derive(Default)]
pub struct ProgressView {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    stages: [Option<Progress>; LABELS.len()],
    last_draw: Option<Instant>,
}

impl ProgressView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&self, stage: Stage, progress: Progress) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        state.stages[index(stage)] = Some(progress);
        let complete = progress.done == progress.total;
        let due = state
            .last_draw
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(100));
        if complete || due {
            state.last_draw = Some(Instant::now());
            draw(&state.stages);
        }
    }

    pub fn finish(&self) {
        if let Ok(state) = self.state.lock()
            && state.last_draw.is_some()
        {
            draw(&state.stages);
            eprintln!();
        }
    }
}

fn draw(stages: &[Option<Progress>; LABELS.len()]) {
    let mut bytes = 0;
    let parts: Vec<String> = LABELS
        .iter()
        .zip(stages)
        .filter_map(|(label, p)| {
            let p = p.as_ref()?;
            bytes += p.downloaded_bytes;
            Some(format!("{label} {}/{}", p.done, p.total))
        })
        .collect();
    let mb = bytes as f64 / (1024.0 * 1024.0);
    let mut err = std::io::stderr().lock();
    let _ = write!(err, "\r{}  |  {mb:.1} MB scaricati   ", parts.join("  |  "));
    let _ = err.flush();
}
