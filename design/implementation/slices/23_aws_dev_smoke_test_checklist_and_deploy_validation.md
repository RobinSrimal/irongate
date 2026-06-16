# 23_aws_dev_smoke_test_checklist_and_deploy_validation

## Goal

Prepare and run the first AWS dev deployment smoke validation for the simplified auth template.

At the end of this slice, the repo should have a concrete AWS dev smoke-test checklist and any small helper scripts needed to validate the deployed shape. The validation should prove the SST deployment matches the design assumptions for API Gateway, public/admin Lambda separation, IAM admin protection, DynamoDB key shape, TTL attributes, logging defaults, and stage/account configuration.

## Design Docs Followed

This slice should follow these design documents:

- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/iam.md`
- `design/infra/operator-iam-policy.md`
- `design/infra/stages.md`
- `design/infra/storage.md`
- `design/infra/secrets.md`
- `design/infra/performance.md`
- `design/auth/config/stages.md`
- `design/auth/config/environment.md`
- `design/auth/config/ttls.md`
- `design/auth/store/dynamodb.md`
- `design/auth/store/keys.md`
- `design/auth/store/records.md`
- `design/auth/store/rate-limits.md`
- `design/auth/observability/audit.md`
- `design/auth/testing.md`
- `design/migration.md`
- `design/implementation/slices/14_api_gateway_source_identity_and_route_validation.md`
- `design/implementation/slices/15_storage_kms_iam_and_logging_hardening.md`
- `design/implementation/slices/20_store_boundary_and_in_memory_test_backend.md`
- `design/implementation/slices/21_admin_store_boundary_and_internal_backend_cleanup.md`
- `design/implementation/slices/22_internal_store_query_and_backend_visibility_cleanup.md`

The important design constraint is that this is the first live AWS validation pass after the codebase was simplified. It should verify the deployed infrastructure and runtime boundaries, not add product features.

## Why This Slice Next

The rewrite has moved the auth runtime to the target local shape:

```text
HTTP API
  $default public auth Lambda
  /_admin/* IAM-protected admin Lambda
  DynamoDB AuthTable
```

Static tests now cover many security assumptions, but several important claims are AWS runtime properties:

- API Gateway source IP arrives in Lambda request context.
- Spoofed `x-forwarded-for` and `x-real-ip` do not affect rate-limit identity.
- API Gateway rejects unsigned admin routes before admin code runs.
- Signed IAM admin calls reach the admin Lambda.
- DynamoDB TTL is enabled on `expiry`.
- Short-lived auth records write TTL attributes.
- Raw bearer values do not appear in `pk` or `sk`.
- Lambda roles do not need `dynamodb:Scan`.
- CloudWatch logging and audit mode defaults work in a deployed stage.
- SST stage/profile mapping uses the intended dev account.

This slice turns those assumptions into an executable checklist before broader production-hardening or load testing.

## Scope Decision

This is a dev smoke-validation slice, not a production rollout slice.

In scope:

- Add an AWS dev smoke-test checklist document.
- Add small validation helper scripts only if they avoid repeated manual mistakes.
- Deploy one dev stage with the existing SST app.
- Validate public auth routes and discovery metadata over the deployed API URL.
- Validate admin route IAM protection and SigV4 invocation path.
- Validate API Gateway request-context source IP behavior indirectly through rate-limit records or audit/log output.
- Validate DynamoDB table key shape and TTL attributes using bounded `Query`, not table `Scan`.
- Validate deployed outputs: API URL, API ID, table name, KMS key mode, signing key mode, and admin route ARN pattern.
- Validate log retention and audit mode defaults.
- Record findings and follow-up fixes in the slice result.

Out of scope:

- Production deployment.
- Load testing beyond a small manual smoke loop.
- Performance tuning.
- Changing Lambda memory or timeout unless the smoke test exposes an obvious blocker.
- Adding new auth flows.
- Changing DynamoDB schema.
- Changing client configuration model.
- Rotating secrets.
- Building a dashboard.
- Adding a sample app.

## Target Artifacts

Create:

```text
design/implementation/aws-dev-smoke-test.md
```

The checklist should include:

- Required local prerequisites.
- Required dev-stage environment variables and SST secrets.
- Deploy command.
- Output capture section.
- Public route checks.
- Password registration/login/token flow checks.
- Admin route IAM checks.
- DynamoDB key/TTL checks.
- CloudWatch/audit checks.
- IAM permission checks.
- Cleanup command.
- Findings section.

Optional helper scripts may be added under:

```text
scripts/aws-smoke/
```

Only add scripts when they can be short, deterministic, and safe. Scripts must not print secrets, tokens, authorization codes, refresh tokens, verification links, or raw provider credentials.

## Required Preconditions

The smoke test should require the operator to confirm:

```text
AWS_PROFILE is not overriding SST profile selection
SST dev stage has access to the dev AWS account
cargo-lambda is installed for Rust Lambda packaging
infra/stage-config.ts has dev settings for email, signing, audit, logs, and KMS
SST secret AuthHmacLookupSecret is set for stage dev
SST secret ResendApiKey is set for stage dev
SST secret AuthSigningPrivateKey is set for stage dev only if dev uses local-es256
AUTH_EMAIL_FROM in stage config is a Resend-verified dev sender
auth.clients.toml contains at least one public PKCE test client with localhost or test callback URI
```

For a first dev smoke pass, Google and Apple provider credentials can be omitted if disabled in config. The smoke checklist should focus on password auth, OAuth code/token exchange, refresh, userinfo, logout, admin lifecycle, and infrastructure boundaries.

## Validation Flow

### 1. Local Preflight

Run before deploy:

```text
npm run test:infra
npm run typecheck
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
```

Expected:

```text
all commands exit 0
```

### 2. Deploy Dev Stage

Deploy the dev stage:

```text
npm run deploy -- --stage dev
```

Capture outputs:

```text
ApiUrl
ApiId
AdminRouteArnPattern
TableName
TableKmsKeyArn
SigningKmsKeyArn
```

Validation rules:

- `ApiUrl` is the issuer URL unless `ISSUER_URL` was explicitly set.
- `TableName` exists in the dev account.
- `TableKmsKeyArn` is `aws-owned` by default or a customer managed KMS ARN when `AUTH_TABLE_KMS=customer`.
- `AdminRouteArnPattern` contains only `/_admin/users/*` route scope.

### 3. Public Auth Route Smoke

Check deployed discovery:

```text
curl -sS "$ApiUrl/.well-known/openid-configuration"
curl -sS "$ApiUrl/.well-known/jwks.json"
```

Expected:

- Discovery issuer matches the deployed public URL.
- Discovery does not advertise token introspection.
- Discovery advertises only mounted endpoints.
- JWKS exposes public key material only.
- No response contains private key material.

Check authorize route with a configured public PKCE client:

```text
GET /authorize?response_type=code&client_id=<client>&redirect_uri=<exact-registered-uri>&scope=openid%20email%20offline_access&state=smoke-state&code_challenge=<S256-challenge>&code_challenge_method=S256&provider=password
```

Expected:

- Response redirects or returns the API-only password continuation shape already implemented.
- DynamoDB contains an `oauth:session` record keyed by HMAC digest, not the raw session key.
- `expiry` exists on the session item.

### 4. Password And Token Smoke

Run the minimum password flow against the deployed API:

```text
POST /password/register
consume verification link from the dev Resend inbox or captured dev email destination
GET/POST verification endpoint as designed
GET /authorize for provider=password
POST /password/login
POST /token grant_type=authorization_code
GET /userinfo with access token
POST /token grant_type=refresh_token
POST /oauth/revoke
```

Expected:

- Registration does not issue tokens.
- Verification creates a verified password identity.
- Login is the first password operation that issues an authorization code.
- Token exchange returns JWT access token and ID token when `openid` was granted.
- Refresh returns rotated refresh token when `offline_access` is allowed.
- Userinfo succeeds with access token.
- Revoke is idempotent for the client's refresh token.
- Logs and responses do not expose password hashes, raw reset/verification secrets, authorization-code digests, refresh-token digests, signing keys, provider credentials, or client secrets.

### 5. API Gateway Source Identity Smoke

Exercise a rate-limited endpoint with spoofed forwarded headers:

```text
curl -i "$ApiUrl/authorize?..." \
  -H "x-forwarded-for: 198.51.100.10" \
  -H "x-real-ip: 198.51.100.11"
```

Expected:

- Rate-limit/audit source identity uses API Gateway request context source IP.
- Spoofed forwarded headers do not appear in persisted rate-limit keys.
- No runtime path trusts `x-forwarded-for` or `x-real-ip` in API Gateway mode.

Validation may use bounded DynamoDB `Query` against the `ratelimit` partition or structured logs. Do not use table-wide `Scan`.

### 6. Admin IAM Smoke

Unsigned request:

```text
curl -i "$ApiUrl/_admin/users/user_smoke"
```

Expected:

```text
403 Forbidden from API Gateway or admin route guard
```

Custom admin key request:

```text
curl -i "$ApiUrl/_admin/users/user_smoke" -H "x-admin-key: anything"
```

Expected:

```text
403 Forbidden
```

Signed request with an operator role allowed by `AdminRouteArnPattern`:

```text
SigV4 GET /_admin/users/<subject>
SigV4 POST /_admin/users/<subject>/revoke-sessions
SigV4 POST /_admin/users/<subject>/disable
```

Expected:

- Signed request reaches the admin Lambda.
- Sanitized account response contains subject/status/timestamps only.
- Revoke sessions does not disable the account.
- Disable marks the account inactive and revokes refresh families.
- Admin Lambda does not require Resend, Google, Apple, or local signing private-key secrets.

### 7. DynamoDB Key Shape And TTL Smoke

Use AWS CLI bounded queries, not table scans:

```text
aws dynamodb query \
  --table-name "$TableName" \
  --key-condition-expression "pk = :pk" \
  --expression-attribute-values '{":pk":{"S":"oauth:code"}}'
```

Repeat for relevant known partitions created during the smoke flow:

```text
oauth:session
oauth:code
oauth:refresh
provider:state if Google/Apple smoke is enabled
password:verify
password:reset if reset smoke is run
ratelimit
```

Expected:

- Raw authorization codes are not present in `pk` or `sk`.
- Raw refresh tokens are not present in `pk` or `sk`.
- Raw verification/reset tokens are not present in `pk` or `sk`.
- Provider state values are not present in `pk` or `sk`.
- Short-lived records have `expiry`.
- Runtime expiry timestamps in record values match the intended TTL window.
- DynamoDB table TTL is enabled on `expiry`.

TTL configuration check:

```text
aws dynamodb describe-time-to-live --table-name "$TableName"
```

Expected:

```text
AttributeName = expiry
TimeToLiveStatus = ENABLED or ENABLING
```

### 8. IAM Permission Smoke

Inspect deployed Lambda policies:

```text
aws lambda get-policy --function-name <public-auth-function>
aws lambda get-policy --function-name <admin-function>
```

Or inspect IAM role attached policies through AWS CLI/console.

Expected:

- Public auth Lambda has DynamoDB exact-key/query/transaction permissions required by typed store operations.
- Admin Lambda has only lifecycle-table permissions.
- Neither runtime role requires `dynamodb:Scan`.
- Admin Lambda does not receive provider/email/signing private-key secrets.
- KMS permissions are scoped to configured keys when KMS modes are enabled.

### 9. CloudWatch And Audit Smoke

Check log groups for API Gateway and both Lambdas.

Expected:

- Logs use JSON format where configured.
- Retention matches `AUTH_LOG_RETENTION_DAYS` / infra config.
- `AUTH_AUDIT_LOG_MODE` defaults to `cloudwatch`.
- `AUTH_AUDIT_LOG_MODE=none` can be explicitly configured in a separate validation deploy if needed.
- Audit events do not include tokens, passwords, provider credentials, client secrets, verification links, reset links, or private keys.
- Startup logs do not print secret values.

### 10. Cleanup

For disposable dev validation only:

```text
npm run remove -- --stage dev
```

Expected:

- Dev resources are removed when stage removal policy allows it.
- Production stage remains protected and retained.

Do not run removal against production.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Create `design/implementation/aws-dev-smoke-test.md` with the checklist sections from this slice.
2. Add placeholders for operator-filled values using angle-bracket labels, not real secrets.
3. Add a findings table with rows for manual observations.
4. Run local preflight.
5. Deploy dev stage when AWS credentials and required secrets are available.
6. Capture SST outputs in the smoke-test document.
7. Run public discovery and JWKS checks.
8. Run password registration, verification, login, token, userinfo, refresh, and revoke smoke flow.
9. Run API Gateway source identity spoof-header check.
10. Run unsigned and custom-key admin route checks.
11. Run SigV4-signed admin route checks with an operator IAM role.
12. Run DynamoDB TTL and bounded key-shape queries.
13. Run IAM permission inspection.
14. Run CloudWatch/audit log inspection.
15. Record observed results and follow-up findings.
16. If the dev stage is disposable, remove it.
17. Run final local verification.

## Acceptance Criteria

- `design/implementation/aws-dev-smoke-test.md` exists and is detailed enough for a developer to run without prior context.
- The smoke checklist uses bounded DynamoDB `Query`, not table `Scan`.
- The smoke checklist does not ask developers to paste secrets into committed files.
- Dev deployment outputs are captured when the smoke test is run.
- Each AWS validation item is marked pass, fail, skipped, or blocked.
- Any failure has a concrete follow-up note.
- No production deployment is performed in this slice.
- Local preflight and final verification pass.

## Manual Validation

This slice is itself a manual AWS validation slice.

Do not mark it complete unless either:

- the AWS dev smoke test was run and results were recorded, or
- the slice is explicitly stopped as blocked because AWS credentials/secrets are unavailable.

If blocked, record exactly which prerequisite is missing:

```text
AWS profile
SST dev stage access
Resend dev key
AUTH_EMAIL_FROM
HMAC lookup secret
signing key configuration
client config
operator IAM role
```

## Next Slice

After this slice, define the next slice based on AWS smoke results.

Likely follow-ups:

- Fix any deployed API Gateway/IAM mismatch.
- Fix any missing TTL/key-shape issue.
- Tighten Lambda IAM permissions if the deployed role is broader than the design.
- Add a lightweight smoke-test helper script if manual validation is too error-prone.
- Start a performance/load validation slice only after correctness and security smoke checks pass.
