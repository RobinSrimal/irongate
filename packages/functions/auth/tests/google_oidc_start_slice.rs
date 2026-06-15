use chrono::{Duration, Utc};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::store::keys::StoreKey;
use irongate::store::records::ProviderStateRecord;
use irongate::store::AuthStore;
use irongate::StorageAdapter;

mod support;
use support::TestStorage;

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn provider_state_store_uses_hmac_key_and_consumes_once() {
    let storage = TestStorage::new();
    let store = AuthStore::new(storage.clone());
    let raw_state = "raw-google-provider-state";
    let raw_session = "raw-authorize-session";
    let state_digest = lookup_digest(LOOKUP_SECRET, LookupFamily::ProviderState, raw_state);
    let session_digest = lookup_digest(
        LOOKUP_SECRET,
        LookupFamily::AuthorizeSession,
        raw_session,
    );
    let expires_at = Utc::now() + Duration::minutes(10);

    store
        .create_provider_state(
            &state_digest,
            ProviderStateRecord {
                session_lookup_digest: session_digest.clone(),
                provider: "google".to_string(),
                pkce_verifier: "pkce-verifier".to_string(),
                nonce: "provider-nonce".to_string(),
                created_at: Utc::now(),
                expires_at,
            },
        )
        .await
        .expect("create provider state");

    let key = StoreKey::provider_state(&state_digest);
    assert_ne!(key.sk(), raw_state);
    let stored = storage
        .get(&[key.pk(), key.sk()])
        .await
        .expect("get provider state")
        .expect("provider state");
    let record: ProviderStateRecord =
        serde_json::from_value(stored).expect("provider state json");
    assert_eq!(record.session_lookup_digest, session_digest);
    assert_eq!(record.provider, "google");
    assert_eq!(record.pkce_verifier, "pkce-verifier");
    assert_eq!(record.nonce, "provider-nonce");
    assert_eq!(record.expires_at, expires_at);

    let all_state = storage
        .scan(&["provider:state"])
        .await
        .expect("scan provider state");
    let debug = format!("{all_state:?}");
    assert!(!debug.contains(raw_state));
    assert!(!debug.contains(raw_session));

    let consumed = store
        .take_provider_state(&state_digest)
        .await
        .expect("take provider state")
        .expect("provider state exists");
    assert_eq!(consumed.nonce, "provider-nonce");
    assert!(store
        .take_provider_state(&state_digest)
        .await
        .expect("take provider state again")
        .is_none());
}

#[tokio::test]
async fn provider_state_store_rejects_expired_records() {
    let store = AuthStore::new(TestStorage::new());

    store
        .create_provider_state(
            "expired-provider-state-digest",
            ProviderStateRecord {
                session_lookup_digest: "session-digest".to_string(),
                provider: "google".to_string(),
                pkce_verifier: "pkce-verifier".to_string(),
                nonce: "provider-nonce".to_string(),
                created_at: Utc::now() - Duration::minutes(11),
                expires_at: Utc::now() - Duration::seconds(1),
            },
        )
        .await
        .expect("create expired provider state");

    assert!(store
        .take_provider_state("expired-provider-state-digest")
        .await
        .expect("take expired provider state")
        .is_none());
}
