# Audit Output Tests

Source: [packages/functions/auth/src/audit/tests.rs](../../../../../packages/functions/auth/src/audit/tests.rs).

## What

Byte-level serializer/output and failure-contract tests.

## Why

A successful route or empty database does not prove audit output or safe failure handling.

## How

Capturing partial writers assert complete unique JSON across concurrent emits. Failing sinks/writers
exercise independent fallback, no recursion, none touching neither sink, partial-write/flush errors,
invalid identifiers and the strict size boundary. Synthetic serialization failure uses the same
fallback path. A subprocess checks actual production stdout/stderr under an ERROR-only tracing
filter. These are local output checks, not Lambda/CloudWatch delivery certification.
