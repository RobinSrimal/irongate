# Trusted Source Identity

Source: [packages/functions/auth/src/ratelimit/middleware.rs](../../../../../packages/functions/auth/src/ratelimit/middleware.rs).

## What

Extraction of client IP from API Gateway request context.

## Why

Caller-controlled forwarded headers are not an authentication boundary.

## How

API Gateway v1 identity.source_ip or v2 http.source_ip is trimmed, parsed as IpAddr and canonicalized.
Equivalent IPv6 and IPv4-mapped IPv6 representations yield the same source. Invalid/empty values and
unsupported contexts return None, which the limiter maps to one bounded unknown-source bucket.
x-forwarded-for and x-real-ip are ignored. Unit and request-context tests exercise these boundaries.
