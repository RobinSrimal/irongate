# Auth Integration Tests

Integration tests in this directory are named by auth domain or protocol surface, not by the implementation slice that introduced them.

Use source-local `#[cfg(test)]` modules for focused pure-module tests, especially config parsing, crypto helpers, key helpers, and small request-context helpers.

Keep tests in this directory when they exercise:

- Axum routers.
- OAuth or OIDC protocol flow across modules.
- Account lifecycle behavior through public or IAM-protected APIs.
- End-to-end typed store state across multiple store modules.
- Provider callbacks with fake OIDC clients.

`support/mod.rs` contains shared integration-test infrastructure. New integration test files must not use `*_slice.rs` names or numeric slice prefixes.

`api_routes.rs` covers CORS, OAuth routing, trusted source identity and excluded public routes.
Its `api_routes/password.rs` child module covers password route responses and rate limits using
the same router fixtures. `api_routes/admission.rs` and `rate_limits.rs` exercise independent
email/source limits, changing OAuth clients/provider sessions, protected-work rejection, canonical
source identities and failed client authentication. All run with `cargo test --test api_routes`.

Security regression tests should be named by the behavior or risk they protect. Finding IDs can appear inside test names or comments when they add useful context, but file names should remain domain-oriented.

## DynamoDB Adapter Tests

`dynamodb_requests.rs` runs with ordinary `cargo test`. It sends actual AWS SDK calls to a loopback
HTTP stub to check strong-read flags, continuation requests, error classification, safe diagnostics,
timeouts and refusal to follow redirects. It does not emulate DynamoDB transactions.

`dynamodb_storage.rs` is ignored by default and requires a disposable DynamoDB Local instance. It
checks full-value/TTL replacement, rollback after a confirmed cancellation, stale authorization-code
consumes, invalid transactions, persistent account lifecycle writes and cleanup after success/panic.
Its `dynamodb_storage/refresh.rs` module checks refresh-family TTL renewal, old-token/index retention,
revocation deadlines, logical expiry and competing rotations/revocations. A bounded pre-commit gate
orders competing writers but delegates the actual transaction to DynamoDB. Raw exact-key reads and
small partition queries inspect physical TTL separately from JSON `expires_at`; no scans or sleeps
waiting for background TTL deletion are used.

`dynamodb_storage/recovery.rs` checks atomic proof/index consumption, actual password replacement,
generation invalidation, independent rejection cases, missing-generation failure, overflow and an
expiry boundary with an injected recovery-only clock. `recovery_races.rs` orders reset against other
resets, login, code consumption, refresh issuance/rotation, link creation and disable/delete. The
gate can select a transaction or read without changing backend conditions. Fault cases distinguish
failure before dispatch from a deliberately lost response after a real successful commit. An error
response is not proof of rollback. These are controlled application tests, not production network
certification. Shared recovery fixtures use real domain registration/verification and an in-process
mailbox, never live email or customer accounts.

`registration_contract.rs` and `dynamodb_storage/registration.rs` run eight shared cases from
`support/registration_contract.rs`. They cover six-write activation and independent rollback at
each condition; same/sibling proof competition with different real password hashes and login
checks; issuance racing completion in both orders; stale pending records and identity tombstones;
deleted/recreated binding rejection; controlled-clock expiry; and failure before dispatch versus
a lost response after an actual commit. Exact candidate account/index reads detect loser orphans.
Email, identity and proof keys use the actual HMAC families. The Local child also checks physical
verification TTL, cancellation retaining raw rows/TTL, and all five activated rows remaining
persistent. Logical expiry advances only registration's clock, not storage time or TTL cleanup.

The shared suite reuses `transaction_gate.rs` and the login helper from the crate-root
`recovery_fixture` path module. Recovery imports `crate::support::registration`, sharing the single
registration fixture declared by `support/mod.rs`. Both runners include `mod support`; the Local
runner still supplies `DynamoStorage`, not `TestStorage`. This module organization does not change
the runtime storage backend. Focused commands:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml --test registration_contract
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage registration:: -- --ignored
```

`provider_issuance.rs` and `dynamodb_storage/provider_issuance.rs` run eight shared contracts from
`support/provider_issuance.rs` and its `outcomes.rs` / `races.rs` children. They check usable,
client/PKCE/nonce-bound codes for new and returning Google/Apple identities, stable subjects when
email changes/disappears, and no linking to each other or an existing verified password account.
The password fixture uses actual email-first registration; its credential/account/index records
must remain unchanged. Identity properties and the narrower authorization-code claims are checked
separately. Code and identity keys use the real HMAC lookup families.

Independent condition failures cover every new/deleted write and returning-account/identity/code
guard, including collisions with real usable codes. Exact candidate account/index/code reads prove
rollback and absence of loser orphans. Gates order new, returning and reusable-deleted callbacks
both ways, as well as issuance against disable/delete, missing accounts, generation changes and
identity updates/deletion. Preparation rejects malformed bindings and sessions, and reuse cases
exercise never/immediate/retention policies including missing retention metadata. Controlled clocks
check session expiry before preparation, at finish and immediately before dispatch; invalid TTLs
and timestamp overflow fail without writes. Read failure, pre-dispatch failure and lost response
after real commit remain operational errors with the corresponding stored outcomes.

The Local child separately checks physical code TTL, persistent account/identity/index rows and
raw metadata/TTL retention after a confirmed code collision. Account guards invalidate already
issued codes after disable/delete or generation change; identity-only changes do not imply an
additional identity-history check at token exchange. These tests do not validate upstream JWTs,
live providers, consumed-state rollback, lifecycle epochs or immediate revocation of existing JWTs.

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml --test provider_issuance
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage provider_issuance:: -- --ignored
```

`rate_limits.rs` and `dynamodb_storage/rate_limits.rs` instantiate the same six counter contracts
from `support/rate_contract.rs`: exact window rollover, competing independent budgets, conservative
failure accounting, bounded conflicts/corrupt counters, invalid policies and private identifiers.
`support/rate_control.rs` gates reads before real CAS and injects pre-write failures or a lost
response after a successful write. The Local child also reads physical TTL on creation/rollover.
Limiter evaluation time is explicit; storage time and asynchronous TTL cleanup do not advance with
it. The memory target additionally captures sanitized diagnostics. Targeted commands:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml --test rate_limits
# Requires the disposable emulator described below:
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage rate_limits:: -- --ignored
```

Install Java 17 or newer and download/extract DynamoDB Local using the
[AWS instructions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/DynamoDBLocal.DownloadingAndRunning.html).
From the extracted directory, start a temporary in-memory instance on an unused port:

```sh
java -Djava.library.path=./DynamoDBLocal_lib -jar DynamoDBLocal.jar \
  -inMemory -sharedDb -disableTelemetry -port 18023
```

Keep the emulator on a trusted local machine; the Java launcher can listen on all interfaces and
does not require real AWS credentials. Do not expose its port to an untrusted network.

From the repository root:

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml --test dynamodb_requests
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
```

The fixture uses explicit dummy credentials, a fixed region, disabled SDK retries and a two-second
operation timeout. It rejects non-loopback endpoints, URL credentials, paths, queries and fragments;
`localhost` is pinned to `127.0.0.1`. It never reads AWS profiles, SST state or supplied table names.
Each case creates a unique `irongate-contract-*` table and deletes only that table, including after a
test panic. Process termination cannot guarantee cleanup; stop the in-memory emulator with Ctrl+C
after the run. Do not use a forwarding tunnel to deployed DynamoDB.

When the execution sandbox blocks local sockets, request the narrow permission needed for the
loopback test process; do not switch to a remote endpoint. If approval requires a loopback-only
listener, the standard launcher above is insufficient: configure the embedded server's connector
host to `127.0.0.1` before starting, and verify the listener with `lsof -nP -iTCP:18023 -sTCP:LISTEN`.
Never retry an all-interface launcher after its exposure has been rejected.

These cases do not establish complete equivalence between memory fixtures and DynamoDB, production
IAM correctness, every lifecycle race or physical TTL cleanup. A normal test run with ignored local
cases does not verify those cases.

## Audit Output Tests

`src/audit/tests.rs` checks bounded JSON, concurrent/partial writes, output/flush failures, safe
fallback and explicit suppression. A subprocess captures real stdout/stderr with restrictive
tracing filters. `admin_lifecycle/audit.rs` and `oauth_refresh_revoke/audit.rs` cover all ten events
through real routers and confirm outcomes/state, actor provenance, sensitive-marker exclusion and
signing failure after rotation. `support/audit.rs` injects capturing/failing writers, not fake events.

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml --lib audit::
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test admin_lifecycle --test oauth_refresh_revoke audit::
npm run test:infra
```

The infra command includes a resolved-module harness with inert resource constructors for both
function logging settings. These checks do not contact AWS or prove CloudWatch delivery/retention.
None mode must touch neither audit sink; ordinary operational/access logs remain independent.
