# 25_security_review_hardening

## Goal

Close the concrete security-review gaps found after the first AWS dev smoke validation without adding new auth product scope.

At the end of this slice, the auth core should have stronger abuse controls, better account-containment semantics, audit configuration that is actually honored, and safer stage selection for production deploys.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/store/rate-limits.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/revoke.md`
- `design/auth/api/providers/google.md`
- `design/auth/api/providers/apple.md`
- `design/auth/api/providers/password.md`
- `design/auth/api/admin.md`
- `design/auth/core/account-lifecycle.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/authorization-codes.md`
- `design/auth/store/authorize-sessions.md`
- `design/auth/store/provider-states.md`
- `design/auth/store/refresh-tokens.md`
- `design/auth/observability/audit.md`
- `design/auth/config/stages.md`
- `design/infra/stages.md`
- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/iam.md`
- `design/auth/testing.md`
- `design/scope.md`

The important design constraint is that this is a hardening slice. It must not introduce dashboards, hosted UI, runtime client management, public bootstrap, passwordless OTP, token introspection, opaque access tokens, generic identity providers, or payment features.

## Why This Slice Next

The current core has passed the first useful live AWS smoke checks, but the security review identified seven concrete follow-up issues:

| ID | Issue | Target outcome |
| --- | --- | --- |
| H1 | Rate limiting is non-atomic and fails open on storage errors | Atomic counters and explicit failure policy |
| H2 | Public clients can globally throttle `/token` for other users | Token rate-limit keys include public client and trusted source |
| H3 | Pending reset links survive disable/re-enable | Disable and enable clear pending reset secrets |
| H4 | Public write-amplification endpoints lack limits | Revoke and provider-start endpoints get rate limits and safer audit writes |
| H5 | Authorization codes are consumed before redirect URI and PKCE validation | Code consume validates stored fields before delete or in the same typed operation |
| H6 | `AUTH_AUDIT_LOG_MODE=none` is parsed but not honored | Audit writes obey runtime audit mode |
| H7 | Production stage naming is easy to get wrong | Unsupported/prod-like stage names fail safely |

These are all bounded hardening changes around already-implemented flows. They should be fixed before more feature slices or production rollout work.

## In Scope

### H1: Atomic Rate Limits

Replace the current rate-limit read-modify-write helper with a typed store operation that performs concurrency-safe counter updates.

Required behavior:

- Counter increments are atomic for one `(endpoint, identifier)` bucket.
- Counters retain DynamoDB TTL cleanup.
- The stored counter still carries enough window metadata to reset after the window expires.
- Concurrent requests cannot all pass by reading the same stale count.
- Storage read/write errors do not silently allow sensitive requests.
- Rate-limit errors still return `429 Too Many Requests` with the existing response shape.
- Limiter storage failures return a safe temporary failure for sensitive public endpoints.

Suggested store operation:

```text
check_rate_limit(endpoint, identifier, limit, now) -> allowed | limited | unavailable
```

Implementation may use a DynamoDB transaction or conditional update. The key requirement is that the counter update and limit decision are not a plain get-plus-set race.

### H2: Token Endpoint Rate-Limit Identity

Update `/token` rate-limit identity so a public `client_id` is not the entire bucket.

Required behavior:

- Public clients use a composite token rate-limit identifier:

```text
client:<client_id>:source:<trusted_api_gateway_source_ip>
```

- Confidential clients with validated client authentication may use a client-bound key.
- If client authentication has not yet been validated, the route may use a pre-auth composite key and then validate the client.
- The trusted source must still come from Lambda/API Gateway request context, not `x-forwarded-for` or `x-real-ip`.
- Raw authorization codes, refresh tokens, client secrets, or code verifiers must not enter rate-limit keys.

The implementation should keep the existing public-client behavior compatible for valid users while removing the global client-level denial-of-service bucket.

### H3: Clear Pending Reset Secrets During Account Containment

Make admin containment actions invalidate pending password reset secrets for the subject.

Required behavior:

- `disable_account` admin route clears pending password reset secrets for the subject.
- `enable_account` admin route also clears pending password reset secrets for defense in depth.
- `delete_account` continues clearing pending reset secrets as it does today.
- The mutation response may include `deleted_password_secrets` or a clearer `cleared_password_reset_secrets` count.
- The audit event may include a sanitized count.
- No raw reset token, reset digest, email, or password hash appears in responses, logs, or audit details.

Rationale:

`disable` should be a containment primitive. If an operator disables a suspected compromised account, existing credential-changing links should not survive and become usable again after enable.

### H4: Rate-Limit Public Write-Amplification Endpoints

Add abuse controls to public endpoints that can write records without authenticated user proof.

Required endpoints:

```text
POST /oauth/revoke
GET  /google/authorize
GET  /apple/authorize
```

Required behavior:

- `/oauth/revoke` applies a rate limit based on client id plus trusted source when a public client is used.
- `/oauth/revoke` remains idempotent and client-bound.
- `/oauth/revoke` should not persist an audit event for obviously invalid/random refresh tokens unless the implementation intentionally records a coarse, rate-limited security event.
- Provider-start endpoints apply rate limits based on provider plus trusted source, and preferably authorize-session digest plus trusted source.
- Provider-start rate-limit keys must not contain raw authorize session keys.
- Provider-state records remain TTL-bound.

This is a cost and noise control. It is not expected to change successful OAuth/OIDC behavior.

### H5: Validate Authorization Code Before Consume

Move authorization-code validation into a typed consume operation or change the token flow so invalid client/redirect/PKCE attempts cannot burn a code before validation.

Required behavior:

- Authorization codes remain single-use.
- Expired codes are rejected and cleaned up.
- Client id, redirect URI, PKCE method, and PKCE verifier are validated before deleting the code, or the validation and delete are performed atomically inside one typed store operation.
- A wrong PKCE verifier does not consume the code.
- A wrong redirect URI does not consume the code.
- A wrong client id does not consume the code.
- A valid exchange consumes the code exactly once.
- No raw authorization code appears in DynamoDB keys, logs, audit details, or errors.

Suggested operation:

```text
consume_authorization_code_for_exchange(code_digest, client_id, redirect_uri, code_verifier)
```

The operation may accept the expected PKCE challenge or perform PKCE validation in the token layer before an atomic conditional delete. Keep the final boundary simple and testable.

### H6: Honor Audit Log Mode

Wire audit emission through a runtime-aware audit helper so `AUTH_AUDIT_LOG_MODE=none` disables audit event emission.

Required behavior:

- `cloudwatch` keeps emitting sanitized audit events.
- `none` emits no audit event records.
- Ordinary Lambda startup/error logs still work when audit mode is `none`.
- Public auth and admin Lambda paths both obey their configured audit mode.
- Startup may log the selected audit mode, but must not print secrets.
- Existing audit event sanitization invariants remain.

Because current audit events are persisted through the auth store, the implementation should make the mode decision before calling the persistence path.

### H7: Stage Safety

Make stage selection explicit so production-like deploy commands cannot silently use dev configuration.

Required behavior:

- Supported stages are explicit:

```text
dev
production
```

- `--stage production` uses the prod AWS profile, `retain`, `protect: true`, production stage config, and production KMS defaults.
- `--stage dev` uses the dev AWS profile and dev stage config.
- Ambiguous production-like names such as `prod` fail with a clear error that says to use `production`.
- Unknown stage names fail unless the template intentionally adds an explicit config entry for that stage.
- The setup/template docs should tell users to edit stage names deliberately if they want more environments.

This preserves simple dev/prod behavior and avoids accidental prod deployments with dev removal policy, dev email URLs, or AWS-owned table KMS.

## Out Of Scope

- Changing provider OIDC callback semantics beyond rate limiting provider-start.
- Google or Apple authorized-party (`azp`) hardening.
- Dependency advisory tooling installation.
- WAF, Shield, CloudFront, or API Gateway account-level throttling.
- Full load testing.
- Dashboard or admin UI.
- Local-only dashboard tooling.
- Runtime OAuth client management.
- Generic audit sinks such as S3.
- Changing refresh-token family design except where revoke rate limiting touches `/oauth/revoke`.
- Fixing local ignored `.env` hygiene beyond not committing secrets.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree where practical.

Target modules:

```text
packages/functions/auth/src/store/rate_limits.rs
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/revoke.rs
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/api/admin.rs
packages/functions/auth/src/store/authorization_codes.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/audit.rs
packages/functions/auth/src/config/environment.rs
packages/functions/admin/src/main.rs
sst.config.ts
infra/stage-config.ts
scripts/validate-infra-hardening.mjs
packages/functions/auth/tests/oauth_token_userinfo.rs
packages/functions/auth/tests/oauth_refresh_revoke.rs
packages/functions/auth/tests/oidc_google_start.rs
packages/functions/auth/tests/oidc_apple_start.rs
packages/functions/auth/tests/password_reset.rs
packages/functions/auth/tests/admin_lifecycle.rs
packages/functions/auth/tests/startup_config.rs
```

Do not create a new general-purpose middleware framework unless it removes real duplication from the touched endpoints.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add tests that prove concurrent or repeated rate-limit checks cannot all pass for one bucket.
2. Add a typed rate-limit store operation with atomic DynamoDB behavior and an in-memory test backend equivalent.
3. Replace the current fail-open rate-limit helper with explicit `allowed`, `limited`, and `unavailable` outcomes.
4. Update password and authorize endpoints to use the new helper without changing response shape for normal limits.
5. Update `/token` rate-limit keys so public clients include trusted source identity.
6. Add route tests proving one public client caller cannot exhaust another source's `/token` bucket.
7. Add `/oauth/revoke`, `/google/authorize`, and `/apple/authorize` rate limits.
8. Update revoke audit behavior so random invalid tokens do not create unbounded audit records.
9. Add admin lifecycle tests showing disable and enable clear pending password reset secrets.
10. Wire reset-secret clearing into disable and enable.
11. Add authorization-code tests showing wrong PKCE, wrong redirect URI, and wrong client id do not consume a valid code.
12. Move code exchange validation into a typed consume operation or otherwise validate before delete.
13. Add audit-mode tests for `cloudwatch` and `none` in public auth and admin paths.
14. Route all audit writes through a mode-aware helper.
15. Add stage-config tests or static infra validator checks for `dev`, `production`, `prod`, and unknown stage names.
16. Update SST/stage config so unsupported stage names fail clearly.
17. Update design docs if implementation clarifies rate-limit identities, audit mode, account containment, or stage policy.
18. Run full local verification.
19. Deploy dev and rerun the relevant smoke checks for rate limits, admin disable/enable, audit mode, and stage outputs.

## Tests

### Rate Limit Tests

- Atomic limiter denies the request after the configured count.
- Concurrent same-bucket attempts cannot exceed the configured limit.
- Storage read/write failure returns a safe unavailable outcome for sensitive endpoints.
- Existing `429 Too Many Requests` response shape remains.
- Rate-limit keys do not include raw email, password, reset token, authorization code, refresh token, client secret, provider state, or authorize session key.

### Token Endpoint Tests

- Public client token limits include trusted source identity.
- Requests from source A do not exhaust source B's public-client token bucket.
- Confidential-client token behavior remains compatible after client authentication succeeds.
- Spoofed `x-forwarded-for` and `x-real-ip` do not affect token rate-limit identity.

### Public Write Endpoint Tests

- `/oauth/revoke` is rate-limited.
- Invalid/random revoke tokens do not create unbounded audit rows.
- Valid revoke remains idempotent.
- `/google/authorize` is rate-limited without storing raw session keys.
- `/apple/authorize` is rate-limited without storing raw session keys.
- Successful provider-start still creates provider state and redirects to the provider.

### Account Lifecycle Tests

- Disable clears pending password reset secrets for the subject.
- Enable clears pending password reset secrets for the subject.
- Delete continues clearing pending password reset secrets.
- Disable and enable audit details include only sanitized counts.
- A reset token issued before disable cannot be used after disable/enable.

### Authorization Code Tests

- Wrong PKCE verifier does not consume the authorization code.
- Wrong redirect URI does not consume the authorization code.
- Wrong client id does not consume the authorization code.
- Successful exchange consumes the code.
- Reusing a successfully exchanged code fails.
- Expired codes are rejected and cleaned up.

### Audit Mode Tests

- `AUTH_AUDIT_LOG_MODE=cloudwatch` persists or emits audit events.
- `AUTH_AUDIT_LOG_MODE=none` skips audit event persistence/emission.
- Ordinary error logging still occurs when audit mode is `none`.
- Admin routes obey audit mode.
- Public auth routes obey audit mode.

### Stage Safety Tests

- `dev` resolves to dev profile/config/removal policy.
- `production` resolves to prod profile/config/protection/retention.
- `prod` fails with a clear error.
- An unknown stage fails with a clear error.
- Infra validators catch accidental addition of a broad fallback-to-dev stage path.

## Acceptance Criteria

- Rate-limit counters are atomic and do not fail open silently.
- Public-client `/token` rate limiting cannot be exhausted globally by one source.
- Revoke and provider-start endpoints have abuse limits.
- Invalid revoke attempts do not create unbounded audit noise.
- Disable and enable clear pending password reset secrets.
- Authorization-code validation failures do not burn codes.
- Audit mode `none` is honored in public and admin paths.
- Stage names are explicit and production-like typos fail safely.
- No raw tokens, codes, passwords, reset links, provider state, client secrets, or private keys are stored in rate-limit keys, audit details, logs, or error messages.
- The auth Lambda remains API-only.
- Admin routes remain IAM-protected and served by the separate admin Lambda.

## Manual Validation

Local validation:

```text
npm run test:infra
npm run typecheck
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo test --manifest-path packages/functions/admin/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/admin/Cargo.toml
```

Dev AWS validation after deploy:

```text
npm run deploy -- --stage dev
```

Smoke checks:

- Run password login attempts past the configured limit and confirm only the configured bucket is limited.
- Run token attempts from two distinct source identities if possible and confirm buckets do not collide for a public client.
- Attempt random `/oauth/revoke` spam and confirm rate limiting/audit behavior.
- Start Google/Apple provider handoff repeatedly with one session and confirm rate limiting.
- Request a password reset, disable the account, enable it, then prove the old reset token no longer works.
- Exchange an authorization code with a wrong verifier, then exchange it with the correct verifier and confirm it still works.
- Deploy a temporary validation stage with `AUTH_AUDIT_LOG_MODE=none` or equivalent stage config and confirm audit rows are not written.

Do not run production deployment in this slice.

## Next Slice

After this slice, define the next slice based on the hardening results.

Likely follow-ups:

- Google/Apple OIDC `azp` authorized-party validation for multi-audience ID tokens.
- Sanitized public `server_error` responses.
- Rust dependency advisory tooling such as `cargo-audit` or `cargo-deny`.
- Optional AWS WAF/account-level throttling design for production deployments.
