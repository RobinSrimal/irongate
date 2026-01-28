//! Cookie encryption using RSA-OAEP + AES-GCM.
//!
//! Implements compact JWE format for encrypted cookies.

use serde::{Deserialize, Serialize};

/// Encrypted cookie value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedCookie {
    /// JWE compact serialization
    pub jwe: String,
}

/// Encrypt a value for storage in a cookie.
pub fn encrypt_cookie_value(plaintext: &str, _encryption_key: &[u8]) -> Result<String, String> {
    todo!("Implement RSA-OAEP + AES-GCM encryption")
}

/// Decrypt a cookie value.
pub fn decrypt_cookie_value(jwe: &str, _encryption_key: &[u8]) -> Result<String, String> {
    todo!("Implement RSA-OAEP + AES-GCM decryption")
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
