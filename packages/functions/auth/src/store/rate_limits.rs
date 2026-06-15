//! Rate-limit key helpers for auth flows.

use crate::core::passwords::normalize_email;
use crate::crypto::hmac_lookup::{lookup_digest, LookupFamily};

pub fn password_email_rate_limit_identifier(
    lookup_secret: &[u8],
    email: &str,
    source: Option<&str>,
) -> String {
    let email_part = normalize_email(email).ok().map(|normalized| {
        let digest = lookup_digest(lookup_secret, LookupFamily::Email, &normalized);
        format!("email:{digest}")
    });
    composite_rate_limit_identifier(email_part.as_deref(), source)
}

pub fn source_rate_limit_identifier(source: Option<&str>) -> String {
    composite_rate_limit_identifier(None, source)
}

fn composite_rate_limit_identifier(digest_part: Option<&str>, source: Option<&str>) -> String {
    let source_part = source.unwrap_or("unknown");
    match digest_part {
        Some(digest) => format!("{digest}:source:{source_part}"),
        None => format!("source:{source_part}"),
    }
}
