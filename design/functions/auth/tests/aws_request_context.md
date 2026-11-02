# Source Context Tests

Source: [packages/functions/auth/tests/aws_request_context.rs](../../../../packages/functions/auth/tests/aws_request_context.rs).

## What

API Gateway v1/v2 source IP extraction and forwarded-header rejection.

## Why

A caller cannot choose their rate-limit source through an HTTP header.

## How

Builds Lambda request contexts directly and varies spoofed headers/absent context. These tests check extraction logic, not AWS event delivery.
