# 09_atomic_provider_identity_and_code_issuance

## Status

Implemented and locally verified on 2026-11-05 following the user's implementation request.

Builds on `b6c2d85` (email-first password registration); this definition was the only pending file
at implementation start. No deployment, cloud reconfiguration, reference-repository edit or
deployed-data cleanup was performed. Authoritative design mirrors and operator docs are updated.

## Goal

Commit Google/Apple identity resolution and first-party authorization-code issuance together:

```text
validated provider response + consumed authorize session
  -> prepare identity/account snapshot
  -> one conditional transaction
  -> redirect with authorization code
```

For a new identity, the account, identity, reverse index and code must either all be written or
none be written. For a returning identity, its metadata update and code must share the same
identity/account guards. Preserve the existing issuer/subject identity model and separate accounts
for different sign-in methods, even when their email addresses match.

## Why This Slice

Slice 08 closed the password-registration transaction boundary. Before this slice, callbacks called
`resolve_or_create_google_identity` / `resolve_or_create_apple_identity`, reread the active account,
then called `create_authorization_code` separately.

That left these concrete gaps:

- Account/identity creation or metadata changes can commit even if code issuance fails.
- The final code write guards the account, but not the exact provider identity that was resolved.
- A later account read can adopt a newer credential generation instead of retaining the first
  resolved account snapshot.
- The consumed authorize session can expire during provider exchange/validation without a fresh
  expiry check at the final issuance boundary.
- Identity-resolution storage outages are currently converted to `invalid_grant` in both callbacks.

This is the small transactional prerequisite for the handoff's broader shared-account resolver.
It does not implement email ownership or account linking. Combining those policies, confirmation
flows and lifecycle indexes here would make the change substantially larger.

## Reviewed Reference

Read-only source repository: `/Users/robinsrimal/Desktop/64x64`.
Pin comparison to `4e7207887f86aacba19d1bcf7ffea312055d5620`, as specified by the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant reference files at that commit:

- `packages/functions/auth/src/store/shared_identity.rs`: `identity_plan`, `pin_account`,
  `code_operation` and transactional provider issuance.
- `packages/functions/auth/src/store/email_ownership.rs`: account guard and ownership transaction
  composition, for understanding dependencies rather than copying its email index.
- `packages/functions/auth/src/api/providers/email_confirmation.rs`: separate confirmation-flow
  dependencies that are deliberately not included here.

At definition time the reference worktree is clean and HEAD is
`ed881e143075eb5a0337a9bd6b687dfe0eb91060`. Newer changes are not part of the pinned comparison.
Recheck status/HEAD before implementation; do not modify that repository.

Adapt the transactional principle, not the entire reference resolver. Its email matching, retry
planner, Apple grants, confirmation pages and product-specific wiring are not prerequisites here.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Identity storage](../functions/auth/store/identities.md),
  [accounts](../functions/auth/store/accounts.md) and
  [lifecycle](../functions/auth/core/account-lifecycle.md).
- [Typed store](../functions/auth/src/store/mod.md),
  [record shapes](../functions/auth/src/store/records.md) and
  [credential generation](../functions/auth/src/store/credential_generation.md).
- [Authorization-code operations](../functions/auth/src/store/authorization_codes.md),
  [authorize sessions](../functions/auth/src/store/authorize_sessions.md) and
  [provider state](../functions/auth/src/store/provider_states.md).
- [Google callback](../functions/auth/src/api/providers/google.md) and
  [Apple callback](../functions/auth/src/api/providers/apple.md).
- [DynamoDB adapter](../functions/auth/src/storage/dynamo.md) and
  [test instructions](../../packages/functions/auth/tests/README.md).
- [Google setup](../../docs/setup/07-google-login.md),
  [Apple setup](../../docs/setup/08-apple-login.md) and
  [smoke checks](../../docs/operations/smoke-test.md).

## Scope

In scope:

- One purpose-specific typed provider-issuance operation, used by both existing callbacks.
- Atomic new identity/account/index/code writes and returning-identity/code updates.
- Exact identity and active-account/generation conditions, including deleted-identity reuse.
- Authorize-session expiry checks at preparation and immediately before transaction dispatch.
- Safe distinction between confirmed stale-state rejection and operational/uncertain failures.
- Focused route tests and shared memory/DynamoDB Local transaction contracts.
- Removal of superseded provider-specific resolve-then-issue methods and updates to their callers.
- Exact design mirrors and relevant current-behavior/setup documentation.

Out of scope:

- Shared email ownership, automatic linking, account merging or provider-email confirmation.
- Hosted auth/email-action pages, new routes or changed example hosting.
- Native provider APIs, mobile bridges, SDKs or client UI work.
- Apple grant persistence/revocation or account-deletion orchestration changes.
- Changes to JWT validation, JWKS cache policy, rate limits, OAuth client config or token exchange.
- New record fields, key families, dependencies, IAM permissions, services or migrations.
- Deployment, live provider/email testing or writes to deployed tables.

The simpler alternative is to add an identity guard to the existing code write. That would still
leave new account creation and identity updates committed separately. One transaction closes that
boundary without importing the reference project's broader account architecture.

## Typed Store Contract

Implement in `packages/functions/auth/src/store/provider_issuance.rs`, keeping generic storage
operations inside the store. A prepared snapshot with private fields, followed by a finishing
method, should follow the existing registration/recovery pattern. Names may follow local idioms.

Preparation must:

1. Accept only Google or Apple for this operation, never password or arbitrary provider strings.
2. Receive the HMAC identity digest and the current allowlisted properties only after the callback
   validates the provider token. The store does not perform OIDC validation itself.
3. Receive the consumed authorize-session snapshot and configured code TTL, not an independently
   supplied subject, account generation, client ID, redirect URI or scope.
4. Require the session's selected provider to match and its logical expiry to be in the future.
5. Read the identity consistently, validate its provider/digest binding and classify it as absent,
   active or an eligible deleted tombstone.
6. For an active identity, resolve its existing subject and read the matching active account once.
   Retain that exact identity/account snapshot, including its generation. Missing/inactive account
   or malformed binding fails closed; never fall back to creating another account.
7. For absent or reusable deleted identities, generate a fresh subject with generation zero.
   Apply the existing deleted-identity reuse policy, including its required retention metadata.

Preparation writes nothing. Private prepared state must prevent callers from replacing a checked
subject, generation or identity snapshot. Keep any transaction plan internal to the typed store.

Finishing rechecks the session expiry immediately before dispatch and creates the authorization
code from the prepared state. Use the existing code record fields, configured TTL, HMAC code key,
logical expiry and physical DynamoDB `expiry`. Preserve client ID, exact redirect URI, scope,
downstream PKCE, client state handling and OIDC nonce.

The route generates a high-entropy raw code and passes only its HMAC digest to storage. Return
the raw code only through the normal OAuth redirect after confirmed success. Do not log it or
include it in store errors. Existing identity properties and code properties remain distinct:
retain their current allowlists rather than putting all profile metadata into the code.

### New Or Reusable Deleted Identity

One transaction contains:

| Operation | Condition |
| --- | --- |
| Create active account with fresh subject and generation zero | Account does not exist |
| Create identity or replace eligible tombstone | Identity absent or exactly equals prepared tombstone |
| Create subject-to-identity index | Index does not exist |
| Create authorization code | Code does not exist |

The three persistent records have no TTL. A code collision, competing identity claim, changed
tombstone or pre-existing account/index must reject the whole transaction without overwriting any
winner or leaving an orphan account/index. Deleted reuse never revives the previous subject.

### Returning Active Identity

One transaction contains:

| Operation | Condition |
| --- | --- |
| Check active account | Complete account value equals prepared snapshot |
| Update identity last-seen and existing allowlisted properties | Complete identity value equals prepared snapshot |
| Create authorization code | Code does not exist |

The subject and identity key cannot move because later claims have changed or omitted email.
Do not use email, display name or unsigned callback data as identity authority. Preserve current
metadata behavior without adding email ownership, Gmail normalization or private-relay inference.

Use the existing exact-value condition and account-guard helpers. Do not issue a separate condition
check and write for the same item within a DynamoDB transaction.

### Conflicts, Expiry And Uncertain Outcomes

- Use one application-level transaction attempt. Confirmed conditional rejection requires a fresh
  sign-in; do not silently re-resolve to another subject/generation or retry the upstream exchange.
- Concurrent legitimate callbacks may conflict; success for both is not required. No retry planner
  or compatibility wrapper is needed to preserve the current client contract.
- Preserve SDK/storage error classification. An operational failure, transaction conflict or lost
  response must not be reclassified as invalid credentials or a guaranteed rollback.
- Confirmed stale state returns a fixed `invalid_grant` response. Operational/uncertain failures
  return a fixed server-error response, without raw provider values, record data or SDK text.
- Check expiry with a controllable clock; do not depend on DynamoDB TTL deletion or sleeps. A
  pre-dispatch clock check is not a promise that server processing cannot cross the expiry instant.
- Do not add compensation that deletes an account after a failed response. The transaction may
  already have committed. Unreturned short-lived codes expire normally.

Provider state and authorize-session consumption remain where they are today, before exchange and
validation. This slice does not make external provider exchange, those consumes and code issuance
one transaction. A failed callback can leave them consumed and requires restarting sign-in.

The account condition rejects a state/generation that differs at commit. It is not a history marker
for disable-then-enable changes that restore the same value. Do not claim new lifecycle-epoch or
instantaneous JWT revocation guarantees. If issuance wins a race, existing token-exchange guards
still reject an inactive/stale account later; already-issued JWT semantics remain unchanged.

## Callback Cutover

Keep `GET /google/callback` and `POST /apple/callback`, their upstream token/JWKS checks, state/nonce,
PKCE and exact client redirects. After successful provider validation, each callback uses the new
typed preparation/finish path instead of separate identity resolution, account reread and code
creation. Neither callback can choose a new account generation after preparation.

Keep the API-only response contract and no-email-linking behavior. Remove the superseded
provider-specific resolve methods once callers and tests move. Keep unrelated lifecycle helpers
that still serve existing behavior; do not turn this into a wholesale store/fixture rewrite.

## Expected Change Surface

Primary runtime files:

```text
packages/functions/auth/src/store/provider_issuance.rs  (new)
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/api/providers/apple.rs
```

Use `credential_generation.rs`, `authorization_codes.rs`, `keys.rs` and `records.rs` as existing
contracts. Small helper extraction is allowed only where it prevents actual duplication; avoid
new generic abstractions or unrelated module moves. No runtime schema change is planned.

Tests should extend the existing provider callback suites and add shared store contracts:

```text
packages/functions/auth/tests/provider_issuance.rs
packages/functions/auth/tests/support/provider_issuance.rs
packages/functions/auth/tests/dynamodb_storage/provider_issuance.rs
packages/functions/auth/tests/dynamodb_storage.rs
packages/functions/auth/tests/oidc_google_callback.rs
packages/functions/auth/tests/oidc_apple_callback.rs
```

Reuse `support/transaction_gate.rs`, `support/dynamo.rs` and existing signed provider fixtures.
Split callback-specific cases into child modules before either existing 731/777-line callback
test file approaches 1,000 lines. Do not duplicate the shared contracts for each backend/provider.

## Verification Contracts

### Store And DynamoDB Local

Run the same meaningful contracts against the memory backend and disposable DynamoDB Local:

- New and returning identities produce usable codes with the expected subject, generation and
  client bindings; different providers with matching email remain separate accounts.
- Each new-identity write condition independently rejects all other writes. Check exact candidate
  account/index/code keys for orphans, not just a returned error or aggregate record count.
- Stale active identity/account, missing account, inactive account and generation change cannot
  issue a code or update provider metadata. Exercise concurrent disable/delete through lifecycle
  operations and generation replacement through actual conditional storage semantics.
- Same-identity first sign-ins and returning callbacks prepared from the same snapshot cannot
  overwrite one another's claims. Test both commit orders with deterministic transaction gates.
- Changed/deleted identity after preparation rejects stale issuance. Reusable tombstone races
  cannot replace a winner or recover the old subject. Cover never/immediate/retention policy.
- Code-key collision leaves prior identity metadata unchanged and creates no orphan account.
- Session/provider mismatch and expiry before preparation or before dispatch cause no issuance
  writes. Use a controlled clock; do not rely on scheduling, sleeps or TTL cleanup.
- A failure before dispatch leaves the planned writes absent. A lost response after a real commit
  returns an operational error while the complete committed state remains intact.
- Local raw-item reads verify code TTL matches logical expiry and persistent records have no TTL.

Assertions must inspect storage outcomes and usable/rejected codes. Keep fake conditional-write
semantics aligned with DynamoDB; injected faults supplement, not replace, real transactions.

### HTTP Callbacks

- Both signed-provider success paths return the registered redirect with `code` and original
  `state`, with no access/refresh/ID tokens in the callback response.
- Existing signed JWT rejection and bounded JWKS-refresh tests remain effective; invalid provider
  proofs create no account/identity/code. Do not replace independent rejection cases with mocks.
- New store conditional rejection and operational failure produce distinct, sanitized HTTP errors
  for both callbacks and never a successful code redirect.
- Existing identities retain their subject when signed email is changed/missing; equal emails
  across distinct provider identities do not merge subjects. Apple unsigned profile fields do
  not influence identity or authorization-code claims.
- Replayed state/session is rejected. Document that earlier consumption is not rolled back after
  provider or storage failure.

## Documentation Work

Create `design/functions/auth/src/store/provider_issuance.md` and exact mirrors for every new test
file, preserving `src/`, `tests/` and child directories. Update mirrors for every touched source,
test or README in the same implementation.

Update the identity/code/lifecycle and provider API overviews, threat model and overall overview
where their transaction descriptions change. Remove stale behavior descriptions rather than
appending decision history. Keep the absence of email linking explicit.

Update provider setup and smoke instructions only where they describe the changed callback outcome
or recovery from a failed sign-in. There are no new secrets, setup values, hosting requirements or
deployment commands. Do not describe this slice as completing the whole handoff.

## Work And Validation

1. Recheck repository/reference status and manifests; run the affected current callback baseline.
2. Add typed preparation/atomic finishing and focused memory/Local contracts.
3. Cut both callbacks over and remove superseded provider-resolution entrypoints.
4. Consolidate their existing identity tests into the new observable issuance contract where useful.
5. Update exact design mirrors and affected current-behavior/setup docs.
6. Run affected checks, record actual results and review the final diff for scope and sensitive data.

Expected checks, with focused commands during development and the full auth suite at completion:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
cargo fmt --check --manifest-path packages/functions/auth/Cargo.toml
npm run typecheck
npm run test:infra
npm run test:setup
git diff --check
```

Run the shared transaction contracts on the existing disposable emulator:

```sh
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
```

Follow the test README's loopback-only/dummy-credential setup and fixture-owned table cleanup.
Never use a deployed table. Missing emulator availability is an outstanding verification gap.
Report pre-existing Clippy/whole-crate formatting findings separately and check changed files;
do not fix unrelated formatting. No example rebuild or live provider/device/UI test is required
for this backend-only change, and local tests must not be presented as live acceptance.

## Acceptance Criteria

- Each successful callback commits identity/account work and its authorization code together.
- Code issuance is conditioned on the originally prepared identity and active account generation.
- Returning identities keep stable subjects; no cross-provider/email matching is introduced.
- New/deleted identity races produce no orphan records, accidental resurrection or overwritten codes.
- Session expiry is checked again before dispatch; state/nonce/PKCE/JWT validation stays intact.
- Operational and uncertain outcomes remain distinct from confirmed conditional rejection.
- Memory and DynamoDB Local evidence covers actual conditions, failures and competing writes.
- Exact design mirrors and relevant setup docs describe implemented behavior and its limits.
- No infrastructure, deployment, example hosting or public protocol contract change is made.

## Implementation Results

- Added `store/provider_issuance.rs` with a private, non-cloneable preparation snapshot. Only
  Google/Apple are accepted. Preparation checks session/provider/expiry and persisted bindings,
  retaining the exact returning identity/account or a fresh generation-zero subject.
- New/reusable identities commit account, subject index, identity and code together. Returning
  identities commit their metadata update and code alongside an exact account guard. There is no
  account reread, application retry, cross-provider linking or schema change.
- Both callbacks use the same prepare/finish path and a small shared safe-error mapper. Confirmed
  stale-state/reuse rejection is `invalid_grant`; operational/conflict/uncertain failures are
  `server_error`. Raw store/provider values are not included in these error responses.
- Removed provider-specific resolve-then-issue methods. Existing provider identity tests were
  consolidated into shared observable issuance contracts; independent signed-token rejection,
  JWKS-refresh and code-to-token tests remain.
- Added controlled-clock expiry, exact-key rollback, competing writer, lifecycle/generation,
  tombstone, code-collision and lost-response coverage. HTTP tests also prove no metadata write
  occurs before the gated transaction and that signed email changes do not move subjects.
- Updated exact source/test mirrors, topic design, provider setup and smoke instructions. The
  reference comparison stayed pinned to `4e720788`; its worktree remained read-only.

### Verification Evidence

The pre-change Google and Apple callback suites each passed 15 tests. During integration a test
fixture used the wrong lookup-secret constructor; it was corrected before the final passing run.
Static review found no production issue and prompted stronger pre-transaction HTTP assertions.

| Check | Result |
| --- | --- |
| Full auth suite | 301 passed; 47 Local-only tests ignored by this command |
| Google callback suite | 17 passed |
| Apple callback suite | 17 passed |
| Shared memory provider contracts | 8 passed |
| Full disposable DynamoDB Local suite | 47 passed, including 9 provider cases |
| Admin `cargo check --locked` | Passed |
| Auth Clippy, all targets | Passed with existing repository warnings |
| Root TypeScript, infra and setup checks | Passed |
| Changed Rust files, targeted rustfmt check | Passed |
| Whole-crate `cargo fmt --check` | Existing differences in untouched discovery/config/crypto files |
| File mirrors, relative links, source-size limit and diff whitespace | Passed |

Local verification used the existing disposable-emulator workflow with an explicit loopback
endpoint, dummy credentials and fixture-owned tables. It checked physical TTLs and real DynamoDB
conditional writes/transactions, including both race orderings and complete-state preservation
after a simulated lost response. The emulator was stopped and port 18023 checked clear afterward.

The existing account value is a snapshot, not a lifecycle-history epoch. Earlier state/session
consumption and external exchange remain outside issuance; failed callbacks restart sign-in.
These results do not claim rollback after every error, immediate JWT revocation, shared-account
resolution or live provider/cloud acceptance. No web/native rebuild, UI/device test, live email,
AWS deployment or Cloudflare deployment was performed for this backend-only change.
