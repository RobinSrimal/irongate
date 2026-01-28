//! Google OIDC provider.

use super::oidc::OIDCProvider;
use super::traits::{OAuth2Config, OIDCConfig, Provider, ProviderContext};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

/// Google provider
pub struct GoogleProvider {
    oidc: OIDCProvider,
}

impl GoogleProvider {
    pub fn new(client_id: String, client_secret: String) -> Self {
        let config = OIDCConfig {
            oauth2: OAuth2Config {
                client_id,
                client_secret,
                authorization_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
                scopes: vec![
                    "openid".to_string(),
                    "email".to_string(),
                    "profile".to_string(),
                ],
                pkce: true,
            },
            issuer: "https://accounts.google.com".to_string(),
            jwks_uri: Some("https://www.googleapis.com/oauth2/v3/certs".to_string()),
        };
        Self {
            oidc: OIDCProvider::new("google", config),
        }
    }
}

#[async_trait]
impl Provider for GoogleProvider {
    fn name(&self) -> &str {
        "google"
    }

    fn provider_type(&self) -> &str {
        "oidc"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        ctx: ProviderContext<S>,
    ) -> Router {
        self.oidc.init(router, ctx)
    }
}
