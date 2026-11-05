use super::*;

pub async fn usable_codes(storage: Arc<dyn StorageAdapter>) {
    let mut runtime = irongate::config::environment::RuntimeAuthConfig::for_tests();
    runtime.lookup_secret = irongate::config::environment::LookupSecret::from_string(
        std::str::from_utf8(SECRET).unwrap().to_owned(),
    )
    .unwrap();
    let password_subject = crate::support::registration::register_verified_user(
        &AuthStore::new(storage.clone()),
        &runtime,
        &crate::support::registration::Mailbox::default(),
        "shared@example.test",
        "existing password account owner secret",
    )
    .await;
    let email_digest = lookup_digest(SECRET, LookupFamily::Email, "shared@example.test");
    let password_digest = lookup_digest(
        SECRET,
        LookupFamily::PasswordIdentity,
        "shared@example.test",
    );
    let password_keys = [
        StoreKey::account(password_subject.as_str()),
        StoreKey::identity("password", &password_digest),
        StoreKey::identity_by_subject(password_subject.as_str(), "password", &password_digest),
        StoreKey::password_user(&email_digest),
        StoreKey::password_user_by_subject(password_subject.as_str(), &email_digest),
    ];
    let mut subjects = Vec::new();
    for provider in [IdentityProvider::Google, IdentityProvider::Apple] {
        let f = Fixture::new(storage.clone(), provider, "same-external-id");
        let password_before = f.rows(&password_keys).await;
        assert!(password_before.iter().all(Option::is_some));
        let code = code_digest();
        let props = properties("shared@example.test", "First profile");
        let (tx, task) = f.pause(props.clone(), &code).await;
        let keys = plan_keys(&f, &code, &tx.operations, Kind::New);
        assert!(
            f.rows(&keys).await.iter().all(Option::is_none),
            "preparation writes nothing"
        );
        tx.commit();
        let subject = task.await.unwrap().unwrap();
        assert_ne!(
            subject.as_str(),
            password_subject.as_str(),
            "provider email cannot claim an existing password account"
        );
        f.assert_committed(&subject, &code, &props, 0).await;
        assert!(f
            .store
            .take_authorization_code(&code)
            .await
            .unwrap()
            .is_some());
        assert!(f
            .store
            .take_authorization_code(&code)
            .await
            .unwrap()
            .is_none());
        let original = f.rows(&keys[..3]).await;
        let mut account = original[0].clone().unwrap();
        account["credential_generation"] = json!(7);
        f.replace(&keys[0], account).await;
        for props in [
            properties("changed@example.test", "Changed profile"),
            json!({"name": "No email"}),
        ] {
            *f.clock.lock().unwrap() += Duration::seconds(1);
            let code = code_digest();
            let before = f.rows(&keys[..3]).await;
            let (tx, task) = f.pause(props.clone(), &code).await;
            plan_keys(&f, &code, &tx.operations, Kind::Active);
            assert_eq!(f.rows(&keys[..3]).await, before);
            tx.commit();
            let returning = task.await.unwrap().unwrap();
            assert_eq!(returning.as_str(), subject.as_str());
            f.assert_committed(&subject, &code, &props, 7).await;
            let after = f.rows(&keys[..3]).await;
            assert_eq!(after[0], before[0], "account is guarded, not rewritten");
            assert_eq!(after[2], original[2], "reverse index is stable");
            assert_eq!(
                after[1].as_ref().unwrap()["created_at"],
                original[1].as_ref().unwrap()["created_at"]
            );
            assert!(f
                .store
                .take_authorization_code(&code)
                .await
                .unwrap()
                .is_some());
        }
        subjects.push(subject.as_str().to_owned());
        assert_eq!(f.rows(&password_keys).await, password_before, "provider issuance must leave the password credential and its account/indexes unchanged");
    }
    assert_ne!(
        subjects[0], subjects[1],
        "equal emails and external subjects do not link providers"
    );
}

pub async fn independent_conditions(storage: Arc<dyn StorageAdapter>) {
    for kind in [Kind::New, Kind::Active, Kind::Deleted] {
        for target in 0..4 {
            if kind == Kind::Active && target == 2 {
                continue;
            }
            let f = Fixture::new(
                storage.clone(),
                IdentityProvider::Google,
                &format!("conditions-{kind:?}-{target}"),
            );
            f.seed(kind).await;
            let code = code_digest();
            let (tx, task) = f
                .pause(properties("candidate@example.test", "Candidate"), &code)
                .await;
            let keys = plan_keys(&f, &code, &tx.operations, kind);
            let key = &keys[target];
            if target == 3 {
                // Collide with a real, usable code belonging to an unrelated provider identity.
                let owner = Fixture::new(
                    storage.clone(),
                    IdentityProvider::Apple,
                    &format!("code-owner-{kind:?}"),
                );
                owner
                    .issue(properties("owner@example.test", "Owner"), &code)
                    .await;
            } else {
                let mut value = f
                    .rows(std::slice::from_ref(key))
                    .await
                    .pop()
                    .unwrap()
                    .unwrap_or(json!({"occupied": true}));
                if kind == Kind::Active && target == 0 {
                    value["created_at"] = json!(Utc::now() - Duration::days(1));
                }
                if kind != Kind::New && target == 1 {
                    value["last_seen_at"] = json!(Utc::now() - Duration::days(1));
                }
                f.replace(key, value).await;
            }
            let before = f.rows(&keys).await;
            tx.commit();
            rejected(task.await.unwrap());
            assert_eq!(
                f.rows(&keys).await,
                before,
                "{kind:?} condition {target} must roll back all other writes"
            );
            if target == 3 {
                assert!(
                    f.store
                        .take_authorization_code(&code)
                        .await
                        .unwrap()
                        .is_some(),
                    "collision winner survives"
                );
            }
        }
    }
}

pub async fn prepare_rejections(storage: Arc<dyn StorageAdapter>) {
    let f = Fixture::new(storage.clone(), IdentityProvider::Google, "invalid-session");
    for provider in [IdentityProvider::Password, IdentityProvider::Apple] {
        let result = f
            .store
            .prepare_provider_issuance(
                ProviderIdentityInput {
                    provider,
                    identity_digest: &f.digest,
                    properties: json!({}),
                },
                &f.session,
                TTL,
                DeletedIdentityReusePolicy::Immediate,
            )
            .await;
        rejected(result);
    }
    for selected in [None, Some("unknown".into())] {
        let mut session = f.session.clone();
        session.selected_provider = selected;
        rejected(
            f.store
                .prepare_provider_issuance(
                    ProviderIdentityInput {
                        provider: f.provider,
                        identity_digest: &f.digest,
                        properties: json!({}),
                    },
                    &session,
                    TTL,
                    DeletedIdentityReusePolicy::Immediate,
                )
                .await,
        );
    }
    rejected(
        f.store
            .prepare_provider_issuance(
                ProviderIdentityInput {
                    provider: f.provider,
                    identity_digest: "",
                    properties: json!({}),
                },
                &f.session,
                TTL,
                DeletedIdentityReusePolicy::Immediate,
            )
            .await,
    );
    for ttl in [0, u64::MAX] {
        assert!(matches!(
            f.store
                .prepare_provider_issuance(
                    ProviderIdentityInput {
                        provider: f.provider,
                        identity_digest: &f.digest,
                        properties: json!({})
                    },
                    &f.session,
                    ttl,
                    DeletedIdentityReusePolicy::Immediate
                )
                .await,
            Err(StorageError::DynamoDB(_))
        ));
    }
    assert!(f.rows(&[f.identity_key()]).await[0].is_none());
    assert!(storage
        .query_prefix(&["oauth:code"])
        .await
        .unwrap()
        .is_empty());

    let f = Fixture::new(storage, IdentityProvider::Apple, "invalid-binding");
    let subject = f.seed(Kind::Active).await.unwrap();
    let keys = f.keys(subject.as_str(), &code_digest());
    let original = f.rows(&keys).await;
    for (field, value) in [
        ("provider", json!("google")),
        ("identity_digest", json!("wrong")),
        ("subject", Value::Null),
        ("subject", json!("")),
        ("subject", json!("  ")),
        ("subject", json!("user_missing")),
    ] {
        let mut identity = original[1].clone().unwrap();
        identity[field] = value;
        f.replace(&keys[1], identity).await;
        let before = f.rows(&keys).await;
        rejected(
            f.prepare(&f.store, json!({}), DeletedIdentityReusePolicy::Immediate)
                .await,
        );
        assert_eq!(f.rows(&keys).await, before);
        f.replace(&keys[1], original[1].clone().unwrap()).await;
    }
    for variant in ["missing", "disabled", "deleted", "subject", "generation"] {
        let mut account = original[0].clone().unwrap();
        match variant {
            "missing" => {
                f.storage
                    .remove(&[keys[0].pk(), keys[0].sk()])
                    .await
                    .unwrap();
            }
            "disabled" => {
                f.store.disable_account(&subject).await.unwrap();
            }
            "deleted" => {
                account["status"] = json!("deleted");
                f.replace(&keys[0], account).await;
            }
            "subject" => {
                account["subject"] = json!("user_other");
                f.replace(&keys[0], account).await;
            }
            "generation" => {
                account
                    .as_object_mut()
                    .unwrap()
                    .remove("credential_generation");
                f.replace(&keys[0], account).await;
            }
            _ => unreachable!(),
        }
        let before = f.rows(&keys).await;
        let result = f
            .prepare(&f.store, json!({}), DeletedIdentityReusePolicy::Immediate)
            .await;
        if variant == "generation" {
            assert!(matches!(result, Err(StorageError::DynamoDB(_))));
        } else {
            rejected(result);
        }
        assert_eq!(f.rows(&keys).await, before);
        f.replace(&keys[0], original[0].clone().unwrap()).await;
    }
}

pub async fn reuse_policies(storage: Arc<dyn StorageAdapter>) {
    use DeletedIdentityReusePolicy::{AfterRetention, Immediate, Never};
    for (n, (policy, deadline, allowed)) in [
        (Never, Some(-1), false),
        (Immediate, Some(1), true),
        (AfterRetention, Some(1), false),
        (AfterRetention, Some(-1), true),
        (Immediate, None, false),
        (AfterRetention, None, false),
    ]
    .into_iter()
    .enumerate()
    {
        let f = Fixture::new(
            storage.clone(),
            IdentityProvider::Apple,
            &format!("reuse-{n}"),
        );
        let old = f.seed(Kind::Deleted).await.unwrap();
        let key = f.identity_key();
        let mut tombstone = f
            .rows(std::slice::from_ref(&key))
            .await
            .pop()
            .unwrap()
            .unwrap();
        tombstone["reusable_after"] = json!(deadline.map(|days| Utc::now() + Duration::days(days)));
        f.replace(&key, tombstone).await;
        let code = code_digest();
        let old_keys = f.keys(old.as_str(), &code);
        let before = f.rows(&old_keys).await;
        let props = properties("reused@example.test", "Reused profile");
        let result = f.prepare(&f.store, props.clone(), policy).await;
        assert_eq!(f.rows(&old_keys).await, before, "preparation is read-only");
        if allowed {
            let subject = f
                .store
                .finish_provider_issuance(result.unwrap(), &code)
                .await
                .unwrap();
            assert_ne!(subject.as_str(), old.as_str());
            f.assert_committed(&subject, &code, &props, 0).await;
            assert!(f
                .store
                .take_authorization_code(&code)
                .await
                .unwrap()
                .is_some());
        } else {
            assert!(matches!(result, Err(StorageError::AlreadyExists(_))));
        }
        assert_eq!(f.rows(&old_keys[..1]).await[0], before[0]);
        assert!(
            f.rows(&old_keys[2..3]).await[0].is_none(),
            "old subject index stays deleted"
        );
    }
}
