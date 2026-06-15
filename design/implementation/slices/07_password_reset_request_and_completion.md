# 07_password_reset_request_and_completion

## Goal

Add API-only password reset support for verified password accounts.

At the end of this slice, an application-owned password reset screen can request a reset email for an email address, then submit the emailed reset token with a new password. The auth Lambda consumes the reset token once, updates the Argon2id password hash, and does not issue OAuth authorization codes or tokens.

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/providers/password.md`
- `design/auth/providers/password.md`
- `design/auth/core/passwords.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/password-users.md`
- `design/auth/store/keys.md`
- `design/auth/email/delivery.md`
- `design/auth/email/templates.md`
- `design/auth/config/ttls.md`
- `design/auth/store/rate-limits.md`
- `design/scope.md`

The important design constraint is that password reset remains API-only. This slice must not add hosted reset pages, hosted login UI, passwordless OTP, short numeric reset codes, or tokens after reset.

## Why This Slice Next

Slices 03-06 now cover:

```text
register -> verify email -> password login -> code exchange -> refresh/logout
```

The next smallest missing first-party password capability is:

```text
forgot password -> reset email -> consume reset token -> update password hash
```

This is narrower than Google/Apple login or IAM admin lifecycle. It also removes the remaining legacy reset behavior that stores raw reset codes under `password:reset`.

## In Scope

### Typed Password Reset Secret Store

Extend the existing typed password secret store.

Target code:

```text
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/keys.rs
```

Required store operations:

```text
create_password_reset
consume_password_reset
```

Record shape:

```json
{
  "email_digest": "...",
  "subject": "user_...",
  "purpose": "reset_password",
  "created_at": "...",
  "expires_at": "..."
}
```

Rules:

- Raw reset tokens are generated with high entropy.
- DynamoDB keys use HMAC lookup digests, never raw reset tokens.
- Records carry `expires_at`.
- DynamoDB TTL uses the same expiry value.
- Consuming a reset token is single-use.
- Expired reset tokens are rejected even if DynamoDB TTL has not deleted them.
- Store operations must not use short numeric codes.

### Password Reset Email Template

Add reset email rendering to the existing email template module.

Target code:

```text
packages/functions/auth/src/email/templates.rs
packages/functions/auth/src/email/mod.rs
```

Required behavior:

- Use `AUTH_EMAIL_RESET_SUBJECT` through the existing email config.
- Generate reset URLs from configured deployment state, not request input.
- Append the raw reset token as a `token` query parameter.
- Escape user-controlled display values.
- Do not log rendered templates or reset tokens.

If reset URL base configuration is not already present, add the smallest config required:

```text
AUTH_EMAIL_RESET_URL_BASE
```

The reset URL base should be app-owned, like the verification URL base. The auth Lambda sends the reset link but does not render the reset page.

### Password Reset Domain Operations

Add password reset domain operations under the target provider module.

Target code:

```text
packages/functions/auth/src/providers/password.rs
```

Required operations:

```text
request_password_reset
complete_password_reset
```

`request_password_reset`:

1. Normalize the submitted email.
2. Compute the HMAC email digest.
3. Load the password user record.
4. If no user exists, return the same public outcome as success.
5. If the user exists but is unverified, return the same public outcome as success.
6. If the user exists and is verified, require a subject.
7. Require the subject account to be active.
8. Create a reset secret through the typed store.
9. Send the reset email through the configured email sender.
10. Return a generic `reset_email_sent` status.

The public response must not reveal whether the email exists, whether the account is verified, or whether the account is active.

`complete_password_reset`:

1. Validate the new password with the target password policy.
2. Compute the HMAC reset-token digest.
3. Consume the reset secret through the typed store.
4. Load the password user by stored email digest.
5. Require the stored subject to match the reset record subject.
6. Require the subject account to still be active.
7. Hash the new password with Argon2id.
8. Update the password hash and `password_hash_updated_at`.
9. Return `password_reset`.

Completing a reset must not issue an authorization code, access token, refresh token, or ID token. The user signs in normally after reset.

### Password User Store Update

Add a purpose-specific password-user update operation.

Target code:

```text
packages/functions/auth/src/store/password_users.rs
```

Required operation:

```text
update_password_hash
```

Rules:

- Accept the email digest, expected subject, and new Argon2id password hash.
- Load the existing password user.
- Require `verified=true`.
- Require the stored subject to match the reset secret subject.
- Update `password_hash`, `password_hash_updated_at`, and `updated_at`.
- Use a conditional write so concurrent updates do not overwrite unrelated password-user changes.

### Password Reset API Routes

Add API-only password reset endpoints.

Target code:

```text
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/routes.rs
```

Routes:

```text
POST /password/forgot
POST /password/reset
```

`POST /password/forgot` request:

```json
{
  "email": "user@example.com"
}
```

Success response:

```json
{
  "status": "reset_email_sent"
}
```

`POST /password/reset` request:

```json
{
  "token": "raw-reset-token-from-email",
  "new_password": "correct horse battery staple"
}
```

Success response:

```json
{
  "status": "password_reset"
}
```

Error behavior:

- Unknown email on forgot returns the generic success response.
- Invalid or expired reset token returns `invalid_grant`.
- Invalid new password returns `invalid_request`.
- Inactive or deleted account returns `invalid_grant`.
- Responses must not include tokens.

### Password Reset Rate Limits

Add rate limits for reset request and reset completion.

Target code:

```text
packages/functions/auth/src/config.rs
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/store/rate_limits.rs
```

Recommended endpoint names:

```text
PasswordResetRequest
PasswordResetComplete
```

Required behavior:

- Apply rate limits to `POST /password/forgot`.
- Apply rate limits to `POST /password/reset`.
- Include a normalized email HMAC digest in the reset-request rate-limit identifier when an email is present.
- For reset completion, use source identity only unless a safe token digest helper already exists.
- Do not store raw email addresses, passwords, or reset tokens in rate-limit keys.
- Return `429 Too Many Requests` with the existing rate-limit response shape when the limit is exceeded.

## Out Of Scope

- Automatic login after password reset.
- Revoking existing refresh-token families after password reset.
- Email-change flows.
- Account recovery without email.
- Hosted reset pages.
- Short numeric reset codes.
- Passwordless OTP or magic links.
- Google or Apple login.
- IAM-protected account lifecycle routes.
- Removing the full legacy password provider module.

## Expected Code Shape

Current repo paths should be followed and kept aligned with the design tree.

Target modules:

```text
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/providers/password.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/email/templates.rs
packages/functions/auth/tests/password_registration_slice.rs
packages/functions/auth/tests/password_reset_slice.rs
packages/functions/auth/tests/runtime_route_slice.rs
```

Legacy source files may remain compiled if they are not used by the target reset routes. This slice must not depend on `ui/password.rs` or the legacy `provider/password.rs` reset-code flow.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add failing store tests for HMAC-keyed reset secret creation and single-use consumption.
2. Add `PasswordResetRecord` to typed store records.
3. Implement `create_password_reset` and `consume_password_reset`.
4. Add failing email template tests for reset URL rendering and HTML escaping.
5. Implement reset email rendering and required reset URL config if missing.
6. Add failing password-user store tests for `update_password_hash`.
7. Implement `update_password_hash`.
8. Add domain tests for reset request with known email, unknown email, unverified email, inactive account, invalid token, expired token, weak password, and successful reset.
9. Implement `request_password_reset` and `complete_password_reset`.
10. Add route tests for `POST /password/forgot` and `POST /password/reset`.
11. Add rate-limit tests proving reset routes are rate-limited without raw email, password, or token keys.
12. Mount the routes.
13. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.
14. Commit the completed slice.

## Tests

### Store Tests

- `create_password_reset` stores by HMAC digest, not raw reset token.
- Reset secret record stores `email_digest`, `subject`, `purpose`, `created_at`, and `expires_at`.
- `consume_password_reset` consumes once.
- Expired reset secret is rejected.
- Raw reset token does not appear in `pk` or `sk`.

### Email Template Tests

- Reset email appends `token` to the configured reset URL base.
- Reset email escapes user-controlled email display values.
- Reset email includes the configured reset subject.
- Reset email body does not include API keys or unrelated secrets.

### Domain Tests

- Forgot-password for unknown email returns `reset_email_sent` and does not create a reset secret.
- Forgot-password for unverified email returns `reset_email_sent` and does not create a reset secret.
- Forgot-password for verified active account creates a reset secret and sends one email.
- Forgot-password for deleted account returns `reset_email_sent` and does not create a reset secret.
- Reset with invalid token fails with a safe error.
- Reset with expired token fails.
- Reset with weak password fails before updating storage.
- Reset with valid token updates the Argon2id password hash.
- Reset token reuse fails.
- Successful reset does not issue OAuth codes or tokens.
- User can log in with the new password after reset.
- User cannot log in with the old password after reset.

### Route Tests

- `POST /password/forgot` returns `200 OK` and `reset_email_sent`.
- `POST /password/reset` returns `200 OK` and `password_reset`.
- Reset route responses contain no `code`, `access_token`, `refresh_token`, or `id_token`.
- `POST /password/forgot` is rate-limited.
- `POST /password/reset` is rate-limited.
- Password reset rate-limit keys do not contain raw email addresses, passwords, or reset tokens.

## Acceptance Criteria

- Password reset uses high-entropy link tokens only.
- Raw reset tokens are never stored in DynamoDB keys.
- Reset secrets are short-lived and single-use.
- Forgot-password responses do not reveal account existence or verification state.
- Only verified active password accounts can receive reset emails.
- Reset completion updates the Argon2id password hash through a typed store operation.
- Reset completion does not authenticate the user or issue OAuth tokens.
- Reset routes are rate-limited without storing raw reset-flow secrets in rate-limit keys.
- Existing registration, verification, login, token exchange, refresh, and logout tests continue to pass.
- The auth Lambda remains API-only and renders no reset UI.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual protocol smoke test after implementation:

```text
POST /password/forgot
POST /password/reset
GET /authorize
POST /password/login
POST /token
```

Expected result:

- Forgot-password returns the same public success shape for known and unknown emails.
- The emailed reset link contains a one-time token.
- Reset consumes the token and updates the password.
- Login succeeds with the new password and fails with the old password.
- DynamoDB contains no raw reset token in `pk` or `sk`.

AWS validation is not required for this slice beyond normal deploy smoke testing, because the behavior is auth-core and Resend-email integration was already introduced earlier.

## Next Slice

After this slice, implement `08_google_oidc_login`.

That slice should add Google OIDC only, not Google and Apple together. Apple should be a separate follow-up slice because Apple has different client-secret and key-material requirements.
