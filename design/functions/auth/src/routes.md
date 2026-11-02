# Public Router

Source: [packages/functions/auth/src/routes.rs](../../../../packages/functions/auth/src/routes.rs).

## What

Axum dispatch for discovery, OAuth, password and Google/Apple endpoints.

## Why

The public Lambda exposes authentication APIs without operator lifecycle routes or hosted forms.

## How

Authorize has middleware rate limiting; other handlers apply their own endpoint limits. TraceLayer and an outer CORS layer allow configured browser origins, GET/POST and selected headers. Trusted source IP comes from Lambda context, never forwarded headers.
