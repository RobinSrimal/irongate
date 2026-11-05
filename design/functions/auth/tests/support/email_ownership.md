# Shared Email Ownership Contracts

Source: [packages/functions/auth/tests/support/email_ownership.rs](../../../../../packages/functions/auth/tests/support/email_ownership.rs).

## What

Backend-independent ownership rejection and registration conditions, plus the common runner for
ownership lifecycle and race cases.

## Why

Email proof completion must establish one owner without attaching credentials to another account,
reinterpreting malformed data as availability, or replacing a changed ownership snapshot.

## How

The existing registration fixture supplies HMAC keys, real proof preparation/completion and real
password hashes. Ownership is inspected through test storage; there is no public ownership API.
Deliberate stored fixtures represent independent active owners, malformed bindings/state combinations,
unknown fields and invalid retention ordering. Active owners block preparation even when their
account is disabled, deleted or missing; malformed records and failed owner reads are operational
errors with safe diagnostics. Proofs, pending credentials and ownership remain unchanged on denial.

Controlled registration time tests never/immediate/retention policy, missing reuse permission and
the exact eligibility boundary without advancing storage TTL. An eligible owner cannot override an
ineligible password identity. A real completion/deletion produces the tombstone used in an ordered
stale-snapshot test: a valid competing deadline change rejects the whole completion transaction,
while a fresh preparation can later create a new subject.

The exported runner includes [lifecycle](email_ownership/lifecycle.md) and
[race](email_ownership/races.md) cases. Memory and Local execute the same operations; gates delegate
actual commits to the selected adapter. Raw writes construct intentionally irregular state, not
fake successful backend responses. This suite does not implement provider linking or certify
historical ownership coverage.
