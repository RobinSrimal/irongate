# Audit Emitter

Source: [packages/functions/auth/src/audit.rs](../../../../packages/functions/auth/src/audit.rs).

## What

An injectable, mode-aware synchronous JSON audit emitter with independent output and fallback sinks.

## Why

Auth and admin share safe output/error handling without coupling logging to database transactions.

## How

Production installs stdout/stderr sinks. The typed event is validated, wrapped with a versioned
UUID/timestamp/INFO envelope, serialized and bounded strictly below 8 KiB including its newline.
Stream-wide locks span write_all/flush. WriterSink offers the same complete-write path for injected
writers. None returns before either sink is touched. One failed attempt produces a safe ERROR
diagnostic through stderr; a failed fallback is discarded without recursion. No request fields or
raw errors enter that diagnostic. Local writes do not prove CloudWatch delivery.
See [schema](audit/events.md), [tests](audit/tests.md) and [limits](../observability/audit.md).
