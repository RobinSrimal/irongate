//! Signed controls shared by the two concrete provider validation suites.

use chrono::{DateTime, Utc};
use irongate::storage::StorageAdapter;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::{json, Value};

pub fn claims(issuer: &str, client: &str, nonce: &str, now: DateTime<Utc>) -> Value {
    json!({
        "iss": issuer, "sub": "provider-subject", "aud": client,
        "iat": now.timestamp(), "exp": now.timestamp() + 600, "nonce": nonce
    })
}

pub fn sign(claims: &Value, key: &EncodingKey, kid: Option<&str>) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = kid.map(str::to_owned);
    encode(&header, claims, key).expect("sign test claims")
}

pub fn corrupt_signature(token: &str) -> String {
    let start = token.rfind('.').expect("signature segment") + 1;
    let mut bytes = token.as_bytes().to_vec();
    bytes[start] = if bytes[start] == b'A' { b'B' } else { b'A' };
    String::from_utf8(bytes).expect("ASCII JWT")
}

// Inspection of this memory fixture only, not a production DynamoDB query contract.
pub async fn identity_state(storage: &crate::support::TestStorage) -> Vec<(Vec<String>, Value)> {
    let mut rows = Vec::new();
    for prefix in [
        "account:",
        "identity:",
        "identity_by_subject:",
        "oauth:code",
    ] {
        rows.extend(
            storage
                .query_prefix(&[prefix])
                .await
                .expect("inspect fixture"),
        );
    }
    rows
}

macro_rules! provider_validation_tests {
    ($error:ident, $validate:ident, $issuer:ident, $client:ident) => {
        #[test]
        fn provider_claim_rejections_are_isolated() {
            let now = chrono::DateTime::from_timestamp(1_735_689_600, 0).unwrap();
            let baseline = oidc_validation::claims($issuer, $client, PROVIDER_NONCE, now);
            let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
            let verify = |claims: &serde_json::Value| {
                $validate(
                    &oidc_validation::sign(claims, &key, Some(TEST_KEY_ID)),
                    &jwks(),
                    validation(now),
                )
            };
            assert!(verify(&baseline).is_ok(), "signed control must pass");
            macro_rules! rejects {
                ($field:literal, $value:expr, $variant:ident) => {{
                    let mut invalid = baseline.clone();
                    invalid[$field] = serde_json::json!($value);
                    assert!(
                        matches!(verify(&invalid), Err($error::$variant)),
                        "{} must produce {}",
                        $field,
                        stringify!($variant)
                    );
                }};
            }
            rejects!("iss", "https://untrusted.example", InvalidIssuer);
            rejects!("aud", "other-client", InvalidAudience);
            rejects!("aud", ["other-client", "third-client"], InvalidAudience);
            rejects!("nonce", "wrong-nonce", InvalidNonce);
            rejects!("exp", now.timestamp(), Expired);
            rejects!("exp", now.timestamp() - 1, Expired);
            rejects!("iat", now.timestamp() + 61, FutureIssuedAt);
            rejects!("sub", "", MissingSubject);
            rejects!("sub", " \t", MissingSubject);
            let mut missing = baseline.clone();
            missing.as_object_mut().unwrap().remove("nonce");
            assert!(matches!(verify(&missing), Err($error::InvalidNonce)));

            let mut boundary = baseline.clone();
            boundary["iat"] = serde_json::json!(now.timestamp() + 60);
            assert!(verify(&boundary).is_ok());
            let mut audience = baseline.clone();
            audience["aud"] = serde_json::json!(["other-client", $client]);
            assert!(verify(&audience).is_ok());
            let mut subject = baseline;
            subject["sub"] = serde_json::json!(" unchanged-subject ");
            assert_eq!(verify(&subject).unwrap().sub, " unchanged-subject ");
        }

        #[test]
        fn provider_key_ids_are_required_and_matched_exactly() {
            let now = chrono::DateTime::from_timestamp(1_735_689_600, 0).unwrap();
            let claims = oidc_validation::claims($issuer, $client, PROVIDER_NONCE, now);
            let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
            let sign = |kid| oidc_validation::sign(&claims, &key, kid);
            assert!($validate(&sign(Some(TEST_KEY_ID)), &jwks(), validation(now)).is_ok());
            for kid in [None, Some("unknown"), Some(""), Some(" \t")] {
                let mut keys = jwks();
                if let Some(blank) = kid.filter(|value| value.trim().is_empty()) {
                    keys.keys[0].kid = Some(blank.into());
                }
                assert!(matches!(
                    $validate(&sign(kid), &keys, validation(now)),
                    Err($error::MissingKey)
                ));
            }
            let padded = format!(" {TEST_KEY_ID} ");
            assert!(matches!(
                $validate(&sign(Some(&padded)), &jwks(), validation(now)),
                Err($error::MissingKey)
            ));
        }

        #[test]
        fn provider_keys_and_signatures_are_checked_independently() {
            let now = chrono::DateTime::from_timestamp(1_735_689_600, 0).unwrap();
            let claims = oidc_validation::claims($issuer, $client, PROVIDER_NONCE, now);
            let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
            let token = oidc_validation::sign(&claims, &key, Some(TEST_KEY_ID));
            assert!($validate(&token, &jwks(), validation(now)).is_ok());

            let mut header = Header::new(Algorithm::HS256);
            header.kid = Some(TEST_KEY_ID.into());
            let hs = encode(
                &header,
                &claims,
                &EncodingKey::from_secret(b"test-only-untrusted-key"),
            )
            .unwrap();
            assert!(matches!(
                $validate(&hs, &jwks(), validation(now)),
                Err($error::UnsupportedAlgorithm)
            ));
            assert!(matches!(
                $validate(
                    &oidc_validation::corrupt_signature(&token),
                    &jwks(),
                    validation(now)
                ),
                Err($error::InvalidToken)
            ));
            let mut wrong_type = jwks();
            wrong_type.keys[0].kty = "EC".into();
            assert!(matches!(
                $validate(&token, &wrong_type, validation(now)),
                Err($error::UnsupportedAlgorithm)
            ));
            let mut wrong_alg = jwks();
            wrong_alg.keys[0].alg = Some("HS256".into());
            assert!(matches!(
                $validate(&token, &wrong_alg, validation(now)),
                Err($error::UnsupportedAlgorithm)
            ));

            for usage in ["enc", "", "other"] {
                let mut keys = jwks();
                keys.keys[0].use_ = Some(usage.into());
                assert!(matches!(
                    $validate(&token, &keys, validation(now)),
                    Err($error::InvalidKey)
                ));
            }
            for modulus in [true, false] {
                for component in [None, Some("!".into())] {
                    let mut keys = jwks();
                    if modulus {
                        keys.keys[0].n = component;
                    } else {
                        keys.keys[0].e = component;
                    }
                    assert!(matches!(
                        $validate(&token, &keys, validation(now)),
                        Err($error::InvalidKey)
                    ));
                }
            }
            for (alg, usage) in [(None, Some("sig")), (Some("RS256"), None), (None, None)] {
                let mut keys = jwks();
                keys.keys[0].alg = alg.map(str::to_owned);
                keys.keys[0].use_ = usage.map(str::to_owned);
                assert!($validate(&token, &keys, validation(now)).is_ok());
            }
        }
    };
}

pub(crate) use provider_validation_tests;
