//! JWT verification.
//!
//! Verifies tokens using ES256 (ECDSA with P-256).

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use super::keys::SigningKey;
use super::sign::{AccessTokenClaims, RefreshTokenClaims};

/// Verify an access token.
pub fn verify_access_token(
    token: &str,
    signing_keys: &[SigningKey],
    expected_issuer: &str,
    expected_audience: Option<&str>,
) -> Result<AccessTokenClaims, String> {
    // Extract the kid from the token header
    let header = jsonwebtoken::decode_header(token).map_err(|e| e.to_string())?;
    let kid = header.kid.ok_or("Token missing kid")?;

    // Find the matching key
    let key = signing_keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or("Unknown signing key")?;

    // Set up validation
    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_issuer(&[expected_issuer]);

    if let Some(aud) = expected_audience {
        validation.set_audience(&[aud]);
    }

    // Decode and verify
    let decoding_key =
        DecodingKey::from_ec_pem(key.public_key_pem.as_bytes()).map_err(|e| e.to_string())?;

    let token_data = decode::<AccessTokenClaims>(token, &decoding_key, &validation)
        .map_err(|e| e.to_string())?;

    // Verify it's an access token
    if token_data.claims.mode != "access" {
        return Err("Not an access token".to_string());
    }

    Ok(token_data.claims)
}

/// Verify a refresh token.
pub fn verify_refresh_token(
    token: &str,
    signing_keys: &[SigningKey],
    expected_issuer: &str,
) -> Result<RefreshTokenClaims, String> {
    // Extract the kid from the token header
    let header = jsonwebtoken::decode_header(token).map_err(|e| e.to_string())?;
    let kid = header.kid.ok_or("Token missing kid")?;

    // Find the matching key
    let key = signing_keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or("Unknown signing key")?;

    // Set up validation
    let mut validation = Validation::new(Algorithm::ES256);
    validation.set_issuer(&[expected_issuer]);

    // Decode and verify
    let decoding_key =
        DecodingKey::from_ec_pem(key.public_key_pem.as_bytes()).map_err(|e| e.to_string())?;

    let token_data = decode::<RefreshTokenClaims>(token, &decoding_key, &validation)
        .map_err(|e| e.to_string())?;

    // Verify it's a refresh token
    if token_data.claims.mode != "refresh" {
        return Err("Not a refresh token".to_string());
    }

    Ok(token_data.claims)
}
