use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{distributions::Uniform, Rng};
use reqwest::{redirect::Policy, Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
use url::Url;

const DEFAULT_ISSUER_URL: &str = "https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com";
const DEFAULT_CLIENT_ID: &str = "app";
const DEFAULT_SCOPE: &str = "openid email offline_access";
const CALLBACK_PATH: &str = "/oauth/callback";
const KEYCHAIN_SERVICE: &str = "irongate-example-app";
const KEYCHAIN_ACCOUNT: &str = "refresh-token";
const LOGIN_TIMEOUT_SECONDS: u64 = 180;

#[derive(Debug, Serialize)]
pub struct AppSession {
    pub token_type: String,
    pub expires_in: u64,
    pub scope: Option<String>,
    pub access_token: String,
    pub id_token: Option<String>,
    pub userinfo: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct StoredSessionStatus {
    pub has_refresh_token: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
    id_token: Option<String>,
    scope: Option<String>,
}

#[derive(Clone)]
struct AuthConfig {
    issuer_url: String,
    client_id: String,
    scope: String,
}

impl AuthConfig {
    fn from_env() -> Self {
        Self {
            issuer_url: std::env::var("IRONGATE_ISSUER_URL")
                .unwrap_or_else(|_| DEFAULT_ISSUER_URL.to_string()),
            client_id: std::env::var("IRONGATE_APP_CLIENT_ID")
                .unwrap_or_else(|_| DEFAULT_CLIENT_ID.to_string()),
            scope: std::env::var("IRONGATE_APP_SCOPE")
                .unwrap_or_else(|_| DEFAULT_SCOPE.to_string()),
        }
    }
}

#[tauri::command]
pub async fn login_with_provider(provider: String) -> Result<AppSession, String> {
    if provider != "google" && provider != "apple" {
        return Err("provider must be google or apple".to_string());
    }

    let config = AuthConfig::from_env();
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("failed to bind loopback callback: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("failed to read loopback address: {err}"))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let state = random_url_safe(32);
    let pkce = PkcePair::new();
    let authorize_url = build_authorize_url(
        &config,
        &redirect_uri,
        &state,
        &pkce.challenge,
        &provider,
    )?;

    open::that_detached(authorize_url.as_str())
        .map_err(|err| format!("failed to open system browser: {err}"))?;

    let expected_state = state.clone();
    let callback = tauri::async_runtime::spawn_blocking(move || {
        wait_for_loopback_callback(listener, &expected_state)
    })
    .await
    .map_err(|err| format!("loopback listener task failed: {err}"))??;

    exchange_code_and_build_session(&config, &redirect_uri, &callback.code, &pkce.verifier).await
}

#[tauri::command]
pub async fn login_with_password(email: String, password: String) -> Result<AppSession, String> {
    let config = AuthConfig::from_env();
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("failed to reserve loopback callback: {err}"))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("failed to read loopback address: {err}"))?
        .port();
    drop(listener);

    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let state = random_url_safe(32);
    let pkce = PkcePair::new();
    let client = Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|err| format!("failed to create HTTP client: {err}"))?;
    let authorize_url = build_authorize_url(
        &config,
        &redirect_uri,
        &state,
        &pkce.challenge,
        "password",
    )?;

    let authorize_response = client
        .get(authorize_url)
        .send()
        .await
        .map_err(|err| format!("authorize request failed: {err}"))?;
    let session = redirect_location(&authorize_response)
        .and_then(|location| parse_authorize_session(location, &config.issuer_url))
        .ok_or_else(|| "authorize response did not include a password session".to_string())?;

    let login_response = client
        .post(endpoint(&config.issuer_url, "/password/login")?)
        .form(&[
            ("session", session.as_str()),
            ("email", email.as_str()),
            ("password", password.as_str()),
        ])
        .send()
        .await
        .map_err(|err| format!("password login request failed: {err}"))?;
    let location = redirect_location(&login_response)
        .ok_or_else(|| "password login did not return an authorization code".to_string())?;
    let callback = parse_callback(location, &state)?;

    exchange_code_and_build_session(&config, &redirect_uri, &callback.code, &pkce.verifier).await
}

#[tauri::command]
pub async fn refresh_session() -> Result<AppSession, String> {
    let refresh_token = load_refresh_token()?
        .ok_or_else(|| "no stored refresh token".to_string())?;
    let config = AuthConfig::from_env();
    let client = Client::new();
    let response = client
        .post(endpoint(&config.issuer_url, "/token")?)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", config.client_id.as_str()),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(|err| format!("refresh request failed: {err}"))?;

    let token = read_token_response(response).await?;
    store_refresh_token_if_present(&token)?;
    session_from_token_response(&config, token).await
}

#[tauri::command]
pub async fn logout() -> Result<(), String> {
    let config = AuthConfig::from_env();
    if let Some(refresh_token) = load_refresh_token()? {
        let client = Client::new();
        let _ = client
            .post(endpoint(&config.issuer_url, "/oauth/revoke")?)
            .form(&[
                ("token", refresh_token.as_str()),
                ("token_type_hint", "refresh_token"),
                ("client_id", config.client_id.as_str()),
            ])
            .send()
            .await;
    }

    delete_refresh_token()
}

#[tauri::command]
pub fn stored_session_status() -> Result<StoredSessionStatus, String> {
    Ok(StoredSessionStatus {
        has_refresh_token: load_refresh_token()?.is_some(),
    })
}

async fn exchange_code_and_build_session(
    config: &AuthConfig,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<AppSession, String> {
    let client = Client::new();
    let response = client
        .post(endpoint(&config.issuer_url, "/token")?)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("code", code),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|err| format!("token exchange request failed: {err}"))?;
    let token = read_token_response(response).await?;

    store_refresh_token_if_present(&token)?;
    session_from_token_response(config, token).await
}

async fn session_from_token_response(
    config: &AuthConfig,
    token: TokenResponse,
) -> Result<AppSession, String> {
    let userinfo = fetch_userinfo(config, &token.access_token).await;

    Ok(AppSession {
        token_type: token.token_type,
        expires_in: token.expires_in,
        scope: token.scope,
        access_token: token.access_token,
        id_token: token.id_token,
        userinfo,
    })
}

async fn fetch_userinfo(config: &AuthConfig, access_token: &str) -> Option<serde_json::Value> {
    let client = Client::new();
    let response = client
        .get(endpoint(&config.issuer_url, "/userinfo").ok()?)
        .bearer_auth(access_token)
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    response.json::<serde_json::Value>().await.ok()
}

async fn read_token_response(response: reqwest::Response) -> Result<TokenResponse, String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("token endpoint returned {status}: {body}"));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|err| format!("failed to decode token response: {err}"))
}

fn redirect_location(response: &reqwest::Response) -> Option<&str> {
    if !matches!(response.status(), StatusCode::FOUND | StatusCode::SEE_OTHER) {
        return None;
    }

    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
}

fn build_authorize_url(
    config: &AuthConfig,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
    provider: &str,
) -> Result<Url, String> {
    let mut url = endpoint(&config.issuer_url, "/authorize")?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &config.scope)
        .append_pair("state", state)
        .append_pair("nonce", &random_url_safe(32))
        .append_pair("provider", provider)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn endpoint(issuer_url: &str, path: &str) -> Result<Url, String> {
    Url::parse(issuer_url)
        .and_then(|base| base.join(path))
        .map_err(|err| format!("invalid Irongate issuer URL: {err}"))
}

fn parse_authorize_session(location: &str, issuer_url: &str) -> Option<String> {
    let url = Url::parse(location).or_else(|_| endpoint(issuer_url, location)).ok()?;
    let valid_path = matches!(
        url.path(),
        "/password/login" | "/google/authorize" | "/apple/authorize"
    );
    if !valid_path {
        return None;
    }

    url.query_pairs()
        .find_map(|(key, value)| (key == "session").then(|| value.into_owned()))
}

fn wait_for_loopback_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<CallbackParams, String> {
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("failed to configure loopback listener: {err}"))?;
    let started_at = Instant::now();

    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buffer = [0_u8; 8192];
                let bytes_read = stream
                    .read(&mut buffer)
                    .map_err(|err| format!("failed to read loopback callback: {err}"))?;
                let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                let callback = parse_http_callback_request(&request, expected_state);
                let response_body = if callback.is_ok() {
                    "Irongate sign in complete. You can return to the app."
                } else {
                    "Irongate sign in failed. You can return to the app."
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response.as_bytes());
                return callback;
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if started_at.elapsed() > Duration::from_secs(LOGIN_TIMEOUT_SECONDS) {
                    return Err("timed out waiting for browser callback".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(err) => return Err(format!("failed to accept loopback callback: {err}")),
        }
    }
}

fn parse_http_callback_request(
    request: &str,
    expected_state: &str,
) -> Result<CallbackParams, String> {
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "empty loopback callback request".to_string())?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "malformed loopback callback request".to_string())?;

    parse_callback(path, expected_state)
}

fn parse_callback(location: &str, expected_state: &str) -> Result<CallbackParams, String> {
    let url = Url::parse(location)
        .or_else(|_| Url::parse(&format!("http://127.0.0.1{location}")))
        .map_err(|err| format!("invalid callback URL: {err}"))?;
    if url.path() != CALLBACK_PATH {
        return Err("callback path did not match the expected loopback path".to_string());
    }

    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(error) = params.get("error") {
        return Err(format!("authorization provider returned an error: {error}"));
    }
    let state = params
        .get("state")
        .ok_or_else(|| "callback did not include state".to_string())?;
    if state != expected_state {
        return Err("callback state did not match the login request".to_string());
    }
    let code = params
        .get("code")
        .ok_or_else(|| "callback did not include an authorization code".to_string())?;

    Ok(CallbackParams {
        code: code.clone(),
        state: state.clone(),
    })
}

#[derive(Debug)]
struct CallbackParams {
    code: String,
    #[allow(dead_code)]
    state: String,
}

struct PkcePair {
    verifier: String,
    challenge: String,
}

impl PkcePair {
    fn new() -> Self {
        let verifier = random_pkce_verifier();
        let digest = Sha256::digest(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(digest);

        Self {
            verifier,
            challenge,
        }
    }
}

fn random_pkce_verifier() -> String {
    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._~-";
    random_string(64, ALPHABET)
}

fn random_url_safe(length: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    random_string(length, ALPHABET)
}

fn random_string(length: usize, alphabet: &[u8]) -> String {
    let range = Uniform::from(0..alphabet.len());
    let mut rng = rand::thread_rng();

    (0..length)
        .map(|_| alphabet[rng.sample(range)] as char)
        .collect()
}

fn store_refresh_token_if_present(token: &TokenResponse) -> Result<(), String> {
    if let Some(refresh_token) = token.refresh_token.as_deref() {
        store_refresh_token(refresh_token)?;
    }
    Ok(())
}

fn store_refresh_token(refresh_token: &str) -> Result<(), String> {
    keyring_entry()?
        .set_password(refresh_token)
        .map_err(|err| format!("failed to store refresh token in keychain: {err}"))
}

fn load_refresh_token() -> Result<Option<String>, String> {
    match keyring_entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(format!("failed to read refresh token from keychain: {err}")),
    }
}

fn delete_refresh_token() -> Result<(), String> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!("failed to delete refresh token from keychain: {err}")),
    }
}

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
        .map_err(|err| format!("failed to open keychain entry: {err}"))
}
