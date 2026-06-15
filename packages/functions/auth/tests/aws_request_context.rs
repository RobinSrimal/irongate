use axum::body::Body;
use axum::http::{HeaderMap, Request};
use irongate::ratelimit::middleware::trusted_source_ip;
use lambda_http::aws_lambda_events::apigw::{
    ApiGatewayProxyRequestContext, ApiGatewayRequestIdentity, ApiGatewayV2httpRequestContext,
    ApiGatewayV2httpRequestContextHttpDescription,
};
use lambda_http::request::RequestContext;

fn request_with_context(context: RequestContext) -> Request<Body> {
    Request::builder()
        .uri("/authorize")
        .extension(context)
        .body(Body::empty())
        .unwrap()
}

#[test]
fn source_identity_uses_api_gateway_v2_source_ip() {
    let mut context = ApiGatewayV2httpRequestContext::default();
    context.http = ApiGatewayV2httpRequestContextHttpDescription {
        source_ip: Some("203.0.113.10".to_string()),
        ..Default::default()
    };
    let request = request_with_context(RequestContext::ApiGatewayV2(context));

    assert_eq!(
        trusted_source_ip(request.extensions(), request.headers()),
        Some("203.0.113.10".to_string())
    );
}

#[test]
fn source_identity_uses_api_gateway_v1_source_ip() {
    let context = ApiGatewayProxyRequestContext {
        identity: ApiGatewayRequestIdentity {
            source_ip: Some("203.0.113.11".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let request = request_with_context(RequestContext::ApiGatewayV1(context));

    assert_eq!(
        trusted_source_ip(request.extensions(), request.headers()),
        Some("203.0.113.11".to_string())
    );
}

#[test]
fn source_identity_ignores_spoofed_forwarded_headers() {
    let mut context = ApiGatewayV2httpRequestContext::default();
    context.http = ApiGatewayV2httpRequestContextHttpDescription {
        source_ip: Some("203.0.113.12".to_string()),
        ..Default::default()
    };
    let mut request = request_with_context(RequestContext::ApiGatewayV2(context));
    request
        .headers_mut()
        .insert("x-forwarded-for", "198.51.100.1".parse().unwrap());
    request
        .headers_mut()
        .insert("x-real-ip", "198.51.100.2".parse().unwrap());

    assert_eq!(
        trusted_source_ip(request.extensions(), request.headers()),
        Some("203.0.113.12".to_string())
    );
}

#[test]
fn source_identity_without_context_does_not_read_forwarded_headers() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "198.51.100.1".parse().unwrap());
    headers.insert("x-real-ip", "198.51.100.2".parse().unwrap());
    let request = Request::builder()
        .uri("/authorize")
        .body(Body::empty())
        .unwrap();

    assert_eq!(trusted_source_ip(request.extensions(), &headers), None);
}
