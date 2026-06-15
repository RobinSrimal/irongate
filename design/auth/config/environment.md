# Environment Configuration

Target code: `packages/functions/auth/src/config/environment.rs`

## Owns

- Runtime environment variable parsing.
- Required/optional setting validation.
- Typed config structs for auth modules.

## Target Setting Families

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

V1 uses fixed password policy defaults:

```text
AUTH_PASSWORD_MIN_LENGTH optional, default 12
AUTH_PASSWORD_MAX_LENGTH optional, default 128
```

Composition rules and breached-password checks are not configurable in v1 because they are out of scope.

## Required Email Config

The target core has one email config shape for dev and production:

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
AUTH_EMAIL_VERIFY_URL_BASE
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

Template paths are deployment-time settings only. Startup should fail if a configured path cannot be loaded, contains unsupported variables, or points outside the packaged/allowed template location.

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

Startup should fail if the signing mode is unknown, the selected mode is missing required key material, or discovery metadata cannot be made consistent with the selected signer.

## Audit Logging Config

Audit logging is config-based:

```text
AUTH_AUDIT_LOG_MODE optional, default cloudwatch
```

Supported v1 values:

```text
cloudwatch
none
```

`cloudwatch` emits compact structured JSON audit events to stdout/stderr so Lambda sends them to CloudWatch Logs. `none` explicitly disables security audit event emission.
