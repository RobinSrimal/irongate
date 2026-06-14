# Infra API

Target code: `infra/api.ts`

## Owns

- API Gateway HTTP API creation.
- Route wiring to the auth Lambda.
- Access log retention.
- CORS configuration.
- Optional custom domain configuration.

## Target Behavior

The default deployment uses one HTTP API and one Rust Lambda route. HTTP API is preferred over REST API because it is simpler and cheaper for this template.

The first implementation can keep `$default` routing to the Lambda. If we need stricter API Gateway controls later, routes can become explicit without changing the auth protocol modules.

## Security Invariants

- Production CORS should be restricted to configured origins.
- Logs must not include request bodies, tokens, codes, or secrets.
- Access logs should be structured and retained only as long as needed for operations.
- Rate limiting must use trusted request context data for source IP, not spoofable forwarded headers.
- The auth Lambda integration must preserve or expose API Gateway request context source IP to auth code.
- `x-forwarded-for` and `x-real-ip` are not trusted rate-limit inputs in API Gateway mode.
- `ISSUER_URL` must match the public URL clients use, especially with a custom domain.
- WAF is optional production hardening for abuse-heavy deployments, not part of the minimal template.

## Inputs

- Stage name.
- Optional custom domain.
- Allowed CORS origins.
- Auth Lambda reference.
- API Gateway request context, including source IP.

## Outputs

- Public API URL.
- API Gateway identifier, if needed by later tooling.

## Access Logs

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
