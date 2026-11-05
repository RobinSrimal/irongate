# Auth Testing

## What

Source-local unit tests and domain-oriented integration tests under
[tests](tests/README.md). The public crate is also compiled into its bootstrap binary, so some unit
tests run in both targets.

## Why

Pure configuration/crypto checks and cross-module protocol tests establish different contracts.
Neither substitutes for AWS delivery, storage consistency or real provider/device acceptance.

## How

Run `cargo test --locked --manifest-path packages/functions/auth/Cargo.toml`; select a named
integration target for focused work. [api_routes](tests/api_routes.md) covers public routes and its
[password module](tests/api_routes/password.md) covers password routes. Admin, provider, recovery,
refresh and signer suites have their own file mirrors describing actual coverage and limitations.

[Integration TestStorage](tests/support/mod.md) and the
[unit-test backend](src/storage/test_support.md) avoid deployed services. Their condition, expiry and
pagination semantics are not identical to DynamoDB. Both use full-value Put
replacement with explicit TTL. The [SDK wire suite](tests/dynamodb_requests.md)
checks consistent-read flags, pagination and sanitized rejection of malformed query keys against
a loopback HTTP stub. The
[DynamoDB Local suite](tests/dynamodb_storage.md) separately checks full-value replacement, physical
TTL removal/renewal, persistent accounts and one-time consumes. Its
[refresh module](tests/dynamodb_storage/refresh.md) covers real cancellation rollback, competing
rotations and rotation racing with revocation using a bounded pre-commit gate. It is ignored by
default and must be run explicitly with the local prerequisite in the
[test guide](tests/README.md); neither suite certifies every storage
race or production behavior. Signed provider fixtures and fake KMS/email clients do not prove real
provider, IAM, KMS or mail delivery.

Infrastructure checks include static source checks and a local resolved-config audit harness with
inert resource constructors. Neither proves deployed logging/IAM; smoke tests require separate operator
authorization. No UI tests or live destructive table tests are part of normal validation.

Local [recovery](tests/dynamodb_storage/recovery.md) and
[recovery races](tests/dynamodb_storage/recovery_races.md) check atomic reset, generation-bound
credentials, TTL/persistence, isolated invalid records and competing code/login/refresh/lifecycle
writes. The shared [recovery fixture](tests/support/recovery.md) uses actual domain operations.
Selected transaction/read gates let DynamoDB evaluate stale conditions; controlled faults distinguish
no dispatch from a committed reset whose response was lost. A narrow injected clock proves expiry
after preparation. These recovery cases do not establish registration completion's separate
transaction contract or atomic JWT response delivery.

The registration runner and its Local counterpart share eight-operation activation contracts,
including email owner and reverse-index conditions, pending/resend non-ownership, proof races,
exact rollback and lost-response outcomes. Shared ownership lifecycle/race cases run in
`registration_contract` and the Local `ownership` module. They check active-owner denial without
linking, malformed/read failures, retention boundaries and snapshot guards, cleanup retry and
deletion/reuse/stale-index races. Raw Local checks inspect physical TTL and verify that malformed
index values cannot be silently omitted by either query path. Fixtures use fresh disposable data;
they do not provide historical backfill or make the whole deletion lifecycle atomic. Commands and
backend limitations are in the [test guide](tests/README.md).

The Google/Apple suites instantiate [signed validation contracts](tests/support/oidc_validation.md)
with fixed validation times and specific error assertions. Each negative claim is freshly signed
and independently invalid. Their callback rejection child modules pair bounded refresh counts with
token outcomes and unchanged identity/account/code state for new and existing users. These use
controlled provider clients, not real HTTP caches or live providers; they do not establish cache TTL
behavior or OIDC conformance for unimplemented claims.

[Rate-limit contracts](tests/support/rate_contract.md) run unchanged against integration memory and
DynamoDB Local for independent caps, exact window rollover, real CAS races, conservative failure
accounting, corrupt counters and fail-closed configuration. Local raw reads check physical TTL.
[Route tests](tests/api_routes/rate_limits.md) vary emails/sources, clients and proofs; the
[admission module](tests/api_routes/admission.md) checks protected work and provider/session changes.
Diagnostic capture excludes identities and backend error text. These tests do not establish
production thresholds, edge abuse protection or fairness for users sharing a BFF source.

[Audit tests](src/audit/tests.md) capture serializer/writer bytes, safe failures and real process
streams under restrictive tracing filters. [Admin](tests/admin_lifecycle/audit.md) and
[OAuth](tests/oauth_refresh_revoke/audit.md) route children assert all existing events alongside
state/results, including rotation before failing signing. Output failures do not change auth
outcomes; deliberate dropped-event/diagnostic mutations must be caught by positive checks.
