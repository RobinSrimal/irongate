use chrono::{Duration, Utc};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::store::{AuthStore, DeletedIdentityReusePolicy, IdentityProvider};

mod support;
use support::TestStorage;

#[tokio::test]
async fn deleted_identity_reuse_allocates_new_subject() {
    let store = AuthStore::new(TestStorage::new());
    let email_digest = lookup_digest(
        b"template-local-secret-with-enough-bytes",
        LookupFamily::Email,
        "user@example.com",
    );

    let original = store
        .create_account_with_identity(
            IdentityProvider::Password,
            &email_digest,
            serde_json::json!({ "email": "user@example.com" }),
        )
        .await
        .expect("initial identity");

    store
        .delete_identity(
            IdentityProvider::Password,
            &email_digest,
            Utc::now() - Duration::seconds(1),
        )
        .await
        .expect("delete identity");

    let replacement = store
        .reuse_deleted_identity(
            IdentityProvider::Password,
            &email_digest,
            DeletedIdentityReusePolicy::AfterRetention,
            serde_json::json!({ "email": "user@example.com" }),
        )
        .await
        .expect("reused identity");

    assert_ne!(original.as_str(), replacement.as_str());
}
