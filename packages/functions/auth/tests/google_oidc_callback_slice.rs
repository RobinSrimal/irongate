use chrono::{Duration, Utc};
use irongate::crypto::hmac_lookup::{lookup_digest, LookupFamily};
use irongate::providers::google::{
    google_identity_digest, validate_google_id_token, GoogleIdTokenValidation, GoogleJwk,
    GoogleJwks,
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;

const LOOKUP_SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";
const GOOGLE_ISSUER: &str = "https://accounts.google.com";
const GOOGLE_CLIENT_ID: &str = "google-client-id";
const PROVIDER_NONCE: &str = "provider-nonce";
const TEST_KEY_ID: &str = "google-test-key";
const TEST_RSA_N: &str = "1okldhpIZquS0duQN26-ooaOE2ywCuYI9vMmS5iq6tIHqn62ApyNn4Ax6CAtjkdnAr9XexbCm6TdRKCh75p3KZMiiVH0Ws7iRQhncn-yHDAFLr8b5is7pKEZ53JqVtAAdk2LCBv38Ms58tYeZelU6Q8R6kaKuxsut5RanmS-YbsG59ThzNAZQLHjG1od8T_dCRpFQfOrP1UJa5sWRVhiBng09eH32A5E-onrbY2Ac7pFOpHpsir_rQutcjzjOwhO4jG1r0FPavXLi0yIisXH_cY5HgGkBUEccpcqESruOjwCBfxcPOMXdZtO2z73w9LqlBrjpohjGGe6QIUAsVoZbQ";
const TEST_RSA_E: &str = "AQAB";
const TEST_RSA_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDWiSV2Gkhmq5LR
25A3br6iho4TbLAK5gj28yZLmKrq0geqfrYCnI2fgDHoIC2OR2cCv1d7FsKbpN1E
oKHvmncpkyKJUfRazuJFCGdyf7IcMAUuvxvmKzukoRnncmpW0AB2TYsIG/fwyzny
1h5l6VTpDxHqRoq7Gy63lFqeZL5huwbn1OHM0BlAseMbWh3xP90JGkVB86s/VQlr
mxZFWGIGeDT14ffYDkT6iettjYBzukU6kemyKv+tC61yPOM7CE7iMbWvQU9q9cuL
TIiKxcf9xjkeAaQFQRxylyoRKu46PAIF/Fw84xd1m07bPvfD0uqUGuOmiGMYZ7pA
hQCxWhltAgMBAAECggEACPlU4v3gkf0Z3tkRTToUMB85xE/ooXlpFuvUTYkdCSmp
Zd/bIKdkzdm3w9J2+rR0d3lX2g+HnMXjEugaynBnKYrgVjx+/SIZ9bJIIe7RK4of
WrWCyoaYU1+ryVXXYzrN1bM9c6SqFM8VOoSWDNJ+/QyDDQ4zWKDYZrR4HiXvq6o/
/Qf9mPBLOh12p2IZ85L9f9fLTL4uYUUHSKKAqfWN/DLb7jinnUdok55I47qYuHtH
YFpQK0/3ZnCcbRIzooVOO3bSKbHXACSdZMrTKfk8ELFi1EjaMin6bgsS3SDlSikR
kT2t0rIvfUibh9WRZNtExLEtPPdk7izTDSlpVPHCjwKBgQD3R4kjLxYWIOzOfrGl
H1W1kKHTtKLpsgISGGdaBSSd4fnIpWDIkWs8PlBXadVNHVLBpTuF1s4VxnxJGBVL
XzHbgOohiv0e7M5DHm9TaSPBKANBc0qBlUKdYuE2GfligRuWrSStzfOTL/uh5hh0
cBm8LoZigW9ndw8v8ZIN5LQmywKBgQDeGf8/F5zEi4bbXzVzE3mmyyeeCl+BHJ0g
1Dspndm3/qA55pWcBZU2GKaqK8mXEZytjVM6geo9Z4l5hIH0Fcr03KZJ49zdASe/
U+e3nfjOq/TTrsqt7LjwEEVOGRKYy/jgS9rTEnBKYI+a51ysvT3grfphvo+K7k0R
vHsSH0oBpwKBgQCW5/4mDadB6+f4gNLyvTO2MUTBCRze13ZyCpiQFFFrVKv2Kg7t
d+lkg3bOUdUNUZbefHLd0+BC47WXee4M6FRp67t2qvacN9IMnfc8hQ5/42ZRPAW9
HRThLaXZOXK7DaWDh7i5pNU//ulmvSAxdvQNpqr2VJ1jHAKVtKv4dJkIjwKBgHDj
p+BKwS0JeldAkmtWZ8wGkLF8tkRq5dbM6PFjQUmLS6eCc2LlV40yhGwUa5e0pP11
yur/I69oU/EHEAKfnRROntsJzbYroydVn36t9cwejQeXXX9/xhSHQKLMja5KZsqi
46vLQHYdlIB4vpsyaSQtagmKkW1daKDuO2PfsX8bAoGBAO2lDrTjVUTi0OBSAfDx
zHIJszPyHY/nW4+rrVoE2GmDqFulXZ+gPq6b0G+GHJwzAt/RLMNpy//6D3rG6TzA
mn25y2Yr9HtgOb4aegL+FgOJ7CwINu9lgtbLAKOvYhj2QlVEca927VyUNRHkmeFY
yldT9HITVXtce9FVqgF83Lkz
-----END PRIVATE KEY-----"#;

#[derive(Debug, Serialize)]
struct TestGoogleClaims<'a> {
    iss: &'a str,
    sub: &'a str,
    aud: &'a str,
    exp: i64,
    iat: i64,
    nonce: &'a str,
    email: &'a str,
    email_verified: bool,
}

#[test]
fn google_identity_digest_uses_issuer_and_subject_not_email() {
    let digest = google_identity_digest(LOOKUP_SECRET, GOOGLE_ISSUER, "google-sub-a");
    let same = google_identity_digest(LOOKUP_SECRET, GOOGLE_ISSUER, "google-sub-a");
    let different_sub = google_identity_digest(LOOKUP_SECRET, GOOGLE_ISSUER, "google-sub-b");
    let different_issuer = google_identity_digest(
        LOOKUP_SECRET,
        "https://other.accounts.example",
        "google-sub-a",
    );

    assert_eq!(digest, same);
    assert_ne!(digest, different_sub);
    assert_ne!(digest, different_issuer);
    assert_eq!(
        digest,
        lookup_digest(
            LOOKUP_SECRET,
            LookupFamily::GoogleIdentity,
            "https://accounts.google.com\ngoogle-sub-a",
        )
    );
}

#[test]
fn valid_google_id_token_validates_signature_nonce_and_claims() {
    let now = Utc::now();
    let token = sign_google_id_token(TestGoogleClaims {
        iss: GOOGLE_ISSUER,
        sub: "google-subject",
        aud: GOOGLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: "user@example.com",
        email_verified: true,
    });

    let claims = validate_google_id_token(&token, &jwks(), validation(now))
        .expect("valid google token");

    assert_eq!(claims.iss, GOOGLE_ISSUER);
    assert_eq!(claims.sub, "google-subject");
    assert_eq!(claims.email.as_deref(), Some("user@example.com"));
    assert_eq!(claims.email_verified, Some(true));
    assert_eq!(claims.nonce.as_deref(), Some(PROVIDER_NONCE));
}

#[test]
fn google_id_token_validation_rejects_wrong_security_claims() {
    let now = Utc::now();

    let wrong_issuer = sign_google_id_token(TestGoogleClaims {
        iss: "https://evil.example",
        sub: "google-subject",
        aud: GOOGLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: "user@example.com",
        email_verified: true,
    });
    assert!(validate_google_id_token(&wrong_issuer, &jwks(), validation(now)).is_err());

    let wrong_audience = sign_google_id_token(TestGoogleClaims {
        iss: GOOGLE_ISSUER,
        sub: "google-subject",
        aud: "other-client",
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: "user@example.com",
        email_verified: true,
    });
    assert!(validate_google_id_token(&wrong_audience, &jwks(), validation(now)).is_err());

    let wrong_nonce = sign_google_id_token(TestGoogleClaims {
        iss: GOOGLE_ISSUER,
        sub: "google-subject",
        aud: GOOGLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: "wrong-nonce",
        email: "user@example.com",
        email_verified: true,
    });
    assert!(validate_google_id_token(&wrong_nonce, &jwks(), validation(now)).is_err());

    let expired = sign_google_id_token(TestGoogleClaims {
        iss: GOOGLE_ISSUER,
        sub: "google-subject",
        aud: GOOGLE_CLIENT_ID,
        exp: (now - Duration::minutes(1)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: "user@example.com",
        email_verified: true,
    });
    assert!(validate_google_id_token(&expired, &jwks(), validation(now)).is_err());

    let future_iat = sign_google_id_token(TestGoogleClaims {
        iss: GOOGLE_ISSUER,
        sub: "google-subject",
        aud: GOOGLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: (now + Duration::minutes(10)).timestamp(),
        nonce: PROVIDER_NONCE,
        email: "user@example.com",
        email_verified: true,
    });
    assert!(validate_google_id_token(&future_iat, &jwks(), validation(now)).is_err());

    let empty_subject = sign_google_id_token(TestGoogleClaims {
        iss: GOOGLE_ISSUER,
        sub: "",
        aud: GOOGLE_CLIENT_ID,
        exp: (now + Duration::minutes(10)).timestamp(),
        iat: now.timestamp(),
        nonce: PROVIDER_NONCE,
        email: "user@example.com",
        email_verified: true,
    });
    assert!(validate_google_id_token(&empty_subject, &jwks(), validation(now)).is_err());
}

fn validation(now: chrono::DateTime<Utc>) -> GoogleIdTokenValidation<'static> {
    GoogleIdTokenValidation {
        issuer: GOOGLE_ISSUER,
        client_id: GOOGLE_CLIENT_ID,
        nonce: PROVIDER_NONCE,
        now,
    }
}

fn jwks() -> GoogleJwks {
    GoogleJwks {
        keys: vec![GoogleJwk {
            kty: "RSA".to_string(),
            kid: Some(TEST_KEY_ID.to_string()),
            use_: Some("sig".to_string()),
            alg: Some("RS256".to_string()),
            n: Some(TEST_RSA_N.to_string()),
            e: Some(TEST_RSA_E.to_string()),
        }],
    }
}

fn sign_google_id_token(claims: TestGoogleClaims<'_>) -> String {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KEY_ID.to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).expect("rsa key"),
    )
    .expect("sign google token")
}
