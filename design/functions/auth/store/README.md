# Auth Store

Implementation: [store/mod.rs](../src/store/mod.md) and the file mirrors under
[`src/store`](../src/store/).

## What And Why

`AuthStore` exposes typed account, identity, password, OAuth, refresh and rate-limit operations
to route/provider code. The production constructor owns a DynamoDB backend; tests inject an
internal backend. This keeps auth record/key decisions out of HTTP handlers.

## How

Purpose-specific modules serialize records and construct HMAC-keyed lookups. Account/identity
lifecycle helpers live in `store/mod.rs`; OAuth and password storage have separate modules.
Conditional writes protect individual updates, and transactions group selected multi-record
changes such as refresh rotation. [Recovery](../src/store/recovery.md) groups proof/index consumption,
password replacement and generation increment into one conditional commit. The
[generation helpers](../src/store/credential_generation.md) guard code and refresh writes against
stale accounts; password code issuance also guards the verified hash snapshot.

[Provider issuance](identities.md) prepares private identity/account/session snapshots, then commits
new account/identity/index/code writes or returning identity/code updates with the exact account
generation guard in one transaction. [Registration](../src/store/registration.md) likewise groups
proof consumption and initial credential/account/index writes. Both keep preparation non-mutating.

The generic [storage adapter](../src/storage/adapter.md) still exists underneath the typed store.
Provider-state/session consumption, external exchange, multi-step lifecycle cleanup and JWT delivery
remain outside those transactions. See
[DynamoDB semantics](dynamodb.md), [accounts](accounts.md), [identities](identities.md),
[password secrets](password-secrets.md) and [refresh tokens](refresh-tokens.md).

Record/key details are described in [records](records.md) and [keys](keys.md).
Runtime reads check logical expiry; DynamoDB TTL is asynchronous cleanup.
