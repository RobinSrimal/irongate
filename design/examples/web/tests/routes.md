# Web Route Tests

Source: [packages/examples/web/tests/routes.test.ts](../../../../packages/examples/web/tests/routes.test.ts).

## What

Worker request/response tests with the injected auth transport.

## Why

The BFF's cookie, callback and upstream-request contracts are observable without a browser.

## How

Exercises registration/verification/password and provider starts, callback state rejection, server-held token sessions and logout. Assertions use MemorySessionStore and fake responses, not deployed Cloudflare or cryptographically verified provider tokens.
