# Web Test Fixtures

Source: [packages/examples/web/src/testing.ts](../../../../packages/examples/web/src/testing.ts).

## What

An in-memory session store and fake Irongate HTTP transport.

## Why

Route tests run without credentials, provider calls or deployed infrastructure.

## How

Captures requests and returns controlled authorize/password/token/userinfo/revoke responses. These fixtures support route-contract assertions but do not establish real JWT validation or Durable Object concurrency guarantees.
