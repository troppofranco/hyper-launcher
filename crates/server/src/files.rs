//! File del server per il browser e l'editor dell'interfaccia. Ogni
//! percorso è relativo alla cartella del server e passa da `safe_join`;
//! i collegamenti (symlink, junction) non si seguono mai.

use std::path::{Path, PathBuf};

use hyper_core::{fs, safe_join};
use serde::Serialize;

use crate::{Result, ServerError};

/// I file più grandi non si aprono nell'editor.
pub const MAX_EDIT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ServerFile {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    /// Secondi Unix dell'ultima modifica.
    pub modified: u64,
}

/// Percorso dentro la cartella del server; `""` è la cartella stessa.
/// Rifiuta `..`, percorsi assoluti e qualunque collegamento lungo la strada.
fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim_matches(['/', '\\']);
    if rel.is_empty() {
        return Ok(root.to_owned());
    }
    let path = safe_join(root, rel)?;
    let mut current = root.to_owned();
    for part in Path::new(rel).components() {
        current.push(part);
        if std::fs::symlink_metadata(&current).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(ServerError::Invalid("links are not followed".into()));
        }
    }
    Ok(path)
}

/// Contenuto di una cartella: prima le cartelle, poi i file, in ordine alfabetico.
pub fn list(root: &Path, rel: &str) -> Result<Vec<ServerFile>> {
    let dir = resolve(root, rel)?;
    let mut entries: Vec<ServerFile> = std::fs::read_dir(&dir)?
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let file_type = e.file_type().ok()?;
            if file_type.is_symlink() {
                return None;
            }
            Some(ServerFile {
                name: e.file_name().into_string().ok()?,
                dir: file_type.is_dir(),
                size: if file_type.is_dir() { 0 } else { meta.len() },
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs()),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// Testo di un file (UTF-8, al massimo 2 MB).
pub fn read_text(root: &Path, rel: &str) -> Result<String> {
    let path = resolve(root, rel)?;
    let meta = std::fs::metadata(&path)?;
    if !meta.is_file() {
        return Err(ServerError::Invalid("not a file".into()));
    }
    if meta.len() > MAX_EDIT_BYTES {
        return Err(ServerError::FileTooLarge(meta.len()));
    }
    let bytes = std::fs::read(&path)?;
    if bytes.contains(&0) {
        return Err(ServerError::NotText);
    }
    String::from_utf8(bytes).map_err(|_| ServerError::NotText)
}

/// Salva un file di testo esistente o nuovo (scrittura atomica).
pub fn write_text(root: &Path, rel: &str, content: &str) -> Result<()> {
    if content.len() as u64 > MAX_EDIT_BYTES {
        return Err(ServerError::FileTooLarge(content.len() as u64));
    }
    let path = resolve(root, rel)?;
    if path == root || path.is_dir() {
        return Err(ServerError::Invalid("not a file".into()));
    }
    fs::write_atomic(&path, content.as_bytes())?;
    Ok(())
}

/// Elimina un file o una cartella (mai la cartella del server stessa).
pub fn delete(root: &Path, rel: &str) -> Result<()> {
    let path = resolve(root, rel)?;
    if path == root {
        return Err(ServerError::Invalid(
            "cannot delete the server folder".into(),
        ));
    }
    if path.is_dir() {
        fs::remove_dir_all_if_exists(&path)?;
    } else {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_inside_the_server_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("srv");
        std::fs::create_dir_all(root.join("plugins")).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "no").unwrap();
        write_text(&root, "plugins/config.yml", "a: 1\n").unwrap();
        assert_eq!(read_text(&root, "plugins/config.yml").unwrap(), "a: 1\n");
        assert_eq!(list(&root, "").unwrap()[0].name, "plugins");
        for bad in [
            "../secret.txt",
            "..\\secret.txt",
            "plugins/../../secret.txt",
            "C:/Windows/win.ini",
            "C:secret.txt",
        ] {
            assert!(read_text(&root, bad).is_err(), "{bad}");
            assert!(write_text(&root, bad, "x").is_err(), "{bad}");
            assert!(delete(&root, bad).is_err(), "{bad}");
        }
        assert!(delete(&root, "").is_err());
        assert!(dir.path().join("secret.txt").exists());
        // Una barra iniziale vale come "dalla cartella del server", non dalla radice del disco.
        write_text(&root, "/notes.txt", "ok").unwrap();
        assert!(root.join("notes.txt").is_file());
    }

    #[test]
    fn refuses_binary_and_large_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("world.dat"), [0u8, 1, 2]).unwrap();
        assert!(matches!(
            read_text(dir.path(), "world.dat"),
            Err(ServerError::NotText)
        ));
        let big = "x".repeat(MAX_EDIT_BYTES as usize + 1);
        assert!(matches!(
            write_text(dir.path(), "big.txt", &big),
            Err(ServerError::FileTooLarge(_))
        ));
    }
}
