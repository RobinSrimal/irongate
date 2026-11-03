# Public Route Tests

Source: [packages/functions/auth/tests/api_routes.rs](../../../../packages/functions/auth/tests/api_routes.rs).

## What

CORS, authorize/token routing, trusted source identity and absent public control-plane paths.

## Why

The mounted public router is the integration contract.

## How

Shared config/backend fixtures run Axum through oneshot. Child modules cover
[password routes](api_routes/password.md), [early admission](api_routes/admission.md) and
[independent limits](api_routes/rate_limits.md). All execute under the api_routes integration-test
target, using the current policy map and asserting private keys only after finding stored counters
through bounded endpoint-partition queries.
