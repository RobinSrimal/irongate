# Example Configuration

Source: [infra/examples/config.ts](../../../infra/examples/config.ts).

## What

A view of stageConfig.examples.

## Why

Optional example settings stay separate from auth resource creation.

## How

Exports checked-in example settings only. Importing this module does not instantiate a Worker, native app service or AWS resource.
