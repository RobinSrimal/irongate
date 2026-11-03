# Authorization Codes

Source: [packages/functions/auth/src/store/authorization_codes.rs](../../../../../packages/functions/auth/src/store/authorization_codes.rs).

## What

Creation, read and conditional consumption of HMAC-keyed codes.

## Why

Client, redirect and PKCE checks can occur before the one-use code is removed.

## How

Creation combines a not-exists Put with TTL and a complete active-account snapshot condition at the
code's generation. Password issuance also guards the exact verified password-user snapshot whose
hash was checked. Reads reject expired, inactive or stale-generation records. Conditional consumption
rechecks expiry and guards both account and code; take combines read and delete. Confirmed condition
failure returns no code; other backend errors propagate. Client/redirect/PKCE validation belongs to
the token caller before consumption.
