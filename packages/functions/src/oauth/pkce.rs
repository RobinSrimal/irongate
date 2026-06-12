//! PKCE (Proof Key for Code Exchange) implementation.
//!
//! RFC 7636 - required by default for all clients.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Generate a PKCE code challenge from a verifier.
///
/// Uses S256 method (SHA-256 hash, base64url encoded).
pub fn generate_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let hash = hasher.finalize();
    base64_url_encode(&hash)
}

/// Validate a PKCE code verifier against a challenge.
///
/// CRITICAL: Uses constant-time comparison to prevent timing attacks.
pub fn validate_pkce(verifier: &str, challenge: &str) -> bool {
    let computed = generate_challenge(verifier);

    // Use constant-time comparison
    bool::from(computed.as_bytes().ct_eq(challenge.as_bytes()))
}

/// Generate a random code verifier.
///
/// 43-128 characters, using base64url alphabet.
pub fn generate_verifier() -> String {
    use crate::crypto::random::generate_random_bytes;

    // 32 bytes = 43 base64url characters
    let bytes = generate_random_bytes(32);
    base64_url_encode(&bytes)
}

/// Base64url encode without padding.
fn base64_url_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_challenge_generation() {
        // RFC 7636 Appendix B test vector
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        let challenge = generate_challenge(verifier);
        assert_eq!(challenge, expected_challenge);
    }

    #[test]
    fn test_pkce_validation() {
        let verifier = "test-verifier-string-with-enough-length";
        let challenge = generate_challenge(verifier);

        assert!(validate_pkce(verifier, &challenge));
        assert!(!validate_pkce("wrong-verifier", &challenge));
    }

    #[test]
    fn test_verifier_generation() {
        let verifier = generate_verifier();
        assert!(verifier.len() >= 43);
        assert!(verifier.len() <= 128);
    }
}
