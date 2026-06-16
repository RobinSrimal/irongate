# AWS Dev Smoke Test

This records the first dev deployment smoke validation for the simplified auth template.

- Date: 2026-06-16
- Stage: `dev`
- AWS profile: `irongate-dev`
- Region: `eu-west-1`
- Status: partially passed; full password/token loop is blocked on consuming the Resend-delivered verification email.

## Deployed Outputs

```text
ApiUrl=https://1e88qilxk6.execute-api.eu-west-1.amazonaws.com
ApiId=1e88qilxk6
TableName=irongate-dev-AuthTableTable-wzwedmtx
TableKmsKeyArn=aws-owned
SigningKmsKeyArn=arn:aws:kms:eu-west-1:930800959086:key/0cfcd86f-26eb-4025-9f66-32fb3455e245
AdminRouteArnPattern=arn:aws:execute-api:eu-west-1:930800959086:1e88qilxk6/*/*/_admin/users/*
```

## Commands Run

Local preflight:

```text
npm run test:infra
npm run typecheck
cargo check --manifest-path packages/functions/auth/Cargo.toml --locked
cargo check --manifest-path packages/functions/admin/Cargo.toml --locked
cargo test --manifest-path packages/functions/auth/Cargo.toml --locked
```

Deploy:

```text
npm run deploy -- --stage dev
```

AWS smoke checks used bounded CLI queries and HTTP requests only. No table scans were used.

## Results

| Area | Result | Notes |
| --- | --- | --- |
| SST deploy | Pass | Dev stack deployed successfully after Rust Lambda packaging and lockfile fixes. |
| Public discovery | Pass | `/.well-known/openid-configuration` returns `200`; no introspection endpoint advertised. |
| JWKS | Pass | `/.well-known/jwks.json` returns `200`; no private key material present. |
| Public authorize | Pass | `/authorize` returns `303` to `/password/login?session=<redacted>` and sets a session cookie. |
| Auth session storage | Pass | Bounded query on `oauth:session` shows HMAC-looking `sk` and `expiry`. |
| DynamoDB TTL | Pass | Table TTL is `ENABLED` on `expiry`. |
| API Gateway source identity | Pass | Spoofed `x-forwarded-for` and `x-real-ip` did not appear in the persisted authorize rate-limit key. |
| Unsigned admin route | Pass | `GET /_admin/users/user_smoke` returns `403`. |
| Custom admin key | Pass | `x-admin-key` does not bypass IAM; request returns `403`. |
| SigV4 admin route | Pass | Signed request reaches admin Lambda and returns domain `404` for a missing account. |
| Lambda split | Pass | Public routes point to public auth Lambda; admin routes point to admin Lambda. |
| Lambda runtime | Pass | Both Lambdas use `provided.al2023` and `bootstrap`. |
| IAM actions | Pass | Public/admin roles have exact DynamoDB actions plus public signing KMS permissions; no `dynamodb:Scan` or wildcard table action. |
| Logging | Pass | Lambda logs use JSON logging and 30-day retention. |
| Full password/token loop | Blocked | Requires consuming the verification email delivered by Resend; raw verification token is intentionally not available from DynamoDB or logs. |

## Findings Fixed During Smoke

### Missing Client Config In Lambda Bundle

Initial public requests failed because `auth.clients.toml` was not included in the public Lambda bundle.

Fix:

- `infra/rust-bundle.ts` now supports copying static files into the Lambda bundle.
- `infra/api.ts` copies `auth.clients.toml` for the public auth Lambda.

### Rust Lambda Packaging

SST's Rust packaging expected a `Cargo.toml` at a different shape than this repo uses.

Fix:

- Both Lambdas now build explicitly with `cargo lambda build`.
- SST deploys them as `provided.al2023` custom-runtime Lambdas with `handler: "bootstrap"`.

### Admin Lockfile

The admin Lambda lockfile was stale and failed `--locked` cargo-lambda builds.

Fix:

- `packages/functions/admin/Cargo.lock` was updated.
- Admin app startup was aligned with the current auth-store constructor.

### DynamoDB Transaction Shape

AWS rejected transactions that contained a `ConditionCheck` and `Put` against the same item.

Fix:

- The storage transaction abstraction now supports conditional `Put`.
- One-time secret, password user, account, identity, and refresh store paths use AWS-valid transaction items.
- In-memory test backends now enforce conditional `Put` behavior so local tests match AWS more closely.

### DynamoDB IAM Action

Typed store transactions use transaction condition checks.

Fix:

- Runtime table permissions now include `dynamodb:ConditionCheckItem`.
- Public/admin roles still do not include `dynamodb:Scan` or `dynamodb:*`.

## Remaining Manual Step

To complete the end-to-end password/token loop:

1. Register a test email through `POST /password/register`.
2. Open the Resend-delivered verification link for that test email.
3. Run `/authorize`.
4. Submit `POST /password/login`.
5. Exchange the authorization code at `POST /token`.
6. Call `/userinfo`.
7. Rotate refresh token with `grant_type=refresh_token`.
8. Revoke the refresh-token family with `POST /oauth/revoke`.

This step is intentionally not bypassed from the CLI because the verification token is a user-facing email secret and is not stored raw in DynamoDB.
