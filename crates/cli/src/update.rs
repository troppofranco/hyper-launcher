//! Aggiornamento automatico della CLI installata (la logica sta in
//! `hyper_installer::self_update`, condivisa con l'app grafica).

use anyhow::Result;
use hyper_installer::{Downloader, self_update};

use crate::App;

/// Variabile d'ambiente che disattiva il controllo automatico all'avvio.
pub const DISABLE_ENV: &str = "HYPER_NO_UPDATE";

pub use self_update::current_build;

/// Se c'è una build più nuova la scarica, la verifica e avvia il setup in
/// modo silenzioso; restituisce la build installata. In quel caso il
/// processo deve terminare subito, così il setup può sostituire `hyper.exe`.
pub async fn run(app: &App) -> Result<Option<u64>> {
    if current_build().is_none() {
        return Ok(None);
    }
    let downloader = Downloader::new(app.config.download_concurrency)?;
    Ok(self_update::update_to_latest(&downloader, &app.paths.updates_dir(), false).await?)
}

pub fn installing_message(build: u64) -> String {
    format!(
        "Hyper Launcher si sta aggiornando alla build {build}: tra qualche secondo riesegui il comando."
    )
}
