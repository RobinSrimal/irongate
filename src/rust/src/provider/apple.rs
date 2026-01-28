//! Apple Sign In OIDC provider.

use super::oidc::OIDCProvider;
use super::traits::{OAuth2Config, OIDCConfig, Provider, ProviderContext};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

/// Apple provider
pub struct AppleProvider {
    oidc: OIDCProvider,
}

impl AppleProvider {
    pub fn new(client_id: String, team_id: String, key_id: String, private_key: String) -> Self {
        // Note: Apple requires a JWT for client authentication, not a simple secret
        let config = OIDCConfig {
            oauth2: OAuth2Config {
                client_id,
                client_secret: String::new(), // Generated dynamically as JWT
                authorization_url: "https://appleid.apple.com/auth/authorize".to_string(),
                token_url: "https://appleid.apple.com/auth/token".to_string(),
                scopes: vec!["openid".to_string(), "email".to_string(), "name".to_string()],
                pkce: true,
            },
            issuer: "https://appleid.apple.com".to_string(),
            jwks_uri: Some("https://appleid.apple.com/auth/keys".to_string()),
        };
        Self {
            oidc: OIDCProvider::new("apple", config),
        }
    }
}

#[async_trait]
impl Provider for AppleProvider {
    fn name(&self) -> &str {
        "apple"
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
