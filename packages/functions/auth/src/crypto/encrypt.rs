//! Cookie encryption using RSA-OAEP + AES-GCM.
//!
//! Format: `base64(wrapped_key).base64(nonce).base64(ciphertext+tag)`
//! - AES-256-GCM encrypts the plaintext
//! - RSA-OAEP (SHA-256) wraps the AES key

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::Aead,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use rsa::{Oaep, RsaPrivateKey, RsaPublicKey};
use sha2::Sha256;

/// Encrypt a value for storage in a cookie.
///
/// Returns a dot-separated string: `wrapped_key.nonce.ciphertext`
pub fn encrypt_cookie_value(
    plaintext: &str,
    public_key: &RsaPublicKey,
) -> Result<String, String> {
    // Generate random AES-256 key
    let mut aes_key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut aes_key);

    // Generate random 96-bit nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt plaintext with AES-256-GCM
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| format!("AES init error: {}", e))?;
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("AES encrypt error: {}", e))?;

    // Wrap AES key with RSA-OAEP
    let padding = Oaep::new::<Sha256>();
    let mut rng = rand::thread_rng();
    let wrapped_key = public_key
        .encrypt(&mut rng, padding, &aes_key)
        .map_err(|e| format!("RSA encrypt error: {}", e))?;

    // Encode as dot-separated base64url
    Ok(format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(&wrapped_key),
        URL_SAFE_NO_PAD.encode(&nonce_bytes),
        URL_SAFE_NO_PAD.encode(&ciphertext),
    ))
}

/// Decrypt a cookie value.
///
/// Expects the format produced by `encrypt_cookie_value`.
pub fn decrypt_cookie_value(
    encrypted: &str,
    private_key: &RsaPrivateKey,
) -> Result<String, String> {
    let parts: Vec<&str> = encrypted.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err("Invalid encrypted format".to_string());
    }

    let wrapped_key = URL_SAFE_NO_PAD
        .decode(parts[0])
        .map_err(|_| "Invalid base64 in wrapped key".to_string())?;
    let nonce_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| "Invalid base64 in nonce".to_string())?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| "Invalid base64 in ciphertext".to_string())?;

    if nonce_bytes.len() != 12 {
        return Err("Invalid nonce length".to_string());
    }

    // Unwrap AES key with RSA-OAEP
    let padding = Oaep::new::<Sha256>();
    let aes_key = private_key
        .decrypt(padding, &wrapped_key)
        .map_err(|e| format!("RSA decrypt error: {}", e))?;

    // Decrypt ciphertext with AES-256-GCM
    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| format!("AES init error: {}", e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| format!("AES decrypt error: {}", e))?;

    String::from_utf8(plaintext).map_err(|_| "Invalid UTF-8 in decrypted data".to_string())
}

/// Cookie builder with security defaults
pub struct SecureCookie {
    pub name: String,
    pub value: String,
    pub max_age: Option<i64>,
    pub http_only: bool,
    pub secure: bool,
    pub same_site: SameSite,
    pub path: String,
}

/// SameSite cookie attribute
#[derive(Debug, Clone, Copy)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Default for SecureCookie {
    fn default() -> Self {
        Self {
            name: String::new(),
            value: String::new(),
            max_age: None,
            http_only: true,
            secure: true,
            same_site: SameSite::Strict,
            path: "/".to_string(),
        }
    }
}

impl SecureCookie {
    /// Create a new secure cookie with the given name and value.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            ..Default::default()
        }
    }

    /// Set the max age in seconds.
    pub fn max_age(mut self, seconds: i64) -> Self {
        self.max_age = Some(seconds);
        self
    }

    /// Set SameSite attribute.
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    /// Build the Set-Cookie header value.
    pub fn to_header_value(&self) -> String {
        let mut parts = vec![format!("{}={}", self.name, self.value)];

        if let Some(max_age) = self.max_age {
            parts.push(format!("Max-Age={}", max_age));
        }

        if self.http_only {
            parts.push("HttpOnly".to_string());
        }

        if self.secure {
            parts.push("Secure".to_string());
        }

        let same_site = match self.same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        };
        parts.push(format!("SameSite={}", same_site));

        parts.push(format!("Path={}", self.path));

        parts.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::RsaPrivateKey;

    fn generate_test_keypair() -> (RsaPublicKey, RsaPrivateKey) {
        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let public_key = RsaPublicKey::from(&private_key);
        (public_key, private_key)
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let (pub_key, priv_key) = generate_test_keypair();
        let plaintext = "hello world session data";

        let encrypted = encrypt_cookie_value(plaintext, &pub_key).unwrap();
        let decrypted = decrypt_cookie_value(&encrypted, &priv_key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_produces_three_dot_separated_parts() {
        let (pub_key, _) = generate_test_keypair();
        let encrypted = encrypt_cookie_value("test", &pub_key).unwrap();
        assert_eq!(encrypted.split('.').count(), 3);
    }

    #[test]
    fn test_encrypt_different_ciphertexts_each_time() {
        let (pub_key, _) = generate_test_keypair();
        let a = encrypt_cookie_value("same", &pub_key).unwrap();
        let b = encrypt_cookie_value("same", &pub_key).unwrap();
        assert_ne!(a, b, "Each encryption should produce unique output");
    }

    #[test]
    fn test_decrypt_invalid_format() {
        let (_, priv_key) = generate_test_keypair();
        assert!(decrypt_cookie_value("no-dots-here", &priv_key).is_err());
        assert!(decrypt_cookie_value("only.two", &priv_key).is_err());
    }

    #[test]
    fn test_decrypt_tampered_ciphertext() {
        let (pub_key, priv_key) = generate_test_keypair();
        let encrypted = encrypt_cookie_value("secret", &pub_key).unwrap();

        // Flip a character in the ciphertext part
        let parts: Vec<&str> = encrypted.splitn(3, '.').collect();
        let mut tampered = parts[2].as_bytes().to_vec();
        tampered[0] ^= 0xFF;
        let tampered_str = String::from_utf8_lossy(&tampered).to_string();
        let tampered_encrypted = format!("{}.{}.{}", parts[0], parts[1], tampered_str);

        assert!(decrypt_cookie_value(&tampered_encrypted, &priv_key).is_err());
    }

    #[test]
    fn test_decrypt_wrong_key() {
        let (pub_key, _) = generate_test_keypair();
        let (_, wrong_priv_key) = generate_test_keypair();

        let encrypted = encrypt_cookie_value("secret", &pub_key).unwrap();
        assert!(decrypt_cookie_value(&encrypted, &wrong_priv_key).is_err());
    }

    #[test]
    fn test_encrypt_empty_string() {
        let (pub_key, priv_key) = generate_test_keypair();
        let encrypted = encrypt_cookie_value("", &pub_key).unwrap();
        let decrypted = decrypt_cookie_value(&encrypted, &priv_key).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_encrypt_large_payload() {
        let (pub_key, priv_key) = generate_test_keypair();
        let large = "x".repeat(4096);
        let encrypted = encrypt_cookie_value(&large, &pub_key).unwrap();
        let decrypted = decrypt_cookie_value(&encrypted, &priv_key).unwrap();
        assert_eq!(decrypted, large);
    }

    // SecureCookie tests

    #[test]
    fn test_secure_cookie_defaults() {
        let cookie = SecureCookie::new("session", "abc123");
        let header = cookie.to_header_value();
        assert!(header.contains("session=abc123"));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("Secure"));
        assert!(header.contains("SameSite=Strict"));
        assert!(header.contains("Path=/"));
    }

    #[test]
    fn test_secure_cookie_max_age() {
        let cookie = SecureCookie::new("s", "v").max_age(600);
        let header = cookie.to_header_value();
        assert!(header.contains("Max-Age=600"));
    }

    #[test]
    fn test_secure_cookie_same_site_lax() {
        let cookie = SecureCookie::new("s", "v").same_site(SameSite::Lax);
        let header = cookie.to_header_value();
        assert!(header.contains("SameSite=Lax"));
    }
}
