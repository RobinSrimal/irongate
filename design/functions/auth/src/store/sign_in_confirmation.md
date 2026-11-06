# Atomic Google Email Confirmation

Source: [packages/functions/auth/src/store/sign_in_confirmation.rs](../../../../../packages/functions/auth/src/store/sign_in_confirmation.rs).

## What

Typed creation, proof preparation and atomic completion of a pending Google email confirmation.
The internal [delivery operation](../providers/email_confirmation.md) calls creation; no HTTP route,
cookie, form or configuration exposes confirmation.

## Why

A signed third-party Google address is not sufficient mailbox authority. Fresh email proof and
the original browser binding must authorize only the account state selected before the wait.

## How

`GoogleEmailCandidate` comes from already-validated Google claims lacking direct email authority.
`ConfirmationDigests::from_secrets` requires 32-character base64url request/browser handles and
a nonempty bounded email proof. Each uses a distinct HMAC family. Email proof input includes the
fixed-length request digest and a colon, binding identical codes to different requests.
No raw handle, browser binding or code enters keys, records, errors or logs.

Creation receives a consumed Google authorize-session snapshot, candidate, digests, positive
lifetime and reuse policy. It captures a private [new-provider plan](provider_plan.md), rejecting
active identities. Five condition checks plus a pending-record Put guard account, identity and
ownership state. Only the pending record is written: it neither reserves email nor creates an
account, identity, index or authorization code. Duplicate handles cannot overwrite or extend it.
Confirmed creation returns the exact stored expiry, rather than a fresh deadline calculated after
I/O. The crate-only confirmation clock accessor shares provider time with the delivery operation.

The private record at `oauth:email_confirmation / request_digest` contains a purpose tag, identity
and email binding, normalized address, proof digests, original session, resolution and timestamps.
Its required current shape rejects unknown top-level fields. Logical expiry is capped by the
original session and has matching DynamoDB TTL. Snapshot fields are validated on load; the address
must normalize exactly and match its Email HMAC. Pending records contain sensitive short-lived
contact/snapshot data, but no usable password credential or verified email authority.

Preparation reads the exact key consistently, checks record/session expiry with the provider clock,
then compares both proof digests in constant time. Unknown, expired or mismatched proofs return
None without writes. Malformed records and outages remain sanitized storage errors. The returned
prepared value has private fields, cannot be deserialized by callers and retains the original JSON
for exact deletion. Preparing twice does not consume anything.

Finishing checks expiry and a valid code lookup digest, then performs seven operations: the same
six new-provider issuance operations plus deletion of the exact confirmation value. Client, redirect,
scope, PKCE, nonce and generation come from the stored session/account. State remains in the
stored session, not token claims. Code email is normalized with `email_verified=true` based on
mailbox proof. Code TTL starts at completion; confirmation/session expiry is never extended.
Expiry is rechecked before dispatch. A positive checked code lifetime is required.

There is one application transaction attempt and no re-resolution, compensation or adoption of a
new generation. Conditions guard pending value, account, ownership, identity and indexes. Confirmed
rejection leaves all writes unchanged; a lost response can follow a committed deletion and usable
code. Backend conflict/outage errors retain their classification. Checks before dispatch are not a
server-side deadline or lifecycle-history marker.

This module does not establish browser cookies, CSRF checks, rate limits or email delivery. Proof
comparison alone is not online brute-force protection. Public Google callbacks still reject new
identities without direct authority. Storage contracts and captured-delivery tests exercise these
methods without exposing an HTTP completion flow.
