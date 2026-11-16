//! Validazione dei percorsi provenienti da fonti esterne (entry di archivi zip,
//! manifest remoti, nomi scelti dall'utente) prima di usarli sul filesystem.

use std::path::{Path, PathBuf};

use crate::CoreError;

/// Nomi di dispositivo riservati su Windows: aprirli non crea un file ma
/// accede al dispositivo, indipendentemente dall'estensione.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "COM¹", "COM²",
    "COM³", "LPT¹", "LPT²", "LPT³", "CONIN$", "CONOUT$",
];

/// Controlla che `name` sia un singolo componente di percorso innocuo su ogni
/// piattaforma: niente separatori, niente punti o spazi finali (quindi niente
/// `.`/`..`, nemmeno nelle varianti che Windows normalizza come `.. `), niente `:` (lettere di unità,
/// alternate data stream), niente caratteri di controllo, niente nomi riservati.
pub fn safe_component(name: &str) -> Result<&str, CoreError> {
    let unsafe_path = || CoreError::UnsafePath(name.to_owned());

    if name.is_empty() || name.len() > 255 {
        return Err(unsafe_path());
    }
    if name.chars().any(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(unsafe_path());
    }
    // Windows rimuove punti e spazi finali (".. " diventerebbe ".."): li
    // rifiutiamo del tutto, il che copre anche "." e "..".
    if name.ends_with(['.', ' ']) {
        return Err(unsafe_path());
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(unsafe_path());
    }
    Ok(name)
}

/// Unisce a `base` un percorso relativo non fidato (`/` e `\` sono entrambi
/// separatori), garantendo che il risultato resti dentro `base`.
///
/// Rifiuta percorsi assoluti, prefissi di unità/UNC, componenti `..` e ogni
/// componente che non passa [`safe_component`]. I componenti vuoti e `.`
/// intermedi (es. `a//b`, `a/./b`) vengono ignorati.
pub fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, CoreError> {
    let unsafe_path = || CoreError::UnsafePath(relative.to_owned());

    if relative.starts_with(['/', '\\']) {
        return Err(unsafe_path());
    }

    let mut out = base.to_path_buf();
    let mut pushed = 0usize;
    for part in relative.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        safe_component(part).map_err(|_| unsafe_path())?;
        out.push(part);
        pushed += 1;
    }

    if pushed == 0 {
        return Err(unsafe_path());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_relative_paths() {
        let base = Path::new("base");
        assert_eq!(
            safe_join(base, "org/lwjgl/lwjgl.dll").unwrap(),
            base.join("org").join("lwjgl").join("lwjgl.dll")
        );
        assert_eq!(
            safe_join(base, "a\\b/./c//d").unwrap(),
            base.join("a").join("b").join("c").join("d")
        );
        assert_eq!(
            safe_join(base, "windows/x64/").unwrap(),
            base.join("windows").join("x64")
        );
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        let base = Path::new("base");
        for evil in [
            "../evil",
            "..\\evil",
            "a/../../b",
            "a/../b",
            "/etc/passwd",
            "\\Windows\\System32",
            "C:\\x",
            "C:x",
            "\\\\server\\share\\x",
            "file.txt:stream",
            ".. /x",
            ".../x",
            "",
            ".",
            "./",
            "dir/CON",
            "nul.txt",
            "a\0b",
        ] {
            assert!(safe_join(base, evil).is_err(), "should reject {evil:?}");
        }
    }

    #[test]
    fn component_validation() {
        assert!(safe_component("my-instance 1.21").is_ok());
        assert!(safe_component("1.21.4").is_ok());
        assert!(safe_component("console").is_ok());
        for bad in [
            "",
            "..",
            "com¹.dll",
            "CONOUT$",
            "a/b",
            "a\\b",
            "COM1",
            "x:y",
            "trailing. ",
            "q?",
        ] {
            assert!(safe_component(bad).is_err(), "should reject {bad:?}");
        }
    }
}
