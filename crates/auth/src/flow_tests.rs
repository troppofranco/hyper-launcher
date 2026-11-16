//! Test dell'intera catena di login contro un server mock locale che
//! simula Microsoft, Xbox Live, XSTS e i servizi Minecraft.

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::{Account, AuthError, Endpoints, MemoryStore, MsaClient, Secret, SecretStore};

const UUID_SIMPLE: &str = "069a79f444e94726a5befca90e38aaf5";

/// Predicato sul tipo di errore atteso.
type ErrorCheck = fn(&AuthError) -> bool;

async fn client(server: &MockServer) -> MsaClient {
    let uri = server.uri();
    let endpoints = Endpoints {
        login: format!("{uri}/login"),
        xbox_user: format!("{uri}/xbl"),
        xsts: format!("{uri}/xsts"),
        minecraft: format!("{uri}/mc"),
    };
    MsaClient::new("test-client", endpoints)
        .unwrap()
        .with_poll_unit(Duration::from_millis(5))
}

fn ok(body: serde_json::Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

fn oauth_error(error: &str) -> ResponseTemplate {
    ResponseTemplate::new(400).set_body_json(json!({ "error": error, "error_description": "x" }))
}

async fn mount_device_code(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/login/devicecode"))
        .and(body_string_contains("client_id=test-client"))
        .respond_with(ok(json!({
            "user_code": "ABCD-1234",
            "device_code": "device-secret",
            "verification_uri": "https://microsoft.com/link",
            "expires_in": 900,
            "interval": 1,
        })))
        .mount(server)
        .await;
}

async fn mount_token_success(server: &MockServer, refresh: &str) {
    Mock::given(method("POST"))
        .and(path("/login/token"))
        .respond_with(ok(json!({
            "access_token": "ms-access",
            "refresh_token": refresh,
            "expires_in": 3600,
        })))
        .mount(server)
        .await;
}

/// XBL, XSTS e login_with_xbox riusciti, profilo che possiede il gioco.
async fn mount_xbox_chain(server: &MockServer, mc_token: &str) {
    let xbox = |token: &str| {
        ok(json!({ "Token": token, "DisplayClaims": { "xui": [{ "uhs": "user-hash" }] } }))
    };
    Mock::given(method("POST"))
        .and(path("/xbl/user/authenticate"))
        .and(body_string_contains("d=ms-access"))
        .respond_with(xbox("xbl-token"))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xsts/xsts/authorize"))
        .and(body_string_contains("xbl-token"))
        .respond_with(xbox("xsts-token"))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mc/authentication/login_with_xbox"))
        .and(body_string_contains("XBL3.0 x=user-hash;xsts-token"))
        .respond_with(ok(json!({ "access_token": mc_token, "expires_in": 86400 })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mc/minecraft/profile"))
        .and(header(
            "authorization",
            format!("Bearer {mc_token}").as_str(),
        ))
        .respond_with(ok(
            json!({ "id": UUID_SIMPLE, "name": "Notch", "skins": [] }),
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn full_device_login_stores_only_secrets_in_store() {
    let server = MockServer::start().await;
    mount_device_code(&server).await;
    // Prima "in attesa", poi "rallenta", poi successo.
    Mock::given(method("POST"))
        .and(path("/login/token"))
        .respond_with(oauth_error("authorization_pending"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/login/token"))
        .respond_with(oauth_error("slow_down"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_token_success(&server, "refresh-1").await;
    mount_xbox_chain(&server, "mc-token-1").await;

    let msa = client(&server).await;
    let store = MemoryStore::default();
    let code = msa.start_device_login().await.unwrap();
    assert_eq!(code.user_code, "ABCD-1234");
    assert!(!format!("{code:?}").contains("device-secret"));

    let account = crate::login_microsoft(&msa, &store, &code).await.unwrap();
    let uuid = Uuid::parse_str(UUID_SIMPLE).unwrap();
    assert_eq!(
        account,
        Account::Microsoft {
            name: "Notch".into(),
            uuid
        }
    );
    assert_eq!(
        store
            .get(&format!("msa-refresh:{uuid}"))
            .unwrap()
            .unwrap()
            .expose(),
        "refresh-1"
    );
    assert!(
        store
            .get(&format!("mc-token:{uuid}"))
            .unwrap()
            .unwrap()
            .expose()
            .ends_with(":mc-token-1")
    );

    // accounts.toml non contiene token.
    let mut accounts = crate::AccountStore::default();
    accounts.upsert(account);
    let toml = toml::to_string(&accounts).unwrap();
    for secret in ["refresh-1", "mc-token-1", "ms-access", "xsts-token"] {
        assert!(!toml.contains(secret), "{toml}");
    }
}

#[tokio::test]
async fn declined_and_expired_device_codes() {
    let cases: [(&str, ErrorCheck); 3] = [
        ("authorization_declined", |e| {
            matches!(e, AuthError::AuthorizationDeclined)
        }),
        ("access_denied", |e| {
            matches!(e, AuthError::AuthorizationDeclined)
        }),
        ("expired_token", |e| {
            matches!(e, AuthError::DeviceCodeExpired)
        }),
    ];
    for (error, expected) in cases {
        let server = MockServer::start().await;
        mount_device_code(&server).await;
        Mock::given(method("POST"))
            .and(path("/login/token"))
            .respond_with(oauth_error(error))
            .mount(&server)
            .await;
        let msa = client(&server).await;
        let code = msa.start_device_login().await.unwrap();
        let err = msa.wait_for_device_login(&code).await.unwrap_err();
        assert!(expected(&err), "{error}: {err:?}");
    }
}

#[tokio::test]
async fn xsts_errors_are_mapped() {
    let cases: [(u64, ErrorCheck); 4] = [
        (2148916233, |e| matches!(e, AuthError::NoXboxAccount)),
        (2148916238, |e| matches!(e, AuthError::ChildAccount)),
        (2148916235, |e| {
            matches!(e, AuthError::XboxUnavailableInCountry)
        }),
        (1, |e| matches!(e, AuthError::Xsts(1))),
    ];
    for (xerr, expected) in cases {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xbl/user/authenticate"))
            .respond_with(ok(
                json!({ "Token": "xbl", "DisplayClaims": { "xui": [{ "uhs": "h" }] } }),
            ))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/xsts/xsts/authorize"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "XErr": xerr })))
            .mount(&server)
            .await;
        let err = client(&server)
            .await
            .minecraft_login(&Secret::new("ms"))
            .await
            .unwrap_err();
        assert!(expected(&err), "XErr {xerr}: {err:?}");
    }
}

#[tokio::test]
async fn unapproved_app_and_missing_game() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xbl/user/authenticate"))
        .respond_with(ok(
            json!({ "Token": "xbl", "DisplayClaims": { "xui": [{ "uhs": "h" }] } }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xsts/xsts/authorize"))
        .respond_with(ok(
            json!({ "Token": "xsts", "DisplayClaims": { "xui": [{ "uhs": "h" }] } }),
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mc/authentication/login_with_xbox"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "errorMessage": "Invalid app registration, see https://aka.ms/AppRegInfo for more information"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mc/minecraft/profile"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let msa = client(&server).await;
    assert!(matches!(
        msa.minecraft_login(&Secret::new("ms")).await,
        Err(AuthError::AppNotApproved)
    ));
    assert!(matches!(
        msa.profile(&Secret::new("mc")).await,
        Err(AuthError::GameNotOwned)
    ));
}

#[tokio::test]
async fn session_uses_cache_then_refreshes_and_rotates() {
    let uuid = Uuid::parse_str(UUID_SIMPLE).unwrap();
    let account = Account::Microsoft {
        name: "Notch".into(),
        uuid,
    };
    let store = MemoryStore::default();

    // Token in cache valido: nessuna rete (msa = None).
    let far = hyper_core::unix_now() + 86_400;
    store
        .set(
            &format!("mc-token:{uuid}"),
            &Secret::new(format!("{far}:cached")),
        )
        .unwrap();
    let session = crate::session(&account, &store, None).await.unwrap();
    assert_eq!(session.access_token.unwrap().expose(), "cached");

    // Token scaduto: refresh con rotazione del refresh token.
    store
        .set(&format!("mc-token:{uuid}"), &Secret::new("1:expired"))
        .unwrap();
    store
        .set(&format!("msa-refresh:{uuid}"), &Secret::new("refresh-old"))
        .unwrap();
    assert!(matches!(
        crate::session(&account, &store, None).await,
        Err(AuthError::MissingClientId)
    ));

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/token"))
        .and(body_string_contains("refresh_token=refresh-old"))
        .respond_with(ok(
            json!({ "access_token": "ms-access", "refresh_token": "refresh-new" }),
        ))
        .mount(&server)
        .await;
    mount_xbox_chain(&server, "mc-fresh").await;
    let msa = client(&server).await;
    let session = crate::session(&account, &store, Some(&msa)).await.unwrap();
    assert_eq!(session.access_token.unwrap().expose(), "mc-fresh");
    assert_eq!(
        store
            .get(&format!("msa-refresh:{uuid}"))
            .unwrap()
            .unwrap()
            .expose(),
        "refresh-new"
    );

    // forget cancella tutto.
    crate::forget(&account, &store).unwrap();
    assert!(store.keys().is_empty());
}

#[tokio::test]
async fn revoked_refresh_token_requires_new_login() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login/token"))
        .respond_with(oauth_error("invalid_grant"))
        .mount(&server)
        .await;
    let msa = client(&server).await;
    assert!(matches!(
        msa.refresh(&Secret::new("revoked")).await,
        Err(AuthError::LoginExpired)
    ));
}

#[tokio::test]
async fn offline_session_is_not_secret() {
    let account = Account::offline("Dev").unwrap();
    let session = crate::session(&account, &MemoryStore::default(), None)
        .await
        .unwrap();
    assert!(session.access_token.is_none());
    assert_eq!(session.uuid, account.uuid());
}

#[test]
fn replacing_microsoft_account_forgets_its_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("accounts.toml");
    let store = MemoryStore::default();
    let uuid = Uuid::parse_str(UUID_SIMPLE).unwrap();
    let microsoft = Account::Microsoft {
        name: "Notch".into(),
        uuid,
    };
    store
        .set(&format!("msa-refresh:{uuid}"), &Secret::new("refresh"))
        .unwrap();

    let mut accounts = crate::AccountStore::default();
    accounts
        .add_and_save(&path, microsoft.clone(), &store)
        .unwrap();
    // Un nuovo login dello stesso account non cancella i suoi token.
    accounts.add_and_save(&path, microsoft, &store).unwrap();
    assert_eq!(store.keys().len(), 1);

    // Un account offline con lo stesso nome lo sostituisce: token cancellati.
    accounts
        .add_and_save(&path, Account::offline("Notch").unwrap(), &store)
        .unwrap();
    assert!(store.keys().is_empty());
    assert_eq!(accounts.accounts().len(), 1);
    assert_eq!(accounts.accounts()[0].kind(), "offline");
}
