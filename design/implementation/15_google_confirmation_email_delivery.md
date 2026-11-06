# 15_google_confirmation_email_delivery

## Status

Implemented and locally verified on 2026-11-06.

Builds on `49c4a9e` (Google browser transaction binding). This definition does not authorize
deployment, live email, cloud changes, hosted pages or reference-repository edits.

## Goal

Add the internal, rate-limited delivery operation for a Google email-confirmation challenge:

```text
validated Google candidate + consumed authorize session + admitted browser proof
  -> independent source/email send limits
  -> guarded pending confirmation in the existing table
  -> one code email through the existing sender
  -> request handle and exact expiry, not a login/code/token
```

The operation must generate the actual request and email proofs, preserve the original browser
binding and snapshots, and handle ambiguous storage/delivery failures truthfully. Its output is for
a future HTTP integration, not a newly exposed endpoint.

## Why This Slice

Slices 13 and 14 establish durable atomic confirmation and Google browser admission. There is
still no production operation that generates and delivers the mailbox code under send limits.
Implementing that boundary alone is smaller than callback branching, verification attempt limits,
CSRF/origin policy, confirmation screens and example changes together.

The reference hosts a confirmation form inside auth. Irongate currently remains API-only. This
slice does not silently adopt that architecture or couple core sign-in to the Cloudflare example.
Public Google callbacks continue rejecting new identities without direct Google email authority.
The send limits here are not verification-attempt limits and do not make the flow safe to expose.

## Scope

In scope:

- A small provider-domain operation for beginning Google confirmation.
- A branded HTML/text code email using existing rendering and Resend delivery boundaries.
- A dedicated send admission policy with independent source and normalized-email budgets.
- Minimal typed access to the candidate delivery address and the confirmed store expiry.
- Focused delivery/admission/failure tests, exact design mirrors and accurate setup notes.

Out of scope:

- Calling this operation from Google's public callback or changing current callback responses.
- Public completion/status/resend/cancel endpoints, forms, redirects or confirmation cookies.
- Online code-verification admission, new audit event types or completion orchestration.
- Core-hosted email-action pages, domains, DNS, new hosting or changes to either example.
- A general OTP/passwordless provider, account merge or provider-first password enrollment.
- New dependencies, services, table families, IAM permissions, stage fields, TTL settings or secrets.
- Renaming the shared email-sender interface or unrelated email-template refactors.
- Compatibility readers, migration/backfill, new schema defaults or deployed-data cleanup.
- Native sign-in, Apple grant/deletion work, live email/provider tests or cloud deployment.

## Reviewed Reference

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Reviewed pinned sources:

- `packages/functions/auth/src/api/providers/email_confirmation.rs`: `begin`, code generation,
  source/email send admission and rendered email. Do not copy its hosted routes, fixed cookie
  lifetime or best-effort cancellation after send failure.
- `packages/functions/auth/src/config/rate_limits.rs`: `ProviderEmailSend` policy.

Reference HEAD was `1985d096506c782fde36666361a3a047fbe4bcc8`; unrelated marketing edits and later
slice documents were present and left untouched. Recheck before implementation and keep the pinned
comparison. No product branding, OAuth identifiers, sender values or credentials are imported.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Google callback](../functions/auth/src/api/providers/google.md) and
  [browser binding](../functions/auth/src/api/providers/browser_binding.md).
- [Email authority/candidate](../functions/auth/src/providers/verified_email.md).
- [Confirmation store](../functions/auth/src/store/sign_in_confirmation.md),
  [provider issuance](../functions/auth/src/store/provider_issuance.md) and
  [email ownership](../functions/auth/src/store/email_ownership.md).
- [Email boundary](../functions/auth/src/email/mod.md),
  [rendering](../functions/auth/src/email/templates.md) and
  [email configuration](../functions/auth/src/config/email.md).
- [Rate policies](../functions/auth/src/config/rate_limits.md),
  [admission](../functions/auth/src/store/rate_limits.md) and
  [trusted source](../functions/auth/src/ratelimit/middleware.md).
- [HMAC families](../functions/auth/src/crypto/hmac_lookup.md),
  [TTLs](../functions/auth/src/config/ttls.md) and
  [test guide](../../packages/functions/auth/tests/README.md).
- [Resend setup](../../docs/setup/06-resend-email.md),
  [Google setup](../../docs/setup/07-google-login.md) and
  [smoke guide](../../docs/operations/smoke-test.md).

Update authoritative docs during implementation to describe only what ships. Do not advertise
third-party Google email sign-in or a configurable confirmation page as available.

## Domain Contract

Add `providers/email_confirmation.rs`, following the existing provider-domain/email/store split.
Do not add an HTTP module, router mount or a second way to complete confirmation.

The begin operation accepts the existing store, runtime configuration, rate-limit configuration and
email sender plus:

- An opaque `GoogleEmailCandidate`, constructed only after full Google token validation.
- The consumed Google `AuthorizeSessionRecord`.
- The raw browser proof already admitted by slice 14's typed state consumption.
- Trusted source identity supplied by the caller, not read from forwarded headers.

These are internal preconditions, not a public raw-email/token submission API. The operation must
not manufacture email authority, accept a chosen subject or accept caller-selected proof values.
It validates required handle/session shape and leaves identity/ownership eligibility to the existing
guarded store operation. An existing active provider identity does not enter fresh confirmation.

Required sequence:

1. Obtain the normalized candidate address through a narrow read-only accessor; retain the opaque
   candidate for storage. Do not expose a public constructor or mutable identity/ownership fields.
2. Charge the dedicated source budget and then email budget before creating pending state or sending.
3. Generate a fresh 32-character request handle and independent eight-digit code using existing
   cryptographic helpers (`generate_random_string`, `generate_unbiased_digits`). Preserve leading zeros.
4. Derive `ConfirmationDigests` from request, admitted browser proof and generated code. The email
   digest remains request-bound; raw values never enter stored records, keys, logs or errors.
5. Call `create_sign_in_confirmation` with the existing identity-reuse policy and
   `runtime.ttls.provider_state_seconds` as the requested lifetime. The store remains authoritative
   for `min(now + lifetime, original_session.expires_at)` and every captured-state condition.
6. After confirmed creation, render and send once to the candidate address using the existing
   `VerificationEmailSender`. Do not accept a separate caller-supplied destination.
7. Return only the request handle and exact pending expiry after successful delivery while still
   live. Do not return the mailbox code, browser proof, delivery ID, email, subject, OAuth code or tokens.

Use a minimal internal result without automatic Debug/Serialize exposure of the request handle.
Change creation to return its calculated expiry on confirmed success, rather than recomputing a
possibly later deadline after storage/network waits. Update in-scope test callers directly; do not
retain a compatibility wrapper. No pending-record schema change is needed.

Check expiry before sending and after delivery. Time spent waiting must not extend eligibility or
return an expired continuation as usable. Use a narrow controllable clock for deterministic tests,
coordinated with the existing provider clock; do not add sleeps or change unrelated time sources.
Checks before dispatch cannot guarantee when a remote email service accepts or delivers a message.

## Send Admission

Add only `Endpoint::ProviderEmailSend`, with key name `provider_email_send`. It is a domain admission
category, not a new public route. Match the reference's initial defaults:

```text
source: 20 requests / 60 seconds
email:   3 requests / 60 seconds, independent of source/client/session
```

Use the existing limiter's bounded policies, source-first conservative accounting, purpose-separated
source/email HMAC identifiers and CAS/TTL behavior. Extend the existing email-subject match arm and
secondary-policy validation, not a new counter implementation. Keep the current explicit enabled/
disabled limiter semantics; add no endpoint-specific bypass. Invalid/missing policy or storage
failure fails closed. Rejections preserve the existing rate-limit/unavailable classification and
retry metadata for a future HTTP caller.

Changing email/client/session must not evade the source ceiling. Changing source must not evade
the email ceiling. Denied/failed attempts are not refunded. Counters contain neither raw email/IP
nor browser proof, request handle, code or their unrelated lookup digests.

These limits constrain delivery abuse, not guessing of a short code. No public caller may invoke
the confirmation completion methods until independent verification-attempt admission and the
browser/CSRF transport have been designed and implemented.

## Email And Failure Semantics

Add a focused renderer to the existing `email/templates.rs`, reusing its HTML escaping and
`RenderedEmail`. Use configured brand/support values, a short sign-in-code subject and HTML/text
bodies containing the code and actual expiry. Tell the recipient to enter it only in the sign-in
window they opened and ignore unsolicited requests. There is no action link or account-activation
claim. Do not copy reference product names, add tracking, or imply existing template-path fields
provide override behavior that the renderer does not implement.

Reuse the existing Resend transport, sender address and SST secret. No new delivery service or
secret is necessary. Retain request-bound HMAC proof storage and configured normalization without
alias/dot/domain rewriting.

Failure rules:

- Admission rejection: no pending confirmation, email or identity/code writes.
- Confirmed store rejection: no send and no alternate identity resolution or retry.
- Operational/uncertain store failure: no send; report unavailable, not invalid credentials or
  proof of rollback. A committed but unreturned pending record may remain until expiry.
- Delivery failure or response loss: report delivery unavailable. Do not delete the pending record,
  refund admission, resend automatically or claim the email was not delivered.
- Expiry during delivery: return an expired/unusable outcome without extending or restoring state.
  An email might already be delivered; completion still relies on runtime expiry.

Use fixed safe errors and preserve rate-limit versus conditional versus operational distinctions.
Do not format raw SDK/transport messages, pending snapshots or proof-bearing values into diagnostics.
This is not an atomic store-and-email transaction or exactly-once delivery. There is one
application-level send attempt, not a queue, receipt workflow or compensating cleanup operation.

## Expected Change Surface

```text
packages/functions/auth/src/providers/email_confirmation.rs        new domain operation
packages/functions/auth/src/providers/mod.rs
packages/functions/auth/src/providers/verified_email.rs             read-only candidate accessor
packages/functions/auth/src/store/sign_in_confirmation.rs           return confirmed expiry
packages/functions/auth/src/config/rate_limits.rs
packages/functions/auth/src/store/rate_limits.rs
packages/functions/auth/src/email/templates.rs
packages/functions/auth/src/email/mod.rs
packages/functions/auth/tests/google_confirmation_delivery.rs      focused domain tests
packages/functions/auth/tests/support/sign_in_confirmation.rs       affected create result
packages/functions/auth/tests/README.md
```

Reuse the captured sender, signed/provider-candidate fixtures and transaction gates where useful.
Extend existing shared admission or Local cases only where the new policy/return contract needs
evidence. Mirror every changed/new file at its exact `design/functions/auth/src/` or `tests/` path.
Update applicable overview/threat/email/Google/rate-limit docs and setup notes. Keep each source
under 1,000 lines. Avoid unrelated test relocation or broad email abstractions.

## Verification

Before edits, run the current confirmation and provider/browser baselines. Focus new tests on:

- Captured email contains one generated eight-digit proof, preserves leading zeros in deterministic
  rendering tests, uses escaped configured branding, and contains no action/token link.
- Its actual code plus returned request and original browser proof prepares and completes the
  existing store operation on a separately constructed store. Wrong browser/code/request fails;
  one-use, original client/redirect/PKCE/nonce/generation and session expiry remain intact.
- Beginning creates only pending state and limiter counters, not account/identity/ownership/code
  writes. Existing or ineligible provider identities cannot be used to send fresh confirmation.
- Stored records contain digests, not raw request/browser/code. Domain output and sanitized
  errors do not expose the email proof or recipient. Do not print captured mail on success.
- Vary addresses/sessions under one source, then vary sources for one normalized address. Denied
  attempts create no challenge and send nothing. Use deliberately low policies for focused tests.
- Invalid policy, counter outage, create conflict and pre-dispatch failure stop delivery. A
  controlled lost response after a real creation commit leaves pending state but sends nothing.
- A sender that accepts/captures then fails leaves pending state, receives no automatic retry and
  returns unavailable. Neither delivered nor undelivered is inferred from that error.
- Controlled expiry before sending and while the sender is paused prevents a usable continuation;
  no timestamps are extended. Do not use TTL cleanup waits or wall-clock sleeps.
- Public Google callbacks remain unchanged: authoritative email works and new non-authoritative
  email still rejects. Apple, password delivery and existing rate-limit contracts regress cleanly.

Do not duplicate the full slice 13 race matrix. Reuse its memory/Local contracts to establish that
the small store return change preserves actual conditions, TTL and uncertain-outcome semantics.
Use disposable loopback-only DynamoDB Local with dummy credentials; stop it after tests. No live
Resend, Google, Apple, customer accounts or deployed tables are part of verification.

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test sign_in_confirmation --test oidc_google_start --test oidc_google_callback
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Also check touched Rust formatting, mirrors, relative links and source sizes. Report actual results,
warnings and skipped checks; passing capture tests are not live email or browser certification.

## Acceptance Criteria

- The internal operation generates and stores a browser-bound, request-bound confirmation and
  sends its proof only after confirmed creation and independent send admission.
- Returned expiry comes from the actual stored deadline and is never extended by delivery time.
- Confirmed rejection, operational/uncertain storage failure, delivery failure and rate limiting
  remain distinct and sanitized, without automatic retry or compensating deletion.
- Memory/domain and applicable Local regression checks pass; mirrors/setup accurately describe
  the internal-only capability and missing public completion/verification-attempt protections.
- No public route, hosted page, example dependency, new infrastructure, compatibility path or
  deployment is introduced.

## Implementation Result

- Added the internal `providers/email_confirmation.rs` begin operation, opaque candidate address
  access and `ProviderEmailSend` admission (20 source / 3 normalized-email sends per minute).
- Generates a request-bound eight-digit proof and creates guarded pending state before one email
  send. Creation returns its persisted deadline; delivery checks the same clock before/after I/O.
- Added built-in branded HTML/text code rendering through the existing sender. No extra config,
  service, secret, endpoint or hosted page is added.
- Conditional rejection, expiry, admission, unavailable storage and unavailable delivery remain
  distinct and sanitized. Uncertain responses neither undo pending state nor trigger resend/refund.
- Added captured-delivery contracts and extended existing memory/Local assertions for creation's
  returned expiry. Updated exact mirrors, test guidance, overview/threat/rate/email/Google docs and
  setup/smoke notes. Public callbacks still reject new non-authoritative Google email identities.

The pinned reference and its current HEAD/status were checked; reference and consumer repositories
were left untouched. No product identifiers, credentials or dependencies were copied.

## Verification Result

- Pre-edit confirmation/Google start/callback baseline: 38 passed.
- Focused delivery, confirmation and Google/Apple callback suites: 58 passed.
- Full locked auth suite: 353 passed, 80 DynamoDB Local cases ignored in that ordinary run.
- Separate explicit DynamoDB Local run: all 80 passed on disposable tables with dummy credentials.
  The in-memory emulator listened only on `127.0.0.1:18023`; it was stopped and the closed port checked.
- Auth test-target check and locked admin check passed.
- Auth all-targets Clippy passed with existing warnings: the same 25 warning kind/file pairs as
  the previous slice's captured baseline, with no new pair. This is not a warning-free build.
- `npm run test:infra` passed, including after the test-guide update.
- Touched Rust formatting, exact mirrors, relative Markdown links, source sizes and
  `git diff --check` passed.

No live provider, email, cloud deployment or browser/device test was run. TypeScript/setup/native
builds were not rerun because their implementation/configuration was unchanged. Local captures do
not prove remote delivery or public confirmation acceptance. The remaining public callback,
verification-attempt and browser/CSRF work stays outside this slice; no future slice was created.
