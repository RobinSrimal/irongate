# 08_email_first_password_registration

## Status

Implemented and locally verified on 2026-11-05 following the user's implementation request.
Builds on slice 07 at Irongate commit `af11c113870aaa2bd17175072d9ebb179a3c2249`.
The definition was the only pending file at implementation start. No deployment, cloud
reconfiguration, reference-repository edit or deployed-data cleanup was performed.

## Goal

Let the email owner choose the initial password only after receiving an email proof:

```text
submit email -> receive link -> submit proof and new password
  -> atomically create verified password account -> sign in separately
```

Anonymous registration must not install a password that somebody else later activates by opening
an email. Completing registration must consume the proof and persist the credential together.

Keep this slice to the existing password API, its typed storage operations and the existing web
example caller. It does not introduce hosted pages into the auth Lambda.

## Reviewed Sources

Use the read-only reference `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620`, as specified in the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant reference files at that commit:

- `packages/functions/auth/src/providers/password.rs`.
- `packages/functions/auth/src/api/providers/password.rs`.
- `packages/functions/auth/src/store/password_users.rs` and `password_secrets.rs`.
- `packages/functions/auth/src/api/email_actions.rs`, for deliberate password-setup submission.

The reference HEAD has advanced to `559b77d61aabb310d3564223c6263fb217f5665a` and has unrelated
uncommitted mobile/release changes. Neither is part of this comparison. Do not modify that repo.

At implementation start, reference HEAD was `aaafbcfa93cb2142b8095ff9bc113e120abdf4c9`
with a clean worktree. The comparison remained pinned to the reviewed reference above.

Its final registration implementation includes shared-account resolution and compatibility helpers.
Adapt the email-first behavior, not that entire implementation. Preserve Irongate's existing account
boundaries, credential generations, independent rate limits and structured audit emitter.

## Starting Gaps

| Area | Current behavior | Required behavior |
| --- | --- | --- |
| Registration | Accepts and hashes email/password before ownership proof | Accepts email only; pending record has no password |
| Verification | Consumes proof before a separate account-creation transaction | Proof consumption and account activation commit together |
| Pending retries | Retain the first submitted password | Retain the pending registration, never an anonymous password |
| Proof binding | Verification record identifies only the email lookup | Bind proof to one pending registration instance |
| Web link handling | GET calls `/password/verify` immediately | GET displays a form; only deliberate POST completes setup |
| Fixtures | Several tests activate users through store-only bypass helpers | Use the current registration/completion contract for normal fixtures |

The Tauri example currently signs in but does not implement registration or email verification.
There is no native registration caller to port in this slice.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Password API](../functions/auth/api/providers/password.md),
  [domain behavior](../functions/auth/providers/password.md) and
  [password policy](../functions/auth/core/passwords.md).
- [Password users](../functions/auth/store/password-users.md),
  [one-time secrets](../functions/auth/store/password-secrets.md),
  [accounts](../functions/auth/store/accounts.md) and
  [identity lifecycle](../functions/auth/core/account-lifecycle.md).
- [Typed store](../functions/auth/src/store/mod.md),
  [record shapes](../functions/auth/src/store/records.md) and
  [atomic recovery](../functions/auth/src/store/recovery.md).
- [Independent rate limits](06_independent_source_and_account_rate_limits.md) and
  [auth test instructions](../../packages/functions/auth/tests/README.md).
- [Web routes](../examples/web/src/routes.md) and [web views](../examples/web/src/views.md).
- [Email setup](../../docs/setup/06-resend-email.md),
  [web setup](../../docs/setup/11-cloudflare-web-example.md) and
  [smoke instructions](../../docs/operations/smoke-test.md).

Update these documents and exact file mirrors alongside implementation. Until implementation,
authoritative design must continue describing the current behavior, not this proposal as complete.

## Boundary

In scope: one email-first registration contract, atomic completion, narrowly adapted email copy,
scanner-safe verification in the existing optional Worker, affected fixtures and rejection/race tests.

Keep current routes, HMAC key families, verification TTL configuration, Argon2id policy, Resend
delivery, account/identity indexes and deleted-identity reuse policy. No new dependency is expected.

Out of scope:

- Auth-Lambda HTML email-action pages, custom domains, certificates or new hosting.
- Shared-account resolution, provider/email linking or changing Google/Apple behavior.
- New native flows, a general SDK or registration UI in the Tauri app.
- Password-reset changes, web reset pages or a general web security/UI rewrite.
- New services, permissions, audit event kinds, background cleanup or schema migration tooling.
- Live email tests, cloud deployment, deployed-data cleanup or product-specific configuration.

Email-first registration can use an app-owned completion form without changing hosting. That is
the smallest option compatible with the current API-only core. Optional core-hosted email actions
remain a separate architecture decision; do not silently add them to make this flow work.

## Public Contract

Keep JSON POST endpoints:

```text
POST /password/register
{ "email": "user@example.com" }
-> { "status": "verification_required" }

POST /password/verify
{ "token": "<email-proof>", "new_password": "<chosen-password>" }
-> { "status": "verified", "subject": "user_..." }
```

- Remove `password` from registration inputs and require `new_password` for verification.
  Reject unknown fields on these two DTOs so the old registration shape does not silently succeed.
- Keep email normalization and password policy centralized. Validate the new password before
  hashing or mutating credential state; a policy failure must leave the proof usable.
- Keep registration's independent email/source limits and verification's trusted-source limit.
  Admission must precede storage, hashing and email delivery as applicable. Do not derive a new
  limiter key from a raw token or password.
- Invalid, expired, consumed, wrong-purpose or mismatched proofs return the same safe invalid-grant
  response. Password-policy errors are invalid requests, not storage failures.
- Preserve the existing already-registered response for active password identities. This slice
  does not claim email-enumeration resistance or redesign response timing.
- Confirmed conditional rejection is not an operational outage. Preserve storage error
  classification and safe server-error handling; never include submitted values or raw SDK errors.
- Neither endpoint returns an OAuth code, access/refresh/ID token, nor establishes a login session.
  Login and OAuth continuation remain separate and unchanged.

There is one contract, not a password-first compatibility branch. Do not backfill old pending
passwords, accept token-only verification or auto-delete existing data. Existing pre-production
pending records/proofs using the old contract may be invalidated; document that before a separately
authorized deployment. Existing completed credentials do not need to be rewritten for this change.

## Pending Registration And Proofs

Use the existing password-user and verification key families. Add a registration binding:

| Record | Relevant shape |
| --- | --- |
| Pending password user | Normalized email, fresh `registration_id`, `verified=false`, no subject, hash or hash timestamp, creation/update timestamps |
| Verification proof | Email digest, required matching `registration_id`, `purpose=verify_email`, creation and expiry timestamps |
| Completed password user | Subject, Argon2id hash and hash timestamp, `verified=true`, no pending registration ID |
| Deleted password user | Existing anonymized tombstone, with pending registration ID cleared |

`registration_id` can be an internal UUID; it is not a bearer credential or a lookup key. Represent
its absence explicitly for completed/deleted records. Do not invent a default ID or fall back to
timestamp ordering for old pending records/proofs. Pending eligibility requires an ID, no hash,
no subject, no deletion marker and `verified=false`.

Registration behavior:

1. Normalize email and obtain its HMAC lookup digest.
2. Create a pending record conditionally if absent. Reuse an eligible existing pending record
   without replacing its ID or resetting its creation time.
3. For a deleted record, enforce the existing identity-reuse policy and conditionally replace the
   tombstone with a fresh pending instance. Never reuse its previous subject or registration ID.
4. On a competing pending-create condition failure, reread and use only an eligible pending winner.
   Do not turn arbitrary failures into successful registration or add unbounded retries.
5. Generate a high-entropy verification token. Store only its HMAC lookup digest in the key, the
   registration binding in the value and matching logical expiry/DynamoDB `expiry`.
6. Create the proof with an exact pending-record condition, so completion/replacement cannot be
   ignored between reading the pending user and issuing another proof.
7. Send email only after persistence succeeds, using the existing configured verification URL.

Resends may leave multiple short-lived proofs for the same pending instance. Completing any one
invalidates all others logically because that instance is no longer pending. No partition scan or
synchronous deletion of every sibling proof is needed. TTL cleans up unusable proofs later.

Email delivery is not transactional with DynamoDB. A failed send may leave a pending record and
unused proof; another rate-limited registration can request a fresh link. Do not delete or replace
the pending instance on delivery failure, or promise that an error means nothing was persisted.

## Atomic Completion

Put registration completion in `src/store/registration.rs`, with the same typed-store boundary as
recovery. Replace the current consume-then-activate flow with a private-field prepared snapshot and
one finishing operation; callers must not be able to forge its validated record state.

Preparation loads and validates the proof, its purpose/expiry, the matching eligible pending user
and the existing password-identity reuse constraints. It does not consume the proof. Hash the
chosen password only after this preparation succeeds.

Immediately before dispatch, recheck expiry using a controllable registration clock and validate
the prepared binding. Commit these six operations together using the real storage transaction:

1. Delete the verification proof, conditioned on its complete prepared value.
2. Replace the pending password user with the verified credential, conditioned on its complete
   prepared value. Set its new subject/hash/hash timestamp and clear the registration ID.
3. Create the new active account with credential generation zero, conditioned on absence.
4. Create the password identity, or replace an eligible deleted identity, conditioned on absence
   or the exact tombstone value inspected during preparation.
5. Create the identity-to-subject index, conditioned on absence.
6. Create the password-user-to-subject index, conditioned on absence.

Use a fresh generated subject. Never overwrite an existing active identity/account, enable a
disabled account or move a provider identity. Cross-provider email matching is not implemented here.

Competing submissions, including two different proofs for one pending user, must have only one
successful completion. A confirmed canceled transaction leaves its proof and every credential/index
write unchanged. A successful completion makes sibling proofs and subsequent password changes via
verification fail. It must not return success merely because the email already has a verified user.

Keep operational failures and uncertain outcomes distinct from a confirmed conditional rejection.
An error after a committed write can leave a completed account; do not undo it or report guaranteed
rollback. There is no new retry/receipt service. Expiry is checked at the application boundary before
dispatch; DynamoDB TTL deletion is not authorization and network latency is not a zero-time commit.

Remove superseded standalone verification consumption/activation and password-first pending-user
helpers rather than retaining production bypasses for tests. Preserve unrelated reset cleanup and
lifecycle operations in their existing modules. Adapt normal fixtures to create verified users
through registration and proof completion; deliberately corrupt fixtures may still construct the
specific invalid storage state they test.

## Existing Web And Email Changes

- Registration form asks for email only and posts that shape to Irongate.
- `GET /auth/verify-email?token=...` only renders a password/confirmation form. No Irongate calls,
  proof consumption or account/session creation on GET, refresh, prefetch or HEAD.
- Add `POST /auth/verify-email` accepting the hidden proof plus password/confirmation. Validate
  confirmation, then forward only `{token, new_password}`. The core remains the password-policy
  authority. Do not echo passwords in markup, errors or URLs.
- Protect these two form POSTs with a strict same-origin check before forwarding. Compare a valid
  Origin with the effective web base URL origin; reject missing, `null`, foreign or malformed values.
  When `WEB_BASE_URL` is configured, reject requests to a different origin on these paths. Preserve
  existing request-origin inference for local/first-deploy use, not a new production host policy.
- Preserve CSP, no-store and escaping. Use an origin-only referrer policy for these form pages
  compatible with browser Origin submission, rather than allowing `Origin: null` as a workaround.
  Never send the token-bearing path/query as a Referer; use a clean, relative form action.
- Show fixed safe failure/success messages and a sign-in link after completion. Do not automatically
  log in or create a BFF session. A hidden proof field is necessary for POST, not a license to
  display it in diagnostic text or persist it in browser storage.
- Update verification email wording to describe completing account setup and choosing a password.
  Keep configured brand/sender/URLs, Resend and reset-email behavior unchanged.

Use the current web layout. No styling project, browser automation or UI snapshots are required.
Tests of GET non-mutation, form request boundaries and secret handling are security contract tests.
Do not add unrelated native/example work or require Cloudflare for consumers of the core API.

## Change Surface And Design Mirrors

Primary auth files: `src/providers/password.rs`, `src/api/providers/password.rs`,
`src/store/records.rs`, `src/store/password_users.rs`, `src/store/password_secrets.rs`,
`src/store/mod.rs`, new `src/store/registration.rs`, and `src/email/templates.rs`.

Primary example files: `packages/examples/web/src/routes.ts`, `src/views.ts` and
`tests/routes.test.ts`; adapt `src/testing.ts` only if its request/response fixtures need it.

Keep registration domain/API tests in `tests/password_registration.rs` and
`tests/api_routes/password.rs`. Add focused shared registration fixtures/contracts and
`tests/dynamodb_storage/registration.rs` as needed, reusing the existing disposable Local harness
and deterministic transaction gate. Update affected login/recovery/admin/refresh/admission fixtures
without rewriting their unrelated assertions or introducing numeric slice test files.

Every changed/new source or test must have its exact `design/functions/...` or
`design/examples/...` mirror, preserving `src/`, `tests/` and subdirectories. For example:

```text
packages/functions/auth/src/store/registration.rs
  -> design/functions/auth/src/store/registration.md
packages/examples/web/tests/routes.test.ts
  -> design/examples/web/tests/routes.md
```

Update affected topic overviews and email/web setup/smoke instructions to the single current
contract. Describe what, why and how, not superseded choices. Keep each source file below 1,000
lines; split registration responsibilities if needed rather than enlarging the store facade.

## Verification

Use observable behavior and actual state, not source-string assertions or fake-only success tests.

### API And Domain

- Email-only registration produces a proof but no hash, subject, account or tokens.
- Pending resends retain the binding and cannot install an anonymous password.
- Old email/password registration and token-only verification payloads fail.
- Valid completion installs the submitted password; only it can log in afterward.
- Weak passwords leave proofs usable; missing/expired/wrong-purpose/mismatched proofs cannot set
  credentials. Replay and a second valid sibling proof cannot change the completed password.
- Existing verified/disabled users cannot acquire a replacement password through registration.
- Deleted-identity reuse follows current policy, uses a new subject/binding and rejects old proofs.
- Rate-limit admission, no-token responses and safe error handling survive the contract change.

### Storage Contracts And Races

Run meaningful completion contracts against memory and disposable DynamoDB Local:

- Confirm all six successful state changes and physical verification TTL using exact-key reads.
- Race the same proof with different passwords and two sibling proofs for the same pending user.
  Assert one credential winner, no orphan accounts/indexes and no loser password login.
- Gate proof issuance against completion; it must not issue against an already-replaced snapshot.
- Gate completion against a changed pending record/identity tombstone. Assert no stale activation;
  exercise deletion/re-registration so an earlier instance's proof cannot activate a later one.
- Expire a prepared proof before finish using a controlled clock, not sleeps or TTL deletion.
- Distinguish confirmed cancellation, pre-dispatch failure and a lost response after an actual
  commit. Assert stored outcomes instead of inferring rollback from the returned error.

Reuse actual conditional-write semantics. Do not point tests at deployed tables, live identities or
real email. Missing DynamoDB Local must be reported as an outstanding verification gap, not a pass.

### Web Contract

Assert email-only registration forwarding; no upstream calls on verification GET/HEAD; deliberate
POST forwarding; confirmation and origin rejection before upstream calls; appropriate no-store,
referrer and form boundaries; escaped proof fields; no password echo or session creation. Keep
tests focused on security/HTTP behavior rather than presentation or exact copy.

### Commands And Evidence

Before implementation, run the relevant current auth and web baseline and record existing failures.
During implementation use focused targets, then verify the affected surface:

```sh
cargo fmt --check --manifest-path packages/functions/auth/Cargo.toml
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm --workspace @irongate/example-web test
npm run typecheck
npm run test:infra
npm run test:setup
git diff --check
```

Using the existing trusted local emulator instructions, also run:

```sh
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
```

Report actual results, existing warnings, skipped checks and remaining limitations. Reference-repo
test results are not verification of this port. Real browser/email/deployment acceptance requires
separate permission and is not a completion claim for this local implementation slice.

## Acceptance Criteria

- Registration cannot store an anonymous caller's password before email ownership proof.
- Proof consumption, verified credential, account and indexes are one conditional transaction.
- Proofs bind to one pending instance; races, reuse and deleted-record recreation cannot replace
  credentials or resurrect a previous subject.
- The optional web caller uses email-first registration and non-mutating verification GETs.
- Password login/reset, provider flows, generation invalidation and existing rate limits retain
  their contracts; the core remains API-only and independent of optional hosting.
- No legacy registration/activation path remains solely for compatibility or test convenience.
- Changed files have current design mirrors; setup instructions describe the new contract.
- Local verification, including disposable DynamoDB completion/race checks, is recorded honestly.
- No reference/product code, credentials, infrastructure migration or deployment is imported.

## Implementation Results

- Registration accepts email only. Pending users have a fresh registration binding and no password
  hash or subject; resends preserve that binding and delivery failures do not discard pending state.
- Verification validates the submitted password and proof before hashing. The prepared snapshot
  commits proof consumption, credential, generation-zero account, identity and both indexes in one
  conditional transaction. Superseded consume/activate and password-first fixture helpers are gone.
- Missing proof bindings are invalid proofs, not invented IDs or server outages. Missing pending
  bindings, wrong purposes, expiry, record replacement, replay and sibling proofs cannot activate
  credentials. Operational and uncertain-write errors retain their separate failure semantics.
- The existing Worker renders verification links without mutation and completes only on a
  same-origin POST. It checks confirmation, bounds/escapes proofs, uses strict-origin/no-store,
  and leaves sign-in separate. Native code and infrastructure are unchanged.
- Exact source/test mirrors, topic overviews and setup/smoke instructions describe the current
  contract. Old pending rows require fresh dev state or separately reviewed cleanup before a
  deployment; the implementation does not migrate or delete them automatically.

### Verification Evidence

The pre-change auth suite and 15-test web suite passed. During implementation, two rate-limit
fixtures still sent the removed registration field and failed at JSON extraction; those payloads
were corrected without weakening rate-limit assertions. A later full run required loopback socket
permission for its local HTTP stub; rerunning with that permission passed.

Final local verification:

| Check | Result |
| --- | --- |
| Auth suite, including eight shared in-memory registration contracts | 289 passed; 38 Local-only cases ignored in this command |
| Full disposable DynamoDB Local suite | 38 passed, including nine registration cases |
| Web TypeScript build and contract suite | 62 passed |
| Admin `cargo check --locked` | Passed |
| Auth Clippy, all targets | Passed with existing repository warnings |
| Root TypeScript, infra and setup checks | Passed |
| Changed Rust files, targeted rustfmt check | Passed |
| Whole-crate `cargo fmt --check` | Existing formatting differences remain in untouched files |
| Relative documentation links, file mirrors, source-size limit and diff whitespace | Passed |

The Local run used the existing emulator with Homebrew Java 17, an explicit loopback endpoint,
dummy credentials and fixture-owned disposable tables. It exercised actual conditional writes and
transactions, physical proof TTL, no-orphan competing completions, stale tombstones/registration
instances, expiry and pre-dispatch versus lost-response outcomes. The emulator was stopped afterward.

No live email, browser/device, AWS or Cloudflare acceptance was run. These local results do not
certify production IAM, delivery behavior, every possible race or real network failure handling.
