# Public Route Tests

Source: [packages/functions/auth/tests/api_routes.rs](../../../../packages/functions/auth/tests/api_routes.rs).

## What

CORS, authorize/token routing, trusted source identity and absent public control-plane paths.

## Why

The mounted public router is the integration contract.

## How

Password child-module fixtures complete email-first registration through the domain and mailbox;
they do not activate a password identity through an independent store bypass.

Shared config/backend fixtures run Axum through oneshot. Child modules cover
[password routes](api_routes/password.md), [early admission](api_routes/admission.md) and
[independent limits](api_routes/rate_limits.md). All execute under the api_routes integration-test
target, using the current policy map and asserting private keys only after finding stored counters
through bounded endpoint-partition queries.

Application-state fixtures explicitly install the shared [captured audit emitter](support/audit.md),
so tests do not write production stdout/stderr. Dedicated audit route suites assert emission itself.
