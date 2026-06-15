#![allow(dead_code, deprecated)]
//! Irongate - Security-first OAuth 2.0 Authorization Server
//!
//! This library provides a production-ready OAuth 2.0 authorization server
//! with security-first defaults:
//!
//! - **Mandatory client registration** - No anonymous clients
//! - **Explicit redirect URI allowlist** - No pattern matching
//! - **PKCE required by default** - Can be disabled per-client
//! - **Rate limiting enabled** - Protects against brute force
//! - **Constant-time comparisons** - Prevents timing attacks

pub mod admin;
pub mod audit;
pub mod client;
pub mod config;
pub mod core;
pub mod crypto;
pub mod email;
pub mod error;
pub mod flows;
pub mod jwt;
pub mod oauth;
pub mod provider;
pub mod ratelimit;
pub mod routes;
pub mod storage;
pub mod store;
pub mod subject;
pub mod ui;

// Re-export commonly used types
pub use config::Config;
pub use error::{AuthError, OAuthError, Result};
pub use storage::{DynamoStorage, StorageAdapter};
