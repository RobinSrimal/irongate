# Runtime Metrics

## What

The deployment relies on AWS-provided API Gateway and Lambda operational metrics and logs.

## Why

Request failures, latency and function operation can be observed without scanning raw auth records.

## How

There is no `src/observability/metrics.rs` module or custom login/signup counter emitter in the current
crate. Lambda tracing is configured in [startup](../src/main.md), HTTP tracing in
[routes](../src/routes.md), and log retention in [infra](../../../infra/auth/api.md).
Security audit records have the separate storage behaviour described in [audit](audit.md).
