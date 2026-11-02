# Authorization Codes

Source: [packages/functions/auth/src/store/authorization_codes.rs](../../../../../packages/functions/auth/src/store/authorization_codes.rs).

## What

Creation, read and conditional consumption of HMAC-keyed codes.

## Why

Client, redirect and PKCE checks can occur before the one-use code is removed.

## How

Creation uses a not-exists Put with TTL. Reads reject expired records. delete_authorization_code_if_current deletes only the serialized record previously read; take combines read and delete. Conditional failure returns no code; other backend errors propagate.
