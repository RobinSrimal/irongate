# SST Environment Wrapper

Source: [scripts/sst-with-env.mjs](../../scripts/sst-with-env.mjs).

## What

The local deploy/remove command wrapper.

## Why

Cloudflare deployment credentials can remain in ignored .env rather than versioned config.

## How

Invokes the installed SST CLI through Node, adding --env-file=.env when present, forwarding arguments/environment/stdin and propagating failures. It does not replace SST runtime secrets or AWS SSO.
