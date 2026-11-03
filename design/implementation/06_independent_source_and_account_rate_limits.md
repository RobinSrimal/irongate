# 06_independent_source_and_account_rate_limits

## Status

Implemented and locally verified on 2026-11-03. Builds on
[provider JWT validation](05_provider_jwt_validation_and_rejection_contracts.md), committed as
`8eaffde`. No deployment, infrastructure reconfiguration or reference-repository changes were made.

## Goal

Make existing endpoint limits resistant to changing one component of a request's identity.
Password registration, login and reset requests need separate source and normalized-email budgets.
OAuth and provider authorization need source ceilings in addition to their narrower client/session
budgets. Keep one admission contract over the existing DynamoDB counters.

This is a bounded auth-core change: no new service, auth flow, frontend or product behavior.

## Reviewed Sources

Irongate baseline: `8eaffde`, with a clean worktree after the slice 05 commit.

Read-only reference: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620`. HEAD still matches that commit; unrelated mobile,
release and marketing worktree changes are excluded from the comparison.

Relevant committed reference files:

- `packages/functions/auth/src/config/rate_limits.rs`: endpoint policies and bounded parameters.
- `packages/functions/auth/src/store/rate_limits.rs`: source-first admission, independently scoped
  identifiers, conservative accounting and bounded conditional counter updates.
- `packages/functions/auth/src/ratelimit/middleware.rs`: canonical trusted source IPs.
- `packages/functions/auth/tests/api_routes/rate_limits.rs` and `admission.rs`: changing emails,
  sources, configured/unknown clients and provider sessions; rejection before protected work.
- `packages/functions/auth/tests/support/rate_contract.rs` and `rate_control.rs`: shared memory
  and DynamoDB Local cases, deterministic concurrency and controlled failures.
- `design/implementation/79_irongate_abuse_protection.md` and commit `42cfec8`: rationale.

The reference also has email-first registration, browser email-action pages, provider-email
confirmation and native handlers. None are prerequisites for this slice. Adapt the admission
behavior to Irongate's current password-first API and existing callers; do not copy those flows,
product values, test framework or reference-only endpoint variants.

## Baseline Gaps

| Area | Observed behavior | Required change |
| --- | --- | --- |
| Password identity | One email-digest/source bucket | Separate per-source and per-email counters |
| OAuth identity | Client/source bucket; token collapses unknown client names | Aggregate source ceiling and consistent bounded unknown-client identity |
| Provider start | Provider/session/source bucket | Source ceiling shared by Google and Apple starts, plus existing narrower budget |
| Revoke placement | Client authentication happens before admission | Count parsed attempts before authenticating clients or touching refresh state |
| Source keys | Trusted context is trimmed; raw IP appears in keys/logs | Canonical IP and purpose-separated HMAC identifier |
| Enabled configuration | Missing endpoint policy silently permits requests | Missing/invalid policies fail closed |
| Evidence | Mainly identical requests and one-bucket concurrency | Independent dimension changes, protected-work checks and real counter races |

The existing eight-attempt CAS loop, logical window checks, TTL and unavailable-versus-contention
distinction remain useful. Do not replace them with in-memory runtime limits or a queue.

## Design To Honor

- [Overview](../overview.md), [threat model](../functions/auth/threat-model.md) and
  [testing](../functions/auth/testing.md).
- [Rate-limit overview](../functions/auth/store/rate-limits.md),
  [store implementation](../functions/auth/src/store/rate_limits.md),
  [typed store boundary](../functions/auth/src/store/mod.md) and
  [runtime configuration](../functions/auth/src/config.md).
- [Source identity](../functions/auth/src/ratelimit/middleware.md),
  [lookup digests](../functions/auth/src/crypto/hmac_lookup.md) and
  [router](../functions/auth/src/routes.md).
- [Password API](../functions/auth/src/api/providers/password.md),
  [token](../functions/auth/src/oauth/token.md),
  [revoke](../functions/auth/src/oauth/revoke.md),
  [Google](../functions/auth/src/api/providers/google.md) and
  [Apple](../functions/auth/src/api/providers/apple.md).
- [DynamoDB integration fixture](../functions/auth/tests/support/dynamo.md),
  [storage tests](../functions/auth/tests/dynamodb_storage.md) and
  [test instructions](../../packages/functions/auth/tests/README.md).
- [Stage configuration](../../docs/setup/03-stage-config.md) and
  [deployment instructions](../../docs/setup/09-deploy-auth-core.md).

Update authoritative docs alongside implementation, not ahead of it. Preserve the existing
verification, recovery, credential-generation, provider-validation and admin IAM contracts.

## Admission Contract

### Explicit Endpoint Policies

Replace the single `limits` map with policies containing a source budget and, where required, a
secondary budget. Keep the explicit `enabled` switch. Use one current interface throughout callers
and tests; do not retain composite-only compatibility adapters or dual writes.

Initial defaults below use 60-second fixed windows. They are template starting points, not measured
production capacity. Budgets are per endpoint, not one shared quota across every auth operation.

| Endpoint | Source ceiling | Additional ceiling |
| --- | ---: | --- |
| PasswordRegister | 60 | 5 per normalized email, across sources |
| PasswordLogin | 60 | 5 per normalized email, across sources |
| PasswordResetRequest | 60 | 5 per normalized email, across sources |
| PasswordVerify | 5 | None |
| PasswordResetComplete | 5 | None |
| Authorize | 100 | 100 per configured client/source, or unknown-client/source |
| Token | 100 | 50 per configured client/source, or unknown-client/source |
| OAuthRevoke | 60 | 20 per configured client/source, or unknown-client/source |
| ProviderAuthorize | 60 | 20 per provider/session-digest/source |

The new aggregate defaults follow the reference while retaining Irongate's existing narrower
thresholds. Source-only verification/reset-completion remain at 5, rather than importing the
reference's different email-setup flow and threshold. No new stage/env configuration is needed;
document where the Rust policies are defined. Unused CodeVerify/AdminApi enum values are not proof
of mounted or rate-limited routes; do not expand the endpoint surface to use them.

When enabled, require a policy with the correct dimensions for the endpoint. Validate each count
in `1..=1_000_000` and each window in `1..=86_400` before arithmetic or counter I/O. Missing policy,
invalid bounds, missing required dimension, inappropriate identity type or empty lookup secret
must fail closed as limiter-unavailable. Explicitly disabled limiting performs no counter I/O.

### Identity And Key Privacy

- Source identity comes only from API Gateway v1/v2 request context. Parse with `std::net::IpAddr`,
  canonicalize IPv6 representations and IPv4-mapped IPv6, and never fall back to forwarded headers.
  Missing, blank, invalid or unsupported context uses one bounded unknown-source identity per
  endpoint, not a caller-selected value or an exemption.
- Use existing email normalization. Syntactically valid emails share their email budget across
  IPs, clients and sessions, regardless of account existence. Do not look up an account to choose
  whether to count. Invalid emails consume source allowance without creating arbitrary email rows.
- OAuth secondary identities use only registered client IDs; collapse unregistered names to a
  structural `None` value, not a reserved string that a configured client could also use. Apply
  the rule consistently at authorize, token and revoke. Keep client/source scope, not a global
  client-only bucket that one user can exhaust for every user of a public client.
- Provider authorization retains provider/session-digest/source identity but shares its independent
  source ceiling across Google and Apple starts. Fresh sessions or switching providers cannot
  select a fresh aggregate budget.
- Add purpose-separated HMAC lookup families for source, email, client/source and provider/source
  limiter identities. Use the existing runtime lookup secret. Frame compound inputs unambiguously,
  such as a serialized tuple, rather than delimiter concatenation of caller-selected strings.
- Store counters under the existing `ratelimit` key structure with bounded digest identifiers.
  New limiter keys/values and diagnostics contain no raw IP, email, password, client secret,
  session, code, verification/reset token or refresh token. Do not log the digest identifier either.
  This is a limiter privacy change, not a claim that all audit logging is repaired.

### Source-First Accounting

Check and charge the source bucket before reading or writing a secondary counter. If it denies or
fails, do not allocate a counter for another email/client/session. If a secondary check later denies
or fails, retain the source charge. Never refund or claim atomic multi-bucket accounting.

Use the existing strongly consistent reads and conditional writes. Keep at most eight CAS attempts
per bucket. Check logical fixed-window expiry even when the physical item remains. Counter TTL
stays finite, using the existing twice-window retention; TTL deletion is not authorization.
Reject malformed counters rather than silently resetting them. Check an exhausted count before
incrementing to avoid overflow. Keep an explicit limiter evaluation-time input for focused tests,
without introducing a global clock framework or modifying unrelated store APIs.

Budget exhaustion returns the existing generic 429 shape with positive `Retry-After`, rounded up
from the remaining window. Exhausted CAS contention retains the existing 429/one-second retry
behavior. Storage, decode, configuration and uncertain-write failures return generic 503
limiter-unavailable; they must not reach protected work or be reported as ordinary quota exhaustion.
The counter layer retries confirmed CAS conflicts only, not ambiguous operational write failures.
Diagnostics use fixed operation categories, never raw backend errors that may include keys/values.

### Route Cutover

All existing limiter callers must use the new typed identity/policy contract in this slice:

- Password register/login/forgot count before password hashing/verification, mail delivery or
  account/session/code work. Verify/reset completion remain source-only; do not inspect a bearer
  proof or accept a submitted email merely to derive an extra rate-limit identity.
- Authorize counts before creating an authorize session. Use the query's client ID consistently
  with the authorize handler; an unrelated Basic header must not select its narrower identity.
- Token/revoke count parsed attempts for configured and unconfigured clients before secret
  verification, code/refresh lookup or revocation. Preserve Basic/post authentication rules and
  protocol errors after admission. Do not require successful authentication before counting.
- Google/Apple starts count before reading authorize-session records or creating provider state.
  Keep existing provider-enabled checks, callback state/nonce validation and one-use semantics.

This is not a universal HTTP/edge limiter. Extractor-rejected malformed bodies, early malformed
authentication syntax, disabled providers, discovery, userinfo, callbacks and IAM admin routing are
not expanded into a new protection surface here. Document that limit instead of claiming every
request is counted. Do not add hosted browser forms, native routes or provider-email confirmation.

## Trade-Offs And Boundaries

Independent email budgets reduce distributed guessing but let an attacker temporarily throttle a
known address. Source budgets can affect legitimate users behind NAT or a BFF. In particular, API
Gateway sees the Cloudflare BFF's outbound source, not each end user's browser IP. Preserve that
trust boundary; do not trust `x-forwarded-for` to avoid shared-source throttling. Per-end-user BFF
delegation would require a separately approved design.

Two-dimensional admission normally needs two counter reads and conditional writes rather than one,
plus bounded retries on conflicts. That is the accepted implementation cost of the independent
limits, without new AWS resources. A source-only fix would still allow distributed attacks against
one email; an email-only fix would still allow source-based email spraying. Neither is sufficient
for this contract. These controls do not promise DDoS protection or ideal shared-IP availability.

No backward-compatible key reader, backfill, table scan or credential migration is required. New
counter identities start their own windows; obsolete transient counters may age out through their
existing TTL. Do not delete a deployed table, rotate the lookup secret or rewrite account data.

Out of scope: WAF/CAPTCHA, new AWS services, infrastructure changes, additional email-action pages,
email-first registration, account linking, audit destination changes, native integrations, new
configuration delivery mechanisms, UI tests, load tuning and any deployment.

## Manageable Change Surface

Production changes should be limited to policy/identity definition and existing admission callers:

```text
packages/functions/auth/src/config.rs
packages/functions/auth/src/config/rate_limits.rs       new focused policy module
packages/functions/auth/src/crypto/hmac_lookup.rs
packages/functions/auth/src/ratelimit/middleware.rs
packages/functions/auth/src/store/rate_limits.rs
packages/functions/auth/src/store/mod.rs
packages/functions/auth/src/routes.rs
packages/functions/auth/src/api/providers/password.rs
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/api/providers/apple.rs
packages/functions/auth/src/oauth/token.rs
packages/functions/auth/src/oauth/revoke.rs
```

Move only rate-limit types/defaults out of `config.rs`; do not refactor unrelated runtime config.
Keep the `AuthStore` boundary and one typed admission entry point. The numerous route edits should
be narrow call-site adaptations, not OAuth/provider rewrites. No dependency additions are expected.

Refine the existing `api_routes`, password child, `aws_request_context`, `oauth_refresh_revoke` and
provider-start tests. Use small child modules for rotating-identity/admission scenarios and shared
test-only counter cases for memory and DynamoDB Local. Reuse `support/dynamo.rs` and existing
fixtures; add only a narrowly scoped counter fault/race wrapper if needed. Do not import the
reference's whole storage/clock framework. Keep every source below 1,000 lines; the refresh/revoke
test file is already 808 lines, so new route scenarios should live in a focused child module.

## Focused Verification

### Observable Route Cases

1. One normalized email over changing trusted IPs hits its independent email limit. One trusted IP
   over changing emails hits its independent source limit. Use small policies that isolate each
   dimension, include normalization variants and both known/unknown accounts, and verify an
   unrelated email/source pair remains usable.
2. Exercise register, login and forgot using the real router and current password-first payloads.
   Assert exact admitted responses, then 429. Capture mail and account/proof/code state or protected
   reads to establish that rejection happens before work; a second error status alone is insufficient.
   Source-only verify/reset limits cannot be bypassed by changing their tokens.
3. Rotate registered and unknown client IDs at authorize/token/revoke without resetting the source
   budget. Unknown names share one narrower bucket; a configured client named like the former
   sentinel remains distinct. Another source can still use the same public client. Failed client
   authentication on token/revoke is counted without touching bearer state.
4. Change provider sessions and switch Google/Apple starts at one source. After denial, no new
   provider-state record appears. Preserve valid provider start and callback behavior.
5. Vary spoofed forwarding headers, IPv4/IPv6 representations and missing/invalid API Gateway v1/v2
   context. Invalid emails charge only source. Query nonempty bounded counter partitions before
   asserting key/value privacy; do not pass a privacy test by inspecting an empty result.
6. Limiter failure at either dimension returns generic 503 and no protected side effect. Enabled
   invalid configuration fails closed; disabled configuration performs no counter I/O. Verify 429
   retry headers without exposing internal identity/dimension or raw operational errors.

### Counter And DynamoDB Cases

Run the same meaningful counter cases through the memory fixture and disposable DynamoDB Local:

- Competing requests hold one dimension fixed while varying the other. Successful admissions
  never exceed either cap; source rejection allocates no secondary row.
- Deterministic read/CAS gates produce real stale-write conflicts. Delegate conditional writes
  to the backend under test, not a fake implementation that always succeeds.
- Exact window-boundary reset works while the old row is still physically present. Inspect
  physical `expiry` using the Local fixture; do not wait for TTL cleanup or conflate it with the
  counter's logical window. Inject limiter time without pretending DynamoDB's clock advances.
- Secondary denial/read/write failure retains an already-committed source charge. An operational
  failure, including a deliberately lost response after a real counter write, does not admit the
  request or justify a refund. Use controlled faults, not live outages.
- CAS exhaustion is bounded; malformed counters and invalid policy bounds fail closed.

Consolidate superseded identical-request/one-bucket fixtures when the new cases preserve their
observable coverage. Do not remove unrelated security regressions. Demonstrate that the two main
rotation regressions fail against the old behavior or a targeted guard-bypass mutation, then restore
the guards and rerun. Do not mistake the reference repository's passing tests for verification here.

## Design And Operator Updates

Mirror every touched/new source and test at its exact mapped path, including the new config module,
child tests and shared fixtures. Update the rate-limit overview, HMAC/source-identity docs, affected
route mirrors, testing guide and threat-model limits to describe the implemented what/why/how.
Keep the audit limitations and other still-unresolved threats visible.

Update the test README and its mirror with any new Local test commands. Add a short operator note
to the stage-config setup page identifying Rust policy defaults, shared-source/BFF implications and
the absence of new environment variables or secrets. The deployment guide should state the counter
key cutover behavior without offering compatibility scaffolding or instructing table removal.
No authoritative doc should claim the new protections before implementation verifies them.

## Work And Acceptance

1. Recheck both repositories and the pinned reference; run the existing affected tests as baseline.
2. Implement the single policy/identity contract and all current callers together.
3. Verify rotating-identity route behavior and independent counters/failure accounting, including
   real conditional writes against disposable DynamoDB Local.
4. Update exact design mirrors and operator notes; run regressions and record actual outcomes here.

Before and after the cutover, run the relevant current targets:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test api_routes --test aws_request_context --test oidc_google_start \
  --test oidc_apple_start --test oauth_refresh_revoke
```

Final checks:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/auth/Cargo.toml
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Run the new counter cases explicitly through `--test dynamodb_storage -- --ignored`, using only the
loopback endpoint/dummy-credential/owned-table fixture documented in the test README. Never use a
deployed table or AWS credentials. Clean up owned tables and stop any locally started emulator.
Record emulator prerequisites as blocked if unavailable, not as a passing ignored run. Run
changed-file rustfmt, mirror/link and source-length checks; distinguish existing Clippy warnings
from new ones. TypeScript/setup/native builds are unnecessary unless their inputs change.

Acceptance requires independent email/source ceilings across changing identities, correct early
admission placement, bounded private keys, fail-closed error/configuration handling, conservative
accounting, verified conditional races and preserved auth/recovery/provider regressions. No new
infra, compatibility layer, product behavior or future slice document is part of completion.

## Implementation Result

- Added the focused Rust policy module and one typed rate-limit identity contract. All existing
  password/OAuth/provider-start callers use source-first independent admission; revoke now counts
  parsed attempts before client authentication. Unused limiter-only endpoint variants were removed.
- Added canonical trusted IPs, purpose-separated HMAC identities and structural unknown clients.
  Invalid email inputs consume only source quota. Missing/invalid enabled policies fail closed.
- Preserved bounded CAS, logical windows and cleanup TTL, with remaining-window retry responses,
  conservative accounting and sanitized failure categories. No compatibility reads, scans or
  credential-data changes were introduced.
- Adapted the pinned reference's reusable policy/counter/gate patterns to Irongate's existing
  password-first routes and local fixtures. Existing recovery, JWT validation and provider behavior
  remain unchanged; reference-only endpoints and product values were not imported.
- Added focused route children and shared memory/Local contracts. Superseded one-bucket inline
  limiter tests were consolidated into these observable cases. Touched privacy checks now use
  bounded endpoint queries and nonempty records rather than memory-only broad-prefix assumptions.
- Updated every touched/new file mirror, rate-limit/HMAC/testing/threat-model overviews, the test
  README and setup/deploy notes. Shared NAT/BFF throttling, targeted email denial, sequential quota
  writes and the unchanged audit limitations remain explicit.

### Verification

All Rust commands below used `--locked --offline` with existing local dependencies.

| Check | Result |
| --- | --- |
| Affected route/provider baseline at `8eaffde` | 51 passed |
| Full auth `cargo test` after final edits | 258 passed, 0 failed; 29 Local cases ignored by default |
| Explicit full `dynamodb_storage -- --ignored` | 29 passed against DynamoDB Local 3.3.2 / Java 17 |
| Source-guard bypass mutation | Rotation regression failed: admitted 200 instead of required 429 |
| Email-guard bypass mutation | Rotation regression failed: admitted 200 instead of required 429 |
| Restored guards | Full auth suite passed; no mutation remains |
| Auth and admin `cargo check` | Passed |
| Auth all-target Clippy | Passed with existing unrelated/style warnings; new wrapper-use dead-code warnings resolved |
| `npm run test:infra` | Passed |
| Changed-file rustfmt, exact mirrors, relative links, source lengths and `git diff --check` | Passed |

The full auth count includes unit tests compiled into both library and bootstrap targets. The seven
new Local cases were run explicitly as well as with the full 29-case storage suite; an ordinary
ignored result was not counted as execution. Real Local conditions handled competing CAS writes and
post-commit response-loss faults. Test tables were confirmed absent before stopping the in-memory
emulator. The log-capture regression initially failed under a per-thread subscriber in parallel tests;
one process-wide subscriber made capture consistent, and the final full suite passed.

TypeScript typecheck, setup tests, examples/native builds and live AWS/provider tests were not run:
their implementation/configuration inputs were unchanged and no deployment was authorized. This
slice does not establish production threshold tuning, NAT/BFF fairness, edge DDoS protection or
live cloud failure behavior. No next slice was created.
