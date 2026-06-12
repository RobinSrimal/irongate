//! OIDC provider implementation.
//!
//! Extends OAuth2 with ID token validation via JWKS.

use super::oauth2::OAuth2Provider;
use super::traits::{OIDCConfig, Provider, ProviderContext};
use crate::error::OAuthError;
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// OIDC provider (OpenID Connect)
pub struct OIDCProvider {
    pub oauth2: OAuth2Provider,
    pub config: OIDCConfig,
}

impl OIDCProvider {
    pub fn new(name: impl Into<String>, config: OIDCConfig) -> Self {
        Self {
            oauth2: OAuth2Provider::new(name, config.oauth2.clone()),
            config,
        }
    }
}

#[async_trait]
impl Provider for OIDCProvider {
    fn name(&self) -> &str {
        self.oauth2.name()
    }

    fn provider_type(&self) -> &str {
        "oidc"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        ctx: ProviderContext<S>,
    ) -> Router {
        self.oauth2.init(router, ctx)
    }
}

/// JWKS key set from a provider's JWKS endpoint
#[derive(Debug, Deserialize)]
pub struct JwksResponse {
    pub keys: Vec<JwkKey>,
}

/// Individual JWK key
#[derive(Debug, Deserialize, Clone)]
pub struct JwkKey {
    pub kty: String,
    pub kid: Option<String>,
    #[serde(rename = "use")]
    pub use_: Option<String>,
    pub alg: Option<String>,
    // RSA fields
    pub n: Option<String>,
    pub e: Option<String>,
    // EC fields
    pub crv: Option<String>,
    pub x: Option<String>,
    pub y: Option<String>,
}

/// Fetch JWKS from a provider's JWKS URI.
pub async fn fetch_jwks(jwks_uri: &str) -> Result<JwksResponse, OAuthError> {
    let client = reqwest::Client::new();

    let response = client
        .get(jwks_uri)
        .send()
        .await
        .map_err(|e| OAuthError::ServerError(format!("JWKS fetch failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(OAuthError::ServerError(format!(
            "JWKS fetch returned {}",
            response.status()
        )));
    }

    response
        .json::<JwksResponse>()
        .await
        .map_err(|e| OAuthError::ServerError(format!("Failed to parse JWKS: {}", e)))
}

/// Validate an OIDC ID token.
///
/// Fetches JWKS from the provider, finds the matching key by `kid`,
/// verifies the signature, and validates standard claims (`iss`, `aud`, `exp`).
pub async fn validate_id_token(
    token: &str,
    config: &OIDCConfig,
) -> Result<IdTokenClaims, OAuthError> {
    // Decode JWT header to get kid and algorithm
    let header = decode_header(token)
        .map_err(|e| OAuthError::InvalidGrant(format!("Invalid ID token header: {}", e)))?;

    let kid = header.kid.as_deref();

    // Fetch JWKS
    let jwks_uri = config
        .jwks_uri
        .as_deref()
        .ok_or_else(|| OAuthError::ServerError("No JWKS URI configured".to_string()))?;

    let jwks = fetch_jwks(jwks_uri).await?;

    // Find matching key
    let jwk = if let Some(kid) = kid {
        jwks.keys
            .iter()
            .find(|k| k.kid.as_deref() == Some(kid))
            .ok_or_else(|| {
                OAuthError::InvalidGrant(format!("No matching key found for kid: {}", kid))
            })?
    } else {
        // If no kid in token, use the first signing key
        jwks.keys
            .iter()
            .find(|k| k.use_.as_deref() != Some("enc"))
            .or(jwks.keys.first())
            .ok_or_else(|| OAuthError::ServerError("JWKS has no keys".to_string()))?
    };

    // Build decoding key based on key type
    let decoding_key = match jwk.kty.as_str() {
        "RSA" => {
            let n = jwk
                .n
                .as_deref()
                .ok_or_else(|| OAuthError::ServerError("RSA key missing n".to_string()))?;
            let e = jwk
                .e
                .as_deref()
                .ok_or_else(|| OAuthError::ServerError("RSA key missing e".to_string()))?;
            DecodingKey::from_rsa_components(n, e)
                .map_err(|e| OAuthError::ServerError(format!("Invalid RSA key: {}", e)))?
        }
        "EC" => {
            let x = jwk
                .x
                .as_deref()
                .ok_or_else(|| OAuthError::ServerError("EC key missing x".to_string()))?;
            let y = jwk
                .y
                .as_deref()
                .ok_or_else(|| OAuthError::ServerError("EC key missing y".to_string()))?;
            DecodingKey::from_ec_components(x, y)
                .map_err(|e| OAuthError::ServerError(format!("Invalid EC key: {}", e)))?
        }
        other => {
            return Err(OAuthError::ServerError(format!(
                "Unsupported key type: {}",
                other
            )));
        }
    };

    // Determine algorithm
    let algorithm = match header.alg {
        jsonwebtoken::Algorithm::RS256 => Algorithm::RS256,
        jsonwebtoken::Algorithm::RS384 => Algorithm::RS384,
        jsonwebtoken::Algorithm::RS512 => Algorithm::RS512,
        jsonwebtoken::Algorithm::ES256 => Algorithm::ES256,
        jsonwebtoken::Algorithm::ES384 => Algorithm::ES384,
        other => {
            return Err(OAuthError::ServerError(format!(
                "Unsupported algorithm: {:?}",
                other
            )));
        }
    };

    // Build validation
    let mut validation = Validation::new(algorithm);
    validation.set_issuer(&[&config.issuer]);
    validation.set_audience(&[&config.oauth2.client_id]);
    // Require exp claim
    validation.validate_exp = true;

    // Decode and validate
    let token_data = decode::<IdTokenClaims>(token, &decoding_key, &validation)
        .map_err(|e| OAuthError::InvalidGrant(format!("ID token validation failed: {}", e)))?;

    Ok(token_data.claims)
}

/// ID token claims
#[derive(Debug, Serialize, Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: IdTokenAudience,
    pub exp: i64,
    pub iat: i64,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub nonce: Option<String>,
}

/// Audience can be a single string or array of strings
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum IdTokenAudience {
    Single(String),
    Multiple(Vec<String>),
}

impl IdTokenAudience {
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Self::Single(s) => s == value,
            Self::Multiple(v) => v.iter().any(|s| s == value),
        }
    }
}
