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
orchestration, not live cache behavior. Identity resolution and code issuance use AuthStore;
unsigned callback profile data is not an identity proof.

Code issuance captures the active account generation and conditions the write on that account
snapshot. A confirmed stale-account condition rejects the grant; operational store failures remain
server errors. Provider validation and identity resolution do not depend on password credentials.
