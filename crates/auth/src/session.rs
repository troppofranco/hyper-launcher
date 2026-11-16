//! Sessione di gioco: ciò che serve al comando di avvio, con il token
//! ottenuto (o rinnovato) al momento.

use std::time::{SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::msa::{DeviceCode, MsaClient};
use crate::xbox::McToken;
use crate::{Account, AuthError, Secret, SecretStore};

/// Il token Minecraft in cache viene riusato solo se vale ancora almeno così.
const MIN_REMAINING_SECS: u64 = 5 * 60;

/// Identità e token da passare al gioco.
#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub uuid: Uuid,
    pub access_token: Secret,
    /// `false` per gli account offline, il cui token è un segnaposto.
    pub token_is_secret: bool,
    /// Valore di `${user_type}`: le versioni recenti accettano `msa` anche offline.
    pub user_type: &'static str,
}

fn refresh_key(uuid: Uuid) -> String {
    format!("msa-refresh:{uuid}")
}

fn mc_token_key(uuid: Uuid) -> String {
    format!("mc-token:{uuid}")
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Il token Minecraft è salvato come `<scadenza unix>:<token>`.
fn save_mc_token(store: &dyn SecretStore, uuid: Uuid, token: &McToken) -> Result<(), AuthError> {
    let expires = now() + token.expires_in;
    let value = Secret::new(format!("{expires}:{}", token.access_token.expose()));
    store.set(&mc_token_key(uuid), &value)
}

fn cached_mc_token(store: &dyn SecretStore, uuid: Uuid) -> Result<Option<Secret>, AuthError> {
    let Some(value) = store.get(&mc_token_key(uuid))? else {
        return Ok(None);
    };
    let Some((expires, token)) = value.expose().split_once(':') else {
        return Ok(None);
    };
    let valid = expires
        .parse::<u64>()
        .is_ok_and(|e| e > now() + MIN_REMAINING_SECS);
    Ok(valid.then(|| Secret::new(token)))
}

/// Completa il login avviato con [`MsaClient::start_device_login`]: attende
/// la conferma dell'utente, ottiene il token Minecraft, verifica il possesso
/// del gioco e salva i token nello store. Restituisce l'account da
/// aggiungere ad `accounts.toml`.
pub async fn login_microsoft(
    msa: &MsaClient,
    store: &dyn SecretStore,
    code: &DeviceCode,
) -> Result<Account, AuthError> {
    let tokens = msa.wait_for_device_login(code).await?;
    let mc = msa.minecraft_login(&tokens.access_token).await?;
    let profile = msa.profile(&mc.access_token).await?;
    store.set(&refresh_key(profile.uuid), &tokens.refresh_token)?;
    save_mc_token(store, profile.uuid, &mc)?;
    Ok(Account::Microsoft {
        name: profile.name,
        uuid: profile.uuid,
    })
}

/// Sessione per l'avvio. Per gli account Microsoft riusa il token in cache
/// se valido, altrimenti lo rinnova dal refresh token (salvando quello
/// nuovo, perché Microsoft lo ruota). `msa` serve solo se va rinnovato.
pub async fn session(
    account: &Account,
    store: &dyn SecretStore,
    msa: Option<&MsaClient>,
) -> Result<Session, AuthError> {
    let (name, uuid) = match account {
        Account::Offline { name, uuid } => {
            return Ok(Session {
                name: name.clone(),
                uuid: *uuid,
                access_token: Secret::new("0"),
                token_is_secret: false,
                user_type: "msa",
            });
        }
        Account::Microsoft { name, uuid } => (name, *uuid),
    };

    let session = |name: String, access_token: Secret| Session {
        name,
        uuid,
        access_token,
        token_is_secret: true,
        user_type: "msa",
    };

    if let Some(token) = cached_mc_token(store, uuid)? {
        return Ok(session(name.clone(), token));
    }

    let msa = msa.ok_or(AuthError::MissingClientId)?;
    let refresh = store
        .get(&refresh_key(uuid))?
        .ok_or_else(|| AuthError::LoginExpired)?;
    let tokens = msa.refresh(&refresh).await?;
    store.set(&refresh_key(uuid), &tokens.refresh_token)?;
    let mc = msa.minecraft_login(&tokens.access_token).await?;
    // Verifica che il gioco sia ancora posseduto, che il token appartenga
    // proprio a questo account, e prende il nome attuale.
    let profile = msa.profile(&mc.access_token).await?;
    if profile.uuid != uuid {
        return Err(AuthError::AccountMismatch);
    }
    save_mc_token(store, uuid, &mc)?;
    Ok(session(profile.name, mc.access_token))
}

/// Cancella dallo store tutti i segreti dell'account.
pub fn forget(account: &Account, store: &dyn SecretStore) -> Result<(), AuthError> {
    if let Account::Microsoft { uuid, .. } = account {
        store.delete(&refresh_key(*uuid))?;
        store.delete(&mc_token_key(*uuid))?;
    }
    Ok(())
}
