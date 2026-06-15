//! Google OIDC domain helpers.

use crate::config::google::GoogleConfig;

pub struct GoogleAuthorizeInput<'a> {
    pub config: &'a GoogleConfig,
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub pkce_challenge: &'a str,
}

pub fn build_google_authorization_url(input: GoogleAuthorizeInput<'_>) -> String {
    let mut url = input.config.authorization_url.clone();
    url.query_pairs_mut()
        .append_pair("client_id", &input.config.client_id)
        .append_pair("redirect_uri", input.redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &input.config.scopes.join(" "))
        .append_pair("state", input.state)
        .append_pair("nonce", input.nonce)
        .append_pair("code_challenge", input.pkce_challenge)
        .append_pair("code_challenge_method", "S256");
    url.into()
}
