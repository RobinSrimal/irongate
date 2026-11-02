# Route Wiring Checks

Source: [scripts/validate-infra-routes.mjs](../../scripts/validate-infra-routes.mjs).

## What

Static checks of shared Lambda components, public/admin routes and secret separation.

## Why

Each admin route must target the IAM-protected admin function rather than create a per-route function.

## How

Checks infra/auth/api.ts patterns, IAM route options and allowed environment boundaries. It does not invoke API Gateway or test SigV4 authorization.
