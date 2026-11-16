//! Dove vivono i segreti: il keyring di sistema in produzione, la memoria
//! nei test. Mai su disco in chiaro.

use crate::{AuthError, Secret};

/// Servizio con cui il launcher registra le proprie voci nel keyring.
pub const KEYRING_SERVICE: &str = "hyper-launcher";

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Secret>, AuthError>;
    fn set(&self, key: &str, value: &Secret) -> Result<(), AuthError>;
    /// Elimina la voce; nessun errore se non esiste.
    fn delete(&self, key: &str) -> Result<(), AuthError>;
}

/// Keyring del sistema (Windows Credential Manager, Keychain, Secret
/// Service). La voce `chiave` contiene `v:<valore>` per i valori corti (i
/// token normali: una sola lettura/scrittura) oppure `n:<parti>` quando il
/// valore supera il limite Windows di 2560 byte e viene diviso in
/// `chiave#0`, `chiave#1`, …
#[derive(Debug, Clone)]
pub struct KeyringStore {
    service: String,
}

/// Dimensione massima di un valore in una singola voce, con margine sotto
/// il limite Windows.
const CHUNK: usize = 2000;

impl Default for KeyringStore {
    /// Lo store del launcher (servizio [`KEYRING_SERVICE`]).
    fn default() -> Self {
        Self::new(KEYRING_SERVICE)
    }
}

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

    fn write(&self, key: &str, bytes: &[u8]) -> Result<(), AuthError> {
        Ok(self.entry(key)?.set_secret(bytes)?)
    }

    /// `true` se la voce esisteva.
    fn remove(&self, key: &str) -> Result<bool, AuthError> {
        match self.entry(key)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Cancella le parti da `from` in poi, finché esistono (senza fidarsi
    /// dell'intestazione, che potrebbe essere corrotta o già sparita).
    fn remove_parts_from(&self, key: &str, from: usize) -> Result<(), AuthError> {
        for i in from.. {
            if !self.remove(&format!("{key}#{i}"))? {
                break;
            }
        }
        Ok(())
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<Secret>, AuthError> {
        let corrupt = || AuthError::CorruptSecret(key.to_owned());
        let Some(header) = self.read(key)? else {
            return Ok(None);
        };
        let header = String::from_utf8(header).map_err(|_| corrupt())?;
        if let Some(value) = header.strip_prefix("v:") {
            return Ok(Some(Secret::new(value)));
        }
        let parts: usize = header
            .strip_prefix("n:")
            .and_then(|n| n.parse().ok())
            .ok_or_else(corrupt)?;
        let mut bytes = Vec::new();
        for i in 0..parts {
            bytes.extend(self.read(&format!("{key}#{i}"))?.ok_or_else(corrupt)?);
        }
        String::from_utf8(bytes)
            .map(|s| Some(Secret::new(s)))
            .map_err(|_| corrupt())
    }

    fn set(&self, key: &str, value: &Secret) -> Result<(), AuthError> {
        let value = value.expose();
        if value.len() <= CHUNK {
            // Una sola scrittura atomica; poi via eventuali parti vecchie.
            self.write(key, format!("v:{value}").as_bytes())?;
            return self.remove_parts_from(key, 0);
        }
        // Intestazione tolta prima e scritta per ultima: una scrittura
        // interrotta lascia la voce "assente", mai mezza letta.
        self.remove(key)?;
        let chunks: Vec<&[u8]> = value.as_bytes().chunks(CHUNK).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            self.write(&format!("{key}#{i}"), chunk)?;
        }
        self.write(key, format!("n:{}", chunks.len()).as_bytes())?;
        self.remove_parts_from(key, chunks.len())
    }

    fn delete(&self, key: &str) -> Result<(), AuthError> {
        self.remove(key)?;
        self.remove_parts_from(key, 0)
    }
}

/// Archivio in memoria per i test.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct MemoryStore {
    values: std::sync::Mutex<std::collections::HashMap<String, Secret>>,
}

#[cfg(test)]
impl MemoryStore {
    pub(crate) fn keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.lock().keys().cloned().collect();
        keys.sort();
        keys
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<String, Secret>> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
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
        // Da valore diviso a valore corto: le parti vecchie spariscono.
        store
            .set("chunked", &Secret::new("short:with:colons"))
            .unwrap();
        assert_eq!(
            store.get("chunked").unwrap().unwrap().expose(),
            "short:with:colons"
        );
        assert!(store.read("chunked#0").unwrap().is_none());
        store.delete("chunked").unwrap();
        assert!(store.get("chunked").unwrap().is_none());
    }
}
