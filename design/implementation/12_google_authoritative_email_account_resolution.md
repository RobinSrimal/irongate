# 12_google_authoritative_email_account_resolution

## Status

Implemented and locally verified on 2026-11-05.

Builds on `4c0df0f` (Apple verified-email account resolution). No deployment, cloud reconfiguration,
reference-repository changes or hosted confirmation surface were added.

## Goal

Resolve new Google identities through the same guarded email ownership boundary as Apple, but only
when validated Google claims establish current email authority:

```text
existing Google issuer/subject -> its existing active account
new Google identity + authoritative email + active owner -> that owner's active account
new Google identity + authoritative, available email -> fresh account and ownership
new Google identity without email authority -> reject without account/identity/code writes
```

Account selection, ownership, identity/index writes and authorization-code issuance commit in one
transaction. Existing accounts are never merged or moved because their email claims change.

## Scope Decision

This slice completes the direct Google authority path, not the entire shared-account feature.
It reuses slice 11's store boundary and does not add email sending, confirmation records, browser
cookies, routes or pages. That keeps the implementation focused on provider proof and account
selection rather than a second user-interaction protocol.

The explicit functional limit is that a first-time Google identity without authoritative email
cannot sign in in this slice, even if that email has no owner. A fresh mailbox-confirmation flow
is still needed to support it. Do not preserve the old separate-account creation path as a fallback,
add a configuration switch to bypass proof, or imply that confirmation already exists. Returning
identities still authenticate by their validated issuer/subject binding; this is identity continuity,
not a compatibility branch.

In scope:

- Deserialize Google's signed `hd` claim and classify Google email authority after JWT validation.
- Replace Apple-only proof plumbing with one provider/identity-bound email authority contract.
- Use the existing guarded ownership plan for new Google identities and remove unproven creation.
- Exercise signed callbacks, cross-provider ownership and transaction/lifecycle races.
- Update exact design mirrors and relevant setup/operations docs.

Out of scope:

- Third-party-email confirmation, new email templates, hosted pages or browser-binding protocols.
- Password enrollment into an existing provider account, account merging or generic linking APIs.
- Native provider sign-in, presenter allowlists, SDKs or example UI changes.
- Apple grant storage/revocation and deletion orchestration.
- New record schemas, tables, indexes, IAM permissions, secrets, config or dependencies.
- Compatibility layers, legacy aliases, migrations, backfills or fallback record formats.
- Deployment, live provider testing, production data or reference-repository edits.

## Reviewed Reference

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant pinned sources:

- `packages/functions/auth/src/store/shared_identity.rs`: `VerifiedEmail::google`, identity-first
  resolution and ownership-based issuance.
- `packages/functions/auth/src/api/providers/google.rs`: proof construction after signed validation.
- `packages/functions/auth/src/api/providers/email_confirmation.rs`: demonstrates why non-hosted
  email needs a distinct fresh-proof/browser-binding flow; do not copy that surface in this slice.

At definition time reference HEAD is `904c8b3f96b6856c7d5e68fd534af45745c7f1b8` and its worktree is
clean. Recheck both repositories before implementation and retain the pinned comparison. Do not
copy the reference's retry planner, optional credential generations, native configuration or
product-specific values.

Google's [server-side ID-token guidance](https://developers.google.com/identity/gsi/web/guides/verify-google-id-token)
distinguishes Gmail/Google-hosted email authority from third-party addresses whose ownership may
have changed after initial verification. The policy below follows that distinction and additionally
requires `email_verified == true` for every accepted new Google email proof.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Identity model](../functions/auth/core/identities.md),
  [subjects](../functions/auth/core/subjects.md) and
  [account lifecycle](../functions/auth/core/account-lifecycle.md).
- [Google API](../functions/auth/api/providers/google.md),
  [Google domain](../functions/auth/providers/google.md) and
  [Google implementation](../functions/auth/src/providers/google.md).
- [Apple API](../functions/auth/api/providers/apple.md) and
  [Apple implementation](../functions/auth/src/providers/apple.md).
- [Provider issuance](../functions/auth/src/store/provider_issuance.md),
  [email ownership](../functions/auth/src/store/email_ownership.md),
  [credential guards](../functions/auth/src/store/credential_generation.md) and
  [identity storage](../functions/auth/store/identities.md).
- [Google setup](../../docs/setup/07-google-login.md),
  [Apple setup](../../docs/setup/08-apple-login.md),
  [email setup](../../docs/setup/06-resend-email.md) and
  [test instructions](../../packages/functions/auth/tests/README.md).

Preserve API-only core behavior, admin IAM isolation, optional examples and the existing OAuth/OIDC
exchange. Update authoritative docs when implementation changes, not to advertise this plan as done.

## Email Authority Contract

Add `hd: Option<String>` to `GoogleIdTokenClaims`. It is an input to authority classification, not an
organization authorization policy or a new token output. Do not copy it into access/ID-token claims,
identity metadata, logs or a domain allowlist in this slice.

Construct Google authority only after signature, issuer, audience, time and nonce validation:

1. Require `email_verified == Some(true)` and a valid email under existing normalization rules.
2. Require either the exact normalized email domain `gmail.com`, or a valid signed `hd` value.
3. Validate `hd` as nonempty ASCII DNS labels: at most 253 bytes overall, labels of 1-63 bytes,
   alphanumeric/hyphen characters, and no leading/trailing hyphens. Reject blank labels, whitespace,
   trailing dots and URL syntax. Do not infer `hd` from an email suffix or authorization parameter.
4. Use the established normalized-email HMAC family. Do not collapse dots, plus aliases, private
   relay addresses or alternate mail domains. A valid hosted-domain claim need not equal the email
   suffix; it is Google's signed assertion, not a suffix-matching heuristic.

Missing, malformed or non-authoritative email produces no proof. A missing/invalid hosted-domain
value cannot authorize a non-Gmail address. A valid Gmail proof does not need `hd`. Classification
does not weaken JWT validation or trigger key refresh; the existing bounded MissingKey behavior stays.

Replace `AppleVerifiedEmail` and `ProviderIdentityInput.apple_email` with one current
`ProviderVerifiedEmail` / `verified_email` contract in a small `providers/verified_email.rs` module:

- Private fields bind provider kind, issuer/subject digest, normalized email and email digest.
- Separate Apple and Google constructors receive already-validated typed claims and the lookup
  secret. The constructors themselves do not verify signatures; production callers must do so first.
- No arbitrary-properties, request-deserialization or public raw-email authority constructor.
- Store preparation rejects wrong-provider and wrong-identity proofs before account selection,
  including when a returning identity exists.
- Preserve Apple's existing verified-email and literal private-relay semantics.

Remove the superseded Apple-only type/field; update all callers and fixtures rather than adding
aliases, duplicate input fields or adapters. This shared proof is justified by two actual providers,
not a generic provider/plugin framework.

## Resolution And Issuance

Keep identity lookup first. An active identity retains its subject and requires its matching active
account. Missing/changed email or lost authority cannot move it, claim another email or merge it
with an owner. Returning issuance retains its exact identity/account guards and three operations.

For absent or eligible deleted identities, both providers require an appropriately bound proof:

| Ownership State | Result |
| --- | --- |
| Active owner with matching active account and email index | Prepare attachment to that subject. |
| Owner absent or eligible tombstone | Prepare a fresh generation-zero account and ownership. |
| Missing/inactive account or malformed/missing required owner/index binding | Fail closed, without repair or alternative subject. |
| Ineligible identity or owner tombstone | Reject under the existing reuse policy. |
| No acceptable provider email proof | Reject without new identity/account/owner/index/code writes. |

Replace the separate `NewGoogle` / `NewApple` branches with a new-identity ownership plan and the
returning-identity plan. Both providers use `prepare_email_account`; do not copy its validation or
introduce a second resolver. New identity metadata takes normalized email/verification from proof,
not from arbitrary JSON. Preserve the existing provider-specific metadata allowlists.

New Google issuance uses the same six-item transaction already used by Apple:

1. Create the account if absent, or check the exact active account and credential generation.
2. Create/replace an available owner conditionally, or check the exact active owner.
3. Create the email subject index if absent, or check its exact existing value.
4. Write the absent/reusable identity using its captured condition.
5. Create the identity subject index if absent, even when the account already exists.
6. Create an absent authorization code with matching runtime expiry and DynamoDB TTL.

Each item occurs only once in the transaction. Persistent owner/account/identity/index records keep
no TTL. Identity and owner reuse policy are independent and never revive a deleted subject.

Preparation writes nothing and captures private snapshots. Preserve the consumed session's client,
redirect, scope, PKCE, state and OIDC nonce through the existing protocol boundaries; validate
provider/session/expiry and recheck expiry before dispatch. Do not adopt a newer subject or generation
to rescue a rejected commit. One application transaction attempt, no compensation, no backfill.

Confirmed proof/policy/conditional rejection uses the existing fixed `400 invalid_grant` response.
Storage integrity failures, outages, conflicts and uncertain outcomes retain the sanitized storage
error mapping; they must not masquerade as missing authority. No response discloses whether an
email owner exists. State/session consumption still precedes issuance, so a failed callback requires
fresh sign-in. A lost response can follow a committed transaction; do not claim rollback.

## Cross-Provider And Password Boundaries

A new authorized Google identity can join an owner established by password or Apple, and a new
Apple identity can join a Google-created owner when its literal verified address matches. Returning
identities always keep their own subjects, even if another active account owns a later email claim.
This never merges independently populated accounts.

Pending password registration remains neither an owner nor account-selection authority. If a
provider wins the ownership race, password completion cannot install the pending password. If
password completion wins, stale provider creation fails; a fresh provider transaction may attach.
Adding a password to a provider-first account remains unsupported.

Preserve reset/generation, disable/delete and owner/index guards. Indexed deletion must clean all
linked identities and shared ownership; a retry for an old subject must not alter a newer owner.
Do not add a whole-deletion atomicity claim, lifecycle-history epoch or immediate access-JWT
revocation promise.

## Expected Change Surface

Primary source files:

```text
packages/functions/auth/src/providers/verified_email.rs  (new)
packages/functions/auth/src/providers/mod.rs
packages/functions/auth/src/providers/google.rs
packages/functions/auth/src/providers/apple.rs
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/store/provider_issuance.rs
```

Reuse `store/email_ownership.rs` unchanged unless a concrete shared-boundary defect requires a
scoped fix. Keep every source/test file below 1,000 lines, including inline tests.

Extend signed Google callback tests and current memory/DynamoDB provider contracts. Generalize
existing ownership tests where they truly cover the same behavior; add focused Google authority and
cross-provider cases rather than copying the complete Apple suite. Move responsibility-based test
children only when it reduces actual duplication, updating their exact mirrors and module paths.

Update every touched source/test/README mirror, including the new proof module, plus identity,
provider, ownership, account-lifecycle and token/code overviews affected by the policy. Update
`design/overview.md`, the threat model, `docs/setup/07-google-login.md`, related Apple/email setup
notes and `docs/operations/smoke-test.md`. Describe both the supported sharing and first-time
third-party-email restriction explicitly. No Google-separation assertions or documentation should
remain where they contradict the new current contract; preserve assertions about stable existing
identities and no account merging.

## Verification

Use observable behavior and real transaction conditions, not source-string checks:

- Signed Google callback with each accepted authority form joins an actual password/Apple owner;
  code exchange succeeds with that subject and existing PKCE/client/nonce constraints.
- New authoritative addresses create all ownership/account/identity/code records together.
- Independently reject unverified/missing/malformed email, absent/invalid `hd` on non-Gmail claims,
  lookalike Gmail domains, forged signatures and mismatched proof identity/provider. Raw properties
  or request parameters cannot substitute for signed authority. No partial issuance writes occur.
- Returning Google identities keep their subject with missing email/`hd`, changed email owned by
  another account, or later non-authoritative claims; inactive/missing accounts still reject.
- Preserve independent signature/issuer/audience/nonce/expiry and key-rotation tests. Give success
  fixtures valid authority so a later policy failure cannot hide what each rejection test exercises.
- Exercise each six-item condition failure and usable-code collision. Run shared conditions against
  memory and DynamoDB Local; inspect physical code TTL and persistent owner/index rows separately.
- Gate competing Google/Apple claims for one available email in both winner orders. Only one fresh
  account wins; the loser leaves no account/index/code and does not automatically retry. A new
  attempt attaches to the winner. Concurrent attachments to an unchanged existing owner may both
  succeed for distinct identities; do not introduce a false exclusivity requirement.
- Gate Google against password completion, actual recovery, disable/delete and ownership/index
  changes. Stale prepared issuance must not adopt newer state or install a pending credential.
- Exercise identity/owner retention boundaries and linked deletion/retry without modifying newer
  owners. Reuse shared lifecycle coverage instead of duplicating every case per provider.
- Retain pre-dispatch failure versus committed-response-loss tests, secret-safe error behavior,
  unchanged Apple authority and stable returning identities across the proof-type replacement.

Run the focused baseline before edits. The previous slice's results are reference evidence, not
verification of this implementation:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test oidc_google_callback --test oidc_apple_callback --test provider_issuance
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Use the documented disposable loopback-only emulator, dummy credentials and unique tables, then
stop it. No deployed tables, live email/provider accounts or UI testing. Check touched Rust
formatting, file sizes and design links/mirrors. Run other checks only when their affected surface
changes, and record actual results, failures and skipped checks at completion.

## Acceptance Criteria

- New Google identities select accounts only with provider/identity-bound authoritative email.
- Google and Apple share one guarded new-identity path; returning issuer/subject resolution wins.
- New non-authoritative Google identities fail closed, with no separate-account fallback or claim
  that fresh confirmation exists.
- Six-item atomic issuance, reuse policy, generation/lifecycle guards and secret-safe failures hold
  through both memory and DynamoDB Local contracts.
- Pending password state cannot reserve ownership or install credentials through provider sign-in.
- The superseded Apple-only proof contract and unproven Google creation branch are removed, not
  retained as aliases or compatibility paths. All in-scope callers and fixtures use the new contract.
- Design mirrors and operator docs match actual behavior and its limits.
- No cloud changes, hosted confirmation surface, migration or unrelated product code is introduced.

## Implementation Result

- Google callbacks construct email authority only after full signed ID-token validation. Verified
  Gmail or valid signed Workspace `hd` can select an existing email owner or create new ownership.
  Third-party email without authority rejects without account/identity/code writes or a fallback.
- Apple and Google use one provider/identity-bound `ProviderVerifiedEmail` contract and the same
  six-item ownership transaction. The Apple-only proof and unproven Google creation branch are
  removed. Returning issuer/subject identities keep their existing subject even when email changes
  or authority disappears; accounts are never merged.
- Shared storage contracts cover both providers, cross-provider competing claims, password
  completion, recovery, lifecycle and tombstone races, all transaction conditions, and uncertain
  commit outcomes. Signed Google callback tests exercise authority rejection and actual shared
  subject code exchange. Existing Apple and independent JWT rejection coverage is retained.
- Exact source/test mirrors, identity/storage overviews and setup/operations docs describe the
  current behavior. There are no compatibility aliases, migrations, backfills, dependencies or
  infrastructure changes. Fresh third-party-email confirmation and provider-first password
  enrollment are not implemented.
- The pinned reference supplied the authority and identity-first rules, not product configuration
  or its retry planner. Reference HEAD at implementation was
  `1985d096506c782fde36666361a3a047fbe4bcc8`; its clean worktree was read only.

## Verification Results

| Check | Result |
| --- | --- |
| Pre-edit focused baseline | Passed: 51 tests. |
| Full locked auth suite | Passed: 327 tests; 67 local integration tests ignored by default. |
| Disposable DynamoDB Local suite | Passed: all 67 integration tests. |
| Final provider contracts after fixture-name cleanup | Passed: 17 memory tests and 18 DynamoDB Local tests. |
| Locked admin check | Passed. |
| Auth Clippy, all targets | Passed with existing warnings; no new warning kinds against the prior slice's log. |
| Infrastructure tests | Passed: all six validators. |
| Touched Rust formatting and source limits | Passed; all touched source/test files remain below 1,000 lines. |
| Design mirrors, relative links and whitespace | Passed. |

Initial sandbox attempts to bind/reach test listeners failed with permission errors; approved
loopback runs passed. Intermediate fixture failures exposed old separate-account assumptions and
normalized-email expectations, which were corrected before the successful final runs. Rejection
fixtures now provide valid email authority so later policy checks cannot mask JWT/session failures.

DynamoDB Local used dummy credentials, unique disposable tables and an in-memory listener bound
only to `127.0.0.1:18023`. The emulator was stopped and its listener removed after verification.
No deployed tables, live email/provider accounts, UI checks or cloud changes were used.

No manifests, TypeScript infrastructure, setup tooling or example/native code changed; unrelated
typecheck/setup/native builds were not run. These local checks do not certify live Google/Apple
login or a deployed environment. There are no remaining local verification blockers for this slice.
