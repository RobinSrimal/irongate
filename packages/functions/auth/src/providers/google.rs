//! Google OIDC domain helpers.

use crate::config::google::GoogleConfig;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_IAT_FUTURE_SKEW_SECONDS: i64 = 60;

pub struct GoogleAuthorizeInput<'a> {
    pub config: &'a GoogleConfig,
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub pkce_challenge: &'a str,
}

pub fn build_google_authorization_url(input: GoogleAuthorizeInput<'_>) -> String {
    let mut url = input.config.authorization_url.clone();
    url.query_pairs_mut()
        .append_pair("client_id", &input.config.client_id)
        .append_pair("redirect_uri", input.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &input.config.scopes.join(" "))
        .append_pair("state", input.state)
        .append_pair("nonce", input.nonce)
        .append_pair("code_challenge", input.pkce_challenge)
        .append_pair("code_challenge_method", "S256");
    url.into()
}

pub fn google_callback_uri(issuer_url: Option<&str>) -> String {
    let issuer_url = issuer_url.unwrap_or("https://localhost").trim_end_matches('/');
    format!("{issuer_url}/google/callback")
}

#[derive(Debug, Clone, Copy)]
pub struct GoogleCodeExchangeInput<'a> {
    pub code: &'a str,
    pub redirect_uri: &'a str,
    pub code_verifier: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: Option<String>,
    pub token_type: Option<String>,
    pub expires_in: Option<u64>,
    pub id_token: String,
}

#[async_trait]
pub trait GoogleOidcClient: Send + Sync {
    async fn exchange_code(
        &self,
        config: &GoogleConfig,
        input: GoogleCodeExchangeInput<'_>,
    ) -> Result<GoogleTokenResponse, GoogleOidcError>;

    async fn fetch_jwks(&self, config: &GoogleConfig) -> Result<GoogleJwks, GoogleOidcError>;
}

#[derive(Clone)]
pub struct ReqwestGoogleOidcClient {
    client: reqwest::Client,
}

impl ReqwestGoogleOidcClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestGoogleOidcClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GoogleOidcClient for ReqwestGoogleOidcClient {
    async fn exchange_code(
        &self,
        config: &GoogleConfig,
        input: GoogleCodeExchangeInput<'_>,
    ) -> Result<GoogleTokenResponse, GoogleOidcError> {
        #[derive(Deserialize)]
        struct RawGoogleTokenResponse {
            access_token: Option<String>,
            token_type: Option<String>,
            expires_in: Option<u64>,
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
                ("client_secret", config.client_secret.expose()),
                ("code_verifier", input.code_verifier),
            ])
            .send()
            .await
            .map_err(|_| GoogleOidcError::TokenExchange)?;

        if !response.status().is_success() {
            return Err(GoogleOidcError::TokenExchange);
        }

        let raw = response
            .json::<RawGoogleTokenResponse>()
            .await
            .map_err(|_| GoogleOidcError::TokenExchange)?;
        let id_token = raw.id_token.ok_or(GoogleOidcError::MissingIdToken)?;

        Ok(GoogleTokenResponse {
            access_token: raw.access_token,
            token_type: raw.token_type,
            expires_in: raw.expires_in,
            id_token,
        })
    }

    async fn fetch_jwks(&self, config: &GoogleConfig) -> Result<GoogleJwks, GoogleOidcError> {
        let response = self
            .client
            .get(config.jwks_uri.clone())
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| GoogleOidcError::JwksFetch)?;

        if !response.status().is_success() {
            return Err(GoogleOidcError::JwksFetch);
        }

        response
            .json::<GoogleJwks>()
            .await
            .map_err(|_| GoogleOidcError::JwksFetch)
    }
}

pub fn google_identity_digest(secret: &[u8], issuer: &str, subject: &str) -> String {
    lookup_digest(
        secret,
        LookupFamily::GoogleIdentity,
        &format!("{issuer}\n{subject}"),
    )
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleJwks {
    pub keys: Vec<GoogleJwk>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleJwk {
    pub kty: String,
    pub kid: Option<String>,
    #[serde(rename = "use")]
    pub use_: Option<String>,
    pub alg: Option<String>,
    pub n: Option<String>,
    pub e: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct GoogleIdTokenValidation<'a> {
    pub issuer: &'a str,
    pub client_id: &'a str,
    pub nonce: &'a str,
    pub now: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleIdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: GoogleAudience,
    pub exp: i64,
    pub iat: i64,
    pub nonce: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GoogleAudience {
    Single(String),
    Multiple(Vec<String>),
}

impl GoogleAudience {
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Self::Single(audience) => audience == value,
            Self::Multiple(audiences) => audiences.iter().any(|audience| audience == value),
        }
    }
}

#[derive(Debug, Error)]
pub enum GoogleOidcError {
    #[error("Google code exchange failed")]
    TokenExchange,

    #[error("Google token response is missing ID token")]
    MissingIdToken,

    #[error("Google JWKS fetch failed")]
    JwksFetch,

    #[error("invalid Google ID token header")]
    InvalidHeader,

    #[error("unsupported Google ID token algorithm")]
    UnsupportedAlgorithm,

    #[error("no matching Google JWKS key")]
    MissingKey,

    #[error("invalid Google JWKS key")]
    InvalidKey,

    #[error("invalid Google ID token")]
    InvalidToken,

    #[error("invalid Google issuer")]
    InvalidIssuer,

    #[error("invalid Google audience")]
    InvalidAudience,

    #[error("expired Google ID token")]
    Expired,

    #[error("Google ID token issued-at is too far in the future")]
    FutureIssuedAt,

    #[error("invalid Google provider nonce")]
    InvalidNonce,

    #[error("missing Google subject")]
    MissingSubject,
}

pub fn validate_google_id_token(
    token: &str,
    jwks: &GoogleJwks,
    expected: GoogleIdTokenValidation<'_>,
) -> Result<GoogleIdTokenClaims, GoogleOidcError> {
    let header = decode_header(token).map_err(|_| GoogleOidcError::InvalidHeader)?;
    if header.alg != Algorithm::RS256 {
        return Err(GoogleOidcError::UnsupportedAlgorithm);
    }

    let jwk = matching_jwk(jwks, header.kid.as_deref())?;
    let n = jwk.n.as_deref().ok_or(GoogleOidcError::InvalidKey)?;
    let e = jwk.e.as_deref().ok_or(GoogleOidcError::InvalidKey)?;
    let key = DecodingKey::from_rsa_components(n, e).map_err(|_| GoogleOidcError::InvalidKey)?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;
    let token_data =
        decode::<GoogleIdTokenClaims>(token, &key, &validation).map_err(|_| {
            GoogleOidcError::InvalidToken
        })?;
    let claims = token_data.claims;

    if claims.iss != expected.issuer {
        return Err(GoogleOidcError::InvalidIssuer);
    }
    if !claims.aud.contains(expected.client_id) {
        return Err(GoogleOidcError::InvalidAudience);
    }
    if claims.exp <= expected.now.timestamp() {
        return Err(GoogleOidcError::Expired);
    }
    let max_iat = expected.now + Duration::seconds(MAX_IAT_FUTURE_SKEW_SECONDS);
    if claims.iat > max_iat.timestamp() {
        return Err(GoogleOidcError::FutureIssuedAt);
    }
    if claims.nonce.as_deref() != Some(expected.nonce) {
        return Err(GoogleOidcError::InvalidNonce);
    }
    if claims.sub.trim().is_empty() {
        return Err(GoogleOidcError::MissingSubject);
    }

    Ok(claims)
}

fn matching_jwk<'a>(
    jwks: &'a GoogleJwks,
    kid: Option<&str>,
) -> Result<&'a GoogleJwk, GoogleOidcError> {
    let key = if let Some(kid) = kid {
        jwks.keys
            .iter()
            .find(|key| key.kid.as_deref() == Some(kid))
    } else {
        jwks.keys.iter().find(|key| key.use_.as_deref() != Some("enc"))
    };

    let key = key.ok_or(GoogleOidcError::MissingKey)?;
    if key.kty != "RSA" || key.alg.as_deref().unwrap_or("RS256") != "RS256" {
        return Err(GoogleOidcError::UnsupportedAlgorithm);
    }
    Ok(key)
}
