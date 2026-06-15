use chrono::{TimeZone, Utc};
use irongate::config::apple::{AppleConfig, APPLE_AUDIENCE};
use irongate::crypto::signing::LocalEs256Signer;
use irongate::providers::apple::{
    build_apple_authorization_url, generate_apple_client_secret, AppleAuthorizeInput,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

fn apple_config() -> (AppleConfig, LocalEs256Signer) {
    let signer = LocalEs256Signer::generate().expect("apple signer");
    let private_key = signer.signing_key().private_key_pem.clone();
    let config = AppleConfig::from_values(
        Some("com.example.web"),
        Some("TEAMID1234"),
        Some("KEYID12345"),
        Some("AUTH_APPLE_PRIVATE_KEY"),
        Some(3_600),
        |name| (name == "AUTH_APPLE_PRIVATE_KEY").then(|| private_key.clone()),
    )
    .expect("apple config")
    .expect("apple enabled");

    (config, signer)
}

#[derive(Debug, Deserialize)]
struct AppleClientSecretClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: i64,
    exp: i64,
}

#[test]
fn apple_client_secret_uses_es256_header_and_apple_claims() {
    let (apple, signer) = apple_config();
    let now = Utc.with_ymd_and_hms(2026, 6, 15, 12, 0, 0).unwrap();

    let client_secret =
        generate_apple_client_secret(&apple, now).expect("generate apple client secret");
    let header = decode_header(&client_secret).expect("client secret header");
    assert_eq!(header.alg, Algorithm::ES256);
    assert_eq!(header.kid.as_deref(), Some("KEYID12345"));

    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_audience(&[APPLE_AUDIENCE]);
    validation.validate_exp = false;
    let decoded = decode::<AppleClientSecretClaims>(
        &client_secret,
        &DecodingKey::from_ec_pem(signer.signing_key().public_key_pem.as_bytes())
            .expect("apple public key"),
        &validation,
    )
    .expect("decode client secret");

    assert_eq!(decoded.claims.iss, "TEAMID1234");
    assert_eq!(decoded.claims.sub, "com.example.web");
    assert_eq!(decoded.claims.aud, APPLE_AUDIENCE);
    assert_eq!(decoded.claims.iat, now.timestamp());
    assert_eq!(decoded.claims.exp, now.timestamp() + 3_600);
}

#[test]
fn apple_authorization_url_contains_state_nonce_pkce_and_form_post_without_secrets() {
    let (apple, _) = apple_config();

    let url = build_apple_authorization_url(AppleAuthorizeInput {
        config: &apple,
        redirect_uri: "https://auth.example.com/apple/callback",
        state: "raw-provider-state",
        nonce: "raw-provider-nonce",
        pkce_challenge: "provider-pkce-challenge",
    });

    let parsed = Url::parse(&url).expect("apple authorize url");
    assert_eq!(parsed.scheme(), "https");
    assert_eq!(parsed.host_str(), Some("appleid.apple.com"));
    assert_eq!(parsed.path(), "/auth/authorize");

    let query: HashMap<_, _> = parsed.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("client_id").map(String::as_str),
        Some("com.example.web")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("https://auth.example.com/apple/callback")
    );
    assert_eq!(
        query.get("response_type").map(String::as_str),
        Some("code")
    );
    assert_eq!(query.get("response_mode").map(String::as_str), Some("form_post"));
    assert_eq!(query.get("scope").map(String::as_str), Some("name email"));
    assert_eq!(
        query.get("state").map(String::as_str),
        Some("raw-provider-state")
    );
    assert_eq!(
        query.get("nonce").map(String::as_str),
        Some("raw-provider-nonce")
    );
    assert_eq!(
        query.get("code_challenge").map(String::as_str),
        Some("provider-pkce-challenge")
    );
    assert_eq!(
        query.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert!(!url.contains("BEGIN PRIVATE KEY"));
}
