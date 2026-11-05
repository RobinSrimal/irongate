# 11_apple_verified_email_account_resolution

## Status

Implemented and locally verified on 2026-11-05.

Builds on `9555744` (proof-bound email ownership and deletion guards). No deployment, cloud
reconfiguration, reference-repository changes or data migration was performed.

## Goal

Use validated Apple email ownership to resolve a new Apple identity to one active account:

```text
existing Apple issuer/subject -> existing active account, regardless of later email changes
new Apple identity + verified email with active owner -> that owner's active account
new Apple identity + verified, available email -> fresh account and email ownership
```

Account resolution, Apple identity/index writes and authorization-code issuance commit together.
This allows password-first users to sign in through Apple with the same internal subject. It does
not merge existing accounts or complete the handoff's entire shared-account feature.

## Why This Slice

Slice 09 established private provider-issuance snapshots and atomic identity/code writes. Slice 10
established proof-bound email owners and indexed deletion. Apple can now consume those boundaries
without introducing a confirmation endpoint or another runtime.

Keep this Apple-only. Google needs its own authority checks and a fresh confirmation flow for
non-authoritative email claims. Password enrollment into an existing account also needs a separate
proof contract: current registration links bind a pending registration, not an existing account and
credential generation at issuance. Do not turn that proof into permission to attach a password.

## Reviewed Reference

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant reference files:

- `packages/functions/auth/src/store/shared_identity.rs`: verified-email construction, identity-first
  resolution, ownership-based account selection and transactional code issuance.
- `packages/functions/auth/src/store/email_ownership.rs`: active-owner resolution and conditional
  creation of a fresh account/owner/index.
- `packages/functions/auth/src/api/providers/apple.rs` and `src/providers/apple.rs`: validated Apple
  callback claims and provider identity binding.

At definition time reference HEAD is `1a54b13b2cce253a1e1e7d7fdb2a781d1ccb2de5` and its worktree is
clean. Recheck status and HEAD before implementation; compare the pinned commit, not newer product
changes. Adapt the relevant contracts, not the entire resolver or its retry planner. Preserve
Irongate's required credential generations, private prepared snapshots and anonymized tombstones.

## Design To Honor

- [Overview](../overview.md), [threat model](../functions/auth/threat-model.md) and
  [identity model](../functions/auth/core/identities.md).
- [Account lifecycle](../functions/auth/core/account-lifecycle.md),
  [identity storage](../functions/auth/store/identities.md) and
  [account storage](../functions/auth/store/accounts.md).
- [Provider issuance](../functions/auth/src/store/provider_issuance.md),
  [email ownership](../functions/auth/src/store/email_ownership.md),
  [registration](../functions/auth/src/store/registration.md) and
  [credential guards](../functions/auth/src/store/credential_generation.md).
- [Apple API](../functions/auth/api/providers/apple.md),
  [Apple domain](../functions/auth/providers/apple.md),
  [callback implementation](../functions/auth/src/api/providers/apple.md) and
  [provider implementation](../functions/auth/src/providers/apple.md).
- [Store keys](../functions/auth/store/keys.md),
  [Apple setup](../../docs/setup/08-apple-login.md) and
  [storage test instructions](../../packages/functions/auth/tests/README.md).

Update existing provider-separation statements only for the implemented Apple ownership path.
Preserve the API-only core, IAM admin isolation and optional-example boundary.

## Scope

In scope:

- Typed Apple email authority derived from validated signed ID-token claims.
- Identity-first Apple resolution using existing email-owner records for new/reusable identities.
- Exact owner/account/index guards in the existing atomic provider issuance boundary.
- Focused callback, storage, competing-claim and shared-account lifecycle tests.
- Exact design mirrors and relevant setup/operations documentation.

Out of scope:

- Google account resolution, Google confirmation, generic linking/unlinking APIs or account merging.
- Password enrollment into an existing provider account or changes to password proof semantics.
- New routes, hosted pages, native flows, SDKs, example UI or hosting changes.
- Apple grant persistence/revocation, deletion receipts, queues or lifecycle orchestration.
- New record schemas, tables, indexes, IAM permissions, secrets, configuration or dependencies.
- Backfills, compatibility paths, deployment, live provider login or deployed-table testing.

## Email Authority

Keep profile metadata separate from authority to select an account. Introduce a small opaque Apple
email proof/candidate with private fields, constructed in the production path only after the existing
Apple validator has verified signature, issuer, audience, time and provider nonce.

The proof must:

- Require a signed `email` and `email_verified == true`, using the existing Apple boolean decoder.
- Normalize and validate email through the existing email rules and compute `LookupFamily::Email`
  with the configured HMAC lookup secret.
- Bind the email digest to the validated Apple issuer/subject identity digest. The store must reject
  a candidate supplied for another identity or provider.
- Have no HTTP-deserializable or arbitrary-properties constructor. A helper receiving claims is
  not itself JWT verification; preserve and test the signed-token boundary in the callback.

Do not authorize linking from `properties: Value`, unsigned callback `user` JSON, display names,
request email, or the `is_private_email` flag. A verified private-relay address is its own address;
never infer its destination, strip aliases or rewrite Gmail dots. No raw email enters storage keys
or new audit/error fields.

Missing, malformed or unverified email produces no usable email proof. This prevents creation or
attachment of a new Apple identity, but must not reject an already-linked active Apple identity
solely because its later email claim is missing or changed.

## Resolution Contract

Resolve the validated Apple identity digest first with a consistent exact-key read.

| Identity State | Email/Owner State | Result |
| --- | --- | --- |
| Active | Any later email claims | Keep its persisted subject; require its account active. |
| Absent or reusable tombstone | No valid signed email proof | Reject without creating an identity, owner, account or code. |
| Absent or reusable tombstone | Active owner with matching active account and email index | Attach the new Apple identity to that subject. |
| Absent or reusable tombstone | Owner absent or eligible tombstone | Create a fresh account, owner and indexes. |
| Absent or reusable tombstone | Owner/account/index malformed, missing where required, or inactive | Fail closed; do not create an alternative account or repair data. |
| Ineligible identity tombstone | Any owner | Reject under the existing reuse policy. |

For an active identity, do not consult email ownership to move it, claim a later email, or backfill an
owner. An email now owned by another account does not merge the two. Preserve the existing allowlisted
profile update contract, but never treat that metadata update as an ownership transfer.

For a new/reusable identity with an active owner, validate the owner key/value binding, its active
shape, its subject's active account and the matching email-by-subject index. Capture exact raw owner
and index values plus the active account/generation snapshot. An absent or corrupt reverse index is
an integrity failure, not a request to recreate it. Never fall back to another subject after failure.

For an absent/eligible owner, generate one fresh subject and generation-zero account. Apply the
existing deleted-identity policy independently to both identity and owner tombstones: `never`
rejects, `immediate` still requires stored reuse permission, and `after_retention` requires a reached
reuse timestamp. Missing permission is not permission. Keep owner tombstones anonymized.

A reused Apple identity may attach to an already-active email owner or create a fresh account for
an available email. Neither branch resurrects a deleted subject. No standalone unlink operation is
introduced. Disable/enable retain email ownership; disabled accounts cannot accept a new identity.

## Atomic Issuance

Extend the existing prepared issuance type, not a parallel resolver. Its current `new_account` flag
must no longer conflate a new identity with a new account: attaching a new identity to an existing
owner still needs an identity subject index. Use a private typed plan for these cases; routes must
not assemble generic transactions or replace prepared subjects/generations.

Preparation writes nothing. Finishing preserves the original session, client, redirect, scopes,
PKCE and OIDC nonce, checks session expiry again and writes a short-lived HMAC-keyed code. The code's
subject and required credential generation come from the prepared account snapshot.

Returning Apple identities retain the existing three-operation boundary: exact active-account
guard, exact identity replacement and absent-only code creation. They add no ownership write.

New Apple identities use one of these transactions:

| Operation | Fresh Account/Owner | Existing Active Owner |
| --- | --- | --- |
| Account | Create if absent, generation zero | Check exact active account/generation |
| Email owner | Create if absent or replace exact eligible tombstone | Check exact active owner |
| Email subject index | Create if absent | Check exact matching index |
| Apple identity | Create if absent or replace exact eligible tombstone | Same |
| Identity subject index | Create if absent | Same |
| Authorization code | Create if absent with runtime expiry and matching TTL | Same |

Each row addresses a different item; do not condition-check and write the same item separately in
one DynamoDB transaction. Owner/index records remain persistent without TTL. Identity/code creation
must not commit separately from account selection or ownership creation.

Keep one application-level transaction attempt. A confirmed condition failure applies none of these
writes. Conflicts, operational failures and a lost response after real commit remain distinct; an
error response does not prove rollback. Do not retry by adopting a newer owner, generation or subject,
and do not compensate by deleting potentially committed state. The client restarts sign-in after a
failed callback. Existing provider-state/session consumption remains outside this transaction.

## Password And Lifecycle Boundaries

Pending password registrations own no email and cannot select an account for Apple. Apple sign-in
must neither activate a pending password nor copy a password hash, consume a verification proof or
silently attach a password credential.

Keep password registration completion's active-owner rejection unchanged:

- If password completion wins an ownership race, a stale Apple fresh-account transaction fails;
  a fresh Apple sign-in may resolve that active owner and attach normally.
- If Apple wins, an already-prepared password completion fails atomically. A later attempt still
  rejects the active owner without installing a password. Do not reinterpret the old proof as
  account enrollment or recapture a newer credential generation to make it succeed.

Document the resulting boundary: password-first then Apple can share a subject; Apple-first does
not yet support adding a password. Existing unrelated Apple/password accounts are not merged.

Recovery, disable and delete races must reject stale prepared issuance through exact account guards.
Successful shared-account deletion uses existing indexed cleanup for both identities, password
credentials, refresh state and email ownership. Preserve visible partial-cleanup failures and safe
retry; do not claim the complete deletion operation is atomic or that issued access JWTs are revoked.

Use one current contract with no compatibility branches, migration or backfill. Returning identities
resolve by their persisted issuer/subject binding, not by inferring ownership from profile metadata.
Acceptance uses fresh disposable data.

## Expected Change Surface

Primary runtime files:

```text
packages/functions/auth/src/providers/apple.rs
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/store/provider_issuance.rs
packages/functions/auth/src/store/email_ownership.rs
```

Use small responsibility-based children only if necessary. Keep every source/test file below 1,000
lines. Any shared issuance input change must update Google callers without changing Google's account
selection or accepting Apple proof there. Do not port the reference's generic resolver, retry loop,
optional-generation compatibility or Apple grant storage.

Extend signed Apple callback fixtures and shared provider/ownership contracts. Reuse the current
memory and DynamoDB Local runners, deterministic clocks and storage gates; split focused Apple
ownership cases rather than duplicating entire suites.

For every touched source/test/README, update its exact design mirror. Update the identity and Apple
overviews, threat model, account lifecycle, store ownership/issuance docs,
[Apple setup](../../docs/setup/08-apple-login.md),
[email setup](../../docs/setup/06-resend-email.md) and
[operations guidance](../../docs/operations/smoke-test.md) where behavior changes. Describe actual
what, why and how, including the signup-order boundary. Keep Google docs truthful; do not advertise
complete shared-account parity. No operator instruction should ask for raw auth-record access.

## Verification

Focused observable contracts:

- A signed verified Apple callback for a completed password account returns its existing subject
  in a usable authorization code; PKCE exchange and existing code claims remain valid.
- A new verified Apple address creates the account, owner, both indexes, identity and code together.
  Missing/unverified/malformed email and unsigned `user` overrides cannot authorize that creation.
- Existing Apple identities keep their subjects with changed/missing email, including an email
  owned by another active account. A verified relay address does not match its user's real address.
- Invalid identity/proof bindings, inactive accounts and malformed/missing ownership/index records
  fail without alternate accounts, repairs or codes. Existing JWT/state/nonce rejections stay intact.
- Force each new transaction condition to fail independently and verify no partial writes. Gate
  competing Apple claims and password completion through the real storage boundary; the loser
  leaves no account, index, password or code. A pending registration never installs a credential.
- Reset/generation change, disable, delete and ownership changes invalidate stale prepared issuance.
  Preserve one attempt and distinguish a pre-dispatch failure from a committed response loss.
- Deleting a linked password/Apple account cleans both identities and the shared email ownership;
  retries and deleted-identity/owner reuse cannot alter a newer owner or revive the deleted subject.
- Google issuance retains separate-account behavior. Existing password proof/recovery and admin
  sanitization contracts pass without weakening their assertions.
- DynamoDB Local physical rows prove code TTL and persistent owner/index records. Test real
  conditional writes and rollback, not only generated operation vectors or mocked success.

Run a focused baseline before edits, then the affected contracts and full auth regression suite:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test oidc_apple_callback --test provider_issuance \
  --test registration_contract --test admin_deletion
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Use the documented disposable loopback-only emulator with dummy credentials and unique tables,
then stop it. Never use deployed tables, live accounts or provider credentials. Check touched Rust
formatting, source lengths and design links/mirrors; report unrelated baseline failures without bulk
cleanup. No UI tests or unrelated native builds are required. Local fixtures do not certify live
Apple login or deployment.

## Acceptance Criteria

- New Apple identities can share an active email owner's subject only through validated signed
  email authority, with account/owner/index guards and code issuance in one transaction.
- Existing Apple identities resolve by issuer/subject first and cannot migrate accounts via email.
- Available email claims create one fresh account and owner; private relay addresses remain distinct.
- Pending password credentials, stale proofs and lifecycle races cannot install or revive access.
- Google, password enrollment, API/infra boundaries and anonymized deletion remain as scoped.
- Meaningful callback, memory and DynamoDB Local contracts pass; actual results and remaining
  blockers are recorded here at completion.
- Exact design mirrors and operator docs describe the implemented behavior and its limits.

## Implementation Result

- The Apple callback constructs identity-bound email authority only after signed ID-token validation.
  Unsigned callback data and ordinary profile properties cannot select an account.
- Returning Apple identities retain their persisted subjects. New identities either attach to a
  matching active email owner or create a fresh account and ownership, with all six guarded writes
  or checks committed alongside the authorization code in one transaction.
- Google account selection is unchanged. Apple-first password enrollment remains unsupported;
  pending password proofs cannot install credentials into the account Apple established.
- Shared contracts exercise competing claims, every transaction condition, password completion,
  recovery, lifecycle changes, tombstone reuse, deletion retry and uncertain commit outcomes against
  both memory storage and DynamoDB Local. Signed callback tests cover actual account selection and
  subsequent code exchange.
- Exact source/test mirrors, identity/storage overviews and setup/operations docs describe the
  current contract. No compatibility branches, legacy aliases, backfills or new infrastructure were
  added. The pinned reference supplied the ownership/identity-first contracts, not product code or
  a retry planner.

## Verification Results

| Check | Result |
| --- | --- |
| Pre-edit focused baseline | Passed: 47 tests. |
| Full locked auth suite | Passed: 320 tests; 66 local integration tests ignored by default. |
| Disposable DynamoDB Local suite | Passed: all 66 integration tests, including the new Apple ownership contracts. |
| Locked admin check | Passed. |
| Auth Clippy, all targets | Passed with existing warnings; no new warning kinds against the prior slice's log. |
| Infrastructure tests | Passed: all six validators. |
| Touched Rust formatting and source limits | Passed; all touched source/test files remain below 1,000 lines. |
| Design mirrors, relative links and whitespace | Passed. |

Sandboxed attempts to bind or reach local test listeners initially failed with permission errors.
The final auth and DynamoDB runs passed with approved loopback access. DynamoDB Local used dummy
credentials, unique disposable tables and an in-memory listener bound only to `127.0.0.1:18023`;
the emulator was stopped after verification. No deployed tables or live provider accounts were used.

No manifests, TypeScript infrastructure, setup tooling or example code changed, so unrelated
typecheck/setup/native builds and UI checks were not run. Live Apple/device login and deployed
behavior are not certified by these local fixtures. There are no remaining local verification
blockers for this slice.
