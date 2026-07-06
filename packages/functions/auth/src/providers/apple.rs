//! Sign in with Apple domain helpers.

use crate::config::apple::{AppleConfig, APPLE_AUDIENCE};
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

const MAX_IAT_FUTURE_SKEW_SECONDS: i64 = 60;
const APPLE_JWKS_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

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
    let issuer_url = issuer_url
        .unwrap_or("https://localhost")
        .trim_end_matches('/');
    format!("{issuer_url}/apple/callback")
}

#[derive(Debug, Clone, Copy)]
pub struct AppleCodeExchangeInput<'a> {
    pub code: &'a str,
    pub redirect_uri: &'a str,
    pub code_verifier: &'a str,
    pub client_secret: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppleTokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub id_token: String,
}

#[async_trait]
pub trait AppleOidcClient: Send + Sync {
    async fn exchange_code(
        &self,
        config: &AppleConfig,
        input: AppleCodeExchangeInput<'_>,
    ) -> Result<AppleTokenResponse, AppleOidcError>;

    async fn fetch_jwks(&self, config: &AppleConfig) -> Result<AppleJwks, AppleOidcError>;

    async fn refresh_jwks(&self, config: &AppleConfig) -> Result<AppleJwks, AppleOidcError> {
        self.fetch_jwks(config).await
    }
}

#[derive(Clone)]
pub struct ReqwestAppleOidcClient {
    client: reqwest::Client,
    jwks_cache: Arc<RwLock<Option<CachedAppleJwks>>>,
}

#[derive(Clone)]
struct CachedAppleJwks {
    jwks: AppleJwks,
    expires_at: Instant,
}

impl ReqwestAppleOidcClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            jwks_cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn fetch_remote_jwks(&self, config: &AppleConfig) -> Result<AppleJwks, AppleOidcError> {
        let response = self
            .client
            .get(config.jwks_uri.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| AppleOidcError::JwksFetch)?;

        if !response.status().is_success() {
            return Err(AppleOidcError::JwksFetch);
        }

        response
            .json::<AppleJwks>()
            .await
            .map_err(|_| AppleOidcError::JwksFetch)
    }
}

impl Default for ReqwestAppleOidcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AppleOidcClient for ReqwestAppleOidcClient {
    async fn exchange_code(
        &self,
        config: &AppleConfig,
        input: AppleCodeExchangeInput<'_>,
    ) -> Result<AppleTokenResponse, AppleOidcError> {
        #[derive(Deserialize)]
        struct RawAppleTokenResponse {
            access_token: Option<String>,
            token_type: Option<String>,
            expires_in: Option<u64>,
            refresh_token: Option<String>,
            id_token: Option<String>,
        }

        let response = self
            .client
            .post(config.token_url.clone())
            .header("Accept", "application/json")
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", input.code),
                ("redirect_uri", input.redirect_uri),
                ("client_id", config.client_id.as_str()),
                ("client_secret", input.client_secret),
                ("code_verifier", input.code_verifier),
            ])
            .send()
            .await
            .map_err(|_| AppleOidcError::TokenExchange)?;

        if !response.status().is_success() {
            return Err(AppleOidcError::TokenExchange);
        }

        let raw = response
            .json::<RawAppleTokenResponse>()
            .await
            .map_err(|_| AppleOidcError::TokenExchange)?;
        let id_token = raw.id_token.ok_or(AppleOidcError::MissingIdToken)?;

        Ok(AppleTokenResponse {
            access_token: raw.access_token,
            token_type: raw.token_type,
            expires_in: raw.expires_in,
            refresh_token: raw.refresh_token,
            id_token,
        })
    }

    async fn fetch_jwks(&self, config: &AppleConfig) -> Result<AppleJwks, AppleOidcError> {
        let now = Instant::now();
        if let Some(cached) = self.jwks_cache.read().await.as_ref() {
            if cached.expires_at > now {
                return Ok(cached.jwks.clone());
            }
        }

        self.refresh_jwks(config).await
    }

    async fn refresh_jwks(&self, config: &AppleConfig) -> Result<AppleJwks, AppleOidcError> {
        let jwks = self.fetch_remote_jwks(config).await?;
        *self.jwks_cache.write().await = Some(CachedAppleJwks {
            jwks: jwks.clone(),
            expires_at: Instant::now() + APPLE_JWKS_CACHE_TTL,
        });
        Ok(jwks)
    }
}

pub fn apple_identity_digest(secret: &[u8], issuer: &str, subject: &str) -> String {
    lookup_digest(
        secret,
        LookupFamily::AppleIdentity,
        &format!("{issuer}\n{subject}"),
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppleJwks {
    pub keys: Vec<AppleJwk>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppleJwk {
    pub kty: String,
    pub kid: Option<String>,
    #[serde(rename = "use")]
    pub use_: Option<String>,
    pub alg: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct AppleIdTokenValidation<'a> {
    pub issuer: &'a str,
    pub client_id: &'a str,
    pub nonce: &'a str,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleIdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: AppleAudience,
    pub exp: i64,
    pub iat: i64,
    pub nonce: Option<String>,
    pub email: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    pub email_verified: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    pub is_private_email: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AppleAudience {
    Single(String),
    Multiple(Vec<String>),
}

impl AppleAudience {
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Self::Single(audience) => audience == value,
            Self::Multiple(audiences) => audiences.iter().any(|audience| audience == value),
        }
    }
}

#[derive(Debug, Error)]
pub enum AppleOidcError {
    #[error("Apple code exchange failed")]
    TokenExchange,

    #[error("Apple token response is missing ID token")]
    MissingIdToken,

    #[error("Apple JWKS fetch failed")]
    JwksFetch,

    #[error("invalid Apple ID token header")]
    InvalidHeader,

    #[error("unsupported Apple ID token algorithm")]
    UnsupportedAlgorithm,

    #[error("no matching Apple JWKS key")]
    MissingKey,

    #[error("invalid Apple JWKS key")]
    InvalidKey,

    #[error("invalid Apple ID token")]
    InvalidToken,

    #[error("invalid Apple issuer")]
    InvalidIssuer,

    #[error("invalid Apple audience")]
    InvalidAudience,

    #[error("expired Apple ID token")]
    Expired,

    #[error("Apple ID token issued-at is too far in the future")]
    FutureIssuedAt,

    #[error("invalid Apple provider nonce")]
    InvalidNonce,

    #[error("missing Apple subject")]
    MissingSubject,
}

pub fn validate_apple_id_token(
    token: &str,
    jwks: &AppleJwks,
    expected: AppleIdTokenValidation<'_>,
) -> Result<AppleIdTokenClaims, AppleOidcError> {
    let header = decode_header(token).map_err(|_| AppleOidcError::InvalidHeader)?;
    if header.alg != Algorithm::RS256 {
        return Err(AppleOidcError::UnsupportedAlgorithm);
    }

    let jwk = matching_jwk(jwks, header.kid.as_deref())?;
    let n = jwk.n.as_deref().ok_or(AppleOidcError::InvalidKey)?;
    let e = jwk.e.as_deref().ok_or(AppleOidcError::InvalidKey)?;
    let key = DecodingKey::from_rsa_components(n, e).map_err(|_| AppleOidcError::InvalidKey)?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    let token_data = decode::<AppleIdTokenClaims>(token, &key, &validation)
        .map_err(|_| AppleOidcError::InvalidToken)?;
    let claims = token_data.claims;

    if claims.iss != expected.issuer {
        return Err(AppleOidcError::InvalidIssuer);
    }
    if !claims.aud.contains(expected.client_id) {
        return Err(AppleOidcError::InvalidAudience);
    }
    if claims.exp <= expected.now.timestamp() {
        return Err(AppleOidcError::Expired);
    }
    let max_iat = expected.now + ChronoDuration::seconds(MAX_IAT_FUTURE_SKEW_SECONDS);
    if claims.iat > max_iat.timestamp() {
        return Err(AppleOidcError::FutureIssuedAt);
    }
    if claims.nonce.as_deref() != Some(expected.nonce) {
        return Err(AppleOidcError::InvalidNonce);
    }
    if claims.sub.trim().is_empty() {
        return Err(AppleOidcError::MissingSubject);
    }

    Ok(claims)
}

fn matching_jwk<'a>(
    jwks: &'a AppleJwks,
    kid: Option<&str>,
) -> Result<&'a AppleJwk, AppleOidcError> {
    let kid = kid.ok_or(AppleOidcError::MissingKey)?;
    let key = jwks.keys.iter().find(|key| key.kid.as_deref() == Some(kid));

    let key = key.ok_or(AppleOidcError::MissingKey)?;
    if key.kty != "RSA" || key.alg.as_deref().unwrap_or("RS256") != "RS256" {
        return Err(AppleOidcError::UnsupportedAlgorithm);
    }
    Ok(key)
}

fn deserialize_optional_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Bool(value)) => Ok(Some(value)),
        Some(serde_json::Value::String(value)) => match value.as_str() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(serde::de::Error::custom("invalid boolean string")),
        },
        Some(_) => Err(serde::de::Error::custom("invalid boolean value")),
    }
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
