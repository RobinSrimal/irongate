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

## Existing Data

The clean contract invalidates old pending registrations and unbound proofs. Pending rows missing
`registration_id` or containing a password hash cannot be reused by `/password/register`; asking
for a fresh link alone does not fix an old row. Old proofs with absent, empty or non-string bindings
are rejected as invalid proof (`invalid_grant`), with no default ID or timestamp fallback.

Use fresh development state or separately reviewed cleanup of old pending state before deploying
this contract to a populated stage. There is no automatic erasure, migration or backfill, and this
guide authorizes no cleanup or deployment. Existing login and reset do not require ownership rows,
but historical completed accounts are not backfilled and ownership is not inferred from credentials
or provider email claims. Use fresh disposable acceptance data to verify ownership; do not claim
complete coverage for populated tables or rewrite credentials to manufacture it. A historical-data
transition requires separate review and approval. No new table, index resource, secret or permission
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
