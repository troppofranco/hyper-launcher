//! Wrapper sottili su `std::fs` che allegano il percorso all'errore e
//! scrivono in modo atomico (file temporaneo + rename).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::CoreError;

pub fn create_dir_all(path: &Path) -> Result<(), CoreError> {
    std::fs::create_dir_all(path).map_err(|e| CoreError::io(path, e))
}

/// Percorso reale (link e reindirizzamenti risolti) di un file esistente,
/// nella forma normale `C:\…` invece di quella estesa `\\?\C:\…` che
/// `canonicalize` restituisce su Windows e che Java non accetta.
pub fn real_path(path: &Path) -> Result<PathBuf, CoreError> {
    let real = std::fs::canonicalize(path).map_err(|e| CoreError::io(path, e))?;
    Ok(strip_verbatim(real))
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        // Solo `\\?\C:\…`: le forme UNC (`\\?\UNC\…`) restano come sono.
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

pub fn read(path: &Path) -> Result<Vec<u8>, CoreError> {
    std::fs::read(path).map_err(|e| CoreError::io(path, e))
}

pub fn read_to_string(path: &Path) -> Result<String, CoreError> {
    std::fs::read_to_string(path).map_err(|e| CoreError::io(path, e))
}

/// Legge e deserializza un file TOML.
pub fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, CoreError> {
    let text = read_to_string(path)?;
    toml::from_str(&text).map_err(|source| CoreError::TomlParse {
        path: path.to_owned(),
        source,
    })
}

/// Come [`read_toml`], ma se il file non esiste restituisce `T::default()`.
pub fn read_toml_or_default<T>(path: &Path) -> Result<T, CoreError>
where
    T: serde::de::DeserializeOwned + Default,
{
    match read_toml(path) {
        Err(CoreError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(T::default())
        }
        other => other,
    }
}

/// Serializza `value` in TOML e lo scrive atomicamente in `path`.
pub fn write_toml<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    let text = toml::to_string_pretty(value)?;
    write_atomic(path, text.as_bytes())
}

/// Percorso temporaneo accanto a `path` (stessa cartella, quindi il rename è
/// atomico sullo stesso volume): `nome.ext` → `nome.ext.<suffix>`.
pub fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(OsString::from).unwrap_or_default();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Scrive `bytes` in un file temporaneo e lo rinomina su `path`, così un
/// crash a metà non lascia mai un file troncato.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let tmp = sibling_with_suffix(path, "tmp");
    std::fs::write(&tmp, bytes).map_err(|e| CoreError::io(&tmp, e))?;
    rename_replacing(&tmp, path)
}

/// Rinomina `tmp` su `dest` (sostituendolo); se fallisce elimina `tmp`
/// per non lasciare file temporanei orfani.
pub fn rename_replacing(tmp: &Path, dest: &Path) -> Result<(), CoreError> {
    std::fs::rename(tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(tmp);
        CoreError::io(dest, e)
    })
}

/// Rimuove una cartella e il suo contenuto ignorando l'assenza.
pub fn remove_dir_all_if_exists(path: &Path) -> Result<(), CoreError> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::io(path, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("file.toml");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        assert!(!sibling_with_suffix(&path, "tmp").exists());
    }

    #[test]
    fn real_path_is_absolute_and_not_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let real = real_path(dir.path()).unwrap();
        assert!(real.is_absolute());
        assert!(!real.to_string_lossy().starts_with(r"\\?\"));
        assert!(real.is_dir());
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\Users\x")),
            PathBuf::from(r"C:\Users\x")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\server\share")),
            PathBuf::from(r"\\?\UNC\server\share")
        );
    }
}
