//! Account di gioco. In questa fase solo account offline: nessun segreto,
//! quindi salvarli in `accounts.toml` è sicuro. Gli account Microsoft (fase 2)
//! terranno i token esclusivamente nel keyring di sistema.

use std::path::Path;

use hyper_core::{CoreError, fs};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("invalid offline username {0:?}: use 3-16 characters among letters, digits and _")]
    InvalidUsername(String),

    #[error("account {0:?} not found")]
    NotFound(String),

    #[error("no accounts configured")]
    NoAccounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Account {
    Offline { name: String, uuid: Uuid },
}

impl Account {
    /// Crea un account offline con lo UUID che userebbe il gioco stesso.
    pub fn offline(name: &str) -> Result<Self, AuthError> {
        validate_username(name)?;
        Ok(Self::Offline {
            name: name.to_owned(),
            uuid: offline_uuid(name),
        })
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Offline { name, .. } => name,
        }
    }

    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Offline { uuid, .. } => *uuid,
        }
    }

    /// Token da passare al gioco. Per gli account offline il valore non
    /// viene verificato da nessuno: `0` è la convenzione comune.
    pub fn access_token(&self) -> &str {
        match self {
            Self::Offline { .. } => "0",
        }
    }

    /// `true` se [`Self::access_token`] è un segreto da non mostrare mai
    /// (falso per gli account offline, il cui token è un segnaposto).
    pub fn token_is_secret(&self) -> bool {
        match self {
            Self::Offline { .. } => false,
        }
    }

    /// Valore di `${user_type}`. Le versioni recenti riconoscono solo
    /// `msa`/`legacy`/`mojang`; `msa` funziona anche offline.
    pub fn user_type(&self) -> &'static str {
        "msa"
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Offline { .. } => "offline",
        }
    }
}

/// UUID offline come `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)` in
/// Java: MD5 del nome con versione 3 e variante IETF impostate.
pub fn offline_uuid(name: &str) -> Uuid {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Nomi validi per Minecraft: 3–16 caratteri `[A-Za-z0-9_]`.
pub fn validate_username(name: &str) -> Result<(), AuthError> {
    let valid = (3..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if valid {
        Ok(())
    } else {
        Err(AuthError::InvalidUsername(name.to_owned()))
    }
}

/// Elenco account persistito in `accounts.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStore {
    /// Nome dell'account predefinito.
    #[serde(default)]
    default: Option<String>,
    #[serde(default)]
    accounts: Vec<Account>,
}

impl AccountStore {
    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn is_default(&self, account: &Account) -> bool {
        self.default
            .as_deref()
            .is_some_and(|d| d.eq_ignore_ascii_case(account.name()))
    }

    /// Rende predefinito l'account indicato e lo restituisce.
    pub fn set_default(&mut self, name: &str) -> Result<&Account, AuthError> {
        let index = self.position(name)?;
        self.default = Some(self.accounts[index].name().to_owned());
        Ok(&self.accounts[index])
    }

    fn position(&self, name: &str) -> Result<usize, AuthError> {
        self.accounts
            .iter()
            .position(|a| a.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| AuthError::NotFound(name.to_owned()))
    }

    pub fn load(path: &Path) -> Result<Self, AuthError> {
        Ok(fs::read_toml_or_default(path)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), AuthError> {
        Ok(fs::write_toml(path, self)?)
    }

    /// Aggiunge (o sostituisce, confrontando i nomi senza distinzione di
    /// maiuscole) un account. Il primo account diventa il predefinito.
    pub fn upsert(&mut self, account: Account) {
        self.accounts
            .retain(|a| !a.name().eq_ignore_ascii_case(account.name()));
        // Il predefinito segue anche un cambio di maiuscole del nome.
        let replaces_default = self
            .default
            .as_deref()
            .is_none_or(|d| d.eq_ignore_ascii_case(account.name()));
        if replaces_default {
            self.default = Some(account.name().to_owned());
        }
        self.accounts.push(account);
    }

    pub fn find(&self, name: &str) -> Result<&Account, AuthError> {
        Ok(&self.accounts[self.position(name)?])
    }

    /// Account indicato, oppure il predefinito, oppure il primo.
    pub fn select(&self, name: Option<&str>) -> Result<&Account, AuthError> {
        match name.or(self.default.as_deref()) {
            Some(name) => self.find(name),
            None => self.accounts.first().ok_or(AuthError::NoAccounts),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java_name_uuid_from_bytes() {
        // Valore noto: UUID offline di "Notch" nei server in offline-mode.
        assert_eq!(
            offline_uuid("Notch").to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert_eq!(offline_uuid("Notch").get_version_num(), 3);
    }

    #[test]
    fn username_validation() {
        assert!(Account::offline("Dev_01").is_ok());
        for bad in [
            "ab",
            "this_name_is_way_too_long",
            "spa ce",
            "ümlaut",
            "../x",
        ] {
            assert!(Account::offline(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn store_round_trip_and_selection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.toml");

        let mut store = AccountStore::load(&path).unwrap();
        assert!(matches!(store.select(None), Err(AuthError::NoAccounts)));

        store.upsert(Account::offline("Alice").unwrap());
        store.upsert(Account::offline("Bob").unwrap());
        store.upsert(Account::offline("alice").unwrap());
        store.save(&path).unwrap();

        let store = AccountStore::load(&path).unwrap();
        assert_eq!(store.accounts.len(), 2);
        assert_eq!(store.default.as_deref(), Some("alice"));
        // Il predefinito segue la nuova capitalizzazione del nome.
        assert_eq!(store.select(None).unwrap().name(), "alice");
        assert_eq!(store.select(Some("BOB")).unwrap().name(), "Bob");
        assert!(store.select(Some("carol")).is_err());
    }
}
