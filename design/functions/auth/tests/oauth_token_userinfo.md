# Token And UserInfo Tests

Source: [packages/functions/auth/tests/oauth_token_userinfo.rs](../../../../packages/functions/auth/tests/oauth_token_userinfo.rs).

## What

Discovery/JWKS, code exchange, PKCE failure, userinfo token type and refresh issuance.

## Why

Protocol consumers rely on matching metadata, signing keys and token use.

## How

Creates configured signing material and stored codes, invokes handlers and checks JWT claims and responses. Failed PKCE preserves the code. Tests are local and do not establish live deployment configuration.

Seeded codes carry the current account generation, exercising the guarded consumption path rather
than bypassing account binding. Generation remains internal and does not alter JWT claims.
