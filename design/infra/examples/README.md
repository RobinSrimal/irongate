# Example Infrastructure

Source: [infra/examples/README.md](../../../infra/examples/README.md).

## What

Operator-facing notes for the optional Cloudflare web deployment. The current resources live in
[index.ts](index.md), with stage settings exposed by [config.ts](config.md).

## Why

Example hosting must remain optional and independent from the Rust/AWS auth core.

## How

Both checked-in stages disable examples. SST imports example resources only when explicitly enabled;
the web flag then gates a Cloudflare Worker and Durable Object namespace. Local Cloudflare deploy
credentials come from ignored `.env`, loaded by the npm SST wrapper. Runtime auth secrets stay in SST.

The native app creates no cloud resources here. See [web](web.md) and [app](app.md) for the actual
hosting boundaries, and [application design](../../examples/README.md) for client behaviour.
