//! GitHub OAuth2 provider.

use super::oauth2::OAuth2Provider;
use super::traits::{OAuth2Config, Provider, ProviderContext};
use crate::storage::StorageAdapter;
use async_trait::async_trait;
use axum::Router;

/// GitHub provider
pub struct GitHubProvider {
    oauth2: OAuth2Provider,
}

impl GitHubProvider {
    pub fn new(client_id: String, client_secret: String) -> Self {
        let config = OAuth2Config {
            client_id,
            client_secret,
            authorization_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            scopes: vec!["read:user".to_string(), "user:email".to_string()],
            pkce: false,
        };
        Self {
            oauth2: OAuth2Provider::new("github", config),
        }
    }
}

#[async_trait]
impl Provider for GitHubProvider {
    fn name(&self) -> &str {
        "github"
    }

    fn provider_type(&self) -> &str {
        "oauth2"
    }

    fn init<S: StorageAdapter + 'static>(
        &self,
        router: Router,
        ctx: ProviderContext<S>,
    ) -> Router {
        self.oauth2.init(router, ctx)
    }
}
