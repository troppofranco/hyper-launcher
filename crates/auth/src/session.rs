//! Sessione di gioco: ciò che serve al comando di avvio, con il token
//! ottenuto (o rinnovato) al momento.

use hyper_core::unix_now;
use uuid::Uuid;

use crate::msa::{DeviceCode, MsTokens, MsaClient};
use crate::xbox::{McProfile, McToken};
use crate::{Account, AuthError, Secret, SecretStore};

/// Il token Minecraft in cache viene riusato solo se vale ancora almeno così.
const MIN_REMAINING_SECS: u64 = 5 * 60;

/// Identità e token da passare al gioco.
#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub uuid: Uuid,
    /// Token Minecraft; `None` per gli account offline, a cui il gioco
    /// passa il segnaposto `0` (che non è un segreto).
    pub access_token: Option<Secret>,
}

impl Session {
    /// Sessione di un account offline: nessun token, nessuna rete.
    pub fn offline(name: &str, uuid: Uuid) -> Self {
        Self {
            name: name.to_owned(),
            uuid,
            access_token: None,
        }
    }
}

fn refresh_key(uuid: Uuid) -> String {
    format!("msa-refresh:{uuid}")
}

fn mc_token_key(uuid: Uuid) -> String {
    format!("mc-token:{uuid}")
}

/// Il token Minecraft è salvato come `<scadenza unix>:<token>`.
fn save_mc_token(store: &dyn SecretStore, uuid: Uuid, token: &McToken) -> Result<(), AuthError> {
    let expires = unix_now() + token.expires_in;
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
        .is_ok_and(|e| e > unix_now() + MIN_REMAINING_SECS);
    Ok(valid.then(|| Secret::new(token)))
}

/// Dai token Microsoft al token Minecraft, con il profilo che ne verifica il
/// possesso del gioco. Usato sia al login sia al rinnovo.
async fn minecraft_session(
    msa: &MsaClient,
    tokens: &MsTokens,
) -> Result<(McProfile, McToken), AuthError> {
    let mc = msa.minecraft_login(&tokens.access_token).await?;
    let profile = msa.profile(&mc.access_token).await?;
    Ok((profile, mc))
}

/// Completa il login avviato con [`MsaClient::start_device_login`]: attende
/// la conferma dell'utente, ottiene il token Minecraft, verifica il possesso
/// del gioco e salva i token nello store. Restituisce l'account da
/// aggiungere con [`crate::AccountStore::add_and_save`].
pub async fn login_microsoft(
    msa: &MsaClient,
    store: &dyn SecretStore,
    code: &DeviceCode,
) -> Result<Account, AuthError> {
    let tokens = msa.wait_for_device_login(code).await?;
    // L'UUID (chiave dei segreti) si conosce solo dal profilo: i token si
    // salvano dopo, e solo per un account che possiede il gioco.
    let (profile, mc) = minecraft_session(msa, &tokens).await?;
    store.set(&refresh_key(profile.uuid), &tokens.refresh_token)?;
    save_mc_token(store, profile.uuid, &mc)?;
    Ok(Account::Microsoft {
        name: profile.name,
        uuid: profile.uuid,
    })
}

/// Sessione per l'avvio. Per gli account Microsoft riusa il token in cache
/// se valido, altrimenti lo rinnova dal refresh token. `msa` serve solo se
/// va rinnovato.
pub async fn session(
    account: &Account,
    store: &dyn SecretStore,
    msa: Option<&MsaClient>,
) -> Result<Session, AuthError> {
    let (name, uuid) = match account {
        Account::Offline { name, uuid } => return Ok(Session::offline(name, *uuid)),
        Account::Microsoft { name, uuid } => (name, *uuid),
    };

    if let Some(token) = cached_mc_token(store, uuid)? {
        return Ok(Session {
            name: name.clone(),
            uuid,
            access_token: Some(token),
        });
    }

    let msa = msa.ok_or(AuthError::MissingClientId)?;
    let refresh = store
        .get(&refresh_key(uuid))?
        .ok_or(AuthError::LoginExpired)?;
    let tokens = msa.refresh(&refresh).await?;
    // Microsoft ha già invalidato il vecchio refresh token: quello nuovo va
    // salvato subito, anche se i passi successivi falliscono.
    store.set(&refresh_key(uuid), &tokens.refresh_token)?;
    let (profile, mc) = minecraft_session(msa, &tokens).await?;
    if profile.uuid != uuid {
        return Err(AuthError::AccountMismatch);
    }
    save_mc_token(store, uuid, &mc)?;
    Ok(Session {
        name: profile.name,
        uuid,
        access_token: Some(mc.access_token),
    })
}

/// Cancella dallo store tutti i segreti dell'account.
pub fn forget(account: &Account, store: &dyn SecretStore) -> Result<(), AuthError> {
    if let Account::Microsoft { uuid, .. } = account {
        store.delete(&refresh_key(*uuid))?;
        store.delete(&mc_token_key(*uuid))?;
    }
    Ok(())
}
