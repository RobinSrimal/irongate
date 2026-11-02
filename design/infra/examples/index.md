# Example Resources

Source: [infra/examples/index.ts](../../../infra/examples/index.ts).

## What

The opt-in Cloudflare Worker and Durable Object binding.

## Why

The web BFF has server-side session storage without becoming an auth-core dependency.

## How

Both example and web flags gate resource creation. Defines WebSessionObject's SQLite migration and SESSION_OBJECT binding, passes api.url/client/provider flags, and exports the Worker URL. Optional base URL/domain values are passed explicitly; no native-app resources are created.
