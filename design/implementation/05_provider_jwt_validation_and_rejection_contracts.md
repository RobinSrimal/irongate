# 05_provider_jwt_validation_and_rejection_contracts

## Status

Implemented and locally verified. This slice builds on
[atomic password recovery](04_atomic_password_recovery.md) at `5f47599`. No cloud deployment or live
provider acceptance test was performed.

## Goal

Tighten Google and Apple ID-token signing-key selection and prove their existing validation guards
with independently signed rejection cases and callback-level failure checks.

The result is a small auth-core security improvement, not a new provider integration, shared JWT
framework or resource API. Preserve successful provider login and slice 04's generation-bound code
issuance while ensuring invalid provider tokens cannot resolve an identity or issue a code.

## Reviewed Sources

Irongate baseline: `5f47599e4493cc646985ae6c3bc7ca557b006c35`; worktree clean at definition.

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620`. HEAD matches that commit. Its current worktree contains
unrelated mobile/release/marketing changes; comparisons use committed content, not those edits.

Relevant reference paths:

- `packages/functions/auth/src/providers/google.rs` and `apple.rs`: nonblank key IDs and selected
  JWK signing-use validation, alongside the existing signature and claim checks.
- `packages/functions/auth/tests/support/oidc_validation.rs`: signed valid controls and independent
  claim/header/key rejection assertions.
- `packages/functions/auth/tests/oidc_google_callback.rs` and `oidc_apple_callback.rs`: use of those
  assertions with each real provider validator and existing callback fixtures.
- `design/implementation/78_irongate_jwt_validation.md` and commit `027c3b5`: rationale and the
  distinction between provider validation and the reference product API's signing-key cache.

The product-side cache in `packages/functions/product/src/auth/` is not copied. Irongate already
has bounded ten-minute Google/Apple caches and one missing-key refresh attempt. This slice does
not claim those features are absent or import the product API to reproduce them.

## Baseline Gaps

| Area | Current behavior | Change |
| --- | --- | --- |
| Header key ID | Requires Some(kid), then exact lookup | Reject empty/whitespace-only IDs even if a JWK has that same ID |
| Selected JWK use | Checks RSA type and optional algorithm, ignores use | Reject any explicit use other than sig |
| Claim tests | Mostly mutate one claim, but assert only is_err | Start from an accepted control and assert the intended error category |
| Key/signature tests | Missing-kid coverage, limited independent cases | Cover algorithm, key metadata/components and bad signature separately |
| Callback retry tests | Fake client supplies stale then valid keys; success and call count checked | Also prove terminal rejection, bounded retry and no identity/code side effects |

There is no change to account storage, credential generations, token TTLs, provider client IDs,
secrets or deployment configuration. No compatibility aliases, migrations or permissive legacy
validation paths are needed. Preserving valid OAuth/OIDC inputs is protocol interoperability, not
backwards-compatibility scaffolding.

## Design To Honor

- [Overview](../overview.md), [threat model](../functions/auth/threat-model.md) and
  [testing](../functions/auth/testing.md).
- [Google provider](../functions/auth/providers/google.md),
  [implementation](../functions/auth/src/providers/google.md) and
  [HTTP flow](../functions/auth/src/api/providers/google.md).
- [Apple provider](../functions/auth/providers/apple.md),
  [implementation](../functions/auth/src/providers/apple.md) and
  [HTTP flow](../functions/auth/src/api/providers/apple.md).
- [Google callback tests](../functions/auth/tests/oidc_google_callback.md) and
  [Apple callback tests](../functions/auth/tests/oidc_apple_callback.md).
- [Provider-state storage](../functions/auth/store/provider-states.md),
  [authorization codes](../functions/auth/store/authorization-codes.md) and
  [token semantics](../functions/auth/core/tokens.md).
- [Google setup](../../docs/setup/07-google-login.md) and
  [Apple setup](../../docs/setup/08-apple-login.md).

Authoritative docs must describe implemented behavior, not this proposal. Update their what/why/how
alongside implementation; do not mark every JWT security concern resolved by this narrow slice.

## Validation Contract

Apply the same key-selection rules to both existing provider validators:

1. Preserve the RS256-only token-header check. The unverified header selects a key; it is not proof
   of identity and cannot select a new issuer, JWKS URL or algorithm policy.
2. Require a nonblank `kid`. Use trimming only to detect blank values; compare nonblank IDs exactly,
   without normalizing them. Do not fall back to the first key or try arbitrary keys on a miss.
3. Preserve the selected JWK's `kty=RSA` requirement and reject an explicit `alg` other than RS256.
4. Accept absent `use` or `use=sig`; reject explicit `enc`, empty or other non-signing values.
5. Preserve acceptance of absent optional `alg`/`use` metadata when the RSA key and actual RS256
   signature are valid. Do not make optional metadata a new provider configuration requirement.
6. Require usable RSA modulus/exponent material and verify the signature before trusting claims.

Use the existing error categories: missing/blank/unmatched key ID is MissingKey; explicit wrong
type/algorithm is UnsupportedAlgorithm; unusable key material or non-signing use is InvalidKey;
signature/decoding failures are InvalidToken. Do not expose raw tokens or key material in errors.

Preserve the existing claim policy:

- Exact configured issuer and configured client audience membership.
- Expiry is rejected at `exp <= expected.now`, with no new expiry leeway.
- Issued-at accepts the existing 60-second future allowance; greater values reject.
- Nonce is required and must match the original provider transaction.
- Subject must not be empty or whitespace-only. Return its original value, not a rewritten ID.
- Audience string/array decoding and Apple's boolean/string email-flag parsing remain supported.
- Optional email/profile claims do not become identity proofs or authorization to link accounts.

This does not add native presenter/azp policy, new issuer variants, nbf handling or other claim
semantics. Both implementations and the pinned reference currently disable library nbf checking;
do not represent these focused checks as a complete OIDC conformance certification.

## Callback And Cache Boundary

Keep the current reqwest client, ten-minute process-local JWKS cache and callback retry shape.
No network/cache module rewrite is required for the two key-selection checks.

- MissingKey permits at most one forced refresh followed by full validation again. The existing
  category includes absent/blank IDs as well as unmatched IDs; this slice does not redesign that
  classification or introduce an unbounded retry.
- Invalid signature, issuer/audience/nonce/time/subject, explicit non-signing JWK use and unsupported
  algorithms do not trigger a forced refresh to try to turn rejection into success.
- A refreshed set that still cannot validate the token, or a failed refresh, terminates the callback
  without an internal code or identity resolution.
- Preserve current safe OAuth error mapping. Do not widen this into provider outage/status-code
  redesign, cache cooldown, transport limits or distributed cache behavior.

State/session consumption already precedes upstream exchange/validation. Rejection tests must not
mistake that expected consumption for an account mutation, nor require restoring a consumed state.
No invalid callback may create an account/identity/code or update an existing identity's metadata.
Successful callbacks still resolve by validated issuer/subject and create generation-bound codes.

## Focused Evidence

### Signed Validator Cases

For each provider, use its real validation function, an accepted signed baseline, the same trusted
test key and an explicit fixed validation time. Change one security property per rejection and
assert its error category. Re-sign claim mutations so a bad signature cannot hide a claim failure.

Cover:

- Wrong issuer and wrong audience, including an audience array without the configured client.
- Wrong and missing nonce.
- Expiry exactly at now and before now; a valid later expiry is the control.
- Issued-at at now + 60 seconds succeeds; now + 61 rejects independently of expiry.
- Empty and whitespace-only subject.
- Missing, unknown, empty and whitespace-only kid. Include a matching blank-ID JWK for the blank
  cases so the test proves the new guard rather than only a failed lookup. A key ID with changed
  surrounding whitespace must not be normalized into a match.
- Non-RS256 header, wrong JWK type, explicit wrong JWK algorithm and explicit non-signing use.
- Missing or malformed RSA components, and corrupted signature with otherwise valid claims/key.
- Missing optional JWK alg/use accepted; a matching audience array accepted under current policy.

Retain valid provider-specific email/claim parsing controls. Reuse existing test-only signing
fixtures without copying real credentials or product identifiers. Never print full tokens, private
keys, passwords or provider credentials in assertions. Consolidate superseded broad is_err tests
instead of duplicating every case alongside them.

### Callback Rejection And Retry

Extend the existing local callback fixtures with controlled key sets/failures. Use valid provider
state and authorize sessions so tests reach the intended validation path.

For Google and Apple, establish:

- Known key plus a valid signed token still creates a code that exchanges through `/token`.
- Unknown key followed by a valid fresh key succeeds after one forced refresh.
- Unknown key still absent after refresh rejects, with no third fetch and no code/identity write.
- A failed forced refresh rejects without a code or identity write.
- A known key with an invalid signature/claim or explicit non-signing use rejects without forced
  refresh. Check no new account/identity/code and no update of a seeded existing identity where
  appropriate, not merely the response status.

Fetch counts are evidence for the retry bound only when paired with actual validation and callback
outcomes. Fake-client tests do not certify reqwest TTL, HTTP cache behavior or live provider rotation.
Do not add sleeps, live Google/Apple calls, UI tests or DynamoDB tests for this storage-independent
validation change. Retain state replay/missing-state, PKCE/client binding, disabled/deleted account,
identity isolation, generation-bound issuance and refresh/recovery regressions.

## Manageable Change Surface

Expected production changes are confined to:

```text
packages/functions/auth/src/providers/google.rs
packages/functions/auth/src/providers/apple.rs
```

Reuse and refine:

```text
packages/functions/auth/tests/oidc_google_callback.rs
packages/functions/auth/tests/oidc_apple_callback.rs
```

A small `tests/support/oidc_validation.rs` helper may remove genuinely repeated signing/case setup,
as in the reference. Keep concrete provider validation and error assertions visible; do not create
a generic production validator abstraction or copy the reference's wider test framework.

At definition, the callback files had 891 and 948 lines. Replace repeated validation blocks before
adding cases. If necessary, move their pure validation cases into focused child modules under
`tests/oidc_google_callback/` and `tests/oidc_apple_callback/`, reusing parent fixtures. Keep every
source below 1,000 lines rather than compressing code to meet the limit. Avoid unrelated test moves.

Callback source edits are not expected: the existing retry branch already distinguishes MissingKey
from other failures. If verification exposes a direct defect there, keep its fix narrow and update
the exact source mirror. No storage, manifest, example, infrastructure or dependency changes are
expected. Stop and explain before expanding into a different architecture or feature.

## Design Updates During Implementation

- Update exact mirrors for both provider modules and touched callback tests. Add exact mirrors for
  any new support/child test files, preserving `src/` and `tests/` paths.
- Update Google/Apple topic docs with nonblank exact kid matching and selected signing-use rules.
- Update callback mirrors if needed to distinguish validation failure from the bounded MissingKey
  refresh path, without claiming fake-client coverage proves cache freshness.
- Update the testing/threat-model descriptions only where these checks improve their stated
  coverage. Keep remaining limits explicit and remove superseded descriptions.
- Review the two provider setup pages for consistency. No new secret, console setting, callback
  URI or deployment step is introduced; avoid unrelated setup rewrites.

## Work And Verification

1. Recheck current worktree/HEAD and the pinned reference before implementation.
2. Run the two existing callback targets as the local baseline.
3. Apply the two key-selection checks and refine the focused signed validation cases.
4. Verify callback retry/rejection behavior using existing fixtures and meaningful storage effects.
5. Update the affected design mirrors/topic docs and record actual verification here.

Required focused command before and after:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test oidc_google_callback --test oidc_apple_callback
```

Final regression checks:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
git diff --check
```

Run changed-file rustfmt plus mirror/link/source-length checks. Report pre-existing Clippy warnings
separately from new ones. Ordinary Cargo wire fixtures can need loopback permission but must not
contact deployed tables. Explicit DynamoDB Local, TypeScript, setup, example and native builds are
not required unless their inputs change; do not claim ignored cases were rerun.

## Acceptance

- Both real provider validators reject blank IDs even against matching blank-key fixtures and
  reject explicit non-signing JWK use.
- Existing valid provider tokens and absent optional key metadata remain accepted.
- Signature, key and claim failures are exercised independently with specific error assertions.
- Callback rejection cannot issue a code or mutate identity/account state; refresh work stays
  bounded to the existing one retry and does not run for known-key validation failures.
- Existing provider, recovery and OAuth regressions pass in Irongate, not just in the reference.
- Exact design mirrors describe the implemented what/why/how, including testing limits.
- No data migration, compatibility layer, cloud operation, product API, native flow or future
  slice document is introduced.

## Implementation Results

Both provider validators now reject blank header key IDs and explicit non-signing JWK use.
Nonblank IDs still match exactly; valid RSA signatures with omitted optional JWK alg/use remain
accepted. Production changes are limited to the two provider modules. Existing claim rules,
callback error mapping and bounded MissingKey refresh behavior are unchanged.

The callback suites replace their broad rejection assertions and shallow refetch fixtures with:

- Shared signed controls in `tests/support/oidc_validation.rs`, instantiated against each concrete
  validator/error enum. Fixed-time cases isolate the signature, key and claim rejection contracts.
- Provider-specific `rejection.rs` child modules covering known-key failures without refresh,
  unsuccessful refresh with one attempt, and successful refreshed-key code-to-token exchange.
- Before/after account/identity/index/code snapshots for new and existing identities. Invalid
  callbacks leave those records unchanged while consuming state/session as expected.

All seven touched/new Rust files have exact design mirrors. Provider topic docs, callback mirrors,
testing guidance and the threat model describe the current behavior and evidence limits. Google
and Apple setup pages need no changes: no new secret, console setting or callback URL is required.
The pinned reference supplied the key-selection and signed-control direction; no product code,
deployment values, new dependencies or data migration were introduced.

### Verification

All Cargo commands below used `--locked --offline` in this repository:

| Check | Result |
| --- | --- |
| Baseline Google/Apple callback targets | Passed: 12 tests per provider |
| Updated Google/Apple callback targets | Passed: 15 tests per provider |
| Full auth `cargo test` | Passed: 247 test executions, 0 failures, 22 DynamoDB Local cases ignored |
| Auth `cargo check` | Passed |
| Admin `cargo check` | Passed |
| Auth `cargo clippy --all-targets` | Exit 0; existing warnings remain, no new warning locations |
| Changed-file rustfmt check | Passed |
| Changed-source mirror, relative-link and source-length checks | Passed; each source remains below 1,000 lines |
| `git diff --check` | Passed |

Full tests include local loopback SDK wire fixtures, not deployed DynamoDB. Explicit DynamoDB Local
cases were not rerun because storage did not change. TypeScript, infrastructure, setup, example and
native checks were not run because their inputs did not change. Clippy warnings were compared with
the preceding slice's output and unchanged source, rather than being reported as a warning-free run.

No unresolved implementation decision remains within this slice. Its tests do not establish live
Google/Apple login, real HTTP cache TTL behavior or full OIDC conformance. Native presenter/azp and
nbf policy remain outside this scope, as do deployment and product-specific verification caches.
