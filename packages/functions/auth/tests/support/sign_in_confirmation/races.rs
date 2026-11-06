use super::*;

pub async fn conditions(storage: Arc<dyn StorageAdapter>) {
    for existing in [false, true] {
        for creation in [false, true] {
            for target in 0..if creation { 5 } else { 7 } {
                let f = Fixture::new(
                    storage.clone(),
                    &format!("condition-{existing}-{creation}-{target}"),
                );
                if existing {
                    f.password_owner().await;
                }
                if !creation {
                    f.create().await;
                }
                let code = code_digest();
                let (tx, task) = f.pause(creation, &code).await;
                let keys: Vec<_> = tx
                    .operations
                    .iter()
                    .map(|op| {
                        let key = match op {
                            TransactOperation::Put { key, .. }
                            | TransactOperation::Delete { key, .. }
                            | TransactOperation::ConditionCheck { key, .. } => key,
                        };
                        StoreKey::new(key[0].clone(), key[1].clone())
                    })
                    .collect();
                let key = &keys[target];
                if !creation && target == 5 {
                    Provider::new(
                        storage.clone(),
                        IdentityProvider::Apple,
                        &format!("collision-{existing}"),
                    )
                    .issue(
                        properties(&format!("collision-{existing}@example.test"), "Winner"),
                        &code,
                    )
                    .await;
                } else {
                    let mut value = f.provider.rows(std::slice::from_ref(key)).await[0]
                        .clone()
                        .unwrap_or(json!({}));
                    value["competing_write"] = json!(true);
                    f.provider.replace(key, value).await;
                }
                let before = f.provider.rows(&keys).await;
                tx.commit();
                rejected(task.await.unwrap());
                assert_eq!(
                    f.provider.rows(&keys).await,
                    before,
                    "{existing}/{creation}/{target}"
                );
                if !creation && target == 5 {
                    assert!(f
                        .provider
                        .store
                        .take_authorization_code(&code)
                        .await
                        .unwrap()
                        .is_some());
                }
            }
        }
    }
}

pub async fn lifecycle(storage: Arc<dyn StorageAdapter>) {
    let hash =
        irongate::core::passwords::hash_password_for_storage("new account password for recovery")
            .unwrap();
    for creation in [false, true] {
        for action in ["reset", "disable", "delete", "owner", "index", "identity"] {
            let f = Fixture::new(storage.clone(), &format!("lifecycle-{creation}-{action}"));
            let owner = f.password_owner().await;
            if !creation {
                f.create().await;
            }
            let code = code_digest();
            // For completion, mutate after creation but BEFORE preparing, proving the snapshot survived the wait.
            let paused = if creation {
                Some(f.pause(true, &code).await)
            } else {
                None
            };
            match action {
                "reset" => {
                    let digest = lookup_digest(SECRET, LookupFamily::Email, &f.email);
                    let reset = lookup_digest(SECRET, LookupFamily::PasswordReset, &f.email);
                    f.provider
                        .store
                        .create_password_reset(
                            &reset,
                            &digest,
                            owner.as_str(),
                            Utc::now() + Duration::minutes(10),
                        )
                        .await
                        .unwrap();
                    let prepared = f
                        .provider
                        .store
                        .prepare_password_reset(&reset)
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(f
                        .provider
                        .store
                        .finish_password_reset(prepared, &hash)
                        .await
                        .unwrap());
                }
                "disable" => {
                    f.provider.store.disable_account(&owner).await.unwrap();
                }
                "delete" => {
                    f.provider
                        .store
                        .delete_account(&owner, DeletedIdentityReusePolicy::Immediate, 0)
                        .await
                        .unwrap();
                }
                "owner" | "index" => {
                    let keys =
                        provider_contract::ownership::ownership_keys(owner.as_str(), &f.email);
                    let key = &keys[usize::from(action == "index")];
                    let mut value = f.provider.rows(std::slice::from_ref(key)).await[0]
                        .clone()
                        .unwrap();
                    value["subject"] = json!("user_other");
                    f.provider.replace(key, value).await;
                }
                "identity" => {
                    f.provider
                        .issue(properties(&f.email, "Competing identity"), &code_digest())
                        .await;
                }
                _ => unreachable!(),
            }
            let keys = if creation {
                vec![
                    StoreKey::account(owner.as_str()),
                    f.provider.identity_key(),
                    f.key(),
                    StoreKey::authorization_code(&code),
                ]
            } else {
                f.keys(&code).await
            };
            let before = f.provider.rows(&keys).await;
            if let Some((tx, task)) = paused {
                tx.commit();
                rejected(task.await.unwrap());
            } else {
                rejected(f.finish(&code).await);
            }
            assert_eq!(f.provider.rows(&keys).await, before);
        }
    }
}

pub async fn competing_claims(storage: Arc<dyn StorageAdapter>) {
    for same_identity in [false, true] {
        for first_wins in [false, true] {
            let label = format!("competing-{same_identity}-{first_wins}");
            let first = Fixture::new(storage.clone(), &label);
            let second_label = format!("{label}-second");
            let mut second = Fixture::new(
                storage.clone(),
                if same_identity { &label } else { &second_label },
            );
            second.email = first.email.clone();
            first.create().await;
            second.create().await;
            let codes = [code_digest(), code_digest()];
            let keys = [first.keys(&codes[0]).await, second.keys(&codes[1]).await].concat();
            let (a, aj) = first.pause(false, &codes[0]).await;
            let (b, bj) = second.pause(false, &codes[1]).await;
            let (win, wj, lose, lj) = if first_wins {
                (a, aj, b, bj)
            } else {
                (b, bj, a, aj)
            };
            win.commit();
            wj.await.unwrap().unwrap();
            let before = first.provider.rows(&keys).await;
            lose.commit();
            rejected(lj.await.unwrap());
            assert_eq!(first.provider.rows(&keys).await, before);
        }
    }
    for confirmation_wins in [false, true] {
        for provider in [true, false] {
            let f = Fixture::new(
                storage.clone(),
                &format!("claim-{confirmation_wins}-{provider}"),
            );
            f.create().await;
            let code = code_digest();
            let keys = f.keys(&code).await;
            let (confirmation, cj) = f.pause(false, &code).await;
            if provider {
                let other = Provider::new(
                    storage.clone(),
                    IdentityProvider::Apple,
                    &format!("other-{confirmation_wins}"),
                );
                let (tx, job) = other
                    .pause(properties(&f.email, "Other"), &code_digest())
                    .await;
                if confirmation_wins {
                    confirmation.commit();
                    cj.await.unwrap().unwrap();
                    tx.commit();
                    rejected(job.await.unwrap());
                } else {
                    tx.commit();
                    job.await.unwrap().unwrap();
                    let before = f.provider.rows(&keys).await;
                    confirmation.commit();
                    rejected(cj.await.unwrap());
                    assert_eq!(f.provider.rows(&keys).await, before);
                }
            } else {
                let email = lookup_digest(SECRET, LookupFamily::Email, &f.email);
                let proof = lookup_digest(SECRET, LookupFamily::EmailVerification, &f.email);
                f.provider
                    .store
                    .create_pending_password_user(&email, &f.email)
                    .await
                    .unwrap();
                f.provider
                    .store
                    .create_email_verification(&proof, &email, Utc::now() + Duration::minutes(5))
                    .await
                    .unwrap();
                let prepared = f
                    .provider
                    .store
                    .prepare_password_registration(
                        &proof,
                        SECRET,
                        DeletedIdentityReusePolicy::Immediate,
                    )
                    .await
                    .unwrap()
                    .unwrap();
                let hash = irongate::core::passwords::hash_password_for_storage(
                    "password chosen with proof",
                )
                .unwrap();
                if confirmation_wins {
                    confirmation.commit();
                    cj.await.unwrap().unwrap();
                    assert!(f
                        .provider
                        .store
                        .finish_password_registration(prepared, &hash)
                        .await
                        .unwrap()
                        .is_none());
                    assert!(f
                        .provider
                        .store
                        .get_password_user_by_email_digest(&email)
                        .await
                        .unwrap()
                        .unwrap()
                        .password_hash
                        .is_none());
                } else {
                    f.provider
                        .store
                        .finish_password_registration(prepared, &hash)
                        .await
                        .unwrap()
                        .unwrap();
                    let before = f.provider.rows(&keys).await;
                    confirmation.commit();
                    rejected(cj.await.unwrap());
                    assert_eq!(f.provider.rows(&keys).await, before);
                }
            }
        }
    }
}

pub async fn uncertain_outcomes(storage: Arc<dyn StorageAdapter>) {
    for creation in [false, true] {
        for lost in [false, true] {
            let f = Fixture::new(storage.clone(), &format!("outcome-{creation}-{lost}"));
            if !creation {
                f.create().await;
            }
            let code = code_digest();
            let before = f.row().await;
            let (tx, job) = f.pause(creation, &code).await;
            if lost {
                tx.lose_response();
            } else {
                tx.fail_before_dispatch();
            }
            assert!(matches!(job.await.unwrap(), Err(StorageError::DynamoDB(_))));
            if lost && creation {
                assert!(f.row().await.is_some());
                assert!(f
                    .provider
                    .store
                    .get_identity(IdentityProvider::Google, &f.provider.digest)
                    .await
                    .unwrap()
                    .is_none());
            } else if lost {
                assert!(f.row().await.is_none());
                assert!(f
                    .provider
                    .store
                    .take_authorization_code(&code)
                    .await
                    .unwrap()
                    .is_some());
            } else {
                assert_eq!(f.row().await, before);
                assert!(f
                    .provider
                    .store
                    .get_authorization_code(&code)
                    .await
                    .unwrap()
                    .is_none());
            }
        }
    }
}
