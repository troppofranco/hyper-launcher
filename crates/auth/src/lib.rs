//! Account di gioco: offline e Microsoft.
//!
//! `accounts.toml` contiene solo dati pubblici (tipo, nome, UUID). I token
//! Microsoft e Minecraft stanno esclusivamente in un [`SecretStore`] (il
//! keyring di sistema), sono avvolti in [`Secret`] e non vengono mai
//! stampati né scritti su disco in chiaro.

#[cfg(test)]
mod flow_tests;
pub mod msa;
mod secret;
mod session;
mod store;
pub mod xbox;

use std::path::Path;

use hyper_core::{CoreError, fs};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use msa::{DeviceCode, Endpoints, MsaClient};
pub use secret::Secret;
pub use session::{Session, forget, login_microsoft, session};
pub use store::{KeyringStore, MemoryStore, SecretStore};

/// Servizio con cui il launcher registra le voci nel keyring.
pub const KEYRING_SERVICE: &str = "hyper-launcher";

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

    #[error(
        "no Microsoft client ID configured (msa_client_id in config.toml or HYPER_MSA_CLIENT_ID)"
    )]
    MissingClientId,

    #[error("the login code expired before it was confirmed")]
    DeviceCodeExpired,

    #[error("the login was declined in the browser")]
    AuthorizationDeclined,

    #[error("the Microsoft login expired or was revoked: sign in again")]
    LoginExpired,

    #[error("this Microsoft account has no Xbox profile: create one at xbox.com and retry")]
    NoXboxAccount,

    #[error("Xbox Live is not available in this account's country")]
    XboxUnavailableInCountry,

    #[error("this account needs adult verification on xbox.com")]
    AdultVerificationRequired,

    #[error("child account: an adult must add it to a Microsoft family group")]
    ChildAccount,

    #[error("Xbox authorization failed (XErr {0})")]
    Xsts(u64),

    #[error(
        "Mojang rejected this app's client ID (403): it has not been approved for the Minecraft API yet"
    )]
    AppNotApproved,

    #[error(
        "no Minecraft: Java Edition profile on this account: buy the game, or (Game Pass) create a profile name on minecraft.net first"
    )]
    GameNotOwned,

    #[error("Microsoft OAuth error {error}: {description}")]
    OAuth { error: String, description: String },

    #[error("HTTP request to {url} failed: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("cannot build the HTTP client: {0}")]
    HttpClient(#[source] reqwest::Error),

    #[error("{url} answered with HTTP {status}")]
    UnexpectedStatus { url: String, status: u16 },

    #[error("unexpected response: {0}")]
    MalformedResponse(&'static str),

    #[error("system keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("the Minecraft profile does not match this account: sign in again")]
    AccountMismatch,

    #[error("keyring entry {0:?} is corrupted: sign in again")]
    CorruptSecret(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Account {
    Offline {
        name: String,
        uuid: Uuid,
    },
    /// I token sono nel keyring, indicizzati per UUID.
    Microsoft {
        name: String,
        uuid: Uuid,
    },
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
            Self::Offline { name, .. } | Self::Microsoft { name, .. } => name,
        }
    }

    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Offline { uuid, .. } | Self::Microsoft { uuid, .. } => *uuid,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Offline { .. } => "offline",
            Self::Microsoft { .. } => "microsoft",
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

    /// Rimuove l'account e lo restituisce (per cancellarne i segreti con
    /// [`forget`]). Se era il predefinito, il predefinito passa al primo rimasto.
    pub fn remove(&mut self, name: &str) -> Result<Account, AuthError> {
        let account = self.accounts.remove(self.position(name)?);
        if self.is_default(&account) {
            self.default = self.accounts.first().map(|a| a.name().to_owned());
        }
        Ok(account)
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
        // Stesso nome o stesso UUID (un account Microsoft che ha cambiato
        // nome) = stesso account.
        self.accounts.retain(|a| {
            !a.name().eq_ignore_ascii_case(account.name()) && a.uuid() != account.uuid()
        });
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
