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
the same router fixtures. Both run with `cargo test --test api_routes`.

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

These cases do not establish complete equivalence between memory fixtures and DynamoDB, production
IAM correctness, every lifecycle race or physical TTL cleanup. A normal test run with ignored local
cases does not verify those cases.
