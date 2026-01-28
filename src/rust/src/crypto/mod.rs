//! Cryptography module.
//!
//! Provides secure cryptographic operations for:
//! - Password hashing (Argon2)
//! - Client secret hashing (Argon2)
//! - Cookie encryption (RSA-OAEP + AES-GCM)
//! - Secure random generation

pub mod encrypt;
pub mod password;
pub mod random;
pub mod secrets;
