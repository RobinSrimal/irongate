# Auth Infrastructure

Source: [infra/auth/api.ts](../../../infra/auth/api.ts).

## What

One API Gateway HTTP API and two shared ARM64 Rust Lambda functions. Public auth receives the
`$default` route; explicit account lifecycle routes target the admin function with IAM authorization.

## Why

Public authentication and operator lifecycle operations have separate deployed permission and
secret boundaries without creating a Lambda for every route.

## How

- Public auth uses 256 MB; admin uses 128 MB. Both use a 30-second timeout and `provided.al2023`.
- Bundles are built by [rust-bundle](../shared/rust-bundle.md); public auth includes `auth.clients.toml`.
- IAM protects GET user and POST disable, enable, delete and revoke-sessions routes under
  `/_admin/users/{subject}`.
- Both roles receive exact table operations from [config](config.md); only public auth receives
  signing permissions and provider/email/HMAC secrets.
- CORS is implemented by the Rust router using client origins, not by API Gateway configuration.
- Issuer defaults to the generated API URL or an explicit stage override. Setting that override
  does not provision a custom domain; no custom-domain resource is defined here.
- API access and Lambda operational logs use configured CloudWatch retention; Lambda format is JSON.
  The separate audit-mode switch currently gates DynamoDB audit records, not a CloudWatch emitter.

Source identity arrives through API Gateway request context. Forwarded headers are not trusted by
Irongate's rate-limit helper. Example resources are configured outside this module.
