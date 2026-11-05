# Google OIDC Helpers

Source: [packages/functions/auth/src/providers/google.rs](../../../../../packages/functions/auth/src/providers/google.rs).

## What

Authorization URL construction, code exchange, identity digest and signed-ID-token validation.

## Why

Provider claims are accepted only after server-side verification.

## How

A reused reqwest client keeps JWKS for ten minutes behind an async lock; forced refresh is exposed
to the callback. Validation requires an RS256 header and a nonblank kid matched exactly, without
trimming it into a different ID or falling back to another key. The selected key must be RSA;
explicit alg must be RS256 and explicit use must be sig. Absent optional alg/use remain accepted.
Missing or malformed RSA components reject before signature validation.

Signed claims must match the configured issuer, contain the configured audience and exact nonce,
have exp strictly after the supplied time, iat no more than 60 seconds in the future, and a nonblank
subject. Audience string/array forms are supported; accepted subject values are not rewritten.
Missing/blank/unmatched IDs return MissingKey, wrong type/algorithm UnsupportedAlgorithm, invalid
material/use InvalidKey, and signature/decoding failures InvalidToken. Errors contain no raw token.
There is no native presenter/azp or nbf validation in this module.

Claims also deserialize optional signed `hd` for [email authority](verified_email.md). Validation
does not infer it from a requested domain or email suffix. After validation, the callback constructs
provider/identity-bound authority for account selection; `hd` is not persisted or issued as a token
claim. Lack of authority cannot create a new identity but does not move an existing identity.
