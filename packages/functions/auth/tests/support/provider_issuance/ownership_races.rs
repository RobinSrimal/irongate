use super::{ownership::*, *};
use irongate::core::passwords::hash_password_for_storage;

pub async fn cross_provider_claims(storage: Arc<dyn StorageAdapter>) {
    for existing in [false, true] {
        for google_first in [false, true] {
            let label = format!("cross-{existing}-{google_first}");
            let google = Fixture::new(storage.clone(), IdentityProvider::Google, &label);
            let apple = Fixture::new(storage.clone(), IdentityProvider::Apple, &label);
            let email = format!("{label}@example.test");
            let owner = if existing {
                Some(password_owner(&google, &email).await)
            } else {
                None
            };
            let codes = [code_digest(), code_digest()];
            let (gt, gj) = google.pause(properties(&email, "Google"), &codes[0]).await;
            let (at, aj) = apple.pause(properties(&email, "Apple"), &codes[1]).await;
            let gk = plan_keys(&google, &codes[0], &gt.operations, Kind::New);
            let ak = plan_keys(&apple, &codes[1], &at.operations, Kind::New);
            let (first, first_job, second, second_job, loser, loser_keys) = if google_first {
                (gt, gj, at, aj, &apple, &ak)
            } else {
                (at, aj, gt, gj, &google, &gk)
            };
            first.commit();
            let subject = first_job.await.unwrap().unwrap();
            let all_keys = [gk.clone(), ak.clone()].concat();
            let before = google.rows(&all_keys).await;
            second.commit();
            let result = second_job.await.unwrap();
            if let Some(owner) = owner {
                assert_eq!(subject.as_str(), owner.as_str());
                assert_eq!(result.unwrap().as_str(), owner.as_str());
            } else {
                rejected(result);
                assert_eq!(google.rows(&all_keys).await, before);
                assert!(google
                    .rows(&loser_keys[..4])
                    .await
                    .iter()
                    .all(Option::is_none));
                assert!(google
                    .rows(&loser_keys[5..])
                    .await
                    .iter()
                    .all(Option::is_none));
                let fresh = loser
                    .issue(properties(&email, "Fresh sign-in"), &code_digest())
                    .await;
                assert_eq!(fresh.as_str(), subject.as_str());
            }
        }
    }
}

pub async fn independent_conditions(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider) {
    for existing in [false, true] {
        for target in 0..6 {
            let label = format!("provider-condition-{existing}-{target}");
            let f = Fixture::new(storage.clone(), provider, &label);
            let email = format!("{label}@example.test");
            if existing {
                password_owner(&f, &email).await;
            }
            let code = code_digest();
            let (tx, task) = f.pause(properties(&email, "Candidate"), &code).await;
            let keys = plan_keys(&f, &code, &tx.operations, Kind::New);
            let mut changed = f.rows(&keys).await[target].clone().unwrap_or(json!({}));
            changed["competing_write"] = json!(true);
            f.replace(&keys[target], changed).await;
            let before = f.rows(&keys).await;
            tx.commit();
            rejected(task.await.unwrap());
            assert_eq!(
                f.rows(&keys).await,
                before,
                "existing={existing}, condition={target}"
            );
        }
    }
}

pub async fn competing_claims(storage: Arc<dyn StorageAdapter>, provider: IdentityProvider) {
    for same_identity in [false, true] {
        for first_wins in [false, true] {
            let label = format!("provider-competition-{same_identity}-{first_wins}");
            let second_label = format!("{label}-second");
            let first = Fixture::new(storage.clone(), provider, &label);
            let second = Fixture::new(
                storage.clone(),
                provider,
                if same_identity { &label } else { &second_label },
            );
            let email = format!("{label}@example.test");
            let codes = [code_digest(), code_digest()];
            let (tx1, task1) = first.pause(properties(&email, "First"), &codes[0]).await;
            let (tx2, task2) = second.pause(properties(&email, "Second"), &codes[1]).await;
            let keys1 = plan_keys(&first, &codes[0], &tx1.operations, Kind::New);
            let keys2 = plan_keys(&second, &codes[1], &tx2.operations, Kind::New);
            let (win, win_task, lose, lose_task, loser_keys, winner_code) = if first_wins {
                (tx1, task1, tx2, task2, keys2.clone(), &codes[0])
            } else {
                (tx2, task2, tx1, task1, keys1.clone(), &codes[1])
            };
            win.commit();
            let subject = win_task.await.unwrap().unwrap();
            let keys = [keys1, keys2].concat();
            let before = first.rows(&keys).await;
            lose.commit();
            rejected(lose_task.await.unwrap());
            assert_eq!(first.rows(&keys).await, before);
            for index in [0, 2, 3, 5] {
                assert!(first.rows(&loser_keys[index..=index]).await[0].is_none());
            }
            assert_eq!(
                first.rows(&ownership_keys(subject.as_str(), &email)).await[0]
                    .as_ref()
                    .unwrap()["subject"],
                subject.as_str()
            );
            assert!(first
                .store
                .take_authorization_code(winner_code)
                .await
                .unwrap()
                .is_some());
        }
    }
}

pub async fn password_completion_races(
    storage: Arc<dyn StorageAdapter>,
    provider: IdentityProvider,
) {
    let hash = hash_password_for_storage("password chosen with the email proof").unwrap();
    for provider_wins in [false, true] {
        let f = Fixture::new(
            storage.clone(),
            provider,
            &format!("password-race-{provider_wins}"),
        );
        let email = format!("password-race-{provider_wins}@example.test");
        let email_digest = lookup_digest(SECRET, LookupFamily::Email, &email);
        let proof = lookup_digest(SECRET, LookupFamily::EmailVerification, &email);
        f.store
            .create_pending_password_user(&email_digest, &email)
            .await
            .unwrap();
        f.store
            .create_email_verification(&proof, &email_digest, Utc::now() + Duration::hours(1))
            .await
            .unwrap();
        let pending_keys = [
            StoreKey::password_user(&email_digest),
            StoreKey::password_verification(&proof),
        ];
        let pending_before = f.rows(&pending_keys).await;
        let prepared = f
            .store
            .prepare_password_registration(&proof, SECRET, DeletedIdentityReusePolicy::Immediate)
            .await
            .unwrap()
            .unwrap();
        let (paused, mut gate) = PausedStorage::when(storage.clone(), |ops| {
            ops.iter().any(|op|
            matches!(op, TransactOperation::Delete { key, .. } if key[0] == "password:verify"))
        });
        let store = AuthStore::new(paused);
        let chosen_hash = hash.clone();
        let password = tokio::spawn(async move {
            store
                .finish_password_registration(prepared, &chosen_hash)
                .await
        });
        let password_tx = gate.wait().await;
        let password_keys: Vec<_> = password_tx
            .operations
            .iter()
            .filter_map(|op| match op {
                TransactOperation::Put { key, value, .. } if key[0].starts_with("account:") => {
                    Some(value["subject"].as_str().unwrap().to_string())
                }
                _ => None,
            })
            .collect();
        let password_candidate = &password_keys[0];
        let code = code_digest();
        let (provider_tx, provider_task) = f.pause(properties(&email, "Provider"), &code).await;
        let provider_keys = plan_keys(&f, &code, &provider_tx.operations, Kind::New);
        if provider_wins {
            provider_tx.commit();
            let subject = provider_task.await.unwrap().unwrap();
            let before = f.rows(&provider_keys).await;
            password_tx.commit();
            assert!(password.await.unwrap().unwrap().is_none());
            assert_eq!(f.rows(&provider_keys).await, before);
            assert_eq!(f.rows(&pending_keys).await, pending_before);
            for key in [
                StoreKey::account(password_candidate),
                StoreKey::password_user_by_subject(password_candidate, &email_digest),
                StoreKey::email_by_subject(password_candidate, &email_digest),
            ] {
                assert!(f.rows(&[key]).await[0].is_none());
            }
            assert!(f
                .store
                .prepare_password_registration(
                    &proof,
                    SECRET,
                    DeletedIdentityReusePolicy::Immediate
                )
                .await
                .unwrap()
                .is_none());
            assert!(f
                .store
                .get_password_user_by_email_digest(&email_digest)
                .await
                .unwrap()
                .unwrap()
                .password_hash
                .is_none());
            assert_ne!(subject.as_str(), password_candidate);
        } else {
            password_tx.commit();
            let subject = password.await.unwrap().unwrap().unwrap();
            let before = f.rows(&provider_keys).await;
            provider_tx.commit();
            rejected(provider_task.await.unwrap());
            assert_eq!(f.rows(&provider_keys).await, before);
            assert!(f
                .rows(&provider_keys[..4])
                .await
                .iter()
                .all(Option::is_none));
            // Only a fresh sign-in may select the committed owner.
            let fresh = f
                .issue(properties(&email, "Fresh provider"), &code_digest())
                .await;
            assert_eq!(fresh.as_str(), subject.as_str());
        }
    }
}

pub async fn account_races_and_outcomes(
    storage: Arc<dyn StorageAdapter>,
    provider: IdentityProvider,
) {
    let hash = hash_password_for_storage("replacement account password").unwrap();
    for action in [
        "reset", "disable", "delete", "owner", "index", "fail", "lost",
    ] {
        let f = Fixture::new(
            storage.clone(),
            provider,
            &format!("provider-account-{action}"),
        );
        let email = format!("provider-account-{action}@example.test");
        let owner = password_owner(&f, &email).await;
        let code = code_digest();
        let props = properties(&email, "Provider");
        let (tx, task) = f.pause(props.clone(), &code).await;
        let keys = plan_keys(&f, &code, &tx.operations, Kind::New);
        match action {
            "reset" => {
                let digest = lookup_digest(SECRET, LookupFamily::Email, &email);
                f.store
                    .create_password_reset(
                        "reset-proof-digest",
                        &digest,
                        owner.as_str(),
                        Utc::now() + Duration::minutes(10),
                    )
                    .await
                    .unwrap();
                let reset = f
                    .store
                    .prepare_password_reset("reset-proof-digest")
                    .await
                    .unwrap()
                    .unwrap();
                assert!(f.store.finish_password_reset(reset, &hash).await.unwrap());
            }
            "disable" => {
                f.store.disable_account(&owner).await.unwrap();
            }
            "delete" => {
                f.store
                    .delete_account(&owner, DeletedIdentityReusePolicy::Immediate, 0)
                    .await
                    .unwrap();
            }
            "owner" | "index" => {
                let key = &keys[if action == "owner" { 4 } else { 5 }];
                let mut changed = f.rows(std::slice::from_ref(key)).await[0].clone().unwrap();
                changed["subject"] = json!("user_other");
                f.replace(key, changed).await;
            }
            _ => {}
        }
        let before = f.rows(&keys).await;
        if action == "fail" {
            tx.fail_before_dispatch();
        } else if action == "lost" {
            tx.lose_response();
        } else {
            tx.commit();
        }
        let result = task.await.unwrap();
        if action == "fail" || action == "lost" {
            assert!(matches!(result, Err(StorageError::DynamoDB(_))));
        } else {
            rejected(result);
        }
        if action == "lost" {
            f.assert_committed(&owner, &code, &props, 0).await;
            assert!(f
                .store
                .take_authorization_code(&code)
                .await
                .unwrap()
                .is_some());
        } else {
            assert_eq!(f.rows(&keys).await, before);
            assert!(f
                .rows(&[f.identity_key(), StoreKey::authorization_code(&code)])
                .await
                .iter()
                .all(Option::is_none));
        }
    }
}
