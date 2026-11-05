# Provider Email Authority

Source: [packages/functions/auth/src/providers/verified_email.rs](../../../../../packages/functions/auth/src/providers/verified_email.rs).

## What

One opaque `ProviderVerifiedEmail` for Google and Apple account selection.

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
