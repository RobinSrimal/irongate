# Provider Validation Contracts

Source: [packages/functions/auth/tests/support/oidc_validation.rs](../../../../../packages/functions/auth/tests/support/oidc_validation.rs).

## What

Shared test-only signed controls and validation cases instantiated by both provider callback suites.

## Why

Each guard needs an independently invalid input and its expected error; a generic is_err assertion
can pass for an unintended failure. The shared matrix avoids repeating identical signing/assertion
setup without creating a production OIDC abstraction.

## How

Uses each parent's existing test RSA fixture, real provider validator and error enum. A fixed time
keeps pure expiry/iat boundaries deterministic; claim mutations are re-signed. Controls establish
valid signatures before testing issuer, audience, nonce, expiry, future iat, subject, exact kid,
JWK metadata/components and corrupted signatures. Blank-ID cases include a matching blank JWK.
Missing optional alg/use, audience arrays and unchanged accepted subject values have positive cases.

Small helpers create claims, sign and corrupt signatures. identity_state inspects account, identity,
subject-index and code rows only in the memory TestStorage fixture. Its prefix inspection is not a
DynamoDB query contract; rate-limit/audit/state/session changes are outside that snapshot. Assertions
do not print tokens or secret/key material. The helper contains no production credentials or cache
implementation and does not test live provider behavior.
