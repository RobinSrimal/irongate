# Typed Auth Store

Source: [packages/functions/auth/src/store/mod.rs](../../../../../packages/functions/auth/src/store/mod.rs).

## What

The AuthStore facade, account/identity lifecycle and internal record helpers.

## Why

Routes use purpose-specific operations rather than raw backend keys.

## How

Wraps a private Arc<dyn StorageAdapter>. Provider identity resolution and code issuance are owned
by [provider issuance](provider_issuance.md), not separate resolve-then-write methods. Existing
provider identities retain their stored subject, not an email match. Disable, enable and account
tombstoning compare the previous complete value on a replacement Put with no TTL. Accounts remain
persistent. Deletion marks the account then performs indexed cleanup in separate operations, so it
is not one atomic whole-account transaction.

Every new account starts with required `credential_generation=0`. Lifecycle replacements retain it;
recovery advances it conditionally. Separate recovery, registration and provider-issuance clocks
default to UTC and can be injected for deterministic expiry-boundary tests without changing other
store clocks or deleted-identity retention evaluation.
Recovery and generation guards live in their own store modules.

[Registration](registration.md) owns prepare/finish completion: validated private snapshots precede
hashing, and one six-operation transaction consumes the proof and activates the password account.
[Password users](password_users.md) own pending creation/recreation and tombstones;
[password secrets](password_secrets.md) own conditionally bound proof issuance and reset cleanup.
The facade's deleted-identity checks preserve the configured reuse policy. Registration neither
reactivates an account nor links accounts across providers.

Provider issuance captures the exact identity/account once, then commits account/index creation
or an identity update together with a short-lived code. Conditional rejection is not retried by
adopting a newer account generation. Earlier provider/session consumption remains separate.

Audit emission is outside AuthStore; there is no audit persistence facade or new audit table write.

The rate-limit facade accepts a typed identity and delegates source-first admission to
[rate_limits](rate_limits.md); route callers do not construct raw counter keys.
