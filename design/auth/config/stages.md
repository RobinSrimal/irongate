# Auth Stage Configuration

Target code: `packages/functions/auth/src/config/stages.rs`

## Owns

- Auth behavior that differs by stage.
- Safety checks for production.

## Target Behavior

Development and production use the same email delivery shape: Resend is required. Development can still use separate Resend credentials and test domains, but it should exercise the real delivery integration.

Supported stage names are explicit:

```text
dev
production
```

Ambiguous aliases such as `prod` and unknown stage names must fail clearly instead of falling back to development behavior.

## Production Requirements

- Real issuer URL.
- Resend configured for password verification and reset.
- No public admin bootstrap.
- No dev-mode verification bypass.
- Restrictive CORS configured in infra.

## Dev Requirements

- Resend configured with dev credentials.
- `AUTH_EMAIL_FROM` points at a sender/domain allowed by the dev Resend account.
- No console/log email delivery path.
