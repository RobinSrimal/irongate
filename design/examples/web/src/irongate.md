# Web Auth API Calls

Source: [packages/examples/web/src/irongate.ts](../../../../packages/examples/web/src/irongate.ts).

## What

Form/JSON helpers for code exchange, userinfo and refresh revocation.

## Why

OAuth tokens remain in Worker-side calls rather than browser JavaScript.

## How

Uses fetch or an injected test fetcher, with manual redirects for forms. Exchanges use client_id and PKCE without a client secret. Userinfo failures return undefined; revoke currently does not inspect the response status. There is no refresh grant helper here.
