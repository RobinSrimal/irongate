//! Static OAuth client configuration.
//!
//! The target core loads OAuth clients from a config file at startup. This
//! parser intentionally supports only the template's `auth.clients.toml`
//! shape instead of becoming a general TOML layer.

use crate::core::clients::{ClientType, ConfiguredClient, GrantType, TokenEndpointAuthMethod};
use crate::crypto::password::hash_password;
use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum ClientFileError {
    #[error("client config parse error on line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("client config validation error: {0}")]
    Validation(String),

    #[error("client config secret `{0}` is missing")]
    MissingSecret(String),

    #[error("client config file error: {0}")]
    File(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientFile {
    pub clients: Vec<ConfiguredClient>,
}

impl ClientFile {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ClientFileError> {
        let contents = std::fs::read_to_string(path.as_ref())
            .map_err(|err| ClientFileError::File(err.to_string()))?;
        Self::from_toml_str(&contents)
    }

    pub fn from_toml_str(contents: &str) -> Result<Self, ClientFileError> {
        let clients = parse_clients(contents)?;
        validate_clients(clients)
    }

    pub fn from_toml_str_with_secret_resolver<F>(
        contents: &str,
        resolver: F,
    ) -> Result<Self, ClientFileError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut file = Self::from_toml_str(contents)?;

        for client in &mut file.clients {
            if client.client_type == ClientType::Confidential {
                let secret_ref = client.client_secret_ref.as_deref().ok_or_else(|| {
                    ClientFileError::Validation(format!(
                        "confidential client `{}` is missing client_secret_ref",
                        client.client_id
                    ))
                })?;
                let secret = resolver(secret_ref)
                    .ok_or_else(|| ClientFileError::MissingSecret(secret_ref.to_string()))?;
                client.client_secret_hash = Some(
                    hash_password(&secret)
                        .map_err(|err| ClientFileError::Validation(err.to_string()))?,
                );
            }
        }

        Ok(file)
    }

    pub fn client(&self, client_id: &str) -> Option<&ConfiguredClient> {
        self.clients
            .iter()
            .find(|client| client.client_id == client_id)
    }
}

#[derive(Debug, Default)]
struct RawClient {
    client_id: Option<String>,
    client_type: Option<ClientType>,
    redirect_uris: Option<Vec<String>>,
    allowed_grant_types: Option<Vec<GrantType>>,
    allowed_scopes: Option<Vec<String>>,
    pkce_required: Option<bool>,
    token_endpoint_auth_method: Option<TokenEndpointAuthMethod>,
    client_secret_ref: Option<String>,
}

fn parse_clients(contents: &str) -> Result<Vec<RawClient>, ClientFileError> {
    let mut clients = Vec::new();
    let mut current: Option<RawClient> = None;

    for (idx, raw_line) in contents.lines().enumerate() {
        let line_number = idx + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[[clients]]" {
            if let Some(client) = current.take() {
                clients.push(client);
            }
            current = Some(RawClient::default());
            continue;
        }

        let client = current
            .as_mut()
            .ok_or_else(|| parse_error(line_number, "expected [[clients]] before client fields"))?;

        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| parse_error(line_number, "expected key = value"))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "client_id" => client.client_id = Some(parse_string(line_number, value)?),
            "client_type" => {
                client.client_type = Some(
                    ClientType::from_str(&parse_string(line_number, value)?)
                        .map_err(|message| parse_error(line_number, message))?,
                )
            }
            "redirect_uris" => client.redirect_uris = Some(parse_string_array(line_number, value)?),
            "allowed_grant_types" => {
                client.allowed_grant_types = Some(
                    parse_string_array(line_number, value)?
                        .into_iter()
                        .map(|grant| {
                            GrantType::from_str(&grant)
                                .map_err(|message| parse_error(line_number, message))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            }
            "allowed_scopes" => {
                client.allowed_scopes = Some(parse_string_array(line_number, value)?)
            }
            "pkce_required" => client.pkce_required = Some(parse_bool(line_number, value)?),
            "token_endpoint_auth_method" => {
                client.token_endpoint_auth_method = Some(
                    TokenEndpointAuthMethod::from_str(&parse_string(line_number, value)?)
                        .map_err(|message| parse_error(line_number, message))?,
                )
            }
            "client_secret_ref" => {
                client.client_secret_ref = Some(parse_string(line_number, value)?)
            }
            other => {
                return Err(parse_error(
                    line_number,
                    format!("unknown client field `{other}`"),
                ))
            }
        }
    }

    if let Some(client) = current.take() {
        clients.push(client);
    }

    if clients.is_empty() {
        return Err(ClientFileError::Validation(
            "at least one [[clients]] entry is required".into(),
        ));
    }

    Ok(clients)
}

fn validate_clients(raw_clients: Vec<RawClient>) -> Result<ClientFile, ClientFileError> {
    let mut seen = HashSet::new();
    let mut clients = Vec::with_capacity(raw_clients.len());

    for raw in raw_clients {
        let client_id = required(raw.client_id, "client_id")?;
        if !is_valid_client_id(&client_id) {
            return Err(ClientFileError::Validation(format!(
                "client `{client_id}` has an invalid client_id"
            )));
        }
        if !seen.insert(client_id.clone()) {
            return Err(ClientFileError::Validation(format!(
                "duplicate client_id `{client_id}`"
            )));
        }

        let client_type = required(raw.client_type, "client_type")?;
        let redirect_uris = required(raw.redirect_uris, "redirect_uris")?;
        if redirect_uris.is_empty() {
            return Err(ClientFileError::Validation(format!(
                "client `{client_id}` must define at least one redirect URI"
            )));
        }
        for redirect_uri in &redirect_uris {
            validate_redirect_uri(&client_id, redirect_uri)?;
        }

        let allowed_grant_types = required(raw.allowed_grant_types, "allowed_grant_types")?;
        if allowed_grant_types.is_empty() {
            return Err(ClientFileError::Validation(format!(
                "client `{client_id}` must define at least one grant type"
            )));
        }
        if allowed_grant_types.contains(&GrantType::RefreshToken)
            && !allowed_grant_types.contains(&GrantType::AuthorizationCode)
        {
            return Err(ClientFileError::Validation(format!(
                "client `{client_id}` cannot use refresh_token without authorization_code"
            )));
        }

        let allowed_scopes = required(raw.allowed_scopes, "allowed_scopes")?;
        if !allowed_scopes.iter().any(|scope| scope == "openid") {
            return Err(ClientFileError::Validation(format!(
                "client `{client_id}` must include the openid scope"
            )));
        }

        let pkce_required = required(raw.pkce_required, "pkce_required")?;
        let token_endpoint_auth_method =
            required(raw.token_endpoint_auth_method, "token_endpoint_auth_method")?;

        match client_type {
            ClientType::Public => {
                if raw.client_secret_ref.is_some() {
                    return Err(ClientFileError::Validation(format!(
                        "public client `{client_id}` must not define client_secret_ref"
                    )));
                }
                if token_endpoint_auth_method != TokenEndpointAuthMethod::None {
                    return Err(ClientFileError::Validation(format!(
                        "public client `{client_id}` must use token_endpoint_auth_method none"
                    )));
                }
                if !pkce_required {
                    return Err(ClientFileError::Validation(format!(
                        "public client `{client_id}` must require PKCE"
                    )));
                }
            }
            ClientType::Confidential => {
                if raw.client_secret_ref.is_none() {
                    return Err(ClientFileError::Validation(format!(
                        "confidential client `{client_id}` must define client_secret_ref"
                    )));
                }
                if token_endpoint_auth_method == TokenEndpointAuthMethod::None {
                    return Err(ClientFileError::Validation(format!(
                        "confidential client `{client_id}` must use a client secret auth method"
                    )));
                }
            }
        }

        clients.push(ConfiguredClient {
            client_id,
            client_type,
            redirect_uris,
            allowed_grant_types,
            allowed_scopes,
            pkce_required,
            token_endpoint_auth_method,
            client_secret_ref: raw.client_secret_ref,
            client_secret_hash: None,
        });
    }

    Ok(ClientFile { clients })
}

fn required<T>(value: Option<T>, field: &str) -> Result<T, ClientFileError> {
    value.ok_or_else(|| ClientFileError::Validation(format!("missing required field `{field}`")))
}

fn validate_redirect_uri(client_id: &str, value: &str) -> Result<(), ClientFileError> {
    let url = Url::parse(value).map_err(|err| {
        ClientFileError::Validation(format!(
            "client `{client_id}` has invalid redirect URI `{value}`: {err}"
        ))
    })?;

    if url.fragment().is_some() {
        return Err(ClientFileError::Validation(format!(
            "client `{client_id}` redirect URI `{value}` must not include a fragment"
        )));
    }

    let allowed_scheme =
        url.scheme() == "https" || (url.scheme() == "http" && is_loopback_host(url.host_str()));
    if !allowed_scheme {
        return Err(ClientFileError::Validation(format!(
            "client `{client_id}` redirect URI `{value}` must be https or localhost http"
        )));
    }

    Ok(())
}

fn is_loopback_host(host: Option<&str>) -> bool {
    matches!(host, Some("localhost") | Some("127.0.0.1") | Some("::1"))
}

fn is_valid_client_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn parse_string(line: usize, value: &str) -> Result<String, ClientFileError> {
    let value = value.trim();
    if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
        return Err(parse_error(line, "expected quoted string"));
    }
    Ok(value[1..value.len() - 1].to_string())
}

fn parse_string_array(line: usize, value: &str) -> Result<Vec<String>, ClientFileError> {
    let value = value.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err(parse_error(line, "expected string array"));
    }

    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    inner
        .split(',')
        .map(|part| parse_string(line, part.trim()))
        .collect()
}

fn parse_bool(line: usize, value: &str) -> Result<bool, ClientFileError> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(parse_error(line, "expected boolean")),
    }
}

fn parse_error(line: usize, message: impl Into<String>) -> ClientFileError {
    ClientFileError::Parse {
        line,
        message: message.into(),
    }
}
