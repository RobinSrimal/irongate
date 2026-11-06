# Google HTTP Flow

Source: [packages/functions/auth/src/api/providers/google.rs](../../../../../../packages/functions/auth/src/api/providers/google.rs).

## What

Google authorize and GET callback orchestration.

## Why

Provider authentication must be bound to the client's original authorize session and starting browser.

## How

Authorize checks provider configuration, then charges a source ceiling shared with Apple and a
provider/session-digest/source budget before session lookup or state creation. Valid sessions create
separate HMAC-keyed state with nonce, upstream PKCE and a required browser HMAC. State, browser proof,
nonce and verifier are independently generated. State expiry is the lesser of the configured provider
TTL and original session deadline. Only confirmed creation sets the transaction-specific
[host-only cookie](browser_binding.md), with positive remaining lifetime rounded down to seconds.

Callback validates the state handle and exactly one matching cookie, then uses typed state
consumption with the browser HMAC. Wrong proof/provider or replay cannot consume the session or
reach exchange. Missing/malformed persisted Google binding is a sanitized storage error, not an
unbound fallback. Bound provider declines follow the original client error redirect. After confirmed
state consumption, the callback consumes the session, exchanges code and validates the signed ID
token before preparing atomic identity/code issuance. MissingKey, which includes
absent/blank/unmatched IDs, permits one forced refresh and full revalidation. A failed refresh or
second rejection stops the callback; other signature/key/claim failures do not force refresh.
Rejected provider tokens cannot resolve/update identities or create accounts/codes, but their
state/session has already been consumed. After confirmed state consumption, success and handled
terminal failures clear only that transaction's cookie. All handler responses, including query
rejections and rate limits, use no-store and no-referrer. Fake-client tests establish this
orchestration, not browser enforcement, live Google acceptance or HTTP cache freshness.

After validation, `ProviderIdentityInput` carries a provider/identity-bound `ProviderVerifiedEmail`
when Google is authoritative, the issuer/subject HMAC digest and current metadata:
provider, issuer, signed email/verification, name and picture. The consumed session, configured
code TTL and deleted-identity policy go to `prepare_provider_issuance`. Preparation writes nothing
and pins the existing identity/account generation or plans a new subject; the callback does not
reread the account or choose a newer generation.

The callback generates a raw code, passes only its HMAC digest to `finish_provider_issuance`, and
redirects with code and original client state only after confirmed commit. The store atomically
commits identity/account/index work and the code, rechecks session expiry and restricts code claims
to provider, email and email_verified. Signed email changes or omissions cannot move an existing
issuer/subject identity. A new identity requires verified Gmail or valid signed hosted-domain
authority and uses the same six-operation ownership transaction as Apple. An active owner can
originate from password, Google or Apple. Missing authority rejects without creating a separate
account; third-party-mailbox confirmation is not implemented. Signed `hd` is used only by the proof
factory, not copied into metadata or tokens. Request parameters cannot supply authority.

The shared [error mapper](issuance.md) gives confirmed conditional/existence failures a fixed
invalid_grant response and all other storage failures a fixed server_error. Neither exposes SDK
text or redirects with a code. Callback state/session storage failures use the same mapper.
Earlier consumes are not undone; a failed or uncertain callback requires a new sign-in. Lost
responses may leave a committed code, so the callback performs no compensation or issuance retry.
