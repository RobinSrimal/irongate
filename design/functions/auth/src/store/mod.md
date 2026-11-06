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
persistent. Deletion marks the account, cleans identities/password users/reset proofs and revokes
refresh families, then invokes [email ownership cleanup](email_ownership.md) before returning success.
Ownership transition and exact subject-index deletion commit together for each indexed email;
stale-index removal also guards the current owner value or its absence. Errors propagate so the
existing delete action can be retried. Earlier cleanup is not rolled back, and deletion is not one
atomic whole-account transaction. Disable/enable do not change email ownership.

Every new account starts with required `credential_generation=0`. Lifecycle replacements retain it;
recovery advances it conditionally. Separate recovery, registration and provider-issuance clocks
default to UTC and can be injected for deterministic expiry-boundary tests without changing other
store clocks. Email-owner retention uses registration's injected time; password-identity retention
retains its existing evaluator.
Recovery and generation guards live in their own store modules.

The provider-issuance clock also controls provider-state creation/consumption and durable
confirmation. State consumption validates the typed provider/browser proof and deletes the exact
captured value once; it does not retry a newer record or restore state after uncertain outcomes.

[Registration](registration.md) owns prepare/finish completion: validated private snapshots precede
hashing, and one eight-operation transaction consumes the proof and activates the password account
with its email owner and reverse index. Ownership planning stays internal to the store; HTTP
callers cannot assemble it independently or claim an address during pending registration.
[Password users](password_users.md) own pending creation/recreation and tombstones;
[password secrets](password_secrets.md) own conditionally bound proof issuance and reset cleanup.
The facade's deleted-identity checks preserve the configured reuse policy. Registration neither
reactivates an account nor links accounts across providers. An active email owner blocks completion
regardless of its account status; ownership does not attach passwords to another subject.

Provider issuance captures the exact identity/account once, then commits account/index creation
or an identity update together with a short-lived code. Conditional rejection is not retried by
adopting a newer account generation. Earlier provider/session consumption remains separate.

The private [provider plan](provider_plan.md) shares new-identity ownership conditions between direct
issuance and [durable Google confirmation](sign_in_confirmation.md). Confirmation creation writes
only a guarded pending record. Completion consumes both proofs and performs pinned identity/code
issuance atomically. It shares the provider-issuance clock and is not exposed by an HTTP route.

Audit emission is outside AuthStore; there is no audit persistence facade or new audit table write.

The rate-limit facade accepts a typed identity and delegates source-first admission to
[rate_limits](rate_limits.md); route callers do not construct raw counter keys.
