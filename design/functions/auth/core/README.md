# Auth Core

Implementation: [core/mod.rs](../src/core/mod.md).

## What And Why

The core directory holds framework-independent client, scope, subject, token-claim and password
policy types. These shared contracts keep HTTP parsing and AWS SDK calls out of policy primitives.

## How

- [Clients](clients.md) validate configured profiles, redirects, grants and secrets.
- [Scopes](scopes.md) define supported scope names and policy.
- [Subjects](subjects.md) represent persisted user identifiers.
- [Tokens](tokens.md) describe signed claims and opaque refresh state.
- [Passwords](passwords.md) connects policy primitives to password-domain operations.

The topic pages on [account lifecycle](account-lifecycle.md) and [identity](identities.md)
describe behavior implemented mainly in the typed store and API/provider modules, not same-named
files under `src/core`. Shared [errors](errors.md) live in `src/error.rs`.
