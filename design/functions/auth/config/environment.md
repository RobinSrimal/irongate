# Environment Configuration

Implementation: `packages/functions/auth/src/config/environment.rs`

## Owns

- Runtime environment variable parsing.
- Required/optional setting validation.
- Typed config structs for auth modules.

## Setting Families

- Issuer URL.
- Enabled providers.
- Config-only OAuth client definitions or client config source.
- Password policy and email verification settings.
- Resend email delivery settings.
- Email branding, subjects, and template override paths.
- Google and Apple credentials.
- HMAC lookup secret reference.
- Signing mode and signing key references.
- Token and short-lived auth artifact TTLs.
- Deleted identity reuse and retention settings.
- Rate-limit settings.
- Audit logging mode.

## Security Invariants

- Startup fails in every stage when `RESEND_API_KEY` or `AUTH_EMAIL_FROM` is missing.
- `DEV_MODE` is explicit and stage-limited.
- Secrets are not printed in validation errors.

## Client Config

OAuth clients are loaded from a checked-in TOML file:

```text
AUTH_CLIENT_CONFIG_PATH optional, default auth.clients.toml
```

The TOML file stores non-secret client settings and secret reference names. Actual confidential-client secret values are supplied through SST secrets in deployed stages or local environment variables during local development.

## Password Policy Config

The runtime uses fixed password policy defaults:

```text
AUTH_PASSWORD_MIN_LENGTH optional, default 12
AUTH_PASSWORD_MAX_LENGTH optional, default 128
```

The built-in password policy is length-based and configured through minimum and maximum length.

## Required Email Config

The target core has one email config shape for dev and production:

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
AUTH_EMAIL_VERIFY_URL_BASE
AUTH_EMAIL_RESET_URL_BASE
AUTH_EMAIL_REPLY_TO optional
AUTH_EMAIL_BRAND_NAME optional
AUTH_EMAIL_SUPPORT_EMAIL optional
AUTH_EMAIL_VERIFY_SUBJECT optional
AUTH_EMAIL_RESET_SUBJECT optional
AUTH_EMAIL_VERIFY_TEMPLATE_PATH optional
AUTH_EMAIL_RESET_TEMPLATE_PATH optional
```

There is no `EMAIL_PROVIDER` setting in the target core.

`AUTH_EMAIL_VERIFY_URL_BASE` is an app-owned URL. The auth service appends the raw verification token as a `token` query parameter and sends that link by email. The auth Lambda remains API-only and does not render a verification page.

`AUTH_EMAIL_RESET_URL_BASE` is an app-owned URL. The auth service appends the raw password-reset token as a `token` query parameter and sends that link by email. The auth Lambda remains API-only and does not render a reset page.

Template path fields are parsed, but the current renderer does not load or validate their contents.
Only the built-in templates and supported branding/subject settings affect delivered messages.

## Google Provider Config

Google login is optional and first-class. It is enabled only when both values are present:

```text
AUTH_GOOGLE_CLIENT_ID optional pair with AUTH_GOOGLE_CLIENT_SECRET
AUTH_GOOGLE_CLIENT_SECRET optional pair with AUTH_GOOGLE_CLIENT_ID
```

In the SST template, `AUTH_GOOGLE_CLIENT_ID` is a non-secret stage-config value and
`AUTH_GOOGLE_CLIENT_SECRET` is resolved from the `GoogleClientSecret` SST secret.

Startup behavior:

- If both values are absent, Google is disabled.
- If exactly one value is present, startup fails.
- If both values are present, `/authorize?provider=google` and `/google/authorize` are enabled.

The Google authorization, token, issuer, JWKS, and scope values are fixed in code in this implementation. `AUTH_GOOGLE_CLIENT_SECRET` is a runtime secret and must not be logged, stored in DynamoDB, or committed to config files.

## Apple Provider Config

Apple login is optional and first-class. It is enabled only when the required values are present together:

```text
AUTH_APPLE_CLIENT_ID optional group
AUTH_APPLE_TEAM_ID optional group
AUTH_APPLE_KEY_ID optional group
AUTH_APPLE_PRIVATE_KEY_SECRET optional group
AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS optional, default 86400
```

`AUTH_APPLE_PRIVATE_KEY_SECRET` is a secret reference name. The referenced secret value contains the Apple ES256/P-256 PKCS#8 private key PEM used to generate Apple's client-secret JWT.

In the SST template, Apple non-secret identifiers live in checked-in stage config. The actual `.p8`
private key lives in the `ApplePrivateKey` SST secret and is exposed to the auth Lambda as
`AUTH_APPLE_PRIVATE_KEY` only when Apple is enabled for that stage.

Startup behavior:

- If every Apple value is absent, Apple is disabled.
- If only some required Apple values are present, startup fails.
- If the private-key secret reference cannot be resolved, startup fails.
- If the private key is not a valid ES256/P-256 PKCS#8 PEM, startup fails.
- If Apple is enabled, `/authorize?provider=apple` and `/apple/authorize` are enabled.

The Apple authorization, token, issuer, JWKS, audience, and scope values are fixed in code in this implementation. Apple client-secret JWTs are generated at runtime and must not be logged, stored in DynamoDB, or committed to config files.

## Optional TTL Config

TTL values are config-based with safe defaults:

```text
AUTH_ACCESS_TOKEN_TTL_SECONDS optional
AUTH_ID_TOKEN_TTL_SECONDS optional
AUTH_REFRESH_TOKEN_TTL_SECONDS optional
AUTH_AUTH_CODE_TTL_SECONDS optional
AUTH_AUTHORIZE_SESSION_TTL_SECONDS optional
AUTH_PROVIDER_STATE_TTL_SECONDS optional
AUTH_EMAIL_VERIFICATION_TTL_SECONDS optional
AUTH_PASSWORD_RESET_TTL_SECONDS optional
```

See `ttls.md` for defaults, validation rules, and security bounds.

## Account Lifecycle Config

Deleted identity reuse is config-based:

```text
AUTH_DELETED_IDENTITY_REUSE optional, default after_retention
AUTH_DELETED_IDENTITY_RETENTION_DAYS optional, default 30
```

Supported reuse modes are:

```text
after_retention
immediate
never
```

See `account-lifecycle.md` for behavior, validation rules, and security invariants.

## Signing Config

Signing mode is config-based:

```text
AUTH_SIGNING_MODE=local-es256 | kms-es256
AUTH_SIGNING_KEY_ID
AUTH_SIGNING_PRIVATE_KEY_SECRET required for local-es256
AUTH_SIGNING_KMS_KEY_ID required for kms-es256
```

Startup rejects unknown signing modes and missing/invalid required key material. Discovery and
JWKS use the configured signer; this is not a full protocol-conformance certification.

## Audit Logging Config

Audit logging is config-based:

```text
AUTH_AUDIT_LOG_MODE optional, default cloudwatch
```

Supported values:

```text
cloudwatch
none
```

Despite the mode name, `cloudwatch` currently enables DynamoDB audit-record writes. It does not
emit structured audit events to stdout/stderr. `none` skips those writes. Lambda operational logs
still go to CloudWatch independently; see [audit behavior](../observability/audit.md).
