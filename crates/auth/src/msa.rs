//! OAuth Microsoft con device code flow: il launcher mostra un URL e un
//! codice, l'utente conferma nel browser, il launcher ottiene i token.
//! Funziona identico in CLI e UI, senza server di redirect locale.

use std::time::Duration;

use serde::Deserialize;

use crate::{AuthError, Secret};

const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// URL base dei servizi; sostituibili per i test con un server mock.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub login: String,
    pub xbox_user: String,
    pub xsts: String,
    pub minecraft: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            login: "https://login.microsoftonline.com/consumers/oauth2/v2.0".into(),
            xbox_user: "https://user.auth.xboxlive.com".into(),
            xsts: "https://xsts.auth.xboxlive.com".into(),
            minecraft: "https://api.minecraftservices.com".into(),
        }
    }
}

/// Client per l'intera catena Microsoft → Minecraft.
#[derive(Debug, Clone)]
pub struct MsaClient {
    pub(crate) http: reqwest::Client,
    client_id: String,
    pub(crate) endpoints: Endpoints,
    /// Unità dell'intervallo di polling (1 s; ridotta nei test).
    poll_unit: Duration,
}

/// Codice da mostrare all'utente.
#[derive(Debug, Deserialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    /// Secondi di validità del codice.
    pub expires_in: u64,
    interval: u64,
    device_code: Secret,
}

/// Token Microsoft (non ancora Minecraft).
#[derive(Debug, Deserialize)]
pub struct MsTokens {
    pub access_token: Secret,
    pub refresh_token: Secret,
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: String,
}

impl MsaClient {
    pub fn new(client_id: impl Into<String>, endpoints: Endpoints) -> Result<Self, AuthError> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("HyperLauncher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(AuthError::HttpClient)?;
        Ok(Self {
            http,
            client_id: client_id.into(),
            endpoints,
            poll_unit: Duration::from_secs(1),
        })
    }

    #[cfg(test)]
    pub(crate) fn with_poll_unit(mut self, unit: Duration) -> Self {
        self.poll_unit = unit;
        self
    }

    /// Primo passo del login: chiede a Microsoft un codice per l'utente.
    pub async fn start_device_login(&self) -> Result<DeviceCode, AuthError> {
        let url = format!("{}/devicecode", self.endpoints.login);
        let form = [("client_id", self.client_id.as_str()), ("scope", SCOPE)];
        let response = self
            .http
            .post(&url)
            .form(&form)
            .send()
            .await
            .map_err(http(&url))?;
        if !response.status().is_success() {
            return Err(oauth_error(&url, response).await);
        }
        response.json().await.map_err(http(&url))
    }

    /// Attende che l'utente confermi il codice nel browser, rispettando
    /// `interval` e `slow_down` come richiesto dal protocollo.
    pub async fn wait_for_device_login(&self, code: &DeviceCode) -> Result<MsTokens, AuthError> {
        let url = format!("{}/token", self.endpoints.login);
        let deadline = tokio::time::Instant::now() + self.poll_unit * secs(code.expires_in);
        let mut interval = code.interval.max(1);
        loop {
            tokio::time::sleep(self.poll_unit * secs(interval)).await;
            if tokio::time::Instant::now() >= deadline {
                return Err(AuthError::DeviceCodeExpired);
            }
            let form = [
                ("grant_type", DEVICE_CODE_GRANT),
                ("client_id", self.client_id.as_str()),
                ("device_code", code.device_code.expose()),
            ];
            // Un errore di rete transitorio non annulla il login: si riprova
            // al giro successivo finché il codice è valido.
            let response = match self.http.post(&url).form(&form).send().await {
                Ok(response) => response,
                Err(e) => {
                    tracing::warn!(error = %e, "device code poll failed, retrying");
                    continue;
                }
            };
            if response.status().is_success() {
                return response.json().await.map_err(http(&url));
            }
            match oauth_error(&url, response).await {
                AuthError::OAuth { error, .. } if error == "authorization_pending" => {}
                AuthError::OAuth { error, .. } if error == "slow_down" => interval += 5,
                AuthError::OAuth { error, .. } if error == "expired_token" => {
                    return Err(AuthError::DeviceCodeExpired);
                }
                AuthError::OAuth { error, .. }
                    if error == "authorization_declined" || error == "access_denied" =>
                {
                    return Err(AuthError::AuthorizationDeclined);
                }
                other => return Err(other),
            }
        }
    }

    /// Nuovi token dal refresh token. Microsoft lo ruota: il chiamante deve
    /// salvare quello restituito.
    pub async fn refresh(&self, refresh_token: &Secret) -> Result<MsTokens, AuthError> {
        let url = format!("{}/token", self.endpoints.login);
        let form = [
            ("grant_type", "refresh_token"),
            ("client_id", self.client_id.as_str()),
            ("refresh_token", refresh_token.expose()),
            ("scope", SCOPE),
        ];
        let response = self
            .http
            .post(&url)
            .form(&form)
            .send()
            .await
            .map_err(http(&url))?;
        if response.status().is_success() {
            return response.json().await.map_err(http(&url));
        }
        match oauth_error(&url, response).await {
            // Refresh token revocato o scaduto: serve un nuovo login.
            AuthError::OAuth { error, .. } if error == "invalid_grant" => {
                Err(AuthError::LoginExpired)
            }
            other => Err(other),
        }
    }
}

fn secs(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

pub(crate) fn http(url: &str) -> impl FnOnce(reqwest::Error) -> AuthError + '_ {
    move |source| AuthError::Http {
        url: url.to_owned(),
        source,
    }
}

async fn oauth_error(url: &str, response: reqwest::Response) -> AuthError {
    let status = response.status();
    match response.json::<OAuthError>().await {
        Ok(e) => AuthError::OAuth {
            error: e.error,
            description: e.error_description,
        },
        Err(_) => AuthError::UnexpectedStatus {
            url: url.to_owned(),
            status: status.as_u16(),
        },
    }
}
