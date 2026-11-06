# Resend Email

## Goal

Configure email-first password registration and password reset delivery. This guide does not
authorize deployment, live email tests or changes to existing data.

## Inputs Needed

- Resend API key.
- Verified sender/domain.
- Verification URL base.
- Reset URL base.

## Files To Edit

- `infra/shared/stage-config.ts` for non-secret sender and URL values.

## SST Secrets

```bash
npx sst secret set ResendApiKey "<resend api key>" --stage dev
```

## Stage Config

Set:

```text
email.from
email.verifyUrlBase
email.resetUrlBase
email.brandName optional
email.replyTo optional
```

The verification/reset URL bases must point to real handlers in the consuming app. Verification
messages ask the recipient to choose a password and complete account setup. The optional
[web example](11-cloudflare-web-example.md) handles registration completion at
`/auth/verify-email`, not password reset. The auth core serves JSON/protocol endpoints, not hosted
setup pages; no new hosting or infrastructure is required, and Cloudflare is not a core dependency.
Template-path environment fields are parsed but not loaded; email bodies use built-in templates
with branding/subject settings.

## Registration Contract

```text
POST /password/register
{ "email": "user@example.com" }
-> { "status": "verification_required" }

POST /password/verify
{ "token": "<email-proof>", "new_password": "<chosen-password>" }
-> { "status": "verified", "subject": "user_..." }
```

Both request types reject unknown fields. Registration does not accept a password; verification
requires one. Pending state contains no password hash, subject or email ownership claim. Proofs bind
to one pending `registration_id`; resends retain that instance. Verification validates policy and
prepares proof, pending user, identity and email ownership eligibility before hashing. One
eight-operation transaction consumes the proof and writes the verified credential, account,
identity, email owner and three subject indexes. An active email owner rejects completion without
linking accounts,
including when its account is disabled, deleted or missing. Ownership records contain only the email
digest and lifecycle binding, never raw email or a TTL. Completion returns no OAuth code or
tokens and does not log the user in. Sign-in is a separate step.

The app's verification GET/HEAD must not call the completion API or consume a proof. The existing
web example renders a password/confirmation form, then forwards `{token, new_password}` only on
same-origin POST. Its registration form POST has the same explicit Origin check. These pages use
no-store, CSP, escaped hidden proof fields and origin-only referrers; passwords are not echoed or
put in URLs. Core CORS response-access settings are not request admission or CSRF protection.

## Internal Google Confirmation Email

The backend also has an internal Google mailbox-code delivery operation. It reuses the configured
Resend sender, `ResendApiKey`, brand and support values; no extra secret, stage field or service is
required. The built-in HTML/text message contains an eight-digit code and the actual pending expiry,
not an action link. Existing template-path fields do not customize it.

These internal operations are not wired to a public callback or route. Delivery has independent
source/email send limits; completion separately limits source and mailbox attempts before comparing
the code and returns a continuation only after confirmed atomic issuance. Confirmation browser/CSRF
transport remains absent. Do not
enable or smoke-test it by assuming new Google accounts with third-party addresses can sign in.
Captured-delivery tests exercise it without sending real mail. An uncertain storage response stops
sending; an email error may follow remote acceptance. Pending records and charges are retained,
without automatic resend or compensating cleanup. See the
[domain design](../../design/functions/auth/src/providers/email_confirmation.md).

## Existing Data

Registration requires a bound pending `registration_id` with no password hash; absent, empty or
non-string proof bindings are invalid grants, with no default ID or timestamp fallback. There are
no compatibility paths, migrations or backfills. Use fresh disposable data for acceptance; this
guide authorizes no cleanup or deployment.

Completed password accounts own their verified email. New Apple identities with signed verified
email or Google identities with authoritative email can join the active owner. In the reverse order,
password completion rejects an already-owned address instead of installing a password on an
provider account. Pending registration links cannot override that restriction. Login and reset continue
to use the persisted credential subject. No additional table, index resource, secret or permission
is required. See the [API design](../../design/functions/auth/api/providers/password.md).

## Validation

For an explicitly authorized check with a disposable address, submit email only and confirm a
Resend setup email arrives at the configured app or web example URL. Opening or refreshing the link
must only render the form. Submit a policy-valid password and confirmation, then sign in separately
with that password. Weak-password rejection must leave the proof usable; replay and sibling links
after completion must not replace credentials. See the [smoke guide](../operations/smoke-test.md).

Email delivery is outside the storage transaction. A failed send can leave a valid current-contract
pending instance and unused proof; retrying registration can send a fresh link for that instance.
This retry behavior does not apply to old ineligible pending rows. A timeout during completion may
occur after commit; do not infer rollback or erase state from the error alone.

Requesting a reset must not invalidate the current password or sessions. Confirmed completion
atomically changes the password and invalidates older reset links, unexchanged codes and refresh
tokens for that account. It does not log the user in or invalidate already-issued access JWTs;
those remain subject to their expiry. An error response alone does not prove that a timed-out
reset was rolled back. Do not record tokens, links or passwords in deployment notes.

## Done When

- Email-only registration sends a setup email without installing a password.
- GET/HEAD leaves the proof unused; deliberate completion installs the chosen password once.
- Replay and sibling links cannot replace credentials; sign-in remains separate.
- Reset email links point at the intended app URL.
