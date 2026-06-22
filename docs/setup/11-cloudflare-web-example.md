# Cloudflare Web Example

## Goal

Deploy the optional Cloudflare Worker BFF web example and use it with the AWS-hosted Irongate auth
core.

## Inputs Needed

- Working AWS dev deployment.
- Cloudflare credentials in `.env`.
- `examples.enabled = true` and `examples.web.enabled = true` for the stage.
- Web client configured in `auth.clients.toml`.
- Verification and reset URL bases pointing at the Worker origin or custom domain.

## Files To Edit

- `infra/shared/stage-config.ts`
- `auth.clients.toml`
- `.env`

## First Deploy With `workers.dev`

The Worker can infer its public origin from incoming requests. For first deploys, `examples.web.baseUrl`
can stay unset.

After deploy, copy the Worker URL into:

- `auth.clients.toml` redirect URI.
- `infra/shared/stage-config.ts` email verification/reset URL bases.

Then redeploy so Irongate has exact callback and email URLs.

## Commands

```bash
npm run deploy -- --stage dev
```

## Validation

Open the Worker URL and run:

1. Register with email/password.
2. Click the Resend verification link.
3. Login with password.
4. Login with Google if enabled.
5. Login with Apple if enabled.
6. Logout.

## Common Failures

- Email links still point to localhost.
- Worker callback URL is missing from `auth.clients.toml`.
- Cloudflare `.env` values are missing.
- Google or Apple buttons are enabled in stage config but the corresponding SST secret is missing.

## Done When

- The deployed Worker completes password register, verify, login, callback, session, and logout.
- Provider login buttons only appear when the corresponding provider is configured.
