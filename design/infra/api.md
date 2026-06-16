# Infra API

Target code: `infra/api.ts`

## Owns

- API Gateway HTTP API creation.
- Route wiring to the public auth Lambda and admin Lambda.
- Access log retention.
- CORS configuration.
- Optional custom domain configuration.
- IAM authorization on admin lifecycle routes.

## Target Behavior

The default deployment uses one HTTP API and two Rust Lambda integrations. HTTP API is preferred over REST API because it is simpler and cheaper for this template.

The public auth surface can keep `$default` routing to the public auth Lambda. Admin lifecycle routes must be explicit `/_admin/*` routes wired to the admin Lambda with IAM authorization enabled, because API Gateway should reject unsigned admin calls before any admin code runs.

In SST, create the public and admin Lambdas as named `sst.aws.Function` components and route API Gateway to those function ARNs. Do not pass inline `FunctionArgs` to each `api.route` call for these routes, because that creates a separate Lambda per route instead of the intended shared public/admin Lambda boundary.

Target route shape:

```text
$default                         -> public auth Lambda, no IAM
GET  /_admin/users/{subject}     -> admin Lambda, IAM required
POST /_admin/users/{subject}/disable
POST /_admin/users/{subject}/revoke-sessions
POST /_admin/users/{subject}/delete  later deletion slice
```

## Security Invariants

- Production CORS should be restricted to configured origins.
- Logs must not include request bodies, tokens, codes, or secrets.
- Access logs should be structured and retained only as long as needed for operations.
- Rate limiting must use trusted request context data for source IP, not spoofable forwarded headers.
- The auth Lambda integration must preserve or expose API Gateway request context source IP to auth code.
- `x-forwarded-for` and `x-real-ip` are not trusted rate-limit inputs in API Gateway mode.
- `ISSUER_URL` must match the public URL clients use, especially with a custom domain.
- WAF is optional production hardening for abuse-heavy deployments, not part of the minimal template.
- Admin lifecycle routes use API Gateway IAM auth and SigV4, not cookies, bearer tokens, CORS, or custom admin API keys.

## Inputs

- Stage name.
- Optional custom domain.
- Allowed CORS origins.
- Public auth Lambda reference.
- Admin Lambda reference.
- API Gateway request context, including source IP.
- Admin route IAM authorization settings.

## Outputs

- Public API URL.
- API Gateway identifier, if needed by later tooling.
- Admin route ARNs for operator IAM policy examples.

## Access Logs

Retention is config-based. CloudWatch remains the default log destination for v1.

```text
AUTH_AUDIT_LOG_MODE optional, default cloudwatch
AUTH_LOG_RETENTION_DAYS optional
```

`AUTH_AUDIT_LOG_MODE=none` disables auth security audit events, but Lambda and API Gateway operational/error logs can still exist according to infrastructure settings.

Access logs should include operational metadata only:

```text
request id
route
status
latency
source IP from request context
user agent if needed
```

They must not include:

```text
request body
authorization header
cookies
auth codes
refresh tokens
verification or reset secrets
provider tokens
```
