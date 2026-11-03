# Auth Testing

## What

Source-local unit tests and domain-oriented integration tests under
[tests](tests/README.md). The public crate is also compiled into its bootstrap binary, so some unit
tests run in both targets.

## Why

Pure configuration/crypto checks and cross-module protocol tests establish different contracts.
Neither substitutes for AWS delivery, storage consistency or real provider/device acceptance.

## How

Run `cargo test --locked --manifest-path packages/functions/auth/Cargo.toml`; select a named
integration target for focused work. [api_routes](tests/api_routes.md) covers public routes and its
[password module](tests/api_routes/password.md) covers password routes. Admin, provider, recovery,
refresh and signer suites have their own file mirrors describing actual coverage and limitations.

[Integration TestStorage](tests/support/mod.md) and the
[unit-test backend](src/storage/test_support.md) avoid deployed services. Their current conditional
expiry/update semantics are not identical to DynamoDB. The [SDK wire suite](tests/dynamodb_requests.md)
checks consistent-read flags, pagination and error handling against a loopback HTTP stub. The
[DynamoDB Local suite](tests/dynamodb_storage.md) separately checks conditional transactions and
one-time consumes on disposable tables. It is ignored by default and must be run explicitly with
the local prerequisite in the [test guide](tests/README.md); neither suite certifies every storage
race or production behavior. Signed provider fixtures and fake KMS/email clients do not prove real
provider, IAM, KMS or mail delivery.

Infrastructure scripts are static source checks; deployment smoke tests require separate operator
authorization. No UI tests or live destructive table tests are part of normal validation.
