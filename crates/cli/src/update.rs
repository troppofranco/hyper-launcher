//! Aggiornamento automatico della CLI installata.

use anyhow::Result;
use hyper_installer::{Downloader, self_update};

use crate::App;

/// Variabile d'ambiente che disattiva il controllo automatico all'avvio.
pub const DISABLE_ENV: &str = "HYPER_NO_UPDATE";

/// Numero di build incorporato dalla GitHub Action che pubblica il setup;
/// `None` nelle build locali (sviluppo), che non si aggiornano mai da sole.
pub fn current_build() -> Option<u64> {
    option_env!("HYPER_BUILD").and_then(|b| b.parse().ok())
}

/// Repository GitHub (`owner/nome`) da cui arrivano gli aggiornamenti.
fn repo() -> Option<&'static str> {
    option_env!("HYPER_UPDATE_REPO").filter(|r| !r.is_empty())
}

/// Controlla l'ultima release e, se è più nuova, scarica e verifica il
/// setup e lo avvia in modo silenzioso. Restituisce la build installata:
/// in quel caso il processo deve terminare subito, così il setup può
/// sostituire `hyper.exe`.
pub async fn run(app: &App) -> Result<Option<u64>> {
    let (Some(build), Some(repo)) = (current_build(), repo()) else {
        return Ok(None);
    };
    let dir = app.paths.updates_dir();
    let url = self_update::latest_release_url(repo);
    let Some(update) = self_update::check(&url, build).await? else {
        self_update::clean_old_setups(&dir, None);
        return Ok(None);
    };
    eprintln!(
        "È disponibile Hyper Launcher build {} (questa è la build {build}): aggiornamento in corso…",
        update.build
    );
    let downloader = Downloader::new(app.config.download_concurrency)?;
    let setup = self_update::download(&downloader, &update, &dir).await?;
    self_update::clean_old_setups(&dir, Some(&setup));
    self_update::run_installer(&setup)?;
    Ok(Some(update.build))
}

pub fn installing_message(build: u64) -> String {
    format!(
        "Hyper Launcher si sta aggiornando alla build {build}: tra qualche secondo riesegui il comando."
    )
}
