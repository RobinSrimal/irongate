//! Ownership contracts run after validated upstream claims, on either real store backend.

use super::*;
use irongate::config::environment::{LookupSecret, RuntimeAuthConfig};

pub fn opposite(provider: IdentityProvider) -> IdentityProvider {
    match provider {
        IdentityProvider::Google => IdentityProvider::Apple,
        IdentityProvider::Apple => IdentityProvider::Google,
        IdentityProvider::Password => unreachable!(),
    }
}

pub fn runtime() -> RuntimeAuthConfig {
    let mut runtime = RuntimeAuthConfig::for_tests();
    runtime.lookup_secret =
        LookupSecret::from_string(String::from_utf8(SECRET.to_vec()).unwrap()).unwrap();
    runtime
}

pub async fn password_owner(f: &Fixture, email: &str) -> Subject {
    crate::support::registration::register_verified_user(
        &f.store,
        &runtime(),
        &Default::default(),
        email,
        "password owner test credential",
    )
    .await
}

pub fn ownership_keys(subject: &str, email: &str) -> [StoreKey; 2] {
    let digest = lookup_digest(SECRET, LookupFamily::Email, email);
    [
        StoreKey::email_owner(&digest),
        StoreKey::email_by_subject(subject, &digest),
    ]
}

pub async fn shared_accounts(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider) {
    let f = Fixture::new(storage.clone(), provider, "shared-provider");
    let owner = password_owner(&f, "shared@example.test").await;
    let keys = ownership_keys(owner.as_str(), "shared@example.test");
    let before = f.rows(&keys).await;
    let code = code_digest();
    let subject = f
        .issue(properties(" SHARED@EXAMPLE.TEST ", "Provider"), &code)
        .await;
    assert_eq!(subject.as_str(), owner.as_str());
    f.assert_committed(
        &subject,
        &code,
        &properties("shared@example.test", "Provider"),
        0,
    )
    .await;
    assert_eq!(f.rows(&keys).await, before);
    assert!(f
        .store
        .take_authorization_code(&code)
        .await
        .unwrap()
        .is_some());

    let other = password_owner(&f, "other@example.test").await;
    let other_keys = ownership_keys(other.as_str(), "other@example.test");
    let other_before = f.rows(&other_keys).await;
    let identity_before = f.rows(&[f.identity_key()]).await;
    let other_identity = Fixture::new(storage.clone(), provider, "other-proof-subject");
    for wrong_provider in [false, true] {
        let selected = if wrong_provider {
            opposite(provider)
        } else {
            provider
        };
        let mut session = f.session.clone();
        session.selected_provider = Some(selected.as_str().into());
        let props = properties("shared@example.test", "Untrusted proof");
        rejected(
            f.store
                .prepare_provider_issuance(
                    ProviderIdentityInput {
                        provider: selected,
                        identity_digest: &f.digest,
                        verified_email: if wrong_provider {
                            f.verified_email(&props)
                        } else {
                            other_identity.verified_email(&props)
                        },
                        properties: props,
                    },
                    &session,
                    TTL,
                    DeletedIdentityReusePolicy::Immediate,
                )
                .await,
        );
        assert_eq!(f.rows(&[f.identity_key()]).await, identity_before);
    }
    for props in [
        properties("other@example.test", "Changed"),
        json!({}),
        json!({"email": "other@example.test", "email_verified": false}),
    ] {
        let code = code_digest();
        let returning = f.issue(props.clone(), &code).await;
        assert_eq!(returning.as_str(), owner.as_str());
        f.assert_committed(&returning, &code, &props, 0).await;
        assert_eq!(f.rows(&keys).await, before);
        assert_eq!(f.rows(&other_keys).await, other_before);
    }

    let relay = Fixture::new(storage.clone(), IdentityProvider::Apple, "relay-apple");
    let code = code_digest();
    let subject = relay
        .issue(properties("alias@privaterelay.appleid.com", "Relay"), &code)
        .await;
    assert_ne!(subject.as_str(), owner.as_str());
    assert_ne!(subject.as_str(), other.as_str());
    for row in relay
        .rows(&ownership_keys(
            subject.as_str(),
            "alias@privaterelay.appleid.com",
        ))
        .await
    {
        assert_eq!(row.unwrap()["subject"], subject.as_str());
    }
    let other_provider = Fixture::new(storage, opposite(provider), "shared-other-provider");
    let subject = other_provider
        .issue(
            properties("shared@example.test", "Other provider"),
            &code_digest(),
        )
        .await;
    assert_eq!(subject.as_str(), owner.as_str());
    assert_eq!(f.rows(&keys).await, before);
}

pub async fn authority_and_integrity(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider) {
    let f = Fixture::new(storage.clone(), provider, "unproven-provider");
    for props in [
        json!({}),
        json!({"email":"signed@example.test", "email_verified":false}),
        properties("malformed", "Invalid"),
    ] {
        rejected(
            f.prepare(&f.store, props, DeletedIdentityReusePolicy::Immediate)
                .await,
        );
    }
    let other = Fixture::new(storage.clone(), provider, "different-identity");
    let props = properties("signed@example.test", "Signed");
    for requested_provider in [provider, opposite(provider)] {
        let mut session = f.session.clone();
        session.selected_provider = Some(requested_provider.as_str().into());
        rejected(
            f.store
                .prepare_provider_issuance(
                    ProviderIdentityInput {
                        provider: requested_provider,
                        identity_digest: &f.digest,
                        properties: props.clone(),
                        verified_email: if requested_provider == provider {
                            other.verified_email(&props)
                        } else {
                            f.verified_email(&props)
                        },
                    },
                    &session,
                    TTL,
                    DeletedIdentityReusePolicy::Immediate,
                )
                .await,
        );
    }
    // Arbitrary metadata is not an email proof, even with the right signed-identity digest.
    rejected(
        f.store
            .prepare_provider_issuance(
                ProviderIdentityInput {
                    provider: f.provider,
                    identity_digest: &f.digest,
                    properties: props,
                    verified_email: None,
                },
                &f.session,
                TTL,
                DeletedIdentityReusePolicy::Immediate,
            )
            .await,
    );
    assert!(f.rows(&[f.identity_key()]).await[0].is_none());
    assert!(storage
        .query_prefix(&["email:owner"])
        .await
        .unwrap()
        .is_empty());
    assert!(storage
        .query_prefix(&["oauth:code"])
        .await
        .unwrap()
        .is_empty());

    let owner = password_owner(&f, "integrity@example.test").await;
    let keys = [
        vec![StoreKey::account(owner.as_str())],
        ownership_keys(owner.as_str(), "integrity@example.test").to_vec(),
    ]
    .concat();
    let original = f.rows(&keys).await;
    for (target, value) in [
        (0, None),
        (
            0,
            Some({
                let mut v = original[0].clone().unwrap();
                v["status"] = json!("disabled");
                v
            }),
        ),
        (
            0,
            Some({
                let mut v = original[0].clone().unwrap();
                v["status"] = json!("deleted");
                v
            }),
        ),
        (
            0,
            Some({
                let mut v = original[0].clone().unwrap();
                v["subject"] = json!("user_wrong");
                v
            }),
        ),
        (
            1,
            Some(
                json!({"email_digest":"wrong", "subject":owner.as_str(), "deleted_at":null, "reusable_after":null}),
            ),
        ),
        (
            1,
            Some({
                let mut v = original[1].clone().unwrap();
                v["subject"] = Value::Null;
                v
            }),
        ),
        (
            1,
            Some({
                let mut v = original[1].clone().unwrap();
                v["unknown"] = json!(true);
                v
            }),
        ),
        (2, None),
        (
            2,
            Some(json!({"email_digest":"wrong", "subject":owner.as_str()})),
        ),
        (
            2,
            Some({
                let mut v = original[2].clone().unwrap();
                v["subject"] = json!("user_wrong");
                v
            }),
        ),
    ] {
        if let Some(value) = value {
            f.replace(&keys[target], value).await;
        } else {
            storage
                .remove(&[keys[target].pk(), keys[target].sk()])
                .await
                .unwrap();
        }
        let before = f.rows(&keys).await;
        let result = f
            .prepare(
                &f.store,
                properties("integrity@example.test", "Denied"),
                DeletedIdentityReusePolicy::Immediate,
            )
            .await;
        if target == 0 {
            rejected(result);
        } else {
            assert!(matches!(result, Err(StorageError::DynamoDB(_))));
        }
        assert_eq!(f.rows(&keys).await, before);
        assert!(f.rows(&[f.identity_key()]).await[0].is_none());
        f.replace(&keys[target], original[target].clone().unwrap())
            .await;
    }
    // A changed owner/index read cannot be treated as absent on operational failure.
    let (paused, mut gate) = PausedStorage::before_read(storage, keys[1].parts());
    let store = AuthStore::new(paused);
    let task = tokio::spawn(async move {
        f.prepare(
            &store,
            properties("integrity@example.test", "Unavailable"),
            DeletedIdentityReusePolicy::Immediate,
        )
        .await
    });
    gate.wait().await.fail_before_dispatch();
    assert!(matches!(
        task.await.unwrap(),
        Err(StorageError::DynamoDB(_))
    ));
}

pub async fn owner_reuse(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider) {
    use DeletedIdentityReusePolicy::{AfterRetention, Immediate, Never};
    for (n, (policy, offset, allowed)) in [
        (Never, Some(-1), false),
        (Immediate, None, false),
        (Immediate, Some(1), true),
        (AfterRetention, None, false),
        (AfterRetention, Some(1), false),
        (AfterRetention, Some(0), true),
        (AfterRetention, Some(-1), true),
    ]
    .into_iter()
    .enumerate()
    {
        let f = Fixture::new(storage.clone(), provider, &format!("owner-reuse-{n}"));
        let email = format!("reuse-{n}@example.test");
        let owner = password_owner(&f, &email).await;
        f.store.delete_account(&owner, Immediate, 0).await.unwrap();
        let key = ownership_keys(owner.as_str(), &email)[0].clone();
        let now = *f.clock.lock().unwrap();
        f.replace(
            &key,
            json!({"email_digest":key.sk(), "subject":null,
                "deleted_at":now - Duration::days(1),
                "reusable_after":offset.map(|seconds| now + Duration::seconds(seconds)),
            }),
        )
        .await;
        let prepared = f
            .prepare(&f.store, properties(&email, "Reuse"), policy)
            .await;
        if allowed {
            let subject = f
                .store
                .finish_provider_issuance(prepared.unwrap(), &code_digest())
                .await
                .unwrap();
            assert_ne!(subject.as_str(), owner.as_str());
            assert_eq!(
                f.rows(&[key]).await[0].as_ref().unwrap()["subject"],
                subject.as_str()
            );
        } else {
            assert!(matches!(prepared, Err(StorageError::AlreadyExists(_))));
        }
        assert_eq!(
            f.store.get_account(&owner).await.unwrap().unwrap().status,
            AccountStatus::Deleted
        );
    }
}

pub async fn linked_deletion(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider) {
    let f = Fixture::new(storage, provider, "deleted-shared-provider");
    let email = "deleted-shared@example.test";
    let owner = password_owner(&f, email).await;
    assert_eq!(
        f.issue(properties(email, "Provider"), &code_digest())
            .await
            .as_str(),
        owner.as_str()
    );
    let email_digest = lookup_digest(SECRET, LookupFamily::Email, email);
    let password_digest = lookup_digest(SECRET, LookupFamily::PasswordIdentity, email);
    f.store
        .delete_account(&owner, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    for (provider, digest) in [
        (provider, &f.digest),
        (IdentityProvider::Password, &password_digest),
    ] {
        let identity = f
            .store
            .get_identity(provider, digest)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(identity.status, IdentityStatus::Deleted);
        assert!(identity.subject.is_none() && identity.properties.is_none());
        assert!(f
            .rows(&[StoreKey::identity_by_subject(
                owner.as_str(),
                provider.as_str(),
                digest
            )])
            .await[0]
            .is_none());
    }
    let rows = f.rows(&ownership_keys(owner.as_str(), email)).await;
    assert!(rows[0].as_ref().unwrap()["subject"].is_null() && rows[1].is_none());
    let password = f
        .store
        .get_password_user_by_email_digest(&email_digest)
        .await
        .unwrap()
        .unwrap();
    assert!(password.password_hash.is_none() && password.subject.is_none());
    let next = password_owner(&f, email).await;
    assert_ne!(next.as_str(), owner.as_str());
    let code = code_digest();
    let linked = f.issue(properties(email, "Reused provider"), &code).await;
    assert_eq!(linked.as_str(), next.as_str());
    let keys = [
        f.keys(next.as_str(), &code),
        ownership_keys(next.as_str(), email).to_vec(),
    ]
    .concat();
    let before = f.rows(&keys).await;
    f.store
        .delete_account(&owner, DeletedIdentityReusePolicy::Immediate, 0)
        .await
        .unwrap();
    assert_eq!(
        f.rows(&keys).await,
        before,
        "old deletion cannot alter the new owner"
    );
    assert!(f
        .store
        .take_authorization_code(&code)
        .await
        .unwrap()
        .is_some());
}
