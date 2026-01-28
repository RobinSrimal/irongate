//! OIDC provider implementation.
//!
//! Extends OAuth2 with ID token validation.

use super::oauth2::OAuth2Provider;
use super::traits::{OIDCConfig, Provider, ProviderContext};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

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

/// Validate an OIDC ID token
pub async fn validate_id_token(
    token: &str,
    config: &OIDCConfig,
) -> Result<IdTokenClaims, String> {
    todo!("Implement OIDC ID token validation")
}

/// ID token claims
#[derive(Debug, serde::Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
}
