# Cloudflare Web Example

## Goal

Deploy the optional Cloudflare Worker BFF web example and use it with the AWS-hosted Irongate auth
core.

## Inputs Needed

- Working AWS dev deployment.
- Cloudflare credentials in `.env`.
- `examples.enabled = true` and `examples.web.enabled = true` for the stage.
- Web client configured in `auth.clients.toml`.
- Verification URL base pointing at the Worker's `/auth/verify-email` route.
- A separate app-owned reset URL: this Worker does not implement password reset.

## Files To Edit

- `infra/shared/stage-config.ts`
- `auth.clients.toml`
- `.env`

## First Deploy With `workers.dev`

The Worker can infer its public origin from incoming requests. For first deploys, `examples.web.baseUrl`
can stay unset.

After deploy, copy the Worker URL into:

- `auth.clients.toml` redirect URI.
- `infra/shared/stage-config.ts` email verification URL base (`/auth/verify-email`).

Then redeploy so Irongate has exact callback and email URLs.

When setting `examples.web.baseUrl` (the Worker's `WEB_BASE_URL`), use the canonical public HTTP(S)
origin without URL credentials. Registration and completion reject requests to a different origin,
including a different scheme or port. Unset base URLs retain request-origin inference for local and
first-deploy use; that is not a production host allowlist.

Do not point reset links at this Worker: there is no reset page/handler. The core reset API needs
an app-owned form that submits the emailed token and new password.

## Registration Contract

The Worker submits only `{email}` to `POST /password/register`. Opening
`/auth/verify-email?token=...` displays a password/confirmation form; GET, refresh, prefetch and HEAD
do not call Irongate or consume the proof. The proof is bounded to 1,024 characters and escaped in a
hidden field. Missing or oversized proofs produce fixed errors without echoing their values.

Only a deliberate form POST to `/auth/verify-email` forwards `{token, new_password}` to
`POST /password/verify`. Confirmation is checked by the Worker; password policy and atomic proof
consumption/account creation remain the core's responsibility. Completion does not issue OAuth
tokens or create a BFF session. The user signs in separately using the chosen password.

Both form POSTs require a valid same-origin `Origin` header; missing, `null`, malformed or foreign
Origins are rejected. Responses use no-store and `Referrer-Policy: strict-origin` so the proof-bearing
path/query is never sent as a Referer and HTTPS-to-HTTP downgrades send no referrer. Form actions have
no query string, and error pages never echo passwords, proofs or upstream error bodies.

Deploy the matching auth-core and Worker contracts together in a separately authorized deployment.
The core no longer accepts email/password registration or token-only verification. Pre-contract
pending users with a stored password hash or missing registration binding are deliberately rejected
by registration; requesting another link does not repair these records. Before deployment, use
fresh/disposable development state or arrange separately reviewed, explicitly authorized cleanup
of incompatible pending data. There is no automatic deletion or backfill.

An obsolete proof can be replaced through rate-limited registration only when its pending user
already satisfies the new contract. Resending does not recover every old pending state. Existing
completed credentials remain usable for login/reset, but are not backfilled with email ownership.
Fresh disposable acceptance registrations exercise that boundary; do not infer ownership coverage
for historical accounts. The core's owner claim does not link provider accounts or create a Worker
session. See the [core data guidance](06-resend-email.md#existing-data) before a populated deployment.

## Commands

```bash
npm run deploy -- --stage dev
```

## Google Browser Navigation

The existing provider-login route redirects the browser to Irongate. Keep that navigation rather
than fetching `/google/authorize` inside the Worker: Google requires the short-lived transaction
cookie set on the HTTPS auth issuer. It is independent of the Worker's application-session cookie,
is not copied between hosts and is not exposed to JavaScript. The browser returns it on Google's
top-level GET callback, and confirmed state consumption clears it. Restart old/unbound attempts
with a fresh sign-in. See [Google setup](07-google-login.md#browser-transaction).

## Validation

Run the local Worker contract suite without cloud credentials:

```bash
npm --workspace @irongate/example-web test
```

This builds TypeScript and uses a fake upstream; it does not certify real email delivery, core proof
consumption or deployed Cloudflare behavior. After an authorized deployment, manually open the
Worker URL and run:

1. Register with email only.
2. Open the Resend verification link and confirm that it asks for a password and confirmation.
3. Refresh the form; account setup must still require deliberate submission.
4. Submit matching passwords, then sign in separately with the chosen password.
5. Login with Google if enabled.
6. Login with Apple if enabled.
7. Logout.

## Common Failures

- Email links still point to localhost.
- Worker callback URL is missing from `auth.clients.toml`.
- Cloudflare `.env` values are missing.
- Google or Apple buttons are enabled in stage config but the corresponding SST secret is missing.
- Registration/completion returns 403 because the request origin differs from `WEB_BASE_URL`, or
  the POST lacks a valid same-origin `Origin`. Do not permit `Origin: null` to bypass this boundary.
- An obsolete verification link, incompatible pre-contract pending record or mismatched core/Worker
  contract prevents registration/completion. See the pending-data boundary above before retrying.

## Done When

- Email-only registration and deliberate password completion work without logging in automatically.
- Opening or refreshing the verification link alone does not activate the account.
- Separate password login, callback, session and logout work.
- Provider login buttons only appear when the corresponding provider is configured.

This validates an integration example, not a production-ready BFF. Server-side expiry enforcement
and refresh handling remain absent. Origin checks cover registration/completion only; login/logout
do not gain general CSRF protection from this change.
Read the [current web boundaries](../../design/examples/web.md) before adapting it.
