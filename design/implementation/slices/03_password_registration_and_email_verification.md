# 03_password_registration_and_email_verification

## Goal

Implement first-party password registration and email verification without issuing OAuth authorization codes or tokens.

At the end of this slice, a caller can register with email and password, receive a Resend verification email, and verify the email through a single-use verification token. Verification creates or attaches the generated account subject and password identity, but it does not authenticate the user.

## Why This Slice Next

The previous slices established config-only clients, runtime auth config, typed store primitives, and removed the old runtime control plane. The next security boundary is the registration-verification split.

This slice directly addresses the earlier security finding where registration could authenticate an unverified email. By ending the slice before login and auth-code issuance, we can test and review the invariant clearly:

```text
registration + verification email != authenticated session
```

## Design Docs Followed

This slice should follow these design documents:

- `design/auth/api/providers/password.md`
- `design/auth/providers/password.md`
- `design/auth/core/passwords.md`
- `design/auth/core/subjects.md`
- `design/auth/core/identities.md`
- `design/auth/crypto/passwords.md`
- `design/auth/crypto/hmac-lookups.md`
- `design/auth/email/README.md`
- `design/auth/email/delivery.md`
- `design/auth/email/resend.md`
- `design/auth/email/templates.md`
- `design/auth/store/password-users.md`
- `design/auth/store/password-secrets.md`
- `design/auth/store/accounts.md`
- `design/auth/store/identities.md`
- `design/auth/store/keys.md`
- `design/auth/store/records.md`
- `design/auth/config/environment.md`
- `design/auth/config/ttls.md`
- `design/scope.md`

## In Scope

### Password Policy

Implement the v1 password policy:

- minimum length 12
- maximum length 128
- no composition rules
- no breached-password API integration
- Argon2id PHC string storage

Password policy errors should be deterministic and should not log the supplied password.

### Email Normalization

Normalize email before lookup and storage:

- trim leading/trailing whitespace
- lowercase the domain
- lowercase the local part for v1 simplicity
- reject malformed email strings

The normalized email may be stored in the password user record while the account is active. DynamoDB keys must use HMAC lookup digests, not raw email addresses.

### Runtime Email Config

Extend startup config with the required registration email settings:

```text
RESEND_API_KEY
AUTH_EMAIL_FROM
AUTH_EMAIL_VERIFY_URL_BASE
AUTH_EMAIL_REPLY_TO optional
AUTH_EMAIL_BRAND_NAME optional
AUTH_EMAIL_SUPPORT_EMAIL optional
AUTH_EMAIL_VERIFY_SUBJECT optional
AUTH_EMAIL_VERIFY_TEMPLATE_PATH optional
```

`AUTH_EMAIL_VERIFY_URL_BASE` is an app-owned URL such as:

```text
https://app.example.com/auth/verify-email
```

The auth service appends the raw verification token as a query parameter:

```text
?token=<url-encoded-token>
```

The auth Lambda remains API-only. It does not render a verification page.

### Email Templates

Add deterministic built-in verification email templates:

- default subject
- HTML body
- plain text body if the Resend request supports it cleanly

Supported variables:

```text
brand_name
email
verification_url
expires_minutes
support_email
```

Template overrides may be supported if they are already straightforward. If override validation would make the slice too large, keep overrides documented but postpone implementation to a focused follow-up.

### Resend Delivery

Add a small email delivery module with:

```text
send_verification_email(to, rendered_message) -> delivery_id
```

Rules:

- Resend is the only runtime delivery provider.
- Tests use a fake email sender; no live network calls in tests.
- Delivery errors do not mark users verified.
- Logs and errors do not include full verification URLs or tokens.

### Password User Store

Add typed password user store operations:

```text
create_unverified_password_user
get_password_user_by_email_digest
mark_password_user_verified
```

Record shape:

```json
{
  "email": "normalized@example.com",
  "subject": "optional user_...",
  "password_hash": "argon2id PHC string",
  "password_hash_updated_at": "...",
  "verified": false,
  "created_at": "...",
  "updated_at": "..."
}
```

Rules:

- Key by HMAC email digest.
- Store password hash only as Argon2id PHC string.
- Do not create an authenticated session or OAuth code.
- Existing unverified registration may create a fresh verification token and resend the email.
- Existing verified email should return a safe conflict-style error without revealing password hash details.

### Verification Secret Store

Add typed verification secret operations:

```text
create_email_verification
consume_email_verification
```

Verification token behavior:

- high entropy random value
- HMAC lookup digest in `pk`/`sk`
- raw token sent by email once
- short TTL from `AUTH_EMAIL_VERIFICATION_TTL_SECONDS`
- `expires_at` stored inside the record
- DynamoDB TTL stored in the item `expiry` attribute
- single-use consume
- expired records rejected before DynamoDB TTL deletion

Record shape:

```json
{
  "email_digest": "...",
  "purpose": "verify_email",
  "created_at": "...",
  "expires_at": "..."
}
```

### Verification Flow

Consuming a valid verification token should:

1. Load and delete the verification record atomically where the storage API allows.
2. Load the password user by email digest.
3. If the user is unverified, create a generated account subject and password identity.
4. Mark the password user verified and store the subject.
5. Return a safe success response without OAuth code or tokens.

If the user is already verified, the endpoint should be idempotent only when safe. It must not create a second subject for the same active password identity.

### API Endpoints

Add API-only endpoints:

```text
POST /password/register
POST /password/verify
```

Registration request:

```json
{
  "email": "user@example.com",
  "password": "correct horse battery staple"
}
```

Registration response:

```json
{
  "status": "verification_required"
}
```

Verification request:

```json
{
  "token": "raw-verification-token"
}
```

Verification response:

```json
{
  "status": "verified"
}
```

Neither response includes:

- OAuth authorization code
- access token
- refresh token
- ID token

## Out Of Scope

- Password login.
- Authorization-code issuance after password login.
- Token exchange changes.
- Refresh-token rotation changes.
- `/userinfo`.
- Password reset.
- Google or Apple login.
- Hosted UI or auth-owned verification page.
- Resend domain/sender verification automation.
- Alternate email providers.

## Expected Code Shape

Target modules:

```text
packages/functions/auth/src/config/environment.rs
packages/functions/auth/src/core/passwords.rs
packages/functions/auth/src/email/mod.rs
packages/functions/auth/src/email/templates.rs
packages/functions/auth/src/email/resend.rs
packages/functions/auth/src/providers/password.rs
packages/functions/auth/src/store/password_users.rs
packages/functions/auth/src/store/password_secrets.rs
packages/functions/auth/src/routes.rs
```

The existing legacy password provider code may remain compiled if it is no longer used by target routes. Avoid broad deletion in this slice unless it is required to keep route behavior unambiguous.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Add password policy and email normalization with unit tests.
2. Add email config, template rendering, and fake-delivery tests.
3. Add Resend request builder with tests that assert no secret/token logging.
4. Add password user and verification secret typed store operations.
5. Add registration domain operation that creates an unverified user, creates a verification secret, and calls email delivery.
6. Add verification domain operation that consumes the token, creates account + password identity, and marks the user verified.
7. Add `POST /password/register` and `POST /password/verify` routes.
8. Add route tests proving registration and verification never return OAuth codes or tokens.
9. Run full Rust tests, `cargo check`, `npm run typecheck`, and setup-script tests.

## Tests

### Password Tests

- Password shorter than 12 characters fails.
- Password longer than 128 characters fails.
- Valid password hashes as Argon2id PHC.
- Password plaintext is not stored in records.
- Email normalization is deterministic.
- Malformed email fails.

### Email Config And Template Tests

- Missing `RESEND_API_KEY` fails startup config validation.
- Missing `AUTH_EMAIL_FROM` fails startup config validation.
- Missing `AUTH_EMAIL_VERIFY_URL_BASE` fails startup config validation.
- Default verification template renders expected variables.
- Unknown template variables fail validation if override templates are implemented.
- Rendered output escapes user-controlled email display values.

### Store Tests

- Password user key uses HMAC email digest, not raw email.
- Verification token key uses HMAC token digest, not raw token.
- Verification record stores `expires_at` and DynamoDB TTL.
- Expired verification token is rejected.
- Verification token is single-use.
- Verifying creates a generated subject and password identity.
- Re-verifying cannot create a second active subject for the same password identity.

### Route Tests

- `POST /password/register` returns `verification_required`.
- Registration stores an unverified password user.
- Registration sends one verification email through fake delivery.
- Registration response contains no `code`, `access_token`, `refresh_token`, `id_token`, or `sub`.
- `POST /password/verify` with a valid token returns `verified`.
- Verification response contains no OAuth code or tokens.
- Invalid or expired verification token returns a safe error.
- `/authorize` and `/token` behavior from slice 02 remains intact.

## Acceptance Criteria

- Registration creates an unverified password user and sends a verification email.
- Verification consumes a single-use HMAC-stored token and marks the user verified.
- Verified password users have a generated persisted subject and password identity.
- Registration never authenticates the user.
- Verification never issues OAuth authorization codes or tokens.
- Raw passwords, raw verification tokens, and raw email addresses do not appear in DynamoDB keys.
- Resend is required for runtime email delivery.
- The auth Lambda remains API-only.

## Manual Validation

Local validation:

```text
cargo test --manifest-path packages/functions/auth/Cargo.toml
cargo check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:setup
```

Manual route smoke test with a fake or test delivery mode should be avoided unless the implementation exposes an explicit test hook. Real Resend delivery should be validated only in an AWS/dev environment with test-domain credentials.

AWS validation after deploy:

```text
curl -X POST <api-url>/password/register \
  -H 'content-type: application/json' \
  -d '{"email":"dev@example.com","password":"a valid long password"}'
```

Expected AWS result:

- API returns `verification_required`
- Resend receives a verification email request
- DynamoDB contains no raw verification token in `pk` or `sk`

## Next Slice

After this slice, implement `04_password_login_and_authorization_code`.

That slice should:

- consume an existing authorize session
- verify normalized email and password
- require verified email
- require active account
- issue an OAuth authorization code
- still avoid token issuance until the token-exchange slice
