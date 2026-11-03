use super::{
    dynamo::{self, LocalTable, RawItem},
    transaction_gate::PausedStorage,
};
use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Duration, Utc};
use irongate::{
    error::StorageError,
    storage::{StorageAdapter, TransactOperation},
    store::{
        keys::StoreKey,
        refresh::{
            CreateRefreshTokenInput, CreatedRefreshToken, RefreshTokenStoreError,
            RevokeRefreshTokenOutcome,
        },
        AuthStore, IdentityProvider,
    },
};
use serde_json::json;
use std::collections::BTreeMap;

const SECRET: &[u8] = b"local-refresh-test-secret-32-bytes";
const CLIENT: &str = "test-client";

fn input(subject: &str, expires_at: DateTime<Utc>) -> CreateRefreshTokenInput {
    CreateRefreshTokenInput {
        client_id: CLIENT.into(),
        subject: subject.into(),
        subject_type: "user".into(),
        scope: "openid offline_access".into(),
        properties: json!({}),
        expires_at,
    }
}

async fn account(table: &LocalTable) -> (AuthStore, String) {
    let store = AuthStore::new(table.storage.clone());
    let subject = store
        .create_account_with_identity(IdentityProvider::Password, "identity", json!({}))
        .await
        .unwrap();
    (store, subject.as_str().into())
}

async fn fixture(table: &LocalTable) -> (AuthStore, String, CreatedRefreshToken) {
    let (store, subject) = account(table).await;
    let created = store
        .create_refresh_token(SECRET, input(&subject, Utc::now() + Duration::hours(1)))
        .await
        .unwrap();
    (store, subject, created)
}

fn token_keys(subject: &str, digest: &str) -> [StoreKey; 3] {
    [
        StoreKey::refresh_token(digest),
        StoreKey::refresh_by_subject(subject, digest),
        StoreKey::refresh_by_client(CLIENT, digest),
    ]
}

async fn assert_deadline(table: &LocalTable, key: &StoreKey, deadline: DateTime<Utc>) {
    let row = table.raw_item(key).await.expect("refresh row exists");
    assert_eq!(row.expiry, Some(deadline.timestamp()), "physical expiry");
    assert_eq!(row.value["expires_at"], json!(deadline), "logical expiry");
}

async fn snapshot(table: &LocalTable, subject: &str) -> BTreeMap<(String, String), RawItem> {
    let mut rows = BTreeMap::new();
    for pk in [
        "oauth:refresh".into(),
        "oauth:refresh_family".into(),
        StoreKey::refresh_by_subject_pk(subject),
        format!("oauth:refresh_by_client:{CLIENT}"),
    ] {
        let result = table
            .client
            .query()
            .table_name(&table.name)
            .consistent_read(true)
            .key_condition_expression("pk = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()))
            .limit(20)
            .send()
            .await
            .expect("query fixture partition");
        assert!(
            result.last_evaluated_key().is_none_or(|key| key.is_empty()),
            "fixture exceeds bounded snapshot"
        );
        for item in result.items() {
            let sk = item["sk"].as_s().expect("sort key").clone();
            rows.insert((pk.clone(), sk), RawItem::from_attributes(item));
        }
    }
    rows
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn creation_is_complete_or_rolls_back_on_a_real_condition_failure() {
    dynamo::run(|table| async move {
        let (store, subject) = account(&table).await;
        let deadline = Utc::now() + Duration::hours(1);
        let (paused, mut gate) = PausedStorage::new(table.storage.clone());
        let contender = AuthStore::new(paused);
        let (result, (before, conflicting_key)) = tokio::join!(
            contender.create_refresh_token(SECRET, input(&subject, deadline)),
            async {
                let pending = gate.wait().await;
                let key = pending
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        TransactOperation::Put { key, .. }
                            if key.first().map(String::as_str) == Some("oauth:refresh_family") =>
                        {
                            Some(key.clone())
                        }
                        _ => None,
                    })
                    .expect("family creation");
                // Occupy the generated family key after the auth flow has prepared its writes.
                // The backend, not the gate, must reject NotExists and roll back other items.
                table
                    .storage
                    .set(
                        &[&key[0], &key[1]],
                        json!({"competing_record":true}),
                        Some(deadline),
                    )
                    .await
                    .unwrap();
                let before = snapshot(&table, &subject).await;
                pending.commit();
                (before, key)
            }
        );
        assert!(matches!(result, Err(StorageError::AlreadyExists(_))));
        assert!(
            snapshot(&table, &subject).await == before,
            "no partial issuance"
        );
        table
            .storage
            .remove(&[&conflicting_key[0], &conflicting_key[1]])
            .await
            .unwrap();

        let created = store
            .create_refresh_token(SECRET, input(&subject, deadline))
            .await
            .unwrap();
        for key in token_keys(&subject, &created.refresh_digest)
            .into_iter()
            .chain([StoreKey::refresh_family(&created.family_id)])
        {
            assert_deadline(&table, &key, deadline).await;
            let row = table.raw_item(&key).await.unwrap();
            assert!(row.value["subject"] == subject);
            assert!(row.value["family_id"] == created.family_id);
            assert_eq!(row.value["client_id"], CLIENT);
            assert!(!row.value.to_string().contains(&created.raw_token));
        }
        assert_eq!(snapshot(&table, &subject).await.len(), 4);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn rotation_renews_family_ttl_and_revocation_preserves_deadlines() {
    dynamo::run(|table| async move {
        let (store, subject, first) = fixture(&table).await;
        let family_key = StoreKey::refresh_family(&first.family_id);
        let t1 = first.expires_at + Duration::hours(1);
        let rotated = store
            .rotate_refresh_token(SECRET, &first.raw_token, CLIENT, t1)
            .await
            .unwrap();
        for key in token_keys(&subject, &first.refresh_digest) {
            assert_deadline(&table, &key, first.expires_at).await;
        }
        for key in token_keys(&subject, &rotated.refresh_digest)
            .into_iter()
            .chain([family_key.clone()])
        {
            assert_deadline(&table, &key, t1).await;
        }
        assert!(
            table.raw_item(&family_key).await.unwrap().value["current_refresh_digest"]
                == rotated.refresh_digest
        );
        assert!(
            table
                .raw_item(&StoreKey::refresh_token(&first.refresh_digest))
                .await
                .unwrap()
                .value["replaced_by"]
                == rotated.refresh_digest
        );

        let before = snapshot(&table, &subject).await;
        assert_eq!(
            store
                .revoke_refresh_token_family(SECRET, &rotated.raw_token, "another-client")
                .await
                .unwrap(),
            RevokeRefreshTokenOutcome::NotFound
        );
        assert!(snapshot(&table, &subject).await == before);

        // A shortened caller-supplied lifetime is still the policy: no implicit max(T0,T1).
        let t2 = t1 - Duration::minutes(30);
        let next = store
            .rotate_refresh_token(SECRET, &rotated.raw_token, CLIENT, t2)
            .await
            .unwrap();
        assert_deadline(&table, &family_key, t2).await;
        for key in token_keys(&subject, &rotated.refresh_digest) {
            assert_deadline(&table, &key, t1).await;
        }
        assert_eq!(
            store
                .revoke_refresh_tokens_for_subject(&subject)
                .await
                .unwrap(),
            1
        );
        for key in token_keys(&subject, &next.refresh_digest)
            .into_iter()
            .chain([family_key.clone()])
        {
            assert_deadline(&table, &key, t2).await;
        }
        assert!(!table.raw_item(&family_key).await.unwrap().value["revoked_at"].is_null());
        assert!(!table
            .raw_item(&StoreKey::refresh_token(&next.refresh_digest))
            .await
            .unwrap()
            .value["revoked_at"]
            .is_null());
        assert!(matches!(
            store
                .rotate_refresh_token(SECRET, &next.raw_token, CLIENT, t1)
                .await,
            Err(RefreshTokenStoreError::Invalid)
        ));
        let revoked = snapshot(&table, &subject).await;
        assert_eq!(
            store
                .revoke_refresh_token_family(SECRET, &next.raw_token, CLIENT)
                .await
                .unwrap(),
            RevokeRefreshTokenOutcome::AlreadyRevoked
        );
        assert!(snapshot(&table, &subject).await == revoked);
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn stale_rotation_cannot_overwrite_a_winner_or_leave_successor_rows() {
    dynamo::run(|table| async move {
        let (store, subject, first) = fixture(&table).await;
        let (paused, mut gate) = PausedStorage::new(table.storage.clone());
        let contender = AuthStore::new(paused);
        let winner_deadline = first.expires_at + Duration::hours(1);
        let loser_deadline = first.expires_at + Duration::hours(2);
        let (loser, (winner, committed)) = tokio::join!(
            contender.rotate_refresh_token(SECRET, &first.raw_token, CLIENT, loser_deadline),
            async {
                let pending = gate.wait().await;
                let winner = store
                    .rotate_refresh_token(SECRET, &first.raw_token, CLIENT, winner_deadline)
                    .await
                    .unwrap();
                let committed = snapshot(&table, &subject).await;
                pending.commit();
                (winner, committed)
            }
        );
        assert!(matches!(
            loser,
            Err(RefreshTokenStoreError::Storage(
                StorageError::ConditionFailed(_)
            ))
        ));
        assert!(
            snapshot(&table, &subject).await == committed,
            "losing transaction made no partial writes"
        );
        assert_eq!(committed.len(), 7);
        assert_deadline(
            &table,
            &StoreKey::refresh_family(&first.family_id),
            winner_deadline,
        )
        .await;
        assert!(matches!(
            store
                .rotate_refresh_token(SECRET, &first.raw_token, CLIENT, loser_deadline)
                .await,
            Err(RefreshTokenStoreError::ReuseDetected)
        ));
        assert!(matches!(
            store
                .rotate_refresh_token(SECRET, &winner.raw_token, CLIENT, loser_deadline)
                .await,
            Err(RefreshTokenStoreError::Invalid)
        ));
        assert_deadline(
            &table,
            &StoreKey::refresh_family(&first.family_id),
            winner_deadline,
        )
        .await;
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn stale_rotation_cannot_resurrect_a_revoked_family() {
    dynamo::run(|table| async move {
        let (store, subject, first) = fixture(&table).await;
        let (paused, mut gate) = PausedStorage::new(table.storage.clone());
        let contender = AuthStore::new(paused);
        let (loser, revoked) = tokio::join!(
            contender.rotate_refresh_token(
                SECRET,
                &first.raw_token,
                CLIENT,
                first.expires_at + Duration::hours(1)
            ),
            async {
                let pending = gate.wait().await;
                assert_eq!(
                    store
                        .revoke_refresh_token_family(SECRET, &first.raw_token, CLIENT)
                        .await
                        .unwrap(),
                    RevokeRefreshTokenOutcome::Revoked
                );
                let revoked = snapshot(&table, &subject).await;
                pending.commit();
                revoked
            }
        );
        assert!(matches!(
            loser,
            Err(RefreshTokenStoreError::Storage(
                StorageError::ConditionFailed(_)
            ))
        ));
        assert!(
            snapshot(&table, &subject).await == revoked,
            "revocation and TTL survived stale writer"
        );
        assert_eq!(revoked.len(), 4);
        assert_deadline(
            &table,
            &StoreKey::refresh_family(&first.family_id),
            first.expires_at,
        )
        .await;
        assert_deadline(
            &table,
            &StoreKey::refresh_token(&first.refresh_digest),
            first.expires_at,
        )
        .await;
        assert!(matches!(
            store
                .rotate_refresh_token(SECRET, &first.raw_token, CLIENT, first.expires_at)
                .await,
            Err(RefreshTokenStoreError::Invalid)
        ));
    })
    .await;
}

#[tokio::test]
#[ignore = "requires explicit loopback DYNAMODB_LOCAL_ENDPOINT"]
async fn logical_token_and_family_expiry_do_not_depend_on_ttl_cleanup() {
    dynamo::run(|table| async move {
        let (store, subject, first) = fixture(&table).await;
        let token_key = StoreKey::refresh_token(&first.refresh_digest);
        let family_key = StoreKey::refresh_family(&first.family_id);
        for key in [&token_key, &family_key] {
            let original = table.raw_item(key).await.unwrap();
            let mut expired = original.value.clone();
            expired["expires_at"] = json!(Utc::now() - Duration::hours(1));
            table
                .storage
                .set(&[key.pk(), key.sk()], expired, Some(first.expires_at))
                .await
                .unwrap();
            assert!(
                table
                    .storage
                    .get(&[key.pk(), key.sk()])
                    .await
                    .unwrap()
                    .is_some(),
                "row still physically readable"
            );
            let before = snapshot(&table, &subject).await;
            assert!(matches!(
                store
                    .rotate_refresh_token(
                        SECRET,
                        &first.raw_token,
                        CLIENT,
                        first.expires_at + Duration::hours(1)
                    )
                    .await,
                Err(RefreshTokenStoreError::Invalid)
            ));
            assert!(snapshot(&table, &subject).await == before);
            table
                .storage
                .set(
                    &[key.pk(), key.sk()],
                    original.value,
                    Some(first.expires_at),
                )
                .await
                .unwrap();
        }
    })
    .await;
}
