# Public Router

Source: [packages/functions/auth/src/routes.rs](../../../../packages/functions/auth/src/routes.rs).

## What

Axum dispatch for discovery, OAuth, password and Google/Apple endpoints.

## Why

The public Lambda exposes authentication APIs without operator lifecycle routes or hosted forms.

## How

Authorize middleware charges independent source and configured-client/source budgets before its
handler. It reads client_id from the query, not Basic authentication; unregistered names map to None.
Other handlers apply their own typed endpoint admission. TraceLayer and an outer CORS layer allow
configured browser origins, GET/POST and selected headers. Trusted source IP comes from Lambda
context, never forwarded headers.
