# Public Route Tests

Source: [packages/functions/auth/tests/api_routes.rs](../../../../packages/functions/auth/tests/api_routes.rs).

## What

CORS, authorize/token routing, trusted source identity and absent public control-plane paths.

## Why

The mounted public router is the integration contract.

## How

Shared config/backend fixtures run Axum through oneshot. The api_routes/password.rs child module covers password routes using those same fixtures. Both execute under the api_routes integration-test target.
