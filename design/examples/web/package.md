# Web Package

Source: [packages/examples/web/package.json](../../../packages/examples/web/package.json).

## What

The optional TypeScript Worker package and its build/dev/test commands.

## Why

The example can be checked without deploying Cloudflare resources.

## How

tsc emits dist output; the dev command starts the Node adapter and test uses Node's test runner over compiled tests. The package is private and has no dependency on auth DynamoDB.
