# Auth Integration Tests

Integration tests in this directory are named by auth domain or protocol surface, not by the implementation slice that introduced them.

Use source-local `#[cfg(test)]` modules for focused pure-module tests, especially config parsing, crypto helpers, key helpers, and small request-context helpers.

Keep tests in this directory when they exercise:

- Axum routers.
- OAuth or OIDC protocol flow across modules.
- Account lifecycle behavior through public or IAM-protected APIs.
- End-to-end typed store state across multiple store modules.
- Provider callbacks with fake OIDC clients.

`support/mod.rs` contains shared integration-test infrastructure. New integration test files must not use `*_slice.rs` names or numeric slice prefixes.

Security regression tests should be named by the behavior or risk they protect. Finding IDs can appear inside test names or comments when they add useful context, but file names should remain domain-oriented.
