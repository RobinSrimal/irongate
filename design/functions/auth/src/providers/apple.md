# Apple OIDC Helpers

Source: [packages/functions/auth/src/providers/apple.rs](../../../../../packages/functions/auth/src/providers/apple.rs).

## What

Apple URL/code exchange, client-secret JWT generation, JWKS caching and ID-token validation.

## Why

Apple credentials and provider claims have different signing keys and audiences from Irongate tokens.

## How

Client-secret JWTs use the configured ES256 private key/team/key ID. Apple ID tokens require RS256,
a nonblank exactly matched kid and a selected RSA key. Explicit JWK alg must be RS256 and explicit
use must be sig; absent optional metadata remains accepted. Blank IDs are rejected even if the set
has an equally blank ID, and nonblank IDs are not normalized. Missing/malformed RSA components and
bad signatures fail closed.

Claims require exact configured issuer, audience membership, expiry strictly after the supplied
time, at most 60 seconds of future iat, the original nonce and a nonblank subject. Subject values
are not rewritten. Audience strings/arrays and string/boolean email flags remain supported.
Missing/blank/unmatched IDs return MissingKey, wrong type/algorithm UnsupportedAlgorithm, invalid
material/use InvalidKey, and signature/decoding failures InvalidToken. The module does not check nbf
or implement native presenter policy. The client caches JWKS for ten minutes and exposes forced
refresh; ID-token validation is distinct from Apple's ES256 client-secret generation.
