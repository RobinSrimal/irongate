# Provider Email Authority

Source: [packages/functions/auth/src/providers/verified_email.rs](../../../../../packages/functions/auth/src/providers/verified_email.rs).

## What

Opaque `ProviderVerifiedEmail` authority for Google/Apple account selection and a distinct
`GoogleEmailCandidate` for the store-only mailbox-confirmation path.

## Why

Signed profile metadata and current mailbox authority are not interchangeable. Authority must bind
the provider and its issuer/subject identity, not merely an address supplied alongside a request.

## How

Factories receive typed claims only after the callback validates the complete ID token. They do not
verify signatures themselves. Both require true email verification and existing email normalization.
Apple accepts the literal verified address, including private relay. Google additionally requires
exact `gmail.com` or signed `hd` with valid ASCII DNS labels (1-63 bytes each, at most 253 overall,
alphanumeric/hyphen only, no empty or edge-hyphen labels). Hosted domain need not equal email domain.
Missing authority returns None. No alias, plus-address, dot or relay-destination collapsing occurs.

Private fields hold provider kind, identity digest, email and Email HMAC digest. There is no public
raw-email or JSON-deserializable constructor. Store code checks provider and identity together before
using the proof. Unit tests exercise domain-length boundaries, literal key normalization and binding;
signed callback tests establish the actual JWT-to-proof boundary. Proofs are not Debug/Serialize.

`GoogleEmailCandidate::from_google_claims` also requires already-validated claims. It rejects
missing/malformed email, blank issuer/subject and claims eligible for direct Google authority.
Its private fields bind the identity digest, normalized email and Email HMAC; signed but unverified
email is permitted as a candidate, not authority. It cannot be passed as `ProviderVerifiedEmail`.
Only successful browser/email proof completion can authorize its pinned account plan. Candidate
tests distinguish direct authority from literal third-party email and reject malformed input.
