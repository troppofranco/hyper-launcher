//! Analisi dei crash: dal crash report del gioco (o dalla coda del log, o
//! dalla console di un server) si ricava la causa più probabile in parole
//! semplici, con un consiglio. Solo regole su testi noti di Minecraft, dei
//! loader e di Java: se nessuna regola riconosce l'errore si mostra la
//! descrizione del report così com'è.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Esito dell'analisi, per l'interfaccia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashInfo {
    /// Titolo breve ("Memoria esaurita").
    pub title: String,
    /// Dettaglio dal report (la riga che spiega, es. quale mod manca).
    pub cause: Option<String>,
    /// Cosa fare.
    pub hint: String,
    /// Report completo, se esiste.
    pub file: Option<PathBuf>,
}

/// Quanto leggere al massimo di un report o di un log.
const MAX_READ: u64 = 512 * 1024;

fn line_with<'a>(text: &'a str, needle: &str) -> Option<&'a str> {
    text.lines().find(|l| l.contains(needle)).map(str::trim)
}

fn clip(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 300 {
        format!("{}…", s.chars().take(300).collect::<String>())
    } else {
        s.to_owned()
    }
}

/// Riconosce la causa da un testo (report, log o console). `None` se non
/// c'è nessun errore noto.
pub fn analyze_text(text: &str) -> Option<CrashInfo> {
    let info = |title: &str, cause: Option<&str>, hint: &str| CrashInfo {
        title: title.to_owned(),
        cause: cause.map(clip),
        hint: hint.to_owned(),
        file: None,
    };

    if let Some(l) =
        line_with(text, "OutOfMemoryError").or_else(|| line_with(text, "Java heap space"))
    {
        return Some(info(
            "Memoria esaurita",
            Some(l),
            "Aumenta la memoria dedicata (impostazioni dell'istanza o del server) oppure togli qualche mod pesante.",
        ));
    }
    if let Some(l) = line_with(text, "UnsupportedClassVersionError") {
        return Some(info(
            "Versione di Java non adatta",
            Some(l),
            "Una mod o il gioco richiedono un Java più recente: lascia vuoto il percorso di Java nelle impostazioni dell'istanza, così il launcher usa quello giusto.",
        ));
    }
    // Fabric / Quilt: "Mod 'Sodium' (sodium) 0.6.0 requires any version of fabric-api, which is missing!"
    if let Some(l) = text.lines().map(str::trim).find(|l| {
        l.contains(" requires ")
            && (l.contains("which is missing")
                || l.contains("is installed!")
                || l.contains("but only"))
    }) {
        return Some(info(
            "Mod mancante o incompatibile",
            Some(l.trim_start_matches(['-', ' '])),
            "Installa la mod richiesta (o la versione giusta) dalla scheda Mod, oppure disattiva quella che la richiede.",
        ));
    }
    if line_with(text, "Incompatible mods found").is_some()
        || line_with(text, "Incompatible mod set").is_some()
    {
        let detail = text
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("- ") || l.contains("is incompatible with"));
        return Some(info(
            "Mod incompatibili",
            detail,
            "Due mod non possono stare insieme o una richiede un'altra versione: aggiorna o disattiva quella indicata.",
        ));
    }
    // NeoForge / Forge: "Missing or unsupported mandatory dependencies:" e
    // poi "Mod ID: 'x', Requested by: 'y'".
    if line_with(text, "Missing or unsupported mandatory dependencies").is_some() {
        let detail = line_with(text, "Requested by");
        return Some(info(
            "Mod mancante o incompatibile",
            detail,
            "Installa la mod richiesta (o la versione giusta) dalla scheda Mod, oppure disattiva quella che la richiede.",
        ));
    }
    if let Some(l) = line_with(text, "Found duplicate mods")
        .or_else(|| line_with(text, "DuplicateModsFoundException"))
        .or_else(|| line_with(text, "Duplicate mods found"))
    {
        return Some(info(
            "Mod duplicate",
            Some(l),
            "La stessa mod è installata due volte (magari in versioni diverse): elimina una delle due copie.",
        ));
    }
    if let Some(l) = text.lines().map(str::trim).find(|l| {
        l.contains("Mixin apply") && l.contains("failed") || l.contains("MixinApplyError")
    }) {
        return Some(info(
            "Una mod non è compatibile con questa versione",
            Some(l),
            "La mod indicata nel messaggio non funziona con questa versione del gioco o con un'altra mod: aggiornala o disattivala.",
        ));
    }
    if let Some(l) = line_with(text, "Pixel format not accelerated")
        .or_else(|| line_with(text, "GLFW error 65542"))
        .or_else(|| line_with(text, "does not appear to support OpenGL"))
        .or_else(|| line_with(text, "Couldn't set pixel format"))
    {
        return Some(info(
            "Problema dei driver video",
            Some(l),
            "Aggiorna i driver della scheda video dal sito del produttore (NVIDIA, AMD o Intel) e riavvia il PC.",
        ));
    }
    if let Some(l) = line_with(text, "FAILED TO BIND TO PORT") {
        return Some(info(
            "Porta già in uso",
            Some(l),
            "Un altro programma (o un altro server) usa questa porta: chiudilo o cambia la porta nelle impostazioni del server.",
        ));
    }
    if let Some(l) = line_with(text, "You need to agree to the EULA") {
        return Some(info(
            "EULA non accettata",
            Some(l),
            "Apri il file eula.txt del server e imposta eula=true (il launcher lo fa da solo alla creazione).",
        ));
    }
    // Crash report senza regola: la sua descrizione.
    if let Some(l) = line_with(text, "Description:") {
        let exception = text
            .lines()
            .map(str::trim)
            .skip_while(|l| !l.starts_with("Description:"))
            .find(|l| l.contains("Exception") || l.contains("Error"));
        return Some(CrashInfo {
            title: clip(l.trim_start_matches("Description:")),
            cause: exception.map(clip),
            hint: "Apri il report completo: la prima riga con \"Exception\" di solito indica la mod coinvolta.".to_owned(),
            file: None,
        });
    }
    None
}

fn read_tail(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    if len > MAX_READ {
        file.seek(SeekFrom::Start(len - MAX_READ)).ok()?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Analizza un'uscita con errore del gioco: il crash report scritto dopo
/// `since` (secondi Unix) se c'è, altrimenti la coda di `logs/latest.log`.
/// Sempre un risultato: senza regole note, un messaggio generico.
pub fn analyze_game(game_dir: &Path, since: u64) -> CrashInfo {
    let newest_report = std::fs::read_dir(game_dir.join("crash-reports"))
        .ok()
        .into_iter()
        .flat_map(|d| d.flatten())
        .filter_map(|e| {
            let modified = e
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            (modified + 2 >= since && e.file_name().to_string_lossy().ends_with(".txt"))
                .then(|| (modified, e.path()))
        })
        .max_by_key(|(m, _)| *m)
        .map(|(_, p)| p);
    let candidates = newest_report
        .iter()
        .cloned()
        .chain(std::iter::once(game_dir.join("logs").join("latest.log")));
    for path in candidates {
        if let Some(mut info) = read_tail(&path).as_deref().and_then(analyze_text) {
            info.file = Some(path);
            return info;
        }
    }
    CrashInfo {
        title: "Il gioco si è chiuso con un errore".to_owned(),
        cause: None,
        hint: "Non ho trovato una causa nota: apri il log per i dettagli, o prova a disattivare le mod aggiunte di recente.".to_owned(),
        file: newest_report.or_else(|| Some(game_dir.join("logs").join("latest.log"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_common_crashes() {
        let fabric = "Incompatible mods found!\n - Mod 'Sodium' (sodium) 0.6.0 requires any version of fabric-api, which is missing!\n";
        let c = analyze_text(fabric).unwrap();
        assert_eq!(c.title, "Mod mancante o incompatibile");
        assert!(c.cause.unwrap().starts_with("Mod 'Sodium'"));

        let oom =
            "Exception in thread \"Render thread\" java.lang.OutOfMemoryError: Java heap space";
        assert_eq!(analyze_text(oom).unwrap().title, "Memoria esaurita");

        let neo = "Missing or unsupported mandatory dependencies:\n\tMod ID: 'geckolib', Requested by: 'alexsmobs', Expected range: '[4.4,)', Actual version: '[MISSING]'";
        assert!(
            analyze_text(neo)
                .unwrap()
                .cause
                .unwrap()
                .contains("Requested by")
        );

        let port = "[21:00:00 WARN]: **** FAILED TO BIND TO PORT!";
        assert_eq!(analyze_text(port).unwrap().title, "Porta già in uso");

        let report = "---- Minecraft Crash Report ----\nDescription: Rendering overlay\n\njava.lang.NullPointerException: boom\n\tat x.y";
        let c = analyze_text(report).unwrap();
        assert_eq!(c.title, "Rendering overlay");
        assert!(c.cause.unwrap().contains("NullPointerException"));

        assert!(analyze_text("[INFO] Done (3.2s)!").is_none());
    }

    #[test]
    fn reads_the_newest_report() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crash-reports")).unwrap();
        std::fs::write(
            dir.path().join("crash-reports/crash-1.txt"),
            "Description: x\njava.lang.OutOfMemoryError",
        )
        .unwrap();
        let c = analyze_game(dir.path(), 0);
        assert_eq!(c.title, "Memoria esaurita");
        assert!(c.file.unwrap().ends_with("crash-1.txt"));
        let none = analyze_game(&dir.path().join("missing"), 0);
        assert_eq!(none.title, "Il gioco si è chiuso con un errore");
    }
}
