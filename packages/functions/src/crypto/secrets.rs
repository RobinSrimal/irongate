//! Client secret handling.
//!
//! Uses Argon2 for secure secret hashing with constant-time verification.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use super::random::generate_random_string;

/// Generate a new client secret.
pub fn generate_client_secret() -> String {
    generate_random_string(32)
}

/// Hash a client secret for storage.
///
/// Uses Argon2id with default parameters.
pub fn hash_client_secret(secret: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(secret.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verify a client secret against its hash.
///
/// Uses constant-time comparison to prevent timing attacks.
pub fn verify_client_secret(provided: &str, stored_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    Argon2::default()
        .verify_password(provided.as_bytes(), &parsed_hash)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let secret = "test-secret-123";
        let hash = hash_client_secret(secret).unwrap();

        assert!(verify_client_secret(secret, &hash));
        assert!(!verify_client_secret("wrong-secret", &hash));
    }

    #[test]
    fn test_different_salts() {
        let secret = "same-secret";
        let hash1 = hash_client_secret(secret).unwrap();
        let hash2 = hash_client_secret(secret).unwrap();

        // Same secret should produce different hashes (different salts)
        assert_ne!(hash1, hash2);

        // But both should verify
        assert!(verify_client_secret(secret, &hash1));
        assert!(verify_client_secret(secret, &hash2));
    }
}
