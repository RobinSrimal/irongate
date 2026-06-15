//! Minimal OAuth client helpers used by target routes.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::error::OAuthError;

/// Parse Basic auth header and extract client_id and client_secret.
///
/// Expects `Authorization: Basic base64(client_id:client_secret)`.
/// Splits on the first `:` since the secret may contain colons.
pub fn parse_basic_auth(auth_header: Option<&str>) -> Result<(String, String), OAuthError> {
    let header = auth_header
        .ok_or_else(|| OAuthError::InvalidClient("Authorization header required".to_string()))?;

    let encoded = header
        .strip_prefix("Basic ")
        .ok_or_else(|| OAuthError::InvalidClient("Invalid Authorization scheme".to_string()))?;

    let decoded = STANDARD.decode(encoded.trim()).map_err(|_| {
        OAuthError::InvalidClient("Invalid base64 in Authorization header".to_string())
    })?;

    let credentials = String::from_utf8(decoded)
        .map_err(|_| OAuthError::InvalidClient("Invalid UTF-8 in credentials".to_string()))?;

    let (client_id, client_secret) = credentials
        .split_once(':')
        .ok_or_else(|| OAuthError::InvalidClient("Invalid Basic auth format".to_string()))?;

    if client_id.is_empty() || client_secret.is_empty() {
        return Err(OAuthError::InvalidClient(
            "Empty client_id or client_secret".to_string(),
        ));
    }

    Ok((client_id.to_string(), client_secret.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;

    fn basic_header(user: &str, pass: &str) -> String {
        format!("Basic {}", STANDARD.encode(format!("{}:{}", user, pass)))
    }

    #[test]
    fn test_parse_basic_auth_valid() {
        let header = basic_header("my_client", "s3cret");
        let (id, secret) = parse_basic_auth(Some(&header)).unwrap();
        assert_eq!(id, "my_client");
        assert_eq!(secret, "s3cret");
    }

    #[test]
    fn test_parse_basic_auth_secret_with_colons() {
        let header = basic_header("client", "pass:with:colons");
        let (id, secret) = parse_basic_auth(Some(&header)).unwrap();
        assert_eq!(id, "client");
        assert_eq!(secret, "pass:with:colons");
    }

    #[test]
    fn test_parse_basic_auth_missing_header() {
        assert!(parse_basic_auth(None).is_err());
    }

    #[test]
    fn test_parse_basic_auth_wrong_scheme() {
        assert!(parse_basic_auth(Some("Bearer token123")).is_err());
    }

    #[test]
    fn test_parse_basic_auth_invalid_base64() {
        assert!(parse_basic_auth(Some("Basic !!!not-base64!!!")).is_err());
    }

    #[test]
    fn test_parse_basic_auth_no_colon() {
        let header = format!("Basic {}", STANDARD.encode("nocredentialssplit"));
        assert!(parse_basic_auth(Some(&header)).is_err());
    }

    #[test]
    fn test_parse_basic_auth_empty_client_id() {
        let header = basic_header("", "secret");
        assert!(parse_basic_auth(Some(&header)).is_err());
    }

    #[test]
    fn test_parse_basic_auth_empty_secret() {
        let header = basic_header("client", "");
        assert!(parse_basic_auth(Some(&header)).is_err());
    }
}
