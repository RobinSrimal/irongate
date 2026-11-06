//! Google transaction cookies; never application-session cookies.

use crate::error::OAuthError;
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use cookie::{Cookie, SameSite};

pub(super) fn valid_handle(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub(super) fn cookie_name(state_digest: &str) -> String {
    format!("__Host-ig-google-{state_digest}")
}

pub(super) fn read(headers: &HeaderMap, state_digest: &str) -> Result<String, OAuthError> {
    let name = cookie_name(state_digest);
    let mut proof = None;
    for header in headers.get_all(header::COOKIE) {
        for parsed in Cookie::split_parse(header.to_str().map_err(|_| invalid())?) {
            let cookie = parsed.map_err(|_| invalid())?;
            if cookie.name() == name {
                if proof.is_some() || !valid_handle(cookie.value()) {
                    return Err(invalid());
                }
                proof = Some(cookie.value().to_owned());
            }
        }
    }
    proof.ok_or_else(invalid)
}

pub(super) fn header(state_digest: &str, proof: &str, max_age: i64) -> HeaderValue {
    Cookie::build((cookie_name(state_digest), proof.to_owned()))
        .path("/")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::seconds(max_age))
        .build()
        .to_string()
        .parse()
        .expect("generated cookie has safe ASCII fields")
}

pub(super) fn protect(result: Result<Response, OAuthError>) -> Response {
    let mut response = match result {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

pub(super) fn invalid() -> OAuthError {
    OAuthError::InvalidRequest("invalid or expired provider transaction".into())
}
