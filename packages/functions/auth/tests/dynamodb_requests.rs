#[allow(dead_code)]
#[path = "support/dynamo.rs"]
mod dynamo;
#[path = "support/dynamo_stub.rs"]
mod stub;

use axum::{
    body::to_bytes,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use irongate::{
    error::{IrongateError, OAuthError, StorageError},
    storage::{DynamoStorage, StorageAdapter, TransactOperation},
    store::{records::AuthorizationCodeRecord, AuthStore},
};
use serde_json::{json, Value};
use stub::{Logs, Reply, Stub};
use tracing::instrument::WithSubscriber;

fn storage(stub: &Stub) -> DynamoStorage {
    DynamoStorage::new(dynamo::client(&stub.endpoint), "wire-test".into())
}

fn put() -> TransactOperation {
    TransactOperation::Put {
        key: vec!["test".into(), "record".into()],
        value: json!({"secret":"request-marker"}),
        expiry: None,
        condition: None,
    }
}

fn cancellation(reasons: Option<Value>) -> Value {
    let mut value =
        json!({"__type":"TransactionCanceledException", "Message":"sensitive-service-marker"});
    if let Some(reasons) = reasons {
        value["CancellationReasons"] = reasons;
    }
    value
}

#[tokio::test]
async fn reads_are_consistent_on_all_pages_and_preserve_continuations() {
    let continuation = json!({"pk":{"S":"test"},"sk":{"S":"a"}});
    let item = json!({"pk":{"S":"test"},"sk":{"S":"b"},"value":{"S":"42"}});
    let stub = Stub::start(vec![
        Reply::json(StatusCode::OK, json!({"Item":item})),
        Reply::json(
            StatusCode::OK,
            json!({"Items":[],"LastEvaluatedKey":continuation}),
        ),
        Reply::json(StatusCode::OK, json!({"Items":[item]})),
        Reply::json(
            StatusCode::OK,
            json!({"Items":[],"LastEvaluatedKey":continuation}),
        ),
        Reply::json(StatusCode::OK, json!({"Items":[item]})),
    ])
    .await;
    let storage = storage(&stub);
    assert_eq!(storage.get(&["test", "b"]).await.unwrap(), Some(json!(42)));
    assert_eq!(
        storage.query_prefix(&["test"]).await.unwrap(),
        vec![(vec!["test".into(), "b".into()], json!(42))]
    );
    let (first, cursor) = storage
        .query_prefix_page(&["test"], 10, None)
        .await
        .unwrap();
    assert!(first.is_empty());
    let cursor = cursor.expect("empty page still has continuation");
    let (second, cursor) = storage
        .query_prefix_page(&["test"], 10, Some(&cursor))
        .await
        .unwrap();
    assert_eq!(second.len(), 1);
    assert!(cursor.is_none());
    let requests = stub.requests();
    assert_eq!(requests.len(), 5);
    assert!(requests[0].0.ends_with(".GetItem"));
    for (target, _) in &requests[1..] {
        assert!(target.ends_with(".Query"));
    }
    for (_, body) in &requests {
        assert_eq!(body["ConsistentRead"], true);
    }
    assert_eq!(requests[2].1["ExclusiveStartKey"], continuation);
    assert_eq!(requests[4].1["ExclusiveStartKey"], continuation);
    assert_eq!(requests[4].1["Limit"], 10);
}

#[tokio::test]
async fn cancellation_reasons_are_classified_without_leaking_service_data() {
    let cases = [
        (
            Some(json!([{"Code":"None"},{"Code":"ConditionalCheckFailed"}])),
            "condition",
        ),
        (Some(json!([{"Code":"TransactionConflict"}])), "conflict"),
        (
            Some(json!([{"Code":"ConditionalCheckFailed"},{"Code":"TransactionConflict"}])),
            "conflict",
        ),
        (
            Some(json!([{"Code":"ConditionalCheckFailed"},{"Code":"ValidationError"}])),
            "operational",
        ),
        (
            Some(json!([{"Code":"TransactionConflict"},{"Code":"ProvisionedThroughputExceeded"}])),
            "operational",
        ),
        (
            Some(json!([{"Code":"ConditionalCheckFailed"},{"Code":"ThrottlingError"}])),
            "operational",
        ),
        (
            Some(
                json!([{"Code":"ConditionalCheckFailed"},{"Code":"ItemCollectionSizeLimitExceeded"}]),
            ),
            "operational",
        ),
        (
            Some(json!([{"Code":"sensitive-unknown-code"}])),
            "operational",
        ),
        (
            Some(json!([{"Code":"ConditionalCheckFailed"},{"Code":"sensitive-unknown-code"}])),
            "operational",
        ),
        (
            Some(json!([{"Code":"ConditionalCheckFailed"},{}])),
            "operational",
        ),
        (Some(json!([{"Code":"None"}])), "operational"),
        (Some(json!([])), "operational"),
        (None, "operational"),
    ];
    for (mut reasons, expected) in cases {
        if let Some(Value::Array(reasons)) = &mut reasons {
            for reason in reasons {
                reason["Message"] = json!("sensitive-reason-marker");
                reason["Item"] = json!({"value":{"S":"sensitive-item-marker"}});
            }
        }
        let stub = Stub::start(vec![Reply::json(
            StatusCode::BAD_REQUEST,
            cancellation(reasons),
        )])
        .await;
        let logs = Logs::default();
        let error = storage(&stub)
            .transact(vec![put()])
            .with_subscriber(logs.subscriber())
            .await
            .unwrap_err();
        match expected {
            "condition" => assert!(matches!(error, StorageError::ConditionFailed(_))),
            "conflict" => assert!(matches!(error, StorageError::TransactionConflict)),
            _ => {
                assert!(matches!(error, StorageError::DynamoDB(_)));
                assert!(logs.text().contains("auth storage transaction failed"));
                assert!(logs.text().contains("unclassified_cancellation"));
            }
        }
        assert_safe(error, &logs.text()).await;
        assert_eq!(stub.requests().len(), 1, "no hidden test retries");
    }
}

async fn assert_safe(error: StorageError, diagnostics: &str) {
    let display = error.to_string();
    let oauth = OAuthError::ServerError(display.clone()).into_response();
    assert_eq!(oauth.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let oauth = to_bytes(oauth.into_body(), 4096).await.unwrap();
    let generic = IrongateError::Storage(error).into_response();
    assert_eq!(generic.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let generic = to_bytes(generic.into_body(), 4096).await.unwrap();
    for text in [
        display.as_str(),
        diagnostics,
        std::str::from_utf8(&oauth).unwrap(),
        std::str::from_utf8(&generic).unwrap(),
    ] {
        for marker in [
            "sensitive-",
            "request-marker",
            "CancellationReasons",
            "ConditionalCheckFailed",
        ] {
            assert!(!text.contains(marker), "unsafe transaction diagnostic");
        }
    }
}

#[tokio::test]
async fn service_and_malformed_response_failures_remain_operational() {
    let mut replies: Vec<Reply> = [
        "AccessDeniedException",
        "ThrottlingException",
        "ProvisionedThroughputExceededException",
        "ValidationException",
        "sensitive-unknown-code",
    ]
    .into_iter()
    .map(|code| {
        Reply::json(
            StatusCode::BAD_REQUEST,
            json!({"__type":code,"Message":"ConditionalCheckFailed sensitive-service-marker"}),
        )
    })
    .collect();
    replies.push(Reply {
        status: StatusCode::BAD_REQUEST,
        body: "sensitive-invalid-response".into(),
        headers: HeaderMap::new(),
    });
    for reply in replies {
        let stub = Stub::start(vec![reply]).await;
        let logs = Logs::default();
        let error = storage(&stub)
            .transact(vec![put()])
            .with_subscriber(logs.subscriber())
            .await
            .unwrap_err();
        assert!(matches!(error, StorageError::DynamoDB(_)));
        assert!(logs.text().contains("auth storage transaction failed"));
        assert_safe(error, &logs.text()).await;
        assert_eq!(stub.requests().len(), 1);
    }
}

#[tokio::test]
async fn transport_timeout_and_redirect_do_not_become_rejections_or_follow_redirects() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let storage = DynamoStorage::new(dynamo::client(&endpoint), "wire-test".into());
    // Keep the socket open without accepting/responding to exercise the bounded timeout.
    let logs = Logs::default();
    let timeout = storage
        .transact(vec![put()])
        .with_subscriber(logs.subscriber())
        .await
        .unwrap_err();
    assert!(matches!(timeout, StorageError::DynamoDB(_)));
    assert!(logs.text().contains("category=\"timeout\""));
    drop(listener);
    let dispatch = storage
        .transact(vec![put()])
        .with_subscriber(logs.subscriber())
        .await
        .unwrap_err();
    assert!(matches!(dispatch, StorageError::DynamoDB(_)));
    assert!(logs.text().contains("category=\"dispatch\""));
    let destination = Stub::start(vec![]).await;
    let mut reply = Reply::json(StatusCode::TEMPORARY_REDIRECT, json!({}));
    reply
        .headers
        .insert("location", destination.endpoint.parse().unwrap());
    let redirect = Stub::start(vec![reply]).await;
    let error = DynamoStorage::new(dynamo::client(&redirect.endpoint), "wire-test".into())
        .transact(vec![put()])
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::DynamoDB(_)));
    assert_eq!(redirect.requests().len(), 1);
    assert!(destination.requests().is_empty());
}

#[tokio::test]
async fn typed_consume_rejects_only_confirmed_condition_failures() {
    let record = AuthorizationCodeRecord {
        client_id: "web".into(),
        redirect_uri: "https://example.test/callback".into(),
        subject: "user_test".into(),
        subject_type: "user".into(),
        properties: json!({}),
        code_challenge: None,
        code_challenge_method: None,
        scope: "openid".into(),
        oidc_nonce: None,
        created_at: chrono::Utc::now(),
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
    };
    for (reason, condition) in [
        ("ConditionalCheckFailed", true),
        ("TransactionConflict", false),
        ("ValidationError", false),
    ] {
        let stub = Stub::start(vec![Reply::json(
            StatusCode::BAD_REQUEST,
            cancellation(Some(json!([{"Code":reason}]))),
        )])
        .await;
        let result = AuthStore::new(storage(&stub))
            .delete_authorization_code_if_current("digest", record.clone())
            .await;
        if condition {
            assert!(result.unwrap().is_none());
        } else {
            assert!(result.is_err());
        }
    }
}

#[test]
fn local_endpoint_guard_rejects_nonlocal_or_ambiguous_targets() {
    for endpoint in [
        "https://127.0.0.1:8000",
        "http://dynamodb.eu-west-1.amazonaws.com",
        "http://127.0.0.1.example.com",
        "http://192.0.2.1",
        "http://user@127.0.0.1",
        "http://:password@localhost",
        "http://localhost/table",
        "http://localhost?table=production",
        "http://localhost#fragment",
        "not a url",
    ] {
        assert!(
            dynamo::local_endpoint(endpoint).is_err(),
            "accepted unsafe endpoint"
        );
    }
    for endpoint in [
        "http://127.0.0.1:8000",
        "http://[::1]:8000",
        "http://localhost:8000",
    ] {
        assert!(dynamo::local_endpoint(endpoint).is_ok());
    }
    assert_eq!(
        dynamo::local_endpoint("http://localhost:8000")
            .unwrap()
            .host_str(),
        Some("127.0.0.1")
    );
}
