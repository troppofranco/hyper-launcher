//! Hash SHA1: tipo validato e verifica dei file su disco.

use std::fmt;
use std::io::Read;
use std::path::Path;

use hyper_core::CoreError;
use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::{InstallerError, Result};

/// SHA1 esadecimale validato (40 caratteri hex, normalizzato in minuscolo).
///
/// La validazione avviene già in deserializzazione: gli hash degli asset
/// diventano percorsi su disco (`objects/ab/abcd…`), quindi un valore
/// arbitrario da un manifest non deve mai arrivare al filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct Sha1Hex(String);

impl Sha1Hex {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Sha1Hex {
    type Error = InstallerError;

    fn try_from(value: String) -> Result<Self> {
        if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
            Ok(Self(value.to_ascii_lowercase()))
        } else {
            Err(InstallerError::InvalidSha1(value))
        }
    }
}

impl std::str::FromStr for Sha1Hex {
    type Err = InstallerError;

    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s.to_owned())
    }
}

impl fmt::Display for Sha1Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Calcola lo SHA1 di un file leggendolo a blocchi; restituisce anche la
/// dimensione letta.
pub fn sha1_file(path: &Path) -> Result<(Sha1Hex, u64), CoreError> {
    let mut file = std::fs::File::open(path).map_err(|e| CoreError::io(path, e))?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|e| CoreError::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    Ok((Sha1Hex(hex::encode(hasher.finalize())), size))
}

pub fn check_size(path: &Path, expected: Option<u64>, actual: u64) -> Result<()> {
    match expected {
        Some(expected) if expected != actual => Err(InstallerError::SizeMismatch {
            path: path.to_owned(),
            expected,
            actual,
        }),
        _ => Ok(()),
    }
}

pub fn check_hash(path: &Path, expected: &Sha1Hex, actual: &Sha1Hex) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(InstallerError::HashMismatch {
            path: path.to_owned(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

/// Verifica che il file abbia l'hash (ed eventualmente la dimensione) attesi.
/// La dimensione è controllata prima, così un file troncato non viene letto.
pub fn verify_file(path: &Path, expected: &Sha1Hex, expected_size: Option<u64>) -> Result<()> {
    if expected_size.is_some() {
        let len = std::fs::metadata(path)
            .map_err(|e| CoreError::io(path, e))?
            .len();
        check_size(path, expected_size, len)?;
    }
    let (actual, _) = sha1_file(path)?;
    check_hash(path, expected, &actual)
}

/// `true` se il file esiste ed è integro; `false` se manca o non corrisponde.
pub fn file_is_valid(path: &Path, expected: &Sha1Hex, expected_size: Option<u64>) -> Result<bool> {
    match verify_file(path, expected, expected_size) {
        Ok(()) => Ok(true),
        Err(InstallerError::HashMismatch { .. } | InstallerError::SizeMismatch { .. }) => Ok(false),
        Err(InstallerError::Core(CoreError::Io { source, .. }))
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// Rende definitivo un download già verificato (`check`): se la verifica è
/// fallita il file parziale viene eliminato e `dest` non viene toccato,
/// altrimenti `part` viene rinominato atomicamente su `dest`.
pub fn commit(part: &Path, dest: &Path, check: Result<()>) -> Result<()> {
    if let Err(e) = check {
        let _ = std::fs::remove_file(part);
        return Err(e);
    }
    Ok(hyper_core::fs::rename_replacing(part, dest)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA1("abc") — vettore di test FIPS 180-1.
    const ABC_SHA1: &str = "a9993e364706816aba3e25717850c26c9cd0d89d";

    #[test]
    fn sha1_of_known_vector() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc.txt");
        std::fs::write(&path, b"abc").unwrap();
        let (hash, size) = sha1_file(&path).unwrap();
        assert_eq!(hash.as_str(), ABC_SHA1);
        assert_eq!(size, 3);
    }

    #[test]
    fn sha1hex_validation_and_normalization() {
        let upper: Sha1Hex = ABC_SHA1.to_uppercase().parse().unwrap();
        assert_eq!(upper.as_str(), ABC_SHA1);
        assert!("abc".parse::<Sha1Hex>().is_err());
        assert!(
            "../../aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .parse::<Sha1Hex>()
                .is_err()
        );
        assert!("g".repeat(40).parse::<Sha1Hex>().is_err());
    }

    #[test]
    fn verify_detects_mismatch_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"abc").unwrap();
        let good: Sha1Hex = ABC_SHA1.parse().unwrap();
        let bad: Sha1Hex = "0".repeat(40).parse().unwrap();

        verify_file(&path, &good, Some(3)).unwrap();
        assert!(matches!(
            verify_file(&path, &bad, None),
            Err(InstallerError::HashMismatch { .. })
        ));
        assert!(matches!(
            verify_file(&path, &good, Some(4)),
            Err(InstallerError::SizeMismatch { .. })
        ));
        assert!(file_is_valid(&path, &good, Some(3)).unwrap());
        assert!(!file_is_valid(&path, &bad, None).unwrap());
        assert!(!file_is_valid(&dir.path().join("missing"), &good, None).unwrap());
    }

    #[test]
    fn commit_only_moves_verified_files() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("lib.jar.part");
        let dest = dir.path().join("lib.jar");
        let good: Sha1Hex = ABC_SHA1.parse().unwrap();

        std::fs::write(&part, b"corrupted").unwrap();
        let check = verify_file(&part, &good, None);
        assert!(commit(&part, &dest, check).is_err());
        assert!(!part.exists(), "corrupted part file must be deleted");
        assert!(!dest.exists(), "destination must not be created");

        std::fs::write(&part, b"abc").unwrap();
        let check = verify_file(&part, &good, Some(3));
        commit(&part, &dest, check).unwrap();
        assert!(!part.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"abc");
    }
}
