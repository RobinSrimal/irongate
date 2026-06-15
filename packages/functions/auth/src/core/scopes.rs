//! Scope constants for the target OIDC-compatible core.

pub const OPENID: &str = "openid";
pub const PROFILE: &str = "profile";
pub const EMAIL: &str = "email";
pub const OFFLINE_ACCESS: &str = "offline_access";

pub const DEFAULT_SUPPORTED_SCOPES: &[&str] = &[OPENID, PROFILE, EMAIL, OFFLINE_ACCESS];
