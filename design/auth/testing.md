# Auth Testing

Target code: auth crate tests plus integration tests around the Rust Lambda where useful.

## Owns

- Security regression test plan.
- Store behavior tests.
- Provider-flow tests.
- Config validation tests.

## Required Regression Tests

Security scan coverage:

- Registration with verification required does not issue an OAuth code.
- Login for an unverified password user fails.
- Public admin bootstrap route does not exist in the target core.
- Admin lifecycle routes reject unsigned requests at API Gateway/IAM.
- Admin lifecycle routes do not accept custom admin API keys.
- Rate-limit identity ignores spoofed `x-forwarded-for` and `x-real-ip`.
- Verification/reset link-token consume paths enforce expiry.

Password policy and hashing:

- Registration rejects passwords shorter than 12 characters.
- Registration rejects passwords longer than 128 characters.
- Password policy does not require composition rules.
- Password hashes are Argon2id PHC strings.
- Password hashes use unique salts.
- Successful login can trigger rehash when stored Argon2id parameters are stale.
- Breached-password checks are not called in v1.

Storage security:

- Authorization code key uses HMAC lookup digest, not raw code.
- Refresh token key uses HMAC lookup digest, not raw token.
- Verification/reset keys use HMAC lookup digest, not raw link token.
- Expired records are rejected before DynamoDB TTL deletion.
- Refresh rotation is atomic and detects reuse.
- `/oauth/revoke` revokes a refresh token family/session without scanning the table.
- `/oauth/revoke` returns success for already revoked or missing refresh tokens when client auth is valid.
- `/oauth/revoke` rejects attempts to revoke a refresh token owned by another client.
- `/oauth/revoke` does not revoke already-issued access JWTs.
- Disabled or deleted accounts cannot receive new tokens.
- Account deletion revokes refresh tokens and removes password hash material.
- Account deletion strips email/contact/profile metadata from password and identity records.
- Account deletion leaves only minimal account and identity tombstones.
- Account deletion behavior is not affected by deleted identity reuse config.

OIDC compatibility:

- `/.well-known/openid-configuration` advertises only implemented flows and ES256 signing.
- Discovery metadata does not advertise token introspection.
- Discovery metadata advertises refresh-token revocation.
- `/token` returns an ID token when the granted scope includes `openid`.
- `/token` rejects `client_credentials` in v1.
- `/authorize` preserves client `nonce` through the authorization code.
- `/authorize` does not render hosted login, consent, account-selection, or provider-selection UI.
- Unsupported OIDC authorization parameters fail safely when they change authentication semantics.
- ID token `aud` is the OAuth client ID.
- Access token audience is not confused with ID token audience.
- Access tokens are self-contained JWTs and are not persisted for introspection.
- Initial ID token nonce is present when the client authorize request supplied one.
- Refresh-response ID tokens, if issued, omit nonce and preserve original `iss`, `sub`, and `aud`.
- Userinfo returns email/profile claims only when the access token grants the corresponding scope.
- Userinfo rejects disabled/deleted subjects, but resource APIs using local JWT validation rely on token expiry.
- JWKS exposes public keys only.

Signing configuration:

- `local-es256` fails startup without local signing key material.
- `kms-es256` fails startup without a KMS signing key reference.
- Discovery signing metadata matches the configured signer.

Provider behavior:

- Google identity uses issuer plus subject, not email.
- Apple identity uses issuer plus subject, not email.
- Provider state is single-use.
- OIDC nonce is validated.
- Verified Google and Apple sign-ins persist minimal identity records.
- Existing identity records cannot be silently reassigned to another subject.
- Deleted identity mappings cannot be silently recreated with the same subject.
- Matching email across password and OIDC identities does not auto-link accounts.

Client configuration:

- Missing or malformed client config file fails startup.
- Missing SST/local secret for a confidential client's `client_secret_ref` fails startup.
- Invalid client redirect URIs fail startup.
- Public clients require PKCE.
- Public clients cannot declare a client secret ref.
- Runtime routes cannot create, update, disable, or delete clients.
- Client lookup uses the read-only config registry, not a DynamoDB scan.

TTL configuration:

- Invalid TTL values fail startup.
- Access-token TTL must be shorter than refresh-token TTL.
- Authorization-code TTL cannot exceed authorize-session TTL.
- Provider-state TTL cannot exceed authorize-session TTL.
- Store records write DynamoDB `expiry` from configured TTL-derived `expires_at`.
- Runtime expiry checks reject expired records before DynamoDB TTL deletion.

Account lifecycle configuration:

- Unknown deleted identity reuse mode fails startup.
- `after_retention` defaults to 30 days.
- `after_retention` rejects invalid retention-day values.
- `immediate` allows the same identity to create a new account with a different subject after deletion.
- `after_retention` blocks reuse before the retention window and allows reuse after it with a different subject.
- `never` blocks deleted identity reuse.

Email behavior:

- Missing `RESEND_API_KEY` fails startup.
- Missing `AUTH_EMAIL_FROM` fails startup.
- Verification and reset emails contain link URLs, not short numeric codes.
- Invalid configured email template path fails startup.
- Email template overrides reject unknown variables.
- Rendered email templates escape user-controlled display values.
- Resend delivery failure does not mark users verified.
- Password reset request does not reveal whether an email exists.

Audit logging:

- Default audit log mode is `cloudwatch`.
- `AUTH_AUDIT_LOG_MODE=none` disables audit event emission explicitly.
- Audit events are structured JSON.
- Audit events do not include tokens, passwords, provider credentials, client secrets, or verification/reset links.
- Audit source identity uses trusted API Gateway request context data.

## Test Boundaries

Runtime uses Resend only. Tests may use a mock email sender internally, but the production configuration model should not expose a console or provider switch.

## AWS Validation

Before production confidence:

- Deploy to AWS dev account.
- Confirm API Gateway source IP is available in request context.
- Confirm spoofed forwarded headers do not affect rate-limit keys.
- Confirm DynamoDB TTL attributes are written on short-lived records.
- Confirm no raw bearer values appear in `pk` or `sk`.
- Run load tests for `/authorize`, password login, `/token`, refresh rotation, and email verification consume.
- Measure cold start and compare 256 MB vs 512 MB Lambda memory.
