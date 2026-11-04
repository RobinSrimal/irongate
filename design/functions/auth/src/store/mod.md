# Typed Auth Store

Source: [packages/functions/auth/src/store/mod.rs](../../../../../packages/functions/auth/src/store/mod.rs).

## What

The AuthStore facade, account/identity lifecycle and internal record helpers.

## Why

Routes use purpose-specific operations rather than raw backend keys.

## How

Wraps a private Arc<dyn StorageAdapter>. Account/identity/index creation is transactional; existing
provider identities resolve to their stored subject, not by email. Disable, enable and account
tombstoning compare the previous complete value on a replacement Put with no TTL. Accounts remain
persistent. Deletion marks the account then performs indexed cleanup in separate operations, so it
is not one atomic whole-account transaction.

Every new account starts with required `credential_generation=0`. Lifecycle replacements retain it;
recovery advances it conditionally. The recovery-only clock defaults to UTC and can be injected for
expiry-boundary tests. Recovery and generation guards live in their own store modules.

Audit emission is outside AuthStore; there is no audit persistence facade or new audit table write.

The rate-limit facade accepts a typed identity and delegates source-first admission to
[rate_limits](rate_limits.md); route callers do not construct raw counter keys.
