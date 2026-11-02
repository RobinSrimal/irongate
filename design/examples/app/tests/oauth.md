# App OAuth Tests

Source: [packages/examples/app/tests/oauth.test.ts](../../../../packages/examples/app/tests/oauth.test.ts).

## What

Tests of authorize URL generation and continuation parsing.

## Why

Client/provider request bindings need checks without UI automation.

## How

Node tests use local URL fixtures and assert the public-client parameters and rejection of unsupported continuation paths. They do not exercise the Rust loopback listener.
