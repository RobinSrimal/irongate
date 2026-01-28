//! Provider module.
//!
//! OAuth/OIDC provider implementations for identity federation.

pub mod apple;
pub mod code;
pub mod github;
pub mod google;
pub mod oauth2;
pub mod oidc;
pub mod password;
pub mod traits;

pub use traits::*;
