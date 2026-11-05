# Web Test Fixtures

Source: [packages/examples/web/src/testing.ts](../../../../packages/examples/web/src/testing.ts).

## What

An in-memory session store and fake Irongate HTTP transport.

## Why

Route tests run without credentials, provider calls or deployed infrastructure.

## How

Captures outbound URLs, methods and bodies and returns controlled authorize/password/token/userinfo/
revoke responses. Registration returns `{status: "verification_required"}`; completion returns
`{status: "verified", subject: "user_test"}` without OAuth tokens. Provider flags are optional fixture
inputs. Tests can replace the transport or session store to observe rejection paths and non-mutation.

The fake does not validate registration proofs, enforce password policy or implement atomic account
activation. These fixtures support HTTP route-contract assertions, not real auth-core behavior, JWT
validation or Durable Object concurrency guarantees.
