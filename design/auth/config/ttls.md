# TTL Configuration

Target code: `packages/functions/auth/src/config/ttls.rs`

## Owns

- Parsing TTL environment variables.
- Applying safe defaults.
- Validating TTL relationships and bounds.
- Supplying typed durations to core and store modules.

## Decision

Token and short-lived auth artifact lifetimes are deployment configuration.

This gives template users control over the security, latency, and KMS-signing cost tradeoff. Shorter access-token TTLs reduce leaked-token lifetime but increase refresh frequency and signing calls. Longer access-token TTLs reduce refresh traffic but increase the impact window of a leaked access token.

## Runtime Config

```text
AUTH_ACCESS_TOKEN_TTL_SECONDS optional, default 3600
AUTH_ID_TOKEN_TTL_SECONDS optional, default 3600
AUTH_REFRESH_TOKEN_TTL_SECONDS optional, default 2592000
AUTH_AUTH_CODE_TTL_SECONDS optional, default 300
AUTH_AUTHORIZE_SESSION_TTL_SECONDS optional, default 600
AUTH_PROVIDER_STATE_TTL_SECONDS optional, default 600
AUTH_EMAIL_VERIFICATION_TTL_SECONDS optional, default 900
AUTH_PASSWORD_RESET_TTL_SECONDS optional, default 900
```

`AUTH_ID_TOKEN_TTL_SECONDS` controls first-party ID tokens issued by this auth server. It does not control Google or Apple provider ID-token validation.

## Default Policy

| Setting | Default | Purpose |
| --- | ---: | --- |
| `AUTH_ACCESS_TOKEN_TTL_SECONDS` | 3600 | Limits bearer access-token lifetime. |
| `AUTH_ID_TOKEN_TTL_SECONDS` | 3600 | Limits first-party OIDC ID-token lifetime. |
| `AUTH_REFRESH_TOKEN_TTL_SECONDS` | 2592000 | Allows 30-day sessions with rotation. |
| `AUTH_AUTH_CODE_TTL_SECONDS` | 300 | Keeps authorization-code replay window short. |
| `AUTH_AUTHORIZE_SESSION_TTL_SECONDS` | 600 | Keeps browser authorize sessions short-lived. |
| `AUTH_PROVIDER_STATE_TTL_SECONDS` | 600 | Keeps external provider callback state short-lived. |
| `AUTH_EMAIL_VERIFICATION_TTL_SECONDS` | 900 | Keeps verification links/codes short-lived. |
| `AUTH_PASSWORD_RESET_TTL_SECONDS` | 900 | Keeps reset links/codes short-lived. |

## Validation Rules

- Every TTL is a positive integer number of seconds.
- Access-token TTL must be shorter than refresh-token TTL.
- ID-token TTL must not exceed refresh-token TTL.
- Authorization-code TTL should be no longer than the authorize-session TTL.
- Provider-state TTL should be no longer than the authorize-session TTL.
- Verification and reset TTLs should be short enough that email compromise windows stay bounded.
- Production config should reject extreme values instead of silently accepting them.

Suggested production bounds:

| Setting family | Suggested range |
| --- | --- |
| Access token | 5 minutes to 24 hours |
| Refresh token | 1 day to 90 days |
| Authorization code | 1 minute to 10 minutes |
| Authorize session | 1 minute to 15 minutes |
| Provider state | 1 minute to 15 minutes |
| Email verification | 5 minutes to 24 hours |
| Password reset | 5 minutes to 30 minutes |

## Security Invariants

- Runtime expiry checks use the configured TTL-derived `expires_at`, not DynamoDB TTL deletion timing.
- DynamoDB `expiry` attributes are derived from the same `expires_at` values.
- Store update paths must preserve existing expiry values unless the operation explicitly creates a replacement record.
- TTL validation errors must not print secrets or raw token/code values.
