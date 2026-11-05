//! Shared provider transaction contracts; the runner supplies the real backend.

#[path = "provider_issuance/outcomes.rs"]
pub mod outcomes;
#[path = "provider_issuance/ownership.rs"]
pub mod ownership;
#[path = "provider_issuance/ownership_races.rs"]
pub mod ownership_races;
#[path = "provider_issuance/races.rs"]
pub mod races;

use crate::transaction_gate::{PausedStorage, PendingTransaction};
use chrono::{DateTime, Duration, Utc};
use irongate::{
    core::subjects::Subject,
    crypto::hmac_lookup::{lookup_digest, LookupFamily},
    error::StorageError,
    providers::{
        apple::{apple_identity_digest, AppleIdTokenClaims},
        google::{google_identity_digest, GoogleIdTokenClaims},
        verified_email::ProviderVerifiedEmail,
    },
    storage::{StorageAdapter, TransactOperation},
    store::{
        keys::StoreKey,
        provider_issuance::{PreparedProviderIssuance, ProviderIdentityInput},
        records::{AccountStatus, AuthorizationCodeRecord, AuthorizeSessionRecord, IdentityStatus},
        AuthStore, DeletedIdentityReusePolicy, IdentityProvider,
    },
};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};
use tokio::task::JoinHandle;

pub const TTL: u64 = 300;
pub const SECRET: &[u8] = b"test-only-provider-lookup-secret-v1";
const APPLE_ISSUER: &str = "https://appleid.apple.com";
const GOOGLE_ISSUER: &str = "https://accounts.google.com";
pub type IssuanceTask = JoinHandle<Result<Subject, StorageError>>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    New,
    Active,
    Deleted,
}

pub struct Fixture {
    pub storage: Arc<dyn StorageAdapter>,
    pub store: AuthStore,
    pub provider: IdentityProvider,
    pub digest: String,
    pub external_subject: String,
    pub session: AuthorizeSessionRecord,
    pub clock: Arc<Mutex<DateTime<Utc>>>,
}

pub fn properties(email: &str, name: &str) -> Value {
    json!({"email": email, "email_verified": true, "name": name, "picture": "https://example.test/profile.png"})
}

pub fn code_digest() -> String {
    lookup_digest(
        SECRET,
        LookupFamily::AuthorizationCode,
        &uuid::Uuid::new_v4().to_string(),
    )
}

impl Fixture {
    pub fn new(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider, label: &str) -> Self {
        let digest = match provider {
            IdentityProvider::Google => google_identity_digest(SECRET, GOOGLE_ISSUER, label),
            IdentityProvider::Apple => apple_identity_digest(SECRET, APPLE_ISSUER, label),
            IdentityProvider::Password => panic!("provider fixture is for Google/Apple"),
        };
        let now = Utc::now();
        let clock = Arc::new(Mutex::new(now));
        let store = Self::clocked(storage.clone(), clock.clone());
        Self {
            storage,
            store,
            provider,
            digest,
            external_subject: label.into(),
            clock,
            session: AuthorizeSessionRecord {
                client_id: "web".into(),
                redirect_uri: "https://app.example.test/callback?fixed=1".into(),
                state: Some("client-state-not-a-code-property".into()),
                scope: "openid profile email offline_access".into(),
                oidc_nonce: Some("downstream-oidc-nonce".into()),
                code_challenge: Some(irongate::oauth::pkce::generate_challenge(
                    "provider-contract-downstream-verifier-with-enough-entropy",
                )),
                code_challenge_method: Some("S256".into()),
                selected_provider: Some(provider.as_str().into()),
                created_at: now,
                expires_at: now + Duration::hours(1),
            },
        }
    }

    fn clocked<S: StorageAdapter + 'static>(
        storage: S,
        clock: Arc<Mutex<DateTime<Utc>>>,
    ) -> AuthStore {
        AuthStore::new(storage).with_provider_issuance_clock(move || *clock.lock().unwrap())
    }

    pub async fn prepare(
        &self,
        store: &AuthStore,
        props: Value,
        policy: DeletedIdentityReusePolicy,
    ) -> Result<PreparedProviderIssuance, StorageError> {
        store
            .prepare_provider_issuance(
                ProviderIdentityInput {
                    provider: self.provider,
                    identity_digest: &self.digest,
                    verified_email: self.verified_email(&props),
                    properties: props,
                },
                &self.session,
                TTL,
                policy,
            )
            .await
    }

    // Store contracts begin after upstream validation; callback tests exercise real signatures.
    pub fn verified_email(&self, props: &Value) -> Option<ProviderVerifiedEmail> {
        let mut claims = json!({
            "sub": self.external_subject, "aud": "test-client",
            "iat": Utc::now().timestamp(), "exp": (Utc::now() + Duration::minutes(5)).timestamp(),
            "email": props.get("email"), "email_verified": props.get("email_verified"),
        });
        match self.provider {
            IdentityProvider::Apple => {
                claims["iss"] = json!(APPLE_ISSUER);
                let claims: AppleIdTokenClaims = serde_json::from_value(claims).unwrap();
                ProviderVerifiedEmail::from_apple_claims(&claims, SECRET)
            }
            IdentityProvider::Google => {
                // Store fixtures model a validated Workspace claim, not email-suffix inference.
                claims["iss"] = json!(GOOGLE_ISSUER);
                claims["hd"] = json!("workspace.example.test");
                let claims: GoogleIdTokenClaims = serde_json::from_value(claims).unwrap();
                ProviderVerifiedEmail::from_google_claims(&claims, SECRET)
            }
            IdentityProvider::Password => unreachable!(),
        }
    }

    pub async fn issue(&self, props: Value, digest: &str) -> Subject {
        let prepared = self
            .prepare(&self.store, props, DeletedIdentityReusePolicy::Immediate)
            .await
            .unwrap();
        self.store
            .finish_provider_issuance(prepared, digest)
            .await
            .unwrap()
    }

    pub async fn seed(&self, kind: Kind) -> Option<Subject> {
        if kind == Kind::New {
            return None;
        }
        let digest = code_digest();
        let subject = self
            .issue(
                properties(&format!("{}@example.test", self.digest), "Original profile"),
                &digest,
            )
            .await;
        self.store
            .take_authorization_code(&digest)
            .await
            .unwrap()
            .unwrap();
        if kind == Kind::Deleted {
            self.store
                .delete_account(&subject, DeletedIdentityReusePolicy::Immediate, 0)
                .await
                .unwrap();
        }
        Some(subject)
    }

    pub fn identity_key(&self) -> StoreKey {
        StoreKey::identity(self.provider.as_str(), &self.digest)
    }

    pub fn keys(&self, subject: &str, code: &str) -> Vec<StoreKey> {
        vec![
            StoreKey::account(subject),
            self.identity_key(),
            StoreKey::identity_by_subject(subject, self.provider.as_str(), &self.digest),
            StoreKey::authorization_code(code),
        ]
    }

    pub async fn rows(&self, keys: &[StoreKey]) -> Vec<Option<Value>> {
        let mut rows = Vec::new();
        for key in keys {
            rows.push(self.storage.get(&[key.pk(), key.sk()]).await.unwrap());
        }
        rows
    }

    pub async fn replace(&self, key: &StoreKey, value: Value) {
        let expected = self.rows(std::slice::from_ref(key)).await.pop().unwrap();
        assert!(self
            .storage
            .compare_and_set(&[key.pk(), key.sk()], expected.as_ref(), value, None)
            .await
            .unwrap());
    }

    pub async fn pause(&self, props: Value, code: &str) -> (PendingTransaction, IssuanceTask) {
        let (paused, mut gate) = PausedStorage::when(self.storage.clone(), issuance);
        let store = Self::clocked(paused, self.clock.clone());
        let prepared = self
            .prepare(&store, props, DeletedIdentityReusePolicy::Immediate)
            .await
            .unwrap();
        let code = code.to_owned();
        let task =
            tokio::spawn(async move { store.finish_provider_issuance(prepared, &code).await });
        (gate.wait().await, task)
    }

    pub async fn assert_committed(
        &self,
        subject: &Subject,
        code: &str,
        props: &Value,
        generation: u64,
    ) -> AuthorizationCodeRecord {
        let rows = self.rows(&self.keys(subject.as_str(), code)).await;
        assert!(rows.iter().all(Option::is_some));
        let account = self.store.get_account(subject).await.unwrap().unwrap();
        assert_eq!(account.status, AccountStatus::Active);
        assert_eq!(account.subject, subject.as_str());
        assert_eq!(account.credential_generation, generation);
        let identity = self
            .store
            .get_identity(self.provider, &self.digest)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(identity.status, IdentityStatus::Active);
        assert_eq!(identity.subject.as_deref(), Some(subject.as_str()));
        assert_eq!(identity.provider, self.provider.as_str());
        assert_eq!(identity.identity_digest, self.digest);
        assert_eq!(identity.properties.as_ref(), Some(props));
        assert_eq!(identity.last_seen_at, *self.clock.lock().unwrap());
        let index = rows[2].as_ref().unwrap();
        assert_eq!(index["subject"], subject.as_str());
        assert_eq!(index["provider"], self.provider.as_str());
        assert_eq!(index["identity_digest"], self.digest);
        let code = self
            .store
            .get_authorization_code(code)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(code.subject, subject.as_str());
        assert_eq!(code.subject_type, "user");
        assert_eq!(code.credential_generation, generation);
        assert_eq!(code.client_id, self.session.client_id);
        assert_eq!(code.redirect_uri, self.session.redirect_uri);
        assert_eq!(code.scope, self.session.scope);
        assert_eq!(code.code_challenge, self.session.code_challenge);
        assert_eq!(
            code.code_challenge_method,
            self.session.code_challenge_method
        );
        assert_eq!(code.oidc_nonce, self.session.oidc_nonce);
        assert_eq!(
            code.properties,
            json!({"provider": self.provider.as_str(), "email": props.get("email").and_then(Value::as_str), "email_verified": props.get("email_verified").and_then(Value::as_bool).unwrap_or(false)})
        );
        assert_eq!(code.created_at, *self.clock.lock().unwrap());
        assert_eq!(
            code.expires_at,
            code.created_at + Duration::seconds(TTL as i64)
        );
        code
    }
}

pub fn issuance(operations: &[TransactOperation]) -> bool {
    operations
        .iter()
        .any(|op| matches!(op, TransactOperation::Put { key, .. } if key[0] == "oauth:code"))
}

pub fn candidate(operations: &[TransactOperation]) -> String {
    operations
        .iter()
        .find_map(|op| match op {
            TransactOperation::Put { key, value, .. } if key[0] == "oauth:code" => {
                Some(value["subject"].as_str().unwrap().to_owned())
            }
            _ => None,
        })
        .expect("issuance includes a code")
}

pub fn plan_keys(
    f: &Fixture,
    code: &str,
    operations: &[TransactOperation],
    kind: Kind,
) -> Vec<StoreKey> {
    let mut keys = f.keys(&candidate(operations), code);
    if kind != Kind::Active {
        let digest = operations
            .iter()
            .find_map(|op| match op {
                TransactOperation::Put { key, .. }
                | TransactOperation::ConditionCheck { key, .. }
                    if key[0] == "email:owner" =>
                {
                    Some(key[1].as_str())
                }
                _ => None,
            })
            .expect("new provider identity guards ownership");
        keys.extend([
            StoreKey::email_owner(digest),
            StoreKey::email_by_subject(&candidate(operations), digest),
        ]);
    }
    let actual = operations
        .iter()
        .map(|op| match op {
            TransactOperation::Put { key, .. }
            | TransactOperation::Delete { key, .. }
            | TransactOperation::ConditionCheck { key, .. } => key.clone(),
        })
        .collect::<BTreeSet<_>>();
    let expected = keys
        .iter()
        .enumerate()
        .filter(|(n, _)| kind != Kind::Active || *n != 2)
        .map(|(_, key)| key.parts())
        .collect::<BTreeSet<_>>();
    assert_eq!(operations.len(), expected.len());
    assert_eq!(actual, expected);
    keys
}

pub fn rejected<T>(result: Result<T, StorageError>) {
    assert!(
        matches!(result, Err(StorageError::ConditionFailed(_))),
        "confirmed stale state must remain a conditional rejection"
    );
}

macro_rules! provider_cases {
    ($runner:path $(, $attribute:meta)?) => {
        #[tokio::test] $(#[$attribute])? async fn cross_provider_claims() { $runner(provider_contract::ownership_races::cross_provider_claims).await; }
        #[tokio::test] $(#[$attribute])? async fn ownership_independent_conditions() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership_races::independent_conditions(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_competing_claims() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership_races::competing_claims(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_password_completion_races() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership_races::password_completion_races(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_account_races_and_outcomes() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership_races::account_races_and_outcomes(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_shared_accounts() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership::shared_accounts(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_authority_and_integrity() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership::authority_and_integrity(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_owner_reuse() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership::owner_reuse(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn ownership_linked_deletion() { for provider in [irongate::store::IdentityProvider::Apple, irongate::store::IdentityProvider::Google] { $runner(move |storage| provider_contract::ownership::linked_deletion(storage, provider)).await; } }
        #[tokio::test] $(#[$attribute])? async fn provider_usable_codes() { $runner(provider_contract::outcomes::usable_codes).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_independent_conditions() { $runner(provider_contract::outcomes::independent_conditions).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_prepare_rejections() { $runner(provider_contract::outcomes::prepare_rejections).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_reuse_policies() { $runner(provider_contract::outcomes::reuse_policies).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_competing_callbacks() { $runner(provider_contract::races::competing_callbacks).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_lifecycle_and_identity_races() { $runner(provider_contract::races::lifecycle_and_identity_races).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_expiry_boundaries() { $runner(provider_contract::races::expiry_boundaries).await; }
        #[tokio::test] $(#[$attribute])? async fn provider_uncertain_outcomes() { $runner(provider_contract::races::uncertain_outcomes).await; }
    };
}
pub(crate) use provider_cases;
