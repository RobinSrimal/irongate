# 16_kms_es256_signing

## Goal

Make `AUTH_SIGNING_MODE=kms-es256` a real deployment option for access-token and ID-token signing, while keeping `local-es256` as the simple lower-latency default.

At the end of this slice, the public auth Lambda can sign JWTs with a non-exportable AWS KMS asymmetric P-256 key, publish matching JWKS public key material, verify its own access tokens for `/userinfo`, and run with only scoped `kms:Sign` and `kms:GetPublicKey` permissions when KMS signing is enabled.

This slice intentionally does not change OAuth/OIDC claim shapes, grant behavior, provider flows, password flows, refresh-token rules, or account lifecycle behavior.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/crypto/signing.md`
- `design/auth/core/tokens.md`
- `design/auth/api/oauth/discovery.md`
- `design/auth/api/oauth/token.md`
- `design/auth/api/oauth/userinfo.md`
- `design/auth/config/environment.md`
- `design/infra/secrets.md`
- `design/infra/iam.md`
- `design/infra/auth-function.md`
- `design/infra/api.md`
- `design/scope.md`

The important design constraint is key custody. In KMS mode, private signing key material must never be exported, stored in DynamoDB, logged, or passed to the Lambda as an environment variable. The Lambda may ask KMS to sign and may fetch the public key for JWKS/verification.

## Why This Slice Next

Slice 15 hardened the deployment boundary around DynamoDB KMS, Lambda permissions, and logging. The next remaining key-custody issue is token signing:

```text
local ES256 private key in Lambda env/secrets
vs
AWS KMS asymmetric key with non-exportable private material
```

Token signing is already centralized enough for this to be a focused slice. The current runtime config parses `kms-es256`, but startup still rejects it as unimplemented. This slice should close that gap without broad legacy cleanup.

## In Scope

### Runtime Signing Facade

Replace the concrete runtime signer field with a signer facade that supports both modes.

Current shape:

```text
RuntimeAuthConfig.signer: LocalEs256Signer
```

Target shape:

```text
RuntimeAuthConfig.signer: TokenSigner

TokenSigner::Local(LocalEs256Signer)
TokenSigner::Kms(KmsEs256Signer)
```

Required signer behavior:

```text
sign_access_token
sign_id_token
verify_access_token
jwks
kid
```

`sign_access_token` and `sign_id_token` should become async at the facade boundary because KMS signing is an AWS API call. Local ES256 may still sign synchronously inside its implementation, but call sites should use the same async facade for both modes.

Routes that issue or verify target tokens must use this facade:

```text
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/userinfo.rs
packages/functions/auth/src/oauth/well_known.rs
```

Do not add a second token path for KMS. Local and KMS modes should share the same OAuth token code.

### Shared JWT ES256 Serialization

Add a small shared JWT signing helper so local and KMS modes produce the same JOSE shape.

Required behavior:

1. Serialize JWT header with:

```json
{
  "alg": "ES256",
  "typ": "JWT",
  "kid": "<AUTH_SIGNING_KEY_ID>"
}
```

2. Serialize claims exactly once.
3. Base64url encode header and claims without padding.
4. Sign the ASCII signing input:

```text
base64url(header) + "." + base64url(claims)
```

5. Append the base64url-encoded JOSE ES256 signature.

KMS-specific rule:

```text
SHA-256(signing_input) -> KMS Sign with MessageType=Digest and SigningAlgorithm=ECDSA_SHA_256
```

AWS KMS returns ASN.1 DER ECDSA signatures. JWT ES256 requires a 64-byte raw JOSE signature:

```text
r || s
```

The KMS signer must convert DER signatures to fixed-width 32-byte `r` plus fixed-width 32-byte `s` before assembling the JWT.

### KMS ES256 Signer

Add a KMS signer implementation.

Suggested target file:

```text
packages/functions/auth/src/crypto/kms_signing.rs
```

Required runtime config:

```text
AUTH_SIGNING_MODE=kms-es256
AUTH_SIGNING_KEY_ID=<public JWT kid>
AUTH_SIGNING_KMS_KEY_ID=<kms key id, key arn, alias name, or alias arn>
```

Required KMS key constraints:

```text
KeySpec=ECC_NIST_P256
KeyUsage=SIGN_VERIFY
SigningAlgorithm=ECDSA_SHA_256
```

Startup/build behavior:

- Load AWS SDK config once per Lambda instance.
- Build one reusable KMS client per warm Lambda instance.
- Fetch the KMS public key before serving requests, or fail startup if it cannot be fetched.
- Reject KMS keys whose public key is not P-256 signing material.
- Build JWKS from KMS public key material only.

Signing behavior:

- Never call `kms:Verify` on the hot path.
- Sign access tokens and ID tokens with `kms:Sign`.
- Use local public-key verification for `/userinfo`.
- Do not log the raw JWT signing input, claims, signature, access token, or ID token.

The KMS signer should be testable without real AWS calls. Use a small internal trait or adapter for the KMS operations needed by the signer:

```text
get_public_key
sign_digest
```

Unit tests should use a fake implementation. Live AWS validation belongs in manual smoke testing, not normal `cargo test`.

### Runtime Config And Startup

Update runtime startup so `kms-es256` no longer returns:

```text
kms-es256 signing is configured but not implemented
```

The existing synchronous config parser can still validate environment values, but signer construction may need an async build step because KMS public-key fetch requires AWS I/O.

Acceptable implementation shapes:

```text
RuntimeAuthConfig::from_env().await
RuntimeAuthConfig::from_env_with_aws_config(...).await
RuntimeAuthConfig::from_env_map_with_signer_factory(...) for tests
```

The final code should keep tests ergonomic. Do not force every pure config test to contact AWS or construct a real KMS client.

Config validation requirements:

- Unknown signing mode still fails.
- `AUTH_SIGNING_KEY_ID` is required in both modes.
- `AUTH_SIGNING_PRIVATE_KEY_SECRET` is required only for `local-es256`.
- `AUTH_SIGNING_KMS_KEY_ID` is required only for `kms-es256`.
- Secrets and private keys must not appear in config error messages.

### Infra KMS Signing Key

Add optional SST infrastructure for a managed signing key when the deployed template uses KMS signing.

Required deployment switch:

```text
AUTH_SIGNING_MODE=local-es256 | kms-es256
```

Default:

```text
AUTH_SIGNING_MODE=local-es256
```

When `AUTH_SIGNING_MODE=kms-es256`, infra should create a stage-specific asymmetric signing key unless the implementation has an equally scoped and tested external-key path.

Managed key requirements:

```text
aws.kms.Key
keySpec: ECC_NIST_P256
keyUsage: SIGN_VERIFY
deletionWindowInDays: 30
alias: alias/<project-name>/auth-signing-<stage>
```

The public auth Lambda should receive:

```text
AUTH_SIGNING_MODE=kms-es256
AUTH_SIGNING_KMS_KEY_ID=<managed key id or alias arn/name>
```

The public auth Lambda role should receive only:

```text
kms:Sign
kms:GetPublicKey
```

on the signing key.

The admin Lambda should not receive signing KMS permissions by default. Admin lifecycle routes do not sign tokens.

When `AUTH_SIGNING_MODE=local-es256`, infra must not create the signing KMS key and must not grant signing KMS permissions.

### Discovery And JWKS

JWKS endpoint behavior must be mode-independent:

```text
GET /.well-known/jwks.json
```

Required behavior:

- Local mode returns public key material derived from the local ES256 private key.
- KMS mode returns public key material from `GetPublicKey`.
- JWKS contains `kty=EC`, `crv=P-256`, `alg=ES256`, `use=sig`, `kid=<AUTH_SIGNING_KEY_ID>`.
- JWKS never contains private key material.

Discovery should continue to advertise only:

```text
id_token_signing_alg_values_supported = ["ES256"]
```

Do not add RS256 or multiple-algorithm metadata in this slice.

### Token Endpoints And Userinfo

Update target token paths to await signer operations.

Required routes affected:

```text
POST /token grant_type=authorization_code
POST /token grant_type=refresh_token
GET /userinfo
GET /.well-known/jwks.json
```

Behavior must remain unchanged except for signing backend:

- Authorization-code exchange still consumes typed authorization codes.
- PKCE validation still happens before token issuance.
- Access-token TTL and ID-token TTL remain config-based.
- Refresh-token rotation still uses HMAC lookup digests and atomic store operations.
- `/userinfo` still rejects disabled or deleted accounts.

### Infra Validation

Add or extend static validation so the deployment shape stays narrow.

Suggested target:

```text
scripts/validate-kms-signing.mjs
```

or extend:

```text
scripts/validate-infra-hardening.mjs
```

Required checks:

- Infra config parses exact signing modes: `local-es256` and `kms-es256`.
- KMS signing key is `ECC_NIST_P256`.
- KMS signing key uses `SIGN_VERIFY`.
- Signing key alias is stage-specific.
- Public auth Lambda receives signing KMS permissions only in KMS mode.
- Signing KMS permissions are exactly `kms:Sign` and `kms:GetPublicKey`.
- Admin Lambda does not receive signing KMS permissions.
- No runtime role includes `kms:*`.

Add the validation command to `package.json` if a new script is created.

## Out Of Scope

- RS256 support.
- OpenID Provider certification work.
- Multiple active signing keys in JWKS.
- KMS key rotation workflow.
- Importing external signing key material.
- Generic external KMS key reference UX beyond the managed key path.
- Secrets Manager signing-key storage.
- Token introspection.
- Opaque access tokens.
- Changes to access-token, ID-token, or refresh-token claim shapes.
- Changes to password, Google, Apple, or admin account lifecycle flows.
- Removing legacy signing-key DynamoDB code. That belongs to the legacy-removal/security-regression slice unless a direct target-path dependency blocks this slice.

## Expected Code Shape

Current repo paths should be followed.

Likely Rust changes:

```text
packages/functions/auth/Cargo.toml
packages/functions/auth/src/main.rs
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/config/signing.rs
packages/functions/auth/src/crypto/signing.rs
packages/functions/auth/src/crypto/kms_signing.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/userinfo.rs
packages/functions/auth/src/oauth/well_known.rs
packages/functions/auth/tests/foundation_slice.rs
packages/functions/auth/tests/token_exchange_slice.rs
packages/functions/auth/tests/kms_signing_slice.rs
```

Likely infra changes:

```text
infra/config.ts
infra/api.ts
infra/signing.ts
sst.config.ts
scripts/validate-infra-hardening.mjs
package.json
```

If `infra/signing.ts` is added, it should own only token-signing KMS resources and permissions. Keep DynamoDB table KMS behavior in `infra/storage.ts`.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add pure tests for ES256 JWT serialization and DER-to-JOSE signature conversion.
2. Extract shared JWT serialization/signature assembly helper.
3. Wrap local ES256 signing in the new `TokenSigner` facade while keeping behavior unchanged.
4. Update `/token`, `/userinfo`, and JWKS call sites to use the facade.
5. Add the KMS signer type behind a fake KMS operation trait.
6. Test KMS public-key parsing, JWKS generation, digest signing input, and DER-to-JOSE conversion without AWS.
7. Wire async runtime signer construction into Lambda startup.
8. Add config tests proving `kms-es256` is accepted and missing KMS key IDs fail safely.
9. Add infra config for `AUTH_SIGNING_MODE`.
10. Add managed KMS signing key creation in infra for `kms-es256`.
11. Grant only `kms:Sign` and `kms:GetPublicKey` to the public auth Lambda in KMS mode.
12. Add or update static infra validation.
13. Run full Rust tests and infra/typecheck validation.

## Tests

### Pure Crypto Tests

- JWT header contains `alg=ES256`, `typ=JWT`, and configured `kid`.
- JWT signing input is `base64url(header) + "." + base64url(claims)`.
- KMS signing hashes the signing input with SHA-256 before requesting `Sign`.
- KMS signing requests `MessageType=Digest`.
- KMS signing requests `SigningAlgorithm=ECDSA_SHA_256`.
- DER P-256 ECDSA signature converts to a 64-byte JOSE signature.
- Invalid DER signatures are rejected.
- Public SPKI P-256 key material converts to a JWKS entry with expected `x` and `y`.
- Non-P-256 public key material is rejected.

### Runtime Config Tests

- `AUTH_SIGNING_MODE=local-es256` still requires `AUTH_SIGNING_PRIVATE_KEY_SECRET`.
- `AUTH_SIGNING_MODE=kms-es256` requires `AUTH_SIGNING_KMS_KEY_ID`.
- `AUTH_SIGNING_MODE=kms-es256` does not require `AUTH_SIGNING_PRIVATE_KEY_SECRET`.
- Unknown signing mode still fails.
- Config errors do not include raw secret values or private key PEM contents.

### Route/Token Tests

- Existing local ES256 token exchange tests still pass.
- Existing refresh rotation tests still pass after async signer call-site changes.
- JWKS endpoint returns the configured local signer key in local mode.
- JWKS endpoint returns the configured KMS public key in KMS mode with a fake KMS client.
- KMS-signed access tokens verify through the KMS signer's cached public key.
- `/userinfo` accepts a KMS-signed access token for an active account.
- `/userinfo` rejects a token signed with a different key ID.
- Token responses do not expose KMS key IDs beyond the JWT `kid` and JWKS public metadata.

### Infra Tests

- `npm run typecheck` passes.
- Infra validation passes in default local mode.
- Infra validation proves `kms:*` is not granted.
- Infra validation proves admin Lambda does not receive signing KMS permissions.
- KMS signing key resource is created only when `AUTH_SIGNING_MODE=kms-es256`.
- KMS signing key has `ECC_NIST_P256` and `SIGN_VERIFY`.

## Acceptance Criteria

- `AUTH_SIGNING_MODE=local-es256` remains working.
- `AUTH_SIGNING_MODE=kms-es256` starts successfully when a valid KMS key is configured.
- KMS mode signs access tokens and ID tokens through AWS KMS.
- KMS mode publishes JWKS from KMS public key material.
- KMS mode verifies access JWTs locally for `/userinfo`.
- KMS ECDSA DER signatures are converted to JOSE raw ES256 signatures.
- No target token route reads JWT private keys from DynamoDB.
- No private signing key material is stored in DynamoDB, logs, API responses, or Lambda environment variables in KMS mode.
- Public auth Lambda receives only `kms:Sign` and `kms:GetPublicKey` for the signing key in KMS mode.
- Admin Lambda does not receive signing KMS permissions.
- Discovery continues to advertise only implemented ES256 behavior.
- Existing OAuth/OIDC, refresh, password, Google, Apple, and admin lifecycle tests remain green.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:infra
npm run test:setup
```

AWS dev validation after implementation:

```text
AUTH_SIGNING_MODE=kms-es256 sst deploy --stage dev
```

Smoke-test:

1. Register and verify a password user, or reuse an existing verified dev user.
2. Run the authorization-code flow through `/authorize`, `/password/login`, and `/token`.
3. Decode the access token header and confirm `alg=ES256` and `kid=<AUTH_SIGNING_KEY_ID>`.
4. Fetch `/.well-known/jwks.json` and confirm the same `kid` is present.
5. Call `/userinfo` with the KMS-signed access token.
6. Confirm CloudTrail shows `kms:Sign` and `kms:GetPublicKey` for the signing key.
7. Confirm there are no raw JWTs, private keys, auth codes, refresh tokens, or signatures in Lambda logs.

## Next Slice

After this slice, implement `17_legacy_removal_and_security_regression`.

That slice should remove or quarantine target-incompatible legacy code paths, including built-in UI rendering, generic runtime storage access, old DynamoDB signing-key creation/scanning, memory storage as a runtime option, and any remaining public/admin behavior outside the target design. It should finish with security regression tests that prove the simplified auth core matches the design invariants.
