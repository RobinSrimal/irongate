# Auth API

Implementation: [api/mod.rs](../src/api/mod.md), [routes.rs](../src/routes.md) and
[OAuth implementation](../src/oauth/mod.md).

## What And Why

Axum handlers translate requests into domain/store operations and protocol responses.
The public function stays API-only so consuming applications own login and account screens.

## How

`routes.rs` mounts discovery, OAuth and provider endpoints with CORS/tracing and authorize
rate limiting. Other endpoint limits run inside handlers. The `api/oauth` files re-export the
handlers implemented in `oauth`; they are not duplicate implementations.
`api/providers` implements password, Google and Apple HTTP entrypoints.

Provider validators, cryptographic primitives and DynamoDB expressions live outside routing.
Email body rendering is in `email/templates.rs`, not a directory of hosted UI pages.

The shared `api/admin.rs` module builds a separate router used only by the admin Lambda.
It is not mounted behind the public API's default integration.
