# Web Session Storage

Source: [packages/examples/web/src/session.ts](../../../../packages/examples/web/src/session.ts).

## What

Memory and Durable Object stores plus session-cookie helpers.

## Why

Deployed browser session data needs server-side storage independent of Worker process lifetime.

## How

Each opaque ID maps to one WebSessionObject with get/put/delete requests. Cookies use __Host names, HttpOnly, Secure, SameSite=Lax and Path=/. The object provides CRUD only: no transaction-level callback consumption, refresh serialization, expiry enforcement or cleanup alarm is implemented.
