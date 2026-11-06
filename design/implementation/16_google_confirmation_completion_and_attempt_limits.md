# 16_google_confirmation_completion_and_attempt_limits

## Status

Implemented and locally verified on 2026-11-06.

Builds on `e8e72a0` (internal Google confirmation email delivery). This definition does not
authorize deployment, live email/provider tests, hosted pages or reference-repository edits.

## Goal

Complete the internal Google mailbox-confirmation operation with bounded online attempts:

```text
request handle + original browser proof + emailed code + trusted source
  -> source attempt admission
  -> browser-bound pending snapshot
  -> independent email attempt admission
  -> code proof validation
  -> atomic pending consumption and pinned identity/code issuance
  -> original client redirect with authorization code and state
```

The operation returns a continuation only after confirmed commit. It does not issue tokens or
expose a public confirmation endpoint. Public Google callbacks retain their current behavior.

## Why This Slice

The store can atomically complete confirmation, and the domain can generate/deliver its proof.
Calling the store's current combined proof preparation directly would still permit unbounded
code comparisons. Delivery limits are not verification-attempt limits.

The reference charges a source budget before loading pending state, then an email-wide budget
after validating the starting browser. Keeping the latter independent of request/client/source
prevents a fresh confirmation request from resetting that address's guessing budget. This needs
a small staged store contract, not a second copy of account resolution or a new counter backend.

HTTP transport, CSRF, cookie transfer and the choice of a confirmation surface remain separate
decisions. This slice does not silently adopt the reference's core-hosted form or require the
Cloudflare example.

## Scope

In scope:

- Internal completion orchestration alongside `begin_google_confirmation`.
- Source-before-lookup and independent normalized-email code-attempt admission.
- Separate browser admission from email-code verification using private typed snapshots.
- Generate and store an authorization-code digest; return only the original OAuth continuation.
- Direct updates to current callers/tests, exact design mirrors and truthful setup guidance.

Out of scope:

- Google callback branching, public confirmation routes, forms, cookies, CSRF/origin transport.
- Status/resend/cancel endpoints, email-action hosting, DNS or example changes.
- New audit events, native login, Apple grant/deletion work or a generic OTP provider.
- New dependencies, resources, IAM permissions, table/record families, secrets or stage fields.
- Account merging, password enrollment on provider accounts, new ownership or lifecycle semantics.
- Compatibility wrappers/readers, schema defaults, migrations, backfills or deployed-data cleanup.

## Reviewed Reference

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Reviewed pinned sources:

- `packages/functions/auth/src/api/providers/email_confirmation.rs`: `submit`, `load`, attempt
  admission and continuation after completion. Retain the separation of source admission, browser
  proof and email admission. Do not copy hosted forms, raw record exposure or transport assumptions.
- `packages/functions/auth/src/config/rate_limits.rs`: source-only `CodeVerify` (5/minute) and
  `ProviderEmailConfirm` (30 source, 5 email/minute).

Reference HEAD was `1985d096506c782fde36666361a3a047fbe4bcc8`; unrelated marketing changes and later
slice documents were present and left untouched. Recheck status before implementation and retain
the pinned comparison. No product identity, host, account or credential belongs in the port.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Confirmation domain](../functions/auth/src/providers/email_confirmation.md),
  [store](../functions/auth/src/store/sign_in_confirmation.md) and
  [private provider plan](../functions/auth/src/store/provider_plan.md).
- [Google callback](../functions/auth/src/api/providers/google.md),
  [browser binding](../functions/auth/src/api/providers/browser_binding.md) and
  [email authority](../functions/auth/src/providers/verified_email.md).
- [Rate policies](../functions/auth/src/config/rate_limits.md),
  [admission](../functions/auth/src/store/rate_limits.md) and
  [rate-limit overview](../functions/auth/store/rate-limits.md).
- [HMAC families](../functions/auth/src/crypto/hmac_lookup.md),
  [authorization codes](../functions/auth/store/authorization-codes.md) and
  [token exchange](../functions/auth/api/oauth/token.md).
- [Test guide](../../packages/functions/auth/tests/README.md),
  [Resend setup](../../docs/setup/06-resend-email.md),
  [Google setup](../../docs/setup/07-google-login.md) and
  [smoke guide](../../docs/operations/smoke-test.md).

Update authoritative docs during implementation to describe what ships, not this unimplemented
plan. Do not advertise public third-party-email Google sign-in as available.

## Staged Store Contract

Replace combined `prepare_sign_in_confirmation` with one coherent staged path. Names below are
suggested; the security distinctions are required:

1. `load_sign_in_confirmation_attempt` validates request/browser handle shape, derives their
   existing purpose-separated HMACs, consistently reads the exact pending key, validates the
   complete persisted record/snapshot and checks runtime expiry and the browser proof. It returns
   an opaque browser-admitted attempt, not an issuance-ready value.
2. A narrow crate-only accessor supplies that attempt's stored normalized address for admission.
   No raw record, mutable session, code digest or caller-selected email is exposed to the domain.
3. After admission, `verify_sign_in_confirmation_attempt` consumes the attempt and checks the
   submitted email code against its original request-bound digest in constant time. Only successful
   proof validation creates `PreparedSignInConfirmation`, the type accepted by finishing.

Both attempt and prepared values retain private fields, with no public constructor or automatic
Debug/Serialize exposure. Browser possession alone must never construct a value accepted by
`finish_sign_in_confirmation`. Do not introduce an `already_verified` boolean or caller-supplied
authority. Update tests directly and remove the superseded combined preparation entry point.

Keep the original JSON snapshot through both phases. Do not reread/re-resolve newer account,
ownership, identity or generation state after waiting on admission. The existing seven-operation
finish transaction still guards all snapshots and consumes the exact pending value with code
creation. Its TTL, retention and uncertain-outcome semantics do not change.

Unknown, expired and browser/code mismatches are safe invalid-confirmation outcomes; corrupt rows
and operational reads remain unavailable, not proof mismatches. Recheck expiry with the existing
provider clock after admission and before transaction dispatch. A failed proof does not delete or
extend pending state. Existing request-bound HMAC framing remains unchanged.

## Attempt Admission

Reuse the current limiter with two explicitly named internal categories:

| Category | Source / 60 seconds | Secondary / 60 seconds | Placement |
| --- | ---: | --- | --- |
| `ProviderEmailAttempt` (`provider_email_attempt`) | 5 | None | Before pending lookup, including malformed/unknown/expired submissions |
| `ProviderEmailConfirm` (`provider_email_confirm`) | 30 | 5 per normalized email across sources | After browser admission, before code comparison |

The first category is confirmation-specific; do not import a generic passwordless `CodeVerify`
endpoint. The second uses the existing source-plus-email policy shape from the reference. Its
source charge is additional conservative accounting, not a replacement for the stricter pre-load
ceiling. This preserves the current counter API without a new phased-counter abstraction.

Rules:

- Only the stored browser-admitted address chooses the email bucket. No submitted email does.
- Changing handles, browser proofs, email codes or client sessions cannot reset source admission.
- Changing sources or starting another confirmation for the same normalized address cannot reset
  its email budget. Do not key that budget by request, browser or client.
- Unknown/expired/wrong-browser requests charge the source category, but never allocate a
  guessed email bucket. Wrong or malformed codes with a valid browser charge both categories.
- Validate code format only after those applicable charges: exactly eight ASCII digits, retaining
  leading zeros, without numeric conversion or implicit normalization. Bound processing before
  hashing; a malformed code is not a reason to skip source admission.
- Existing HMAC source/email identifiers, canonical source handling, bounded CAS/TTL behavior,
  explicit global disable and retry metadata remain intact. Invalid/missing enabled policy or
  counter failure stops protected work. Charges are not refunded after rejection or outage.

These are fixed-window limits, not a lifetime lockout or an exactly-once attempt counter. A window
can reset while a pending challenge remains live. State those limits accurately; do not claim a
maximum total number of guesses across its whole TTL. No new stage-config knobs are required.

## Completion Domain Contract

Add `complete_google_confirmation` to the existing provider module. Inputs are the store, runtime,
rate-limit config, raw request handle, original browser proof, submitted code and trusted source.
It accepts no email, subject, redirect, client ID, chosen OAuth code or replacement session.

Required sequence:

1. Charge pre-load source admission, then obtain the browser-admitted attempt.
2. Charge confirmation source/email admission using only its stored address.
3. Verify the eight-digit code into the private prepared value, checking current expiry.
4. Generate a fresh high-entropy authorization code through existing random helpers. Derive its
   `AuthorizationCode` HMAC; only that digest reaches storage.
5. Build the continuation using a narrow read-only view of the prepared original redirect/state.
   Parse and encode with `url::Url`, preserve registered query parameters and optional original
   state, and fail safely before consumption if the stored redirect cannot be constructed. Match
   existing OAuth redirect semantics; do not add a broader redirect policy or HTTP response here.
6. Call the existing finish operation once with the configured authorization-code TTL.
7. Only after confirmed success return the redirect URI containing the code and original state.

The result should not expose an email, subject, proof, pending record, lookup digest or token, and
must not derive Debug/Serialize. The raw OAuth code belongs only in the returned continuation; do
not log or persist it. No email is sent by completion. Client/redirect/PKCE/nonce/scope/generation
remain pinned by the stored session and existing issuance transaction.

Use a shared safe domain error type for begin/complete if appropriate; rename the current
delivery-specific type and update callers directly instead of retaining an alias. Collapse
unknown/expired/bad-proof completion into an invalid-confirmation result. Preserve admission
classification/retry metadata and conditional versus operational distinctions without embedding
backend strings or proof-bearing values.

A confirmed conditional failure yields no continuation and does not retry against newer state.
An uncertain finish response yields unavailable, even if consumption/code issuance committed.
Never reveal the generated code, claim rollback, restore the proof, refund limits, recreate an
account or retry issuance automatically. A retry after actual commit cannot issue a second code.
Do not reject a confirmed successful commit merely because the confirmation deadline elapsed
while its response was returning; that deadline limits dispatch, not delivery of an already-issued
code. Ordinary authorization-code expiry still applies.

## Expected Change Surface

```text
packages/functions/auth/src/providers/email_confirmation.rs
packages/functions/auth/src/store/sign_in_confirmation.rs
packages/functions/auth/src/config/rate_limits.rs
packages/functions/auth/src/store/rate_limits.rs
packages/functions/auth/tests/google_confirmation_completion.rs       new domain contracts
packages/functions/auth/tests/google_confirmation_delivery.rs         adapted store/error callers
packages/functions/auth/tests/support/sign_in_confirmation.rs         staged shared contracts
packages/functions/auth/tests/support/sign_in_confirmation/races.rs   only if callers require it
packages/functions/auth/tests/README.md
```

Reuse current transaction/rate-control fixtures. A small shared captured-mail helper is justified
only if both delivery/completion tests need it; do not clone fixtures or add general frameworks.
Existing memory/Local runners and rate contracts should be extended only for relevant new behavior.
Keep each source below 1,000 lines and mirror every touched/new file at its exact design path.

Update applicable domain/store/rate/Google overviews, threat model, test guide and setup/smoke notes.
Make clear that the internal completion operation now has attempt admission while public
confirmation, transport/CSRF and live acceptance remain unavailable. No roadmap or future slice
document is part of this work.

## Verification

Run the delivery/confirmation/Google baseline before editing. Focus new evidence on:

- Begin through the actual delivery operation, capture its generated code and complete using a
  separately constructed store. Redirect targets the original URI, encodes state/query correctly
  and carries a usable code. Exchange it through `/token` with original client/PKCE/redirect;
  verify subject, nonce, scope and generation. Wrong exchange bindings still reject.
- Source denial stops pending reads even when requests/candidates/clients vary. Missing/invalid
  policy, counter corruption/outage and uncertain charges fail closed without refunds.
- Wrong browser/unknown request never reaches code validation or email admission. Correct browser
  plus wrong/malformed code charges email attempts without consuming the proof.
- Vary sources and separately created requests for one normalized mailbox. Email exhaustion
  rejects even the correct code and leaves pending state unchanged; a different mailbox has its
  own budget. Do not establish this only by repeating one identical request.
- Leading-zero codes work; empty, short/long, whitespace-padded and non-ASCII codes reject. Existing
  request-bound/cross-request proof tests still pass after the staged API change.
- The browser-admitted type cannot authorize issuance; independent code verification is required.
  Failed code verification, malformed records and unavailable reads preserve their distinctions.
- Pause admission, advance the provider clock and verify expiry before issuance. Use gates, not
  sleeps. Preserve the existing final-dispatch expiry and state-replacement contracts.
- Concurrent completion produces at most one successful continuation. Pre-dispatch failure,
  confirmed condition failure and lost response after real commit preserve actual row outcomes.
  A post-commit response delayed past confirmation expiry still returns its committed continuation.
- Reuse existing memory/Local proof, ownership, lifecycle, generation and physical TTL contracts
  after refactoring their callers. Extend those shared cases for the new staged interval instead
  of duplicating the complete race matrix in a new suite.
- Public Google/Apple/password behavior is unchanged. Delivery regression still verifies no
  automatic resend/deletion on failure. No raw proofs, redirects or codes appear in diagnostics.

Suggested commands (plus touched Rust formatting, mirrors, relative links and source-size checks):

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test google_confirmation_delivery --test sign_in_confirmation --test oidc_google_callback
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Use disposable loopback-only DynamoDB Local with dummy credentials and stop it after verification.
No deployed table, live sender/provider or UI automation is authorized. Record actual passes,
failures, warnings and skipped checks; previous slice results are not verification of this change.

## Acceptance Criteria

- Internal completion bounds source and mailbox attempts before comparing the short code.
- A browser-admitted snapshot is not sufficient issuance authority; only verified email proof
  constructs the private prepared type and finishing preserves all existing transaction guards.
- A confirmed success returns only the original OAuth continuation. Replays, races and uncertain
  failures never trigger re-resolution, automatic retries, rollback claims or secret-bearing errors.
- Focused and shared backend tests pass, including staged expiry, independent admission and actual
  post-commit fault behavior; mirrors and setup docs accurately describe the internal-only scope.
- No public route, hosted surface, infrastructure, compatibility path or deployment is added.

## Implementation Result

- Added internal `complete_google_confirmation`, returning only the original redirect with a fresh
  authorization code after confirmed finish. Begin and complete share the renamed `ConfirmationError`;
  no old alias remains.
- Added source-only `ProviderEmailAttempt` and source/email `ProviderEmailConfirm` policies with
  the specified defaults. The existing limiter, HMAC keys, explicit disable and conservative
  accounting are unchanged. Browser-admitted stored email selects the mailbox budget.
- Replaced combined proof preparation with private browser-attempt and issuance-ready types.
  Verification checks exact eight-ASCII-digit shape, expiry and constant-time request-bound proof;
  the original snapshot remains pinned through admission and finish. Existing store callers/tests
  were updated directly, without a compatibility entry point or record-schema change.
- Added completion contracts for source/email admission, faults, expiry, races and actual uncertain
  or delayed commit responses. Extended shared memory/Local contracts for the staged interval.
- Shared the captured-mail fixture and moved the store-only token-route test into a domain protocol
  child that begins with actual captured delivery and completion, avoiding duplicate coverage.
- Updated exact source/test mirrors, overview, threat/rate/store/domain guidance and setup/smoke
  notes. No public callback, route, UI or infrastructure changed.

The reference HEAD/status was rechecked and left untouched, retaining the pinned comparison and
the product-neutral boundary. No consumer repository or cloud resource was changed.

## Verification Result

- Pre-edit delivery/confirmation/Google callback baseline: 40 passed.
- Focused completion/delivery/confirmation suites: 28 passed.
- Full locked auth suite: 364 passed; 81 Local cases ignored in the ordinary run.
- Explicit DynamoDB Local run: all 81 passed, including the staged proof case and existing physical
  TTL/conditional/race contracts. It used disposable tables, dummy credentials and a verified
  `127.0.0.1:18023` listener. The in-memory emulator was stopped and the closed port checked.
- Auth test-target check, locked admin check and `npm run test:infra` passed.
- Auth all-targets Clippy passed with the same 25 warning kind/file pairs as the slice 15 captured
  baseline, with no new pair. The build is not warning-free.
- Touched Rust formatting, exact mirrors, relative Markdown links, sub-1,000-line source files
  and `git diff --check` passed.

No live email/provider, cloud deployment, UI or device test was run. TypeScript/setup/native builds
were not rerun because their source/configuration was unchanged. Public confirmation transport,
cookie/CSRF integration and live acceptance remain outside this implemented internal boundary.
No future slice document was created.
