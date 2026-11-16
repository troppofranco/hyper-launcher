//! Dal token Microsoft al token Minecraft: Xbox Live (XBL) → XSTS →
//! `login_with_xbox`, poi verifica del profilo (= possesso del gioco).

use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::msa::{MsaClient, json, send};
use crate::{AuthError, Secret};

/// Token Minecraft pronto per il gioco.
#[derive(Debug)]
pub(crate) struct McToken {
    pub access_token: Secret,
    /// Secondi di validità.
    pub expires_in: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct McProfile {
    pub uuid: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxToken {
    token: Secret,
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<UserClaims>,
}

#[derive(Debug, Deserialize)]
struct UserClaims {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct XstsError {
    #[serde(rename = "XErr")]
    xerr: u64,
}

#[derive(Debug, Deserialize)]
struct ProfileResponse {
    id: String,
    name: String,
}

impl MsaClient {
    /// Catena completa dal token di accesso Microsoft al token Minecraft.
    pub(crate) async fn minecraft_login(
        &self,
        ms_access_token: &Secret,
    ) -> Result<McToken, AuthError> {
        let xbl = self.xbox_live(ms_access_token).await?;
        let xsts = self.xsts(&xbl.token).await?;
        let uhs = &xsts
            .display_claims
            .xui
            .first()
            .ok_or(AuthError::MalformedResponse("XSTS without user claims"))?
            .uhs;

        let url = format!(
            "{}/authentication/login_with_xbox",
            self.endpoints.minecraft
        );
        let identity = format!("XBL3.0 x={uhs};{}", xsts.token.expose());
        let body = json!({ "identityToken": identity });
        let response = send(&url, self.http().post(&url).json(&body)).await?;
        match response.status().as_u16() {
            200..=299 => {
                #[derive(Deserialize)]
                struct Login {
                    access_token: Secret,
                    expires_in: u64,
                }
                let login: Login = json(&url, response).await?;
                Ok(McToken {
                    access_token: login.access_token,
                    expires_in: login.expires_in,
                })
            }
            // Mojang rifiuta le app Azure non approvate per la Minecraft API.
            403 => Err(AuthError::AppNotApproved),
            status => Err(AuthError::UnexpectedStatus { url, status }),
        }
    }

    /// Profilo Minecraft dell'account; 404 = nessun profilo Java.
    pub(crate) async fn profile(&self, mc_token: &Secret) -> Result<McProfile, AuthError> {
        let url = format!("{}/minecraft/profile", self.endpoints.minecraft);
        let request = self.http().get(&url).bearer_auth(mc_token.expose());
        let response = send(&url, request).await?;
        match response.status().as_u16() {
            200..=299 => {
                let body: ProfileResponse = json(&url, response).await?;
                let uuid = Uuid::parse_str(&body.id)
                    .map_err(|_| AuthError::MalformedResponse("profile id is not a UUID"))?;
                Ok(McProfile {
                    uuid,
                    name: body.name,
                })
            }
            404 => Err(AuthError::GameNotOwned),
            status => Err(AuthError::UnexpectedStatus { url, status }),
        }
    }

    async fn xbox_live(&self, ms_access_token: &Secret) -> Result<XboxToken, AuthError> {
        let url = format!("{}/user/authenticate", self.endpoints.xbox_user);
        let body = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_access_token.expose()),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        self.xbox_post(&url, &body).await
    }

    async fn xsts(&self, xbl_token: &Secret) -> Result<XboxToken, AuthError> {
        let url = format!("{}/xsts/authorize", self.endpoints.xsts);
        let body = json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token.expose()] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        });
        self.xbox_post(&url, &body).await
    }

    /// POST JSON ai servizi Xbox; un rifiuto con codice `XErr` diventa
    /// l'errore che spiega il motivo.
    async fn xbox_post(&self, url: &str, body: &Value) -> Result<XboxToken, AuthError> {
        let response = send(url, self.http().post(url).json(body)).await?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return json(url, response).await;
        }
        match response.json::<XstsError>().await.map(|e| e.xerr) {
            Ok(2148916233) => Err(AuthError::NoXboxAccount),
            Ok(2148916235) => Err(AuthError::XboxUnavailableInCountry),
            Ok(2148916236 | 2148916237) => Err(AuthError::AdultVerificationRequired),
            Ok(2148916238) => Err(AuthError::ChildAccount),
            Ok(code) => Err(AuthError::Xsts(code)),
            Err(_) => Err(AuthError::UnexpectedStatus {
                url: url.to_owned(),
                status,
            }),
        }
    }
}
