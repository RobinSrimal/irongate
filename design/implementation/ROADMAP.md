# Implementation Roadmap

This roadmap turns the design tree into implementation slices. Each slice should leave the repo in a coherent state with focused tests, not just move code around.

## Principles

- Keep the auth core API-only.
- Keep app and UI decisions deferred.
- Prefer vertical, testable slices over broad rewrites.
- Keep one API Gateway HTTP API, a public Rust auth Lambda, a separate IAM-protected Rust admin Lambda for account lifecycle, and DynamoDB as the default runtime shape.
- Replace generic behavior with typed modules before adding new auth flows on top.
- Do not introduce runtime client management, public bootstrap, passwordless OTP, token introspection, opaque access tokens, or generic OAuth identity providers in v1.

## Slice Sequence

### 01_foundation_primitives_and_discovery

Build the first foundation layer that later flows can use:

- Typed runtime config primitives.
- Static `auth.clients.toml` parser and validator.
- Typed store facade, key helpers, and account/identity records.
- HMAC lookup helpers.
- Generated opaque subjects.
- Signing abstraction with local ES256 support.
- OIDC/OAuth discovery and JWKS endpoints.

This slice creates tested building blocks and public discovery behavior without cutting over the legacy authorize/token routes yet.

### 02_startup_config_and_control_plane_cutover

Wire the foundation into the running Lambda and remove the old runtime control plane:

- Load and validate `auth.clients.toml` at startup.
- Resolve confidential-client secret refs from the configured secret source.
- Add the config-only client registry to application state.
- Use config clients in authorize/token client validation.
- Remove or disable public `/admin/bootstrap`.
- Remove runtime OAuth client create/update/delete routes from the target router.
- Ensure metadata still only advertises implemented flows.

This slice should leave the deployed auth Lambda using config-only clients and no first-deployer-wins bootstrap path.

### 03_password_registration_and_email_verification

Implement the first password account workflow without issuing OAuth codes yet:

- Register with email and password.
- Argon2id password hashing.
- Resend verification email.
- Verification link token storage and consumption.
- Verified password identity creation.
- No login, auth-code issuance, or token issuance.

This slice should leave the system able to create a verified password-backed account, but still unable to authenticate until the next slice.

### 04_password_login_and_authorization_code

Implement password login on top of verified password accounts:

- Login with normalized email and password.
- Reject unverified, disabled, or deleted accounts.
- Consume the existing authorize session.
- Issue an OAuth authorization code after successful login.
- No token exchange changes yet.

### 05_token_exchange_signing_and_userinfo

Complete the first target authorization-code token exchange loop:

- Cut signing/JWKS over to the configured runtime signer before issuing target tokens.
- Authorization-code exchange.
- Access-token and ID-token issuance.
- Runtime-signed `/userinfo`.
- Discovery metadata that advertises only authorization-code behavior.
- No refresh-token issuance or logout yet.

### 06_refresh_rotation_and_logout

Add long-lived session support after the code exchange path is stable:

- Refresh-token issuance when offline access is allowed.
- Refresh-token storage by HMAC lookup digest.
- Refresh-token rotation and reuse detection.
- `/oauth/revoke` for user-facing logout.
- Discovery metadata for refresh and revocation only once mounted.

### 07_password_reset_request_and_completion

Complete the first-party password account workflow:

- Forgot-password request endpoint.
- Password reset email using configurable templates.
- HMAC-keyed reset token storage.
- Single-use reset token consumption.
- Argon2id password hash update.
- Reset route rate limits.
- No automatic login or token issuance after reset.

### 08_google_oidc_start_and_provider_state

Start the first external identity provider flow:

- Google runtime configuration.
- Typed provider-state storage.
- `/authorize provider=google` handoff.
- `/google/authorize` redirect to Google with state, nonce, and PKCE.
- No Google callback or identity mapping yet.

### 09_google_oidc_callback_and_identity

Complete Google OIDC login:

- Google callback route.
- Google code exchange.
- Google ID-token validation.
- Issuer + subject identity mapping.
- Internal authorization-code issuance.
- No auto-linking by email.

### 10_apple_oidc_start_and_client_secret

Start Apple after Google is working:

- Apple runtime configuration.
- Apple client-secret JWT generation.
- `/authorize provider=apple` handoff.
- `/apple/authorize` redirect to Apple with state, nonce, and PKCE.
- Provider state and nonce handling.
- No Apple callback or identity mapping yet.

### 11_apple_oidc_callback_and_identity

Complete Apple OIDC login:

- Apple callback route.
- Apple code exchange using generated client-secret JWT.
- Apple ID-token validation.
- Issuer + subject identity mapping.
- Internal authorization-code issuance.
- No auto-linking by email.

### 12_iam_admin_disable_and_revoke

Add the first IAM-protected operator lifecycle routes:

- IAM-protected `/_admin/*` routes.
- Sanitized account read.
- Disable user.
- Revoke all sessions for a subject.
- Lambda-side guard for expected API Gateway IAM request context.

### 13_iam_admin_delete_tombstones

Add irreversible deletion behavior:

- Mark account deleted.
- Strip password hash and contact metadata.
- Mark identities deleted with fixed anonymized tombstones.
- Apply deleted identity reuse policy.
- Revoke all sessions for a subject.

### 14_api_gateway_source_identity_and_route_validation

Harden the first AWS deployment boundary:

- API Gateway request-context source IP for rate limits.
- Spoofed forwarded-header regression tests.
- Static validation that admin routes use IAM and the admin Lambda.
- Static validation that public auth routes remain public.
- AWS dev smoke checklist for source IP and admin IAM behavior.

### 15_storage_kms_iam_and_logging_hardening

Tighten AWS resource configuration:

- Optional customer managed DynamoDB KMS key.
- Least-privilege DynamoDB permissions for public/admin Lambdas.
- CloudWatch audit logging defaults and retention configurability.
- Operator IAM policy examples or route ARN outputs.

### 16_kms_es256_signing

Add optional non-exportable AWS KMS token signing:

- `AUTH_SIGNING_MODE=kms-es256`.
- KMS asymmetric signing for access and ID tokens.
- JWKS/public-key behavior from KMS public key material.
- Scoped `kms:Sign` and `kms:GetPublicKey` permissions.

### 17_legacy_provider_ui_route_removal

Remove the legacy provider/UI route surface:

- Remove dynamic `/:provider/*` auth routes.
- Remove legacy `src/provider` modules and `ProviderConfig`.
- Remove built-in HTML auth UI modules from the public auth Lambda.
- Stop forwarding generic `PROVIDERS` / `PROVIDER_*` deployment env vars.
- Keep only API-only password, Google, Apple, OAuth, OIDC, and admin lifecycle routes.

### 18_legacy_storage_and_security_regression

Finish the remaining rewrite cleanup:

- Remove unmounted legacy custom-admin and runtime client-management code.
- Remove the old DynamoDB signing-key helper path.
- Remove legacy raw-refresh-token rotation/revocation helpers.
- Add static regression checks for deleted legacy paths.
- Confirm target key-shape tests cover raw bearer-value exclusion.

This slice does not remove test-only storage helpers or redesign the storage adapter. That can happen later if the remaining abstraction gets in the way of the DynamoDB-only target.

### 19_test_consolidation_and_cleanup_baseline

Turn implementation-slice test files into a maintainable Rust test layout:

- Move pure module tests beside the source modules where practical.
- Rename integration tests by auth domain instead of implementation slice.
- Keep router/protocol tests under `packages/functions/auth/tests/`.
- Add test layout documentation.
- Add static validation that prevents new `*_slice.rs` integration test files.

This slice should not change auth behavior or reduce security regression coverage.

### 20_store_boundary_and_in_memory_test_backend

Collapse raw storage exposure behind the typed auth store boundary:

- Make public route/provider code depend on a non-generic `AuthStore`.
- Keep DynamoDB as the only production backend.
- Keep a simple in-memory backend for tests.
- Remove or hide raw `get/set/remove/scan/transact` access from public auth handlers.
- Remove generic `S: StorageAdapter` exposure from the public auth route/API boundary.
- Add static validation for the store boundary.

This slice should preserve fast tests while removing backend pluggability from the runtime design.

## Definition Of Done For Each Slice

Every slice should include:

- Code changes scoped to the slice.
- Unit tests for pure Rust modules.
- Store tests for DynamoDB key and expiry behavior where relevant.
- HTTP handler tests for public API behavior where relevant.
- Config validation tests for new settings.
- Updated docs when the implemented behavior changes the design.
- A short manual validation note if AWS behavior is involved.

## Deferred Decisions

- App/UI/reference website shape.
- Payments.
- Generic OIDC provider registry beyond Google and Apple.
- Machine-to-machine `client_credentials`.
- Token introspection or opaque access tokens.
- Hosted UI.
