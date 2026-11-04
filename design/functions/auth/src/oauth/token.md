# Token Endpoint

Source: [packages/functions/auth/src/oauth/token.rs](../../../../../packages/functions/auth/src/oauth/token.rs).

## What

Client authentication and authorization-code/refresh grants.

## Why

Token issuance joins validated protocol inputs, active-account checks, typed one-use storage and the configured signer.

## How

After form/client-ID parsing, source-first admission precedes grant validation, authentication and
bearer-state reads. A source ceiling remains fixed across client changes; the secondary budget is
per configured client/source, with structural None for every unknown name. Failed client
authentication consumes allowance. Code client/redirect/PKCE checks precede conditional consumption.
Tokens are signed after consumption; offline_access and grant permission gate refresh issuance.
Refresh rotation precedes signing and returns no ID token. Typed rotation audit is attempted
immediately after commit, before signing. Initial refresh audit follows persistence; exchange audit
follows successful preparation. Reuse emits the existing store outcome with a digest, never raw IP.
Emission failures are handled by the injected emitter without changing the token response. Storage and signing failures are not an
all-or-nothing external response transaction.

Code consumption and refresh writes use matching active-account generation guards. Initial refresh
issuance receives the consumed code's generation; rotation carries its original generation forward.
An explicit account/generation rejection becomes invalid_grant without masking unrelated storage
errors. Generation is internal, not a new JWT claim. A transaction authorized before recovery can
finish JWT signing/response delivery afterwards.
