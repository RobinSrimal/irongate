# Local Web Adapter

Source: [packages/examples/web/scripts/dev-server.ts](../../../../packages/examples/web/scripts/dev-server.ts).

## What

A Node HTTP adapter around the Worker fetch handler.

## Why

Local requests can exercise the same route code with in-memory sessions.

## How

Requires IRONGATE_ISSUER_URL and constructs Request URLs from WEB_BASE_URL, which must be supplied for this adapter. Buffers request/response bodies and supplies MemorySessionStore. This is not a Cloudflare emulator or a deployed session backend.
