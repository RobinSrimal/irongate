# 13_atomic_google_email_confirmation_store

## Status

Implemented and locally verified on 2026-11-06, following the user's implementation request.

Built on `880d6f7` (Google authoritative-email account resolution). This delivers the store boundary
only; public Google callback behavior is unchanged. No deployment, hosted auth UI, cloud
reconfiguration or reference-repository changes were made.

## Goal

Implement the typed storage boundary for fresh Google email confirmation, without exposing a new
public flow yet:

```text
validated Google identity + candidate email + consumed OAuth session + browser binding
  -> short-lived pending confirmation, no account/identity/code writes
matching browser proof + email proof + unchanged captured account/ownership state
  -> consume confirmation and issue the identity-bound authorization code atomically
```

The confirmation must survive separate Lambda invocations without adopting a newer account,
credential generation or email owner while the user waits. A proof is not consumed separately from
the guarded issuance it authorizes.

## Why This Slice

Slice 12 supports direct account resolution for verified Gmail and signed Google Workspace email
authority. It deliberately rejects new Google identities whose email needs fresh mailbox proof.
The reference handles these users through browser-bound email confirmation, but that includes
cookies, delivery, rate limits and a hosted form as well as storage.

This slice isolates the durable proof/transaction boundary first. Its completion is a backend
contract, not a claim that third-party Google email sign-in works end to end. Public callbacks retain
slice 12 behavior. Choosing a confirmation presentation/transport must not silently change the
API-only core or make the Cloudflare example mandatory.

## Scope Decision

In scope:

- A Google-specific candidate type that is distinct from verified email authority.
- One typed short-lived confirmation record in the existing AuthTable.
- Purpose-separated request, browser and email-proof lookup digests.
- Creation-time identity/account/ownership snapshots guarded against concurrent changes.
- Opaque prepare/finish operations with proof consumption and code issuance in one transaction.
- Reuse of the existing ownership/issuance rules, with narrowly scoped internal extraction as needed.
- Shared memory/DynamoDB Local rejection, expiry, race and uncertain-outcome contracts.
- Exact design mirrors and accurate current capability/operations documentation.

Out of scope:

- Google callback integration, new HTTP routes, browser cookies or frontend forms.
- Sending emails, new templates, retry/resend UX, delivery receipts or delivery cleanup APIs.
- A generic passwordless/OTP provider, account merge or provider-first password enrollment.
- Hosted verification/reset pages, domains, DNS, certificates or hosting changes.
- New services, tables, indexes, IAM permissions, dependencies or stage/secret configuration.
- Native sign-in, native session work, Apple grants and deletion orchestration.
- Compatibility aliases, fallback schemas, migrations, backfills or speculative extension hooks.
- Live provider/email tests, cloud deployment or reference-repository changes.

The storage methods must not be mounted on an endpoint in this slice. In particular, a short numeric
email proof cannot be exposed without independent source/account attempt limits and the reviewed
browser-binding/CSRF flow. Storage proof comparison alone is not online brute-force protection.

## Reviewed Reference

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant pinned files:

- `packages/functions/auth/src/store/sign_in_confirmation.rs`: request-bound proof comparison,
  captured account state and conditional proof deletion alongside code issuance.
- `packages/functions/auth/src/store/shared_identity.rs`: identity-first ownership planning and
  account/generation pinning.
- `packages/functions/auth/src/api/providers/email_confirmation.rs`: browser binding, request-bound
  email-code HMAC and session-capped expiry. Its routes/forms/delivery are not part of this slice.
- `packages/functions/auth/src/api/providers/google.rs`: candidate selection only after full provider
  validation and distinct handling of existing identities.

Reference HEAD when inspected was `1985d096506c782fde36666361a3a047fbe4bcc8`, with a clean worktree.
Recheck status/HEAD before implementation; retain the pinned comparison. Do not copy its retry loop,
optional generations, product branding or hosted route surface. Irongate retains one application
transaction attempt and its stronger private-snapshot boundary.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Identity model](../functions/auth/core/identities.md) and
  [account lifecycle](../functions/auth/core/account-lifecycle.md).
- [Google API](../functions/auth/api/providers/google.md) and
  [Google provider](../functions/auth/src/providers/google.md).
- [Provider email authority](../functions/auth/src/providers/verified_email.md).
- [Provider issuance](../functions/auth/src/store/provider_issuance.md),
  [email ownership](../functions/auth/src/store/email_ownership.md) and
  [credential guards](../functions/auth/src/store/credential_generation.md).
- [Records](../functions/auth/src/store/records.md), [keys](../functions/auth/store/keys.md),
  [HMAC lookups](../functions/auth/src/crypto/hmac_lookup.md) and
  [DynamoDB](../functions/auth/store/dynamodb.md).
- [Google setup](../../docs/setup/07-google-login.md) and
  [test guide](../../packages/functions/auth/tests/README.md).

Keep the auth/admin boundary, current public routes, optional examples and JWT revocation semantics
unchanged. Authoritative docs describe what actually ships, including the lack of a public
confirmation flow; they must not advertise the full reference behavior as implemented.

## Candidate And Proof Boundary

Introduce a small opaque `GoogleEmailCandidate` constructed from already-validated typed Google
claims and the HMAC lookup secret. The production caller remains responsible for full signature,
issuer, audience, time and transaction-nonce validation before construction.

The candidate binds Google's issuer/subject digest to a valid normalized email and its Email HMAC.
It is appropriate only when direct Google email authority is absent. A missing/malformed address
cannot become a candidate. A signed but unverified email can be a candidate: the subsequent mailbox
proof, not Google's verification flag, would establish current ownership.

The candidate is not `ProviderVerifiedEmail`, has no public raw-email/JSON constructor, and cannot
be passed to ordinary provider issuance as verified authority. Preserve literal normalization:
do not strip aliases, collapse dots, infer hosted domains or reinterpret Apple relay addresses.

Use distinct HMAC lookup families for:

- The high-entropy confirmation request handle.
- The high-entropy browser binding.
- The email proof, including its confirmation-request digest in the HMAC input.

Reuse the established Email and GoogleIdentity families. Bind an email-code digest to exactly one
request using an unambiguous encoding; equal codes for different requests must not yield the same
digest. Raw request/browser values and email codes never enter stored keys, record values, logs or
errors. The pending normalized address is permitted only as short-lived record data, not authority
by itself. There is no new signing/encryption key or secret.

The reference uses an eight-digit code. This slice may use such codes in fixtures to prove the
request binding, but does not introduce a public code generator, passwordless endpoint or retry UX.

## Durable Record

Use one new key family, for example:

```text
pk = oauth:email_confirmation
sk = HMAC(confirmation-request-handle)
```

The record must contain a single required current shape:

```text
purpose = google_sign_in_confirmation
identity_digest
normalized email and email_digest
browser_digest
email-proof digest
original authorize-session snapshot
typed new-identity resolution snapshot
created_at
expires_at
```

The resolution snapshot contains the generated candidate account or exact existing active account
(including required credential generation), the absent/exact eligible Google identity, and the
absent/exact eligible email owner and required subject-index bindings. It must preserve both
selected values and their write/check conditions across invocations. Persist typed data, not an
arbitrary serialized `TransactOperation` list or caller-selected table keys.

Private fields or opaque store-owned types prevent route code from replacing snapshots. Do not
make `PreparedProviderIssuance` a public deserializable authority that a caller can fabricate.
Required snapshot fields have no compatibility defaults. Invalid purpose, inconsistent email or
provider/key binding, malformed snapshots and missing generations fail closed.

Set `expires_at = min(now + requested_confirmation_lifetime, session.expires_at)` with checked
arithmetic and a positive remaining lifetime. This store-only operation accepts a bounded lifetime;
it adds no environment variable in this slice. The original session expiry is never extended.
Write matching DynamoDB `expiry` seconds and enforce both record and session expiry at runtime.
Use one controllable issuance/confirmation clock for deterministic tests, not sleeps or TTL cleanup.

## Store Operations

Names may follow the existing prepare/finish conventions, but keep these boundaries:

### Create Pending Confirmation

- Accept the opaque candidate, consumed Google authorize-session snapshot, proof digests, lifetime
  and existing identity-reuse policy. Do not accept a chosen subject/account or raw stored record.
- Validate selected provider, nonempty/valid digest inputs, logical expiry and candidate binding.
- Read the Google identity first. An active identity does not need this flow: reject creation rather
  than reselecting its account by email. Ordinary callbacks continue to resolve it by issuer/subject.
- For an absent or eligible deleted identity, capture the same ownership/account conditions used by
  new provider issuance. Respect identity and owner retention independently; malformed or inactive
  owner state does not authorize an alternative subject.
- Capture a fresh candidate subject when ownership is available. This is only a plan; pending
  confirmation must not reserve the email, write an account/index or create an authorization code.
- Insert the confirmation conditioned on absence, with exact/absence checks for its captured
  identity, owner, account and relevant indexes in the same transaction. This prevents a reset or
  ownership change between reads and record creation from authorizing a stale challenge.
- A repeated handle cannot overwrite another challenge, extend its expiry or replace its proofs.

Read-only planning before email proof is acceptable; durable identity/account/ownership mutation
before that proof is not. Do not expose an unchecked public provider-issuance method to obtain a plan.

### Prepare Completion

- Load by confirmation request HMAC through a strongly consistent exact-key read.
- Check purpose, required bindings, record expiry and original session expiry.
- Compare the supplied browser and request-bound email-proof digests in constant time. Neither
  proof alone authorizes completion. Lookup by handle alone cannot return a usable issuance plan.
- Return an opaque private prepared completion with the exact stored record and captured resolution.
  Do not accept a caller-provided record snapshot, email, client, redirect, subject or generation.
- Unknown/expired requests and invalid proofs make no writes and expose no owner/account status.
  Preparing twice does not consume anything; only one subsequent finish can win.

This operation proves possession of a binding value. It does not itself prove that a browser cookie
was established at OAuth start, enforce HTTP Origin or rate-limit submissions; those are deliberately
not exposed or claimed in this slice.

### Finish Completion

- Consume the opaque prepared value. Derive code claims from its captured session and account.
  Preserve client, exact redirect, scope, PKCE challenge/method, state, OIDC nonce and generation.
- Use the same typed ownership and identity rules as ordinary issuance. A small internal plan/code
  builder may be shared; do not create a second resolver or general transaction/plugin framework.
- In one transaction, perform the six new-identity issuance operations and delete the exact pending
  confirmation. Each physical item occurs once. Proof consumption must not precede code issuance.
- Check the captured active account/generation, owner/index values and identity absence/tombstone,
  even if newer state would permit a different valid sign-in. Do not re-resolve after waiting.
- Recheck both expiries immediately before dispatch. Authorization-code expiry starts at finish
  using the configured code lifetime; waiting never extends the original session or confirmation.
- Successful code metadata contains normalized email and `email_verified=true` based on fresh proof.
  It contains no confirmation/browser/email-proof digest, Google token, raw proof or `hd` claim.
- Use one application transaction attempt. A confirmed condition failure leaves the proof and all
  other candidate writes unchanged; a request must not adopt a newer subject or generation to retry.
- Keep conditional rejection distinct from conflicts, operational failures and uncertain outcomes.
  A lost response can follow a committed proof deletion and usable code; never promise rollback or
  compensate by deleting identity/account state.

If another flow establishes the Google identity while the confirmation is pending, the captured
absence/tombstone condition rejects. A fresh ordinary sign-in can resolve that identity; completion
must not bypass mailbox proof through a newly discovered returning-identity branch.

Pending records are not a new account-deletion index. They expire through runtime checks and TTL
cleanup, which is asynchronous. Captured conditions block completion after relevant deletion or
recovery changes; do not claim immediate physical removal of pending email data or whole-deletion
atomicity. Exact account conditions also do not detect disable/enable history that restores the
same value. No lifecycle epoch or immediate access-JWT revocation promise is introduced.

## Expected Change Surface

Primary source modules:

```text
packages/functions/auth/src/providers/verified_email.rs
packages/functions/auth/src/crypto/hmac_lookup.rs
packages/functions/auth/src/store/sign_in_confirmation.rs  (new)
packages/functions/auth/src/store/provider_issuance.rs
packages/functions/auth/src/store/email_ownership.rs
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/keys.rs
packages/functions/auth/src/store/mod.rs
```

Add a focused `tests/sign_in_confirmation.rs` runner, shared support contracts and a
`tests/dynamodb_storage/sign_in_confirmation.rs` child wired into the existing Local runner.
Reuse the current transaction gates, controlled clocks and real registration/provider fixtures.
Split tests by responsibility before any source file reaches 1,000 lines; do not duplicate whole
provider suites or add tests that merely count constructed operations.

Every touched/new source, test and test README gets its exact design mirror. Update ownership,
identity, key/record, lifecycle and threat-model overviews only where the new internal boundary
changes their description. Keep `design/overview.md`, Google setup and smoke-test docs explicit
that the public non-authoritative-email flow still rejects. Do not add operator setup steps for an
unmounted feature or pre-create future slice documents.

## Verification

Focused contracts must prove:

- Creating a confirmation writes no account, identity, owner/index, credential or code and cannot
  overwrite an existing handle. Physical TTL matches logical expiry; no raw proof appears in data.
- Confirmation survives constructing a second AuthStore instance, with the same captured subject,
  session and generation. No in-memory-only handoff or public mutable plan is required.
- Fresh available email and existing password/Apple/Google-owned email complete into the intended
  subject. Code exchange still enforces PKCE, client/redirect and active generation.
- Missing, malformed, expired, wrong-purpose, wrong-browser and wrong-email proofs reject without
  consuming the challenge. The same email code or browser proof from another request cannot replace
  the expected pair. Test each proof independently with all other inputs valid.
- Proof replay, two prepared completions and a real existing-code collision allow at most one commit
  and leave no partial loser account/index/identity/code or prematurely consumed confirmation.
- Every completion condition can independently fail through the actual backend, preserving other
  records and TTL. Inspect physical items in DynamoDB Local, not only logical filtered reads.
- Real reset, disable/delete, identity reuse and owner/index changes both during creation and while
  waiting invalidate stale captured plans. Tests after reset must not simply start a new challenge
  at the new generation and call that invalidation coverage.
- Two confirmations and password/provider ownership claims race in both winner orders. A loser
  cannot adopt the winner's subject, install a pending password or revive a deleted account.
- Expiry between prepare and finish, exact expiry boundaries and a shorter original session all
  reject without depending on DynamoDB's deletion timing.
- Pre-dispatch failure differs from committed-response loss. Conflicts/outages remain operational,
  diagnostics are secret-safe, and no automatic plan refresh or compensation occurs.
- Existing signed Google/Apple callbacks and direct-authority issuance retain slice 12 behavior.
  New third-party-email callbacks still reject; no public route starts accepting these proofs yet.

Run the focused baseline before edits and then the new shared contracts plus regressions:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test provider_issuance --test oidc_google_callback --test oidc_apple_callback
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Use the documented disposable loopback-only emulator, dummy credentials and unique tables; stop
the emulator afterward. No deployed tables or live accounts. Check touched Rust formatting, source
sizes and exact design mirrors/relative links. Record actual results, failures and skipped checks.
No frontend/native build, UI testing or cloud deployment is needed for this store-only slice.

## Acceptance Criteria

- Confirmation requests are durable, purpose-separated, session-limited and browser/email-proof-bound.
- Pending confirmation is not email ownership, a usable credential or verified provider authority.
- Account/generation/ownership selection is pinned before the wait and cannot be replaced at finish.
- One exact-record deletion commits with all identity/ownership/code work, with no partial outcome
  on confirmed conditional rejection and no rollback claim after uncertain responses.
- Memory and DynamoDB Local contracts establish replay, race, expiry and real condition behavior.
- Existing direct provider paths remain valid and the API-only public behavior is unchanged.
- Exact design mirrors and current capability limits are updated in the implementation.
- No compatibility, deployment, hosted UI, new infrastructure or product-specific code is introduced.

## Implementation Result

- Added opaque Google email candidates, purpose-separated proof digests and typed confirmation
  create/prepare/finish operations. Pending state is durable but reserves no account or email.
- Extracted the private new-provider plan and shared code builder. Direct Google/Apple issuance
  and confirmation use the same ownership conditions, without a public unchecked resolver.
- Persisted the exact account/generation/identity/ownership selection. Completion checks both proofs
  and deletes the exact pending record in the seven-operation issuance transaction. No newer state
  is adopted after the wait or on conditional failure.
- Kept the confirmation record private in `store/sign_in_confirmation.rs`; `records.rs` needed no
  change. The additional internal extraction is `store/provider_plan.rs`.
- Added shared memory/Local contracts and a real `/token` exchange check. Updated all exact file
  mirrors, relevant current-design overviews, the test guide and public capability limits in setup
  and smoke documentation. No compatibility reader, public route or configuration was added.
- Compared against the pinned reference files listed above. Reference HEAD remained
  `1985d096506c782fde36666361a3a047fbe4bcc8` with a clean worktree.

## Verification Result

| Check | Result |
| --- | --- |
| Focused pre-edit provider/callback baseline | 54 passed |
| Direct provider/callback regressions after extraction | 54 passed |
| Full locked auth tests | 338 passed; 76 explicitly ignored Local cases |
| Full DynamoDB Local target with ignored cases enabled | All 76 passed, including nine confirmation cases |
| Locked auth and admin checks | Passed |
| Auth all-targets Clippy | Passed with existing warnings; warning sites match the previous slice, allowing shifted lines |
| `npm run test:infra` | Passed, including store boundary and resolved logging checks |
| Touched Rust formatting, source-size, mirror and relative-link checks | Passed; all touched source files below 1,000 lines |
| `git diff --check` | Passed |

An initial new-test compilation error referenced an unsupported transaction variant; it was fixed
before the passing runs. A one-off documentation-check command initially included an empty path;
the corrected check passed. No remaining test failure is being treated as a pass.

Local tests used dummy credentials and disposable unique tables on a verified loopback-only
in-memory emulator. That process was stopped after validation and its listener is absent. No live
Google/email test, frontend/native build, UI test, cloud deployment or deployed-table test was run.
TypeScript/setup checks were not rerun because no TypeScript, manifests or setup implementation
changed. This verification does not certify public confirmation cookies, CSRF, online attempt limits
or delivery: none of those surfaces is exposed by this slice.
