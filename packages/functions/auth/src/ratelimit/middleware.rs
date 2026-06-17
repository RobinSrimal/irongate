//! Rate limiting middleware.
//!
//! Request-source helpers used by store-backed rate limits.

use axum::http::{Extensions, HeaderMap};
use lambda_http::request::RequestContext;

/// Extract trusted source IP from Lambda/API Gateway request context.
///
/// Request headers such as x-forwarded-for and x-real-ip are intentionally
/// ignored here because API Gateway mode should anchor source identity in the
/// request context produced by AWS.
pub fn trusted_source_ip(extensions: &Extensions, _headers: &HeaderMap) -> Option<String> {
    extensions
        .get::<RequestContext>()
        .and_then(trusted_source_ip_from_context)
}

pub fn trusted_source_ip_from_context(context: &RequestContext) -> Option<String> {
    match context {
        RequestContext::ApiGatewayV1(context) => context.identity.source_ip.clone(),
        RequestContext::ApiGatewayV2(context) => context.http.source_ip.clone(),
        _ => None,
    }
    .and_then(non_empty_trimmed)
}

fn non_empty_trimmed(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_source_ip_is_ignored() {
        let mut context =
            lambda_http::aws_lambda_events::apigw::ApiGatewayV2httpRequestContext::default();
        context.http.source_ip = Some("   ".to_string());

        assert!(trusted_source_ip_from_context(&RequestContext::ApiGatewayV2(context)).is_none());
    }
}
