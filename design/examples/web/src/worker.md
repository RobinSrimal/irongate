# Worker Entrypoint

Source: [packages/examples/web/src/worker.ts](../../../../packages/examples/web/src/worker.ts).

## What

The Cloudflare fetch entrypoint and Durable Object class export.

## Why

SST needs an explicit Worker handler and migration-visible session class.

## How

Delegates requests to handleRequest and re-exports WebSessionObject. Infrastructure supplies bindings; no resources are created by importing this file.
