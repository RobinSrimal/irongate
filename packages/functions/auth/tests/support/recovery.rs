//! Product-neutral recovery fixtures using email-first password registration.

use crate::support::registration::register_verified_user;
pub use crate::support::registration::Mailbox;

use chrono::{Duration, Utc};
use irongate::{
    config::environment::RuntimeAuthConfig,
    crypto::{
        hmac_lookup::{lookup_digest, LookupFamily},
        password::verify_password,
    },
    providers::password::*,
    storage::StorageAdapter,
    store::{keys::StoreKey, records::AuthorizeSessionRecord, refresh::*, AuthStore},
};
use std::sync::Arc;

pub const OLD: &str = "original password for recovery";
pub const NEW: &str = "replacement password for recovery";

pub struct Fixture {
    pub store: AuthStore,
    pub runtime: RuntimeAuthConfig,
    pub email: String,
    pub email_digest: String,
    pub subject: String,
    pub mailbox: Mailbox,
}

impl Fixture {
    pub async fn new(storage: Arc<dyn StorageAdapter>, email: &str) -> Self {
        let store = AuthStore::new(storage);
        let runtime = RuntimeAuthConfig::for_tests();
        let mailbox = Mailbox::default();
        let subject = register_verified_user(&store, &runtime, &mailbox, email, OLD)
            .await
            .as_str()
            .to_string();
        let email_digest =
            lookup_digest(runtime.lookup_secret.as_bytes(), LookupFamily::Email, email);
        Self {
            store,
            runtime,
            email: email.into(),
            email_digest,
            subject,
            mailbox,
        }
    }

    pub fn digest(&self, token: &str) -> String {
        lookup_digest(
            self.runtime.lookup_secret.as_bytes(),
            LookupFamily::PasswordReset,
            token,
        )
    }
    pub fn code_digest(&self, code: &str) -> String {
        lookup_digest(
            self.runtime.lookup_secret.as_bytes(),
            LookupFamily::AuthorizationCode,
            code,
        )
    }
    pub fn reset_key(&self, token: &str) -> StoreKey {
        StoreKey::password_reset(&self.digest(token))
    }

    pub async fn request_with(
        &self,
        store: &AuthStore,
    ) -> Result<PasswordResetRequestOutcome, PasswordResetRequestError> {
        request_password_reset(
            store,
            &self.runtime,
            &self.mailbox,
            PasswordResetRequestInput { email: &self.email },
        )
        .await
    }
    pub async fn request(&self) -> String {
        self.request_with(&self.store).await.unwrap();
        self.mailbox.last()
    }
    pub async fn complete(
        &self,
        store: &AuthStore,
        token: &str,
    ) -> Result<(), PasswordResetCompleteError> {
        complete_password_reset(
            store,
            &self.runtime,
            PasswordResetCompleteInput {
                token,
                new_password: NEW,
            },
        )
        .await
        .map(|_| ())
    }
    pub async fn password_is(&self, password: &str) -> bool {
        let user = self
            .store
            .get_password_user_by_email_digest(&self.email_digest)
            .await
            .unwrap()
            .unwrap();
        user.password_hash
            .as_deref()
            .is_some_and(|hash| verify_password(password, hash))
    }
    pub async fn generation(&self) -> u64 {
        self.store
            .active_credential_account(&self.subject)
            .await
            .unwrap()
            .unwrap()
            .credential_generation
    }
    pub async fn login(
        &self,
        store: &AuthStore,
        password: &str,
    ) -> Result<String, PasswordLoginError> {
        let session = uuid::Uuid::new_v4().to_string();
        let digest = lookup_digest(
            self.runtime.lookup_secret.as_bytes(),
            LookupFamily::AuthorizeSession,
            &session,
        );
        store
            .create_authorize_session(
                &digest,
                AuthorizeSessionRecord {
                    client_id: "web".into(),
                    redirect_uri: "https://app.example.com/auth/callback".into(),
                    state: Some("recovery-test".into()),
                    scope: "openid offline_access".into(),
                    oidc_nonce: None,
                    code_challenge: Some(irongate::oauth::pkce::generate_challenge(
                        "test-verifier-string-with-enough-length-for-pkce",
                    )),
                    code_challenge_method: Some("S256".into()),
                    selected_provider: Some("password".into()),
                    created_at: Utc::now(),
                    expires_at: Utc::now() + Duration::minutes(10),
                },
            )
            .await
            .unwrap();
        let result = login_password_user(
            store,
            &self.runtime,
            PasswordLoginInput {
                session: &session,
                email: &self.email,
                password,
            },
        )
        .await?;
        Ok(url::Url::parse(&result.redirect_uri)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "code")
            .expect("code redirect")
            .1
            .into_owned())
    }
    pub async fn refresh(&self, client: &str) -> CreatedRefreshToken {
        self.create_refresh(&self.store, client, self.generation().await)
            .await
            .unwrap()
    }
    pub async fn create_refresh(
        &self,
        store: &AuthStore,
        client: &str,
        generation: u64,
    ) -> Result<CreatedRefreshToken, RefreshTokenStoreError> {
        store
            .create_refresh_token(
                self.runtime.lookup_secret.as_bytes(),
                CreateRefreshTokenInput {
                    credential_generation: generation,
                    client_id: client.into(),
                    subject: self.subject.clone(),
                    subject_type: "user".into(),
                    scope: "openid offline_access".into(),
                    properties: serde_json::json!({}),
                    expires_at: Utc::now() + Duration::hours(1),
                },
            )
            .await
    }
    pub async fn rotate(
        &self,
        store: &AuthStore,
        token: &str,
        client: &str,
    ) -> Result<RotatedRefreshToken, RefreshTokenStoreError> {
        store
            .rotate_refresh_token(
                self.runtime.lookup_secret.as_bytes(),
                token,
                client,
                Utc::now() + Duration::hours(2),
            )
            .await
    }
}
