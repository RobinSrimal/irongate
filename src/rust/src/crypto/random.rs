//! Secure random generation.
//!
//! Provides cryptographically secure random values.

use rand::{rngs::OsRng, Rng, RngCore};

/// Generate a cryptographically secure random string of specified length.
///
/// Uses base64url alphabet (A-Z, a-z, 0-9, -, _).
pub fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut rng = OsRng;
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate unbiased random digits for OTP codes.
///
/// Uses rejection sampling to avoid modulo bias.
pub fn generate_unbiased_digits(length: usize) -> String {
    let mut result = Vec::with_capacity(length);
    let mut rng = OsRng;

    while result.len() < length {
        let byte: u8 = rng.gen();
        // Only use bytes 0-249 to avoid modulo bias (250 / 10 = 25 exactly)
        if byte < 250 {
            result.push((b'0' + (byte % 10)) as char);
        }
    }

    result.into_iter().collect()
}

/// Generate random bytes.
pub fn generate_random_bytes(length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

/// Generate a UUID v4.
pub fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_string_length() {
        let s = generate_random_string(32);
        assert_eq!(s.len(), 32);
    }

    #[test]
    fn test_unbiased_digits_length() {
        let s = generate_unbiased_digits(6);
        assert_eq!(s.len(), 6);
        assert!(s.chars().all(|c| c.is_ascii_digit()));
    }
}
