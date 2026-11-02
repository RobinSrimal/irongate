# Trusted Source Identity

Source: [packages/functions/auth/src/ratelimit/middleware.rs](../../../../../packages/functions/auth/src/ratelimit/middleware.rs).

## What

Extraction of client IP from API Gateway request context.

## Why

Caller-controlled forwarded headers are not an authentication boundary.

## How

API Gateway v1 identity.source_ip or v2 http.source_ip is trimmed and returned; unsupported contexts yield None. x-forwarded-for and x-real-ip are ignored. A unit test covers empty source values.
