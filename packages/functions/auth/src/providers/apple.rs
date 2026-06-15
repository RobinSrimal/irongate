//! Sign in with Apple domain helpers.

use crate::config::apple::{AppleConfig, APPLE_AUDIENCE};
use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;

pub struct AppleAuthorizeInput<'a> {
    pub config: &'a AppleConfig,
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub pkce_challenge: &'a str,
}

pub fn build_apple_authorization_url(input: AppleAuthorizeInput<'_>) -> String {
    let mut url = input.config.authorization_url.clone();
    url.query_pairs_mut()
        .append_pair("client_id", &input.config.client_id)
        .append_pair("redirect_uri", input.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("response_mode", "form_post")
        .append_pair("scope", &input.config.scopes.join(" "))
        .append_pair("state", input.state)
        .append_pair("nonce", input.nonce)
        .append_pair("code_challenge", input.pkce_challenge)
        .append_pair("code_challenge_method", "S256");
    url.into()
}

pub fn apple_callback_uri(issuer_url: Option<&str>) -> String {
    let issuer_url = issuer_url.unwrap_or("https://localhost").trim_end_matches('/');
    format!("{issuer_url}/apple/callback")
}

#[derive(Debug, Serialize)]
struct AppleClientSecretClaims<'a> {
    iss: &'a str,
    sub: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
}

pub fn generate_apple_client_secret(
    config: &AppleConfig,
    now: DateTime<Utc>,
) -> Result<String, jsonwebtoken::errors::Error> {
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(config.key_id.clone());
    let claims = AppleClientSecretClaims {
        iss: &config.team_id,
        sub: &config.client_id,
        aud: APPLE_AUDIENCE,
        iat: now.timestamp(),
        exp: now.timestamp() + config.client_secret_ttl_seconds as i64,
    };
    let key = EncodingKey::from_ec_pem(config.private_key.expose().as_bytes())?;
    encode(&header, &claims, &key)
}
