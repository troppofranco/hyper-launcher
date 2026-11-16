//! Dove vivono i segreti: il keyring di sistema in produzione, la memoria
//! nei test. Mai su disco in chiaro.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::{AuthError, Secret};

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Secret>, AuthError>;
    fn set(&self, key: &str, value: &Secret) -> Result<(), AuthError>;
    /// Elimina la voce; nessun errore se non esiste.
    fn delete(&self, key: &str) -> Result<(), AuthError>;
}

/// Keyring del sistema (Windows Credential Manager, Keychain, Secret
/// Service). Su Windows ogni voce ha un limite di 2560 byte: i valori più
/// lunghi vengono divisi in più voci `chiave#0`, `chiave#1`, … e la voce
/// `chiave` contiene solo il numero di parti.
#[derive(Debug, Clone)]
pub struct KeyringStore {
    service: String,
}

/// Dimensione massima di una parte, con margine sotto il limite Windows.
const CHUNK: usize = 2000;

impl KeyringStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, AuthError> {
        Ok(keyring::Entry::new(&self.service, key)?)
    }

    fn read(&self, key: &str) -> Result<Option<Vec<u8>>, AuthError> {
        match self.entry(key)?.get_secret() {
            Ok(bytes) => Ok(Some(bytes)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn remove(&self, key: &str) -> Result<(), AuthError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn parts(&self, key: &str) -> Result<usize, AuthError> {
        let Some(header) = self.read(key)? else {
            return Ok(0);
        };
        String::from_utf8(header)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| AuthError::CorruptSecret(key.to_owned()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<Secret>, AuthError> {
        let parts = self.parts(key)?;
        if parts == 0 {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        for i in 0..parts {
            let part = self
                .read(&format!("{key}#{i}"))?
                .ok_or_else(|| AuthError::CorruptSecret(key.to_owned()))?;
            bytes.extend(part);
        }
        String::from_utf8(bytes)
            .map(|s| Some(Secret::new(s)))
            .map_err(|_| AuthError::CorruptSecret(key.to_owned()))
    }

    fn set(&self, key: &str, value: &Secret) -> Result<(), AuthError> {
        self.delete(key)?;
        let chunks: Vec<&[u8]> = value.expose().as_bytes().chunks(CHUNK).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            self.entry(&format!("{key}#{i}"))?.set_secret(chunk)?;
        }
        // L'intestazione per ultima: una scrittura interrotta lascia la voce
        // "assente", mai mezza letta.
        self.entry(key)?
            .set_secret(chunks.len().to_string().as_bytes())?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.remove(key)?;
        // Le parti si cancellano finché esistono, senza fidarsi
        // dell'intestazione (che potrebbe essere corrotta o già sparita).
        for i in 0.. {
            let part = format!("{key}#{i}");
            if self.read(&part)?.is_none() {
                break;
            }
            self.remove(&part)?;
        }
        Ok(())
    }
}

/// Archivio in memoria per i test.
#[derive(Debug, Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, Secret>>,
}

impl MemoryStore {
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.lock().keys().cloned().collect();
        keys.sort();
        keys
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Secret>> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<Secret>, AuthError> {
        Ok(self.lock().get(key).cloned())
    }

    fn set(&self, key: &str, value: &Secret) -> Result<(), AuthError> {
        self.lock().insert(key.to_owned(), value.clone());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.lock().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryStore::default();
        assert!(store.get("k").unwrap().is_none());
        store.set("k", &Secret::new("v")).unwrap();
        assert_eq!(store.get("k").unwrap().unwrap().expose(), "v");
        store.delete("k").unwrap();
        store.delete("k").unwrap();
        assert!(store.get("k").unwrap().is_none());
    }

    /// Usa il keyring vero del sistema: `cargo test -- --ignored`.
    #[test]
    #[ignore = "scrive nel keyring di sistema"]
    fn keyring_round_trip_with_chunks() {
        let store = KeyringStore::new("hyper-launcher-test");
        let long = "x".repeat(CHUNK * 2 + 123);
        store.set("chunked", &Secret::new(long.clone())).unwrap();
        assert_eq!(store.get("chunked").unwrap().unwrap().expose(), long);
        store.set("chunked", &Secret::new("short")).unwrap();
        assert_eq!(store.get("chunked").unwrap().unwrap().expose(), "short");
        store.delete("chunked").unwrap();
        assert!(store.get("chunked").unwrap().is_none());
    }
}
