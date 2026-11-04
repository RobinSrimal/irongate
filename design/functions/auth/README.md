# Auth Function Design

The auth crate implements the public Rust Lambda and exports shared modules used by the separate
[admin function](../admin/README.md).

## Current Structure

Exact source mirrors live under [src](src/), manifests under [Cargo](Cargo.md), and integration
coverage under [tests](tests/README.md). Topic folders summarize behavior across those files.

- `api/providers` handles provider HTTP requests; `api/oauth` re-exports handlers in `oauth`.
- `core` contains client, subject, scope, token and password-policy primitives.
- `providers` implements password workflows and Google/Apple helpers.
- `store` provides typed auth operations over the internal `storage` backend.
- `crypto`, `email`, `config`, `ratelimit`, `client` and `subject` contain supporting modules.
- `audit.rs` and `audit/` emit typed security events through existing Lambda logs, separate from storage.

## Public Surface

The router mounts OAuth authorize/token/revoke/userinfo/discovery/JWKS, password
registration/verification/login/reset, and Google/Apple browser OIDC routes.
It returns JSON, errors or redirects, not hosted HTML login pages.

Email bodies use built-in templates with configurable sender/branding/subjects.
Template override paths are parsed but not loaded by the renderer.

## Boundaries

The public router does not mount admin lifecycle operations. Their shared implementation lives
in this crate, but only the admin entrypoint exposes them behind API Gateway IAM.
Optional examples and their hosting remain outside the auth runtime.

The [threat model](threat-model.md), [storage semantics](store/dynamodb.md) and
[testing overview](testing.md) distinguish implemented controls from current limitations.
