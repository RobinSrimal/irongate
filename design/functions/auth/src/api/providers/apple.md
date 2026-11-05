# Apple HTTP Flow

Source: [packages/functions/auth/src/api/providers/apple.rs](../../../../../../packages/functions/auth/src/api/providers/apple.rs).

## What

Apple authorize and form-post callback orchestration.

## Why

Apple's response mode and client-secret JWT need provider-specific handling without changing OAuth client contracts.

## How

Authorize checks provider configuration, then charges a source ceiling shared with Google and a
provider/session-digest/source budget before session lookup or state creation.
Creates bound state/nonce/upstream PKCE, exchanges the returned code using an Apple client-secret
JWT and validates the ID token. MissingKey, including absent/blank/unmatched IDs, permits one forced
refresh and full revalidation. Failed refresh or second rejection stops the callback; other
signature/key/claim failures do not force refresh. Rejected tokens cannot resolve/update identities
or create accounts/codes; state/session consumption already occurred. Fake-client tests prove that
orchestration, not live cache behavior. Unsigned callback `user` profile data is ignored.

After validation, `ProviderIdentityInput` carries the issuer/subject HMAC digest and current metadata:
provider, issuer, signed email/verification and is_private_email. The consumed session, configured
code TTL and deleted-identity policy go to `prepare_provider_issuance`. Preparation pins the exact
identity/account generation or plans a new subject without writes. No subsequent account reread
can substitute a newer generation.

The callback generates a raw code and passes only its HMAC digest to `finish_provider_issuance`.
The store rechecks session expiry and atomically commits identity/account/index work and the code;
code claims contain only provider, email and email_verified. Only confirmed success redirects with
the code and original client state. Changed or missing signed email retains the issuer/subject
identity; unsigned profile fields and equal emails do not authorize linking.

The shared [error mapper](issuance.md) turns confirmed conditional/existence failures into fixed
invalid_grant responses and other storage failures into fixed server_error responses. Callback
state/session storage failures use the same mapper. No SDK text, code redirect, retry or compensating
delete accompanies failure. Consumed state/session are not restored, and a lost response may leave
complete committed state; clients must restart sign-in.
