# Audit Fixtures

Source: [packages/functions/auth/tests/support/audit.rs](../../../../../packages/functions/auth/tests/support/audit.rs).

## What

Shared byte capture, failing writer and envelope checks for route tests.

## Why

Routes need explicit sink injection without live logs, deployed tables or fake success events.

## How

Capture implements Write and uses the real WriterSink/emitter serialization. It exposes bytes and
parsed JSON records; an independent capture receives fallback diagnostics. FailWriter returns a
sensitive marker error to prove it is excluded. Envelope assertions check schema, UUID/time, event
kind and absent private fields. Ordinary state constructors choose captured output explicitly.
