# 15_storage_kms_iam_and_logging_hardening

## Goal

Harden the AWS deployment shape around DynamoDB encryption, Lambda/table permissions, CloudWatch logging, and operator IAM guidance without changing OAuth/OIDC protocol behavior.

At the end of this slice, template users should be able to choose the DynamoDB table encryption mode, configure log retention and audit-log emission, inspect explicit admin route outputs/policy examples, and deploy public/admin Lambdas with narrower table permissions than broad linked defaults.

This slice intentionally stops before KMS ES256 token signing. DynamoDB table encryption and token-signing key custody are related security topics, but they touch different runtime paths and should stay separate.

## Design Docs Followed

This slice should follow these design documents:

- `design/infra/storage.md`
- `design/infra/iam.md`
- `design/infra/secrets.md`
- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/stages.md`
- `design/auth/observability/audit.md`
- `design/auth/observability/README.md`
- `design/storage-security.md`
- `design/scope.md`

The important design constraint is that this is AWS infrastructure hardening for the existing API-only auth core. It must not add dashboard behavior, hosted UI, runtime OAuth client management, public admin bootstrap, or a new storage backend.

## Why This Slice Next

Slice 14 hardened the API Gateway request boundary and route validation. The next deployment risk is the resource boundary:

```text
Lambda role -> DynamoDB table -> optional KMS key -> CloudWatch logs -> operator IAM policy
```

The current infra is intentionally minimal, but production users need explicit controls for table key ownership, retention, audit-log mode, and admin IAM invocation. This slice adds those controls before KMS JWT signing and final legacy removal.

## In Scope

### Infra Config Helpers

Add a small infra configuration module so deployment choices are parsed once and can be statically tested.

Target file:

```text
infra/config.ts
```

Required settings:

```text
AUTH_TABLE_KMS=aws-owned|customer
AUTH_LOG_RETENTION_DAYS=<supported number of days>
AUTH_AUDIT_LOG_MODE=cloudwatch|none
```

Default behavior:

```text
AUTH_TABLE_KMS=aws-owned
AUTH_AUDIT_LOG_MODE=cloudwatch
AUTH_LOG_RETENTION_DAYS=30
```

Validation requirements:

- Reject unknown `AUTH_TABLE_KMS` values.
- Reject unknown `AUTH_AUDIT_LOG_MODE` values.
- Reject unsupported or non-positive `AUTH_LOG_RETENTION_DAYS` values.
- Keep config parsing deterministic and side-effect free so scripts can import or mirror it safely.

### DynamoDB Table KMS Mode

Update `infra/storage.ts` so the physical table still has the same key shape and TTL, but can optionally use a customer managed KMS key.

Required table invariants:

```text
pk string
sk string
ttl expiry
```

Mode behavior:

| Mode | Behavior |
| --- | --- |
| `aws-owned` | Use DynamoDB default encryption and do not create a customer managed KMS key. |
| `customer` | Create a stage/account-specific customer managed KMS key and configure the table to use it. |

Suggested alias pattern:

```text
alias/<project-name>/auth-table-<stage>
```

Implementation details should follow the current SST v4/Pulumi types in this repo. If the exact SST property for DynamoDB customer-managed encryption needs lower-level transforms, keep that logic contained in `infra/storage.ts` and prove it with typecheck plus static validation.

Security requirements:

- Customer key must be stage specific.
- Key rotation should be enabled when supported by the selected AWS key resource.
- KMS mode must not change the logical auth record schema.
- KMS mode must not cause raw bearer values to appear in `pk`, `sk`, logs, outputs, or errors.
- The public/admin Lambdas should only receive KMS permissions needed for DynamoDB table access when customer-managed table encryption requires it.

### Least-Privilege DynamoDB Permissions

Tighten public and admin Lambda table permissions around typed-store access patterns.

Target DynamoDB actions for the public auth Lambda:

```text
dynamodb:GetItem
dynamodb:PutItem
dynamodb:UpdateItem
dynamodb:DeleteItem
dynamodb:Query
dynamodb:TransactWriteItems
```

Target DynamoDB actions for the admin lifecycle Lambda:

```text
dynamodb:GetItem
dynamodb:PutItem
dynamodb:UpdateItem
dynamodb:DeleteItem
dynamodb:Query
dynamodb:TransactWriteItems
```

Disallowed runtime actions unless a later slice proves a concrete need:

```text
dynamodb:Scan
dynamodb:*
iam:*
kms:*
secretsmanager:*
```

The implementation may keep using SST resource links only if static validation proves they do not grant disallowed table actions. If the link grants broader access than the target design, replace or supplement it with explicit permissions in the smallest local infra change that typechecks.

This slice must not modify Rust storage code merely to satisfy IAM if the runtime still uses a scan on a non-hot legacy path. Instead, document any remaining scan dependency and defer removal to the legacy-removal/security-regression slice.

### Public/Admin Environment Boundary

Keep the public and admin Lambda environment split from slice 12 and make it explicit in validation.

Public auth Lambda must keep:

```text
DYNAMODB_TABLE
ISSUER_URL
DEV_MODE=false
RUST_LOG
AUTH_CLIENT_CONFIG_PATH
AUTH_AUDIT_LOG_MODE
AUTH_* public/runtime config
PROVIDERS / PROVIDER_* provider config
RESEND_API_KEY when supplied through auth env or secret mechanism
```

Admin Lambda must keep:

```text
DYNAMODB_TABLE
RUST_LOG
AUTH_AUDIT_LOG_MODE
AUTH_DELETED_IDENTITY_REUSE
AUTH_DELETED_IDENTITY_RETENTION_DAYS
```

Admin Lambda must not receive by default:

```text
RESEND_API_KEY
PROVIDER_*
AUTH_GOOGLE_*
AUTH_APPLE_*
AUTH_SIGNING_PRIVATE_KEY
AUTH_EMAIL_*
```

If customer-managed table KMS requires both Lambdas to reference the table KMS key, that is allowed. Do not use that as a reason to pass provider/email/signing secrets to the admin Lambda.

### CloudWatch Access Log Retention

Make API Gateway access-log retention configurable while keeping CloudWatch as the default.

Required behavior:

```text
AUTH_LOG_RETENTION_DAYS unset -> 30 days
AUTH_LOG_RETENTION_DAYS=7 -> API access log retention equivalent to 7 days
AUTH_LOG_RETENTION_DAYS=0 -> reject for v1
```

Access logs must remain operational only. They must not include:

```text
request body
authorization header
cookies
auth codes
refresh tokens
verification or reset secrets
provider tokens
client secrets
private keys
```

If SST exposes API access-log retention as named strings rather than raw day counts, keep the numeric environment variable as the template input and map it to supported SST retention values in `infra/config.ts`.

### Audit Log Mode

Ensure both public and admin Lambdas receive:

```text
AUTH_AUDIT_LOG_MODE
```

Required behavior:

```text
unset -> cloudwatch
cloudwatch -> emit structured security audit events through Lambda logs
none -> disable security audit event emission only
```

`none` must not disable normal Lambda error logging or API Gateway access logs. If the Rust runtime already implements audit-log mode, this slice should only wire and validate infra defaults. If the Rust runtime is missing mode parsing, add the smallest focused runtime config/test needed to make the infra setting meaningful.

### Operator IAM Guidance And Outputs

Expose enough deployment output for template users to build an operator IAM policy without opening broad API access.

Preferred outputs:

```text
ApiUrl
ApiId or equivalent API identifier
AdminRouteArnPattern or per-route admin ARN patterns
TableName
TableKmsKeyArn optional when customer mode is enabled
```

Add a checked-in policy example or generated output note for operators:

```text
execute-api:Invoke on GET /_admin/users/*
execute-api:Invoke on POST /_admin/users/*/disable
execute-api:Invoke on POST /_admin/users/*/delete
execute-api:Invoke on POST /_admin/users/*/revoke-sessions
```

The policy example must not grant invoke access to public OAuth routes, provider callbacks, password routes, or `$default`.

### Infra Validation

Extend the existing infra validation script or add a sibling script.

Required static checks:

- `AUTH_TABLE_KMS` accepted values are exactly `aws-owned` and `customer`.
- Default table mode is AWS owned.
- Customer mode creates or configures a customer managed KMS key.
- TTL remains `expiry`.
- Public/admin Lambda permissions do not include `dynamodb:Scan`, `dynamodb:*`, `kms:*`, `iam:*`, or broad `secretsmanager:*`.
- Admin Lambda still omits provider/email/signing secrets.
- Public/admin Lambdas both receive `AUTH_AUDIT_LOG_MODE`.
- API access-log retention is config-driven.
- Admin route output or policy example is present and scoped to `/_admin/*`.

If static validation has to inspect source text rather than compiled SST objects, keep the script narrow and explicit. The goal is regression coverage for this template, not a general SST parser.

## Out Of Scope

- KMS ES256 access-token or ID-token signing.
- Secrets Manager migration.
- Rotating the HMAC lookup secret.
- WAF.
- Custom domains.
- Production CORS tightening.
- Dashboard, local admin UI, or reporting.
- Runtime OAuth client management.
- Public admin bootstrap.
- Generic OIDC provider support.
- Removing legacy UI/provider code.
- Removing all table scans from legacy compiled code.
- Live AWS deployment as a required local test.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree where practical.

Target modules and scripts:

```text
infra/config.ts
infra/storage.ts
infra/api.ts
sst.config.ts
scripts/validate-infra-routes.mjs
scripts/validate-infra-hardening.mjs
package.json
design/infra/operator-iam-policy.md
```

Possible Rust touch point if audit mode is not already wired:

```text
packages/functions/auth/src/config/audit.rs
packages/functions/auth/src/config/environment.rs
packages/functions/auth/tests/startup_config_slice.rs
```

Avoid broad Rust auth-flow edits. This slice is mostly infra and config validation.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing infra validation tests for KMS mode parsing, default log retention, audit mode defaults, and admin policy output presence.
2. Add `infra/config.ts` with deterministic parsers for table KMS mode, audit mode, and log retention.
3. Update `infra/storage.ts` to preserve the table shape and support `AUTH_TABLE_KMS=customer`.
4. Add static validation proving TTL remains `expiry` and customer mode creates or configures a customer managed key.
5. Update `infra/api.ts` to use config-driven access-log retention and pass `AUTH_AUDIT_LOG_MODE` to public/admin Lambdas.
6. Add or tighten static validation for public/admin environment separation.
7. Tighten public/admin Lambda DynamoDB permissions or prove current SST links already meet the allowed-action set.
8. Add static validation rejecting broad IAM/KMS/DynamoDB/Secrets Manager permissions in runtime Lambdas.
9. Add SST outputs or a checked-in operator IAM policy example for `/_admin/*` routes.
10. Add validation that operator IAM guidance is scoped only to admin route ARNs.
11. Add minimal Rust audit config tests only if the runtime does not already honor `AUTH_AUDIT_LOG_MODE`.
12. Run `npm run typecheck`, `npm run test:infra`, `npm run test:setup`, full auth Rust tests if Rust changed, and admin crate check if shared config changed.

## Tests

### Infra Config Tests

- Missing `AUTH_TABLE_KMS` resolves to `aws-owned`.
- `AUTH_TABLE_KMS=customer` resolves to customer mode.
- Unknown `AUTH_TABLE_KMS` fails validation.
- Missing `AUTH_AUDIT_LOG_MODE` resolves to `cloudwatch`.
- `AUTH_AUDIT_LOG_MODE=none` is accepted.
- Unknown `AUTH_AUDIT_LOG_MODE` fails validation.
- Missing `AUTH_LOG_RETENTION_DAYS` resolves to 30.
- Supported retention day values map to valid SST retention settings.
- Unsupported, zero, negative, or non-numeric retention values fail validation.

### Storage Infra Tests

- DynamoDB table still has `pk` and `sk` string fields.
- DynamoDB table still has primary index `pk` + `sk`.
- DynamoDB TTL is still `expiry`.
- AWS-owned mode does not create a customer managed KMS key.
- Customer mode creates or configures a customer managed KMS key.
- Customer key alias includes project and stage.
- Customer key rotation is enabled where supported.

### IAM Tests

- Public auth Lambda table permissions include only the allowed DynamoDB runtime actions.
- Admin Lambda table permissions include only the allowed DynamoDB lifecycle actions.
- No runtime Lambda permission contains `dynamodb:Scan`, `dynamodb:*`, `kms:*`, `iam:*`, or broad `secretsmanager:*`.
- Customer table KMS mode grants only the needed key usage, not `kms:*`.
- Admin operator policy examples grant only `execute-api:Invoke` on `/_admin/*`.

### Logging Tests

- API Gateway access-log retention uses the parsed retention config.
- Access-log configuration does not include request body, authorization header, cookies, or query strings containing protocol secrets.
- Public and admin Lambdas receive `AUTH_AUDIT_LOG_MODE`.
- `AUTH_AUDIT_LOG_MODE=none` is visible to runtime config without disabling ordinary error logging.

### Regression Tests

- Public `$default` still routes to the public auth Lambda without IAM.
- Admin lifecycle routes still route to the admin Lambda with IAM.
- Admin Lambda still omits provider/email/signing secrets.
- Public auth Lambda still receives required OAuth/provider/password runtime config.
- Existing auth, provider, refresh, and admin lifecycle tests continue to pass if Rust code changes.

## Acceptance Criteria

- `AUTH_TABLE_KMS=aws-owned` remains the low-friction default.
- `AUTH_TABLE_KMS=customer` configures a stage-specific customer managed table KMS key.
- DynamoDB table keys, primary index, and TTL attribute remain unchanged.
- API access-log retention is config-driven and defaults to 30 days.
- `AUTH_AUDIT_LOG_MODE` defaults to `cloudwatch` and can be set to `none`.
- Public and admin Lambda environments keep the existing secret boundary.
- Public/admin Lambda table permissions avoid broad DynamoDB, KMS, IAM, and Secrets Manager actions.
- Operator IAM guidance or outputs are scoped to `/_admin/*` only.
- Static infra validation covers KMS mode, logging mode, retention, IAM boundaries, and admin route policy guidance.
- KMS ES256 signing remains untouched for slice 16.

## Manual Validation

Local validation:

```text
npm run typecheck
npm run test:infra
npm run test:setup
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/admin/Cargo.toml
```

If no Rust code changes in this slice, the full auth/admin Rust commands can be treated as regression verification after the infra tests pass.

AWS dev validation after deployment:

```text
deploy with AUTH_TABLE_KMS=aws-owned -> table uses default DynamoDB encryption
deploy with AUTH_TABLE_KMS=customer -> table uses customer managed key with expected alias
confirm table TTL remains expiry
confirm API access log retention matches AUTH_LOG_RETENTION_DAYS
confirm public auth Lambda has no broad table/KMS/IAM permissions
confirm admin Lambda has no provider/email/signing secrets
confirm operator policy can invoke only /_admin/* routes
```

Do not require production deployment for local slice completion.

## Next Slice

After this slice, implement `16_kms_es256_signing`.

That slice should:

- add `AUTH_SIGNING_MODE=kms-es256`
- create or reference an asymmetric AWS KMS signing key
- sign access tokens and ID tokens through KMS
- expose JWKS from KMS public key material
- grant only `kms:Sign` and `kms:GetPublicKey`
- keep local ES256 available for developers who intentionally choose it
