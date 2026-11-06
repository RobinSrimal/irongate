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
generation guard in one transaction. [Registration](../src/store/registration.md) groups proof
consumption and initial credential/account/identity/email-owner/index writes into eight operations.
Both keep preparation non-mutating. Pending registration does not claim email ownership. New Google/Apple
identities resolve authoritative email through the internal [ownership module](../src/store/email_ownership.md)
and atomically guard/create owner/account/index state with identity/code writes. Returning identities
take precedence over email. New identities without authority reject without a separate-account
fallback. Password completion still rejects active owners.

The [confirmation store](../src/store/sign_in_confirmation.md) provides a separate, unexposed Google
mailbox-proof boundary. It shares the private new-provider plan, persists guarded snapshots without
claiming ownership, and atomically consumes a browser/email-bound pending record with issuance.
It never re-resolves the account after waiting and adds no route, hosting or delivery dependency.

Delete runs email cleanup after its existing credential/session cleanup. It atomically clears each
indexed owner's subject and deletes its exact reverse index, with guarded stale-index removal.
Errors propagate for retry through the existing delete action; this is not whole-deletion atomicity.
Owner/index/tombstone records have neither raw email nor TTL. There is no historical/lazy backfill.

The generic [storage adapter](../src/storage/adapter.md) still exists underneath the typed store.
Provider-state/session consumption, external exchange, multi-step lifecycle cleanup and JWT delivery
remain outside those transactions. See
[DynamoDB semantics](dynamodb.md), [accounts](accounts.md), [identities](identities.md),
[password secrets](password-secrets.md) and [refresh tokens](refresh-tokens.md).

Record/key details are described in [records](records.md) and [keys](keys.md).
Runtime reads check logical expiry; DynamoDB TTL is asynchronous cleanup.
