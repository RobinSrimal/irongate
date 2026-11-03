use super::{
    dynamo::{self, LocalTable, RawItem},
    recovery_fixture::{Fixture, NEW, OLD},
    transaction_gate::PausedStorage,
};
use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Duration, Utc};
use irongate::{
    core::passwords::hash_password_for_storage,
    error::StorageError,
    providers::password::{
        complete_password_reset, PasswordResetCompleteError, PasswordResetCompleteInput,
    },
    storage::{StorageAdapter, TransactOperation},
    store::{keys::StoreKey, AuthStore},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

pub(super) fn puts(ops: &[TransactOperation], pk: &str) -> bool {
    ops.iter().any(|op| matches!(op, TransactOperation::Put { key, .. } if key.first().is_some_and(|value| value == pk)))
}

pub(super) async fn snapshot(
    table: &LocalTable,
    f: &Fixture,
) -> BTreeMap<(String, String), RawItem> {
    let mut rows = BTreeMap::new();
    for pk in [
        StoreKey::account(&f.subject).pk().to_owned(),
        "password:user".into(),
        "password:reset".into(),
        StoreKey::password_reset_by_subject_pk(&f.subject),
        "oauth:code".into(),
        "oauth:refresh".into(),
        "oauth:refresh_family".into(),
        StoreKey::refresh_by_subject_pk(&f.subject),
        "oauth:refresh_by_client:web".into(),
        "oauth:refresh_by_client:phone".into(),
    ] {
        let result = table
            .client
            .query()
            .table_name(&table.name)
            .consistent_read(true)
            .key_condition_expression("pk = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()))
            .limit(64)
            .send()
            .await
            .unwrap();
        assert!(
            result.last_evaluated_key().is_none_or(|key| key.is_empty()),
            "bounded fixture exceeded"
        );
        for item in result.items() {
            rows.insert(
                (pk.clone(), item["sk"].as_s().unwrap().clone()),
                RawItem::from_attributes(item),
            );
        }
    }
    rows
}

pub(super) async fn restore(table: &LocalTable, key: &StoreKey, row: RawItem) {
    table
        .storage
        .set(
            &[key.pk(), key.sk()],
            row.value,
            row.expiry.map(|n| DateTime::from_timestamp(n, 0).unwrap()),
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_invalidates_only_the_subjects_older_credentials() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "owner@example.com").await;
        let other = Fixture::new(Arc::new(table.storage.clone()), "other@example.com").await;
        let other_link = other.request().await;
        let other_session = other.refresh("web").await;
        let code = f.login(&f.store, OLD).await.unwrap();
        let sessions = [f.refresh("web").await, f.refresh("phone").await];
        let first = f.request().await;
        let second = f.request().await;
        let proof_key = f.reset_key(&second);
        let proof = table.raw_item(&proof_key).await.unwrap();
        let expires_at: DateTime<Utc> =
            serde_json::from_value(proof.value["expires_at"].clone()).unwrap();
        assert_eq!(proof.expiry, Some(expires_at.timestamp()));
        assert_eq!(proof.value["credential_generation"], 0);
        assert!(!proof.value.to_string().contains(&second));
        let index = StoreKey::password_reset_by_subject(&f.subject, &f.digest(&second));
        assert_eq!(table.raw_item(&index).await.unwrap().expiry, proof.expiry);
        // Requesting links did not invalidate the existing session or code.
        let rotated = f
            .rotate(&f.store, &sessions[0].raw_token, "web")
            .await
            .unwrap();
        assert!(f
            .store
            .get_authorization_code(&f.code_digest(&code))
            .await
            .unwrap()
            .is_some());
        f.complete(&f.store, &second).await.unwrap();
        assert_eq!(f.generation().await, 1);
        assert!(f.password_is(NEW).await && !f.password_is(OLD).await);
        assert!(table.raw_item(&proof_key).await.is_none());
        assert!(table.raw_item(&index).await.is_none());
        assert!(table.raw_item(&f.reset_key(&first)).await.is_some());
        for token in [&first, &second] {
            assert!(matches!(
                f.complete(&f.store, token).await,
                Err(PasswordResetCompleteError::InvalidResetToken)
            ));
        }
        assert!(f
            .store
            .take_authorization_code(&f.code_digest(&code))
            .await
            .unwrap()
            .is_none());
        for (session, client) in sessions.iter().zip(["web", "phone"]) {
            assert!(f
                .rotate(&f.store, &session.raw_token, client)
                .await
                .is_err());
        }
        assert!(f.rotate(&f.store, &rotated.raw_token, "web").await.is_err());
        assert!(f.login(&f.store, OLD).await.is_err());
        let code = f.login(&f.store, NEW).await.unwrap();
        let record = f
            .store
            .take_authorization_code(&f.code_digest(&code))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.credential_generation, 1);
        let fresh = f
            .create_refresh(&f.store, "web", record.credential_generation)
            .await
            .unwrap();
        f.rotate(&f.store, &fresh.raw_token, "web").await.unwrap();
        for key in [
            StoreKey::account(&f.subject),
            StoreKey::password_user(&f.email_digest),
        ] {
            assert!(table.raw_item(&key).await.unwrap().expiry.is_none());
        }
        other
            .rotate(&other.store, &other_session.raw_token, "web")
            .await
            .unwrap();
        other.complete(&other.store, &other_link).await.unwrap();
        let fresh_link = f.request().await;
        f.complete(&f.store, &fresh_link).await.unwrap();
        assert_eq!(f.generation().await, 2);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn failed_recovery_condition_preserves_proof_and_password() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "rollback@example.com").await;
        let token = f.request().await;
        let prepared = f
            .store
            .prepare_password_reset(&f.digest(&token))
            .await
            .unwrap()
            .unwrap();
        let user_key = StoreKey::password_user(&f.email_digest);
        let mut row = table.raw_item(&user_key).await.unwrap();
        row.value["updated_at"] = json!(Utc::now() + Duration::seconds(1));
        restore(&table, &user_key, row).await;
        let before = snapshot(&table, &f).await;
        assert!(!f
            .store
            .finish_password_reset(prepared, &hash_password_for_storage(NEW).unwrap())
            .await
            .unwrap());
        assert!(
            snapshot(&table, &f).await == before,
            "rejected transaction had no effects"
        );
        assert!(f.password_is(OLD).await);
        assert_eq!(f.generation().await, 0);
        f.complete(&f.store, &token).await.unwrap();
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn invalid_reset_bindings_and_accounts_fail_without_writes() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "invalid@example.com").await;
        let other = Fixture::new(Arc::new(table.storage.clone()), "mismatch@example.com").await;
        let token = f.request().await;
        let proof_key = f.reset_key(&token);
        let user_key = StoreKey::password_user(&f.email_digest);
        let account_key = StoreKey::account(&f.subject);
        let weak = complete_password_reset(
            &f.store,
            &f.runtime,
            PasswordResetCompleteInput {
                token: &token,
                new_password: "short",
            },
        )
        .await;
        assert!(matches!(weak, Err(PasswordResetCompleteError::Password(_))));
        for (key, field, value) in [
            (&proof_key, "purpose", json!("verify_email")),
            (
                &proof_key,
                "expires_at",
                json!(Utc::now() - Duration::hours(1)),
            ),
            (&proof_key, "subject", json!(other.subject)),
            (&proof_key, "email_digest", json!(other.email_digest)),
            (&proof_key, "credential_generation", json!(1)),
            (&user_key, "verified", json!(false)),
            (&user_key, "deleted_at", json!(Utc::now())),
            (&user_key, "password_hash", serde_json::Value::Null),
            (&account_key, "status", json!("disabled")),
            (&account_key, "status", json!("deleted")),
        ] {
            let original = table.raw_item(key).await.unwrap();
            let mut changed = original.clone();
            changed.value[field] = value;
            restore(&table, key, changed).await;
            let before = snapshot(&table, &f).await;
            assert!(
                matches!(
                    f.complete(&f.store, &token).await,
                    Err(PasswordResetCompleteError::InvalidResetToken)
                ),
                "rejection for {field}"
            );
            assert!(
                snapshot(&table, &f).await == before,
                "invalid reset must not write"
            );
            restore(&table, key, original).await;
        }
        f.complete(&f.store, &token).await.unwrap();
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_rechecks_expiry_and_rejects_generation_overflow() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "expiry@example.com").await;
        let token = f.request().await;
        let proof = table.raw_item(&f.reset_key(&token)).await.unwrap();
        let clock = Arc::new(Mutex::new(Utc::now()));
        let reads_clock = clock.clone();
        let store = f
            .store
            .clone()
            .with_recovery_clock(move || *reads_clock.lock().unwrap());
        let prepared = store
            .prepare_password_reset(&f.digest(&token))
            .await
            .unwrap()
            .unwrap();
        *clock.lock().unwrap() = serde_json::from_value(proof.value["expires_at"].clone()).unwrap();
        let before = snapshot(&table, &f).await;
        assert!(!store
            .finish_password_reset(prepared, &hash_password_for_storage(NEW).unwrap())
            .await
            .unwrap());
        assert!(snapshot(&table, &f).await == before);

        let key = StoreKey::account(&f.subject);
        let mut row = table.raw_item(&key).await.unwrap();
        row.value["credential_generation"] = json!(u64::MAX);
        restore(&table, &key, row).await;
        let token = f.request().await;
        let before = snapshot(&table, &f).await;
        assert!(matches!(
            f.complete(&f.store, &token).await,
            Err(PasswordResetCompleteError::Storage(_))
        ));
        assert!(snapshot(&table, &f).await == before);
        assert!(f.password_is(OLD).await);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn missing_generation_is_not_silently_upgraded() {
    dynamo::run(|table| async move {
        let f = Fixture::new(Arc::new(table.storage.clone()), "required@example.com").await;
        let link = f.request().await;
        let code = f.login(&f.store, OLD).await.unwrap();
        let session = f.refresh("web").await;
        let keys = [
            StoreKey::account(&f.subject),
            f.reset_key(&link),
            StoreKey::authorization_code(&f.code_digest(&code)),
            StoreKey::refresh_token(&session.refresh_digest),
            StoreKey::refresh_family(&session.family_id),
        ];
        for (i, key) in keys.iter().enumerate() {
            let original = table.raw_item(key).await.unwrap();
            let mut changed = original.clone();
            changed
                .value
                .as_object_mut()
                .unwrap()
                .remove("credential_generation");
            restore(&table, key, changed).await;
            let before = snapshot(&table, &f).await;
            match i {
                0 | 1 => assert!(matches!(
                    f.complete(&f.store, &link).await,
                    Err(PasswordResetCompleteError::Storage(_))
                )),
                2 => assert!(f
                    .store
                    .get_authorization_code(&f.code_digest(&code))
                    .await
                    .is_err()),
                _ => assert!(matches!(
                    f.rotate(&f.store, &session.raw_token, "web").await,
                    Err(irongate::store::refresh::RefreshTokenStoreError::Storage(_))
                )),
            }
            assert!(snapshot(&table, &f).await == before);
            restore(&table, key, original).await;
        }
        f.complete(&f.store, &link).await.unwrap();
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn recovery_distinguishes_no_dispatch_from_lost_commit_response() {
    dynamo::run(|table| async move {
        for lost in [false, true] {
            let f = Fixture::new(
                Arc::new(table.storage.clone()),
                &format!("fault-{lost}@example.com"),
            )
            .await;
            let token = f.request().await;
            let other_link = f.request().await;
            let session = f.refresh("web").await;
            let before = snapshot(&table, &f).await;
            let (paused, mut gate) =
                PausedStorage::when(table.storage.clone(), |ops| puts(ops, "password:user"));
            let store = AuthStore::new(paused);
            let (result, ()) = tokio::join!(f.complete(&store, &token), async {
                let pending = gate.wait().await;
                if lost {
                    pending.lose_response();
                } else {
                    pending.fail_before_dispatch();
                }
            });
            assert!(matches!(
                result,
                Err(PasswordResetCompleteError::Storage(StorageError::DynamoDB(
                    _
                )))
            ));
            if lost {
                assert!(f.password_is(NEW).await);
                assert_eq!(f.generation().await, 1);
                assert!(table.raw_item(&f.reset_key(&token)).await.is_none());
                assert!(f.complete(&f.store, &other_link).await.is_err());
                assert!(f.rotate(&f.store, &session.raw_token, "web").await.is_err());
            } else {
                assert!(snapshot(&table, &f).await == before);
                assert!(f.password_is(OLD).await);
                f.rotate(&f.store, &session.raw_token, "web").await.unwrap();
                f.complete(&f.store, &token).await.unwrap();
            }
        }
    })
    .await;
}
