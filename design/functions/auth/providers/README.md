# Providers

Implementation: [providers/mod.rs](../src/providers/mod.md) and
[provider HTTP handlers](../src/api/providers/mod.md).

## What And Why

Password, Google and Apple modules separate provider-specific proof handling from route parsing.
The first-party OAuth server still owns client policy, internal subjects and token issuance.

## How

Google/Apple domain helpers build provider URLs, exchange codes, fetch/cache JWKS and validate signed
ID tokens. Their API handlers consume provider state, resolve identities and issue internal codes.

The password domain module is broader: it coordinates typed storage, email delivery, password
verification and authorization-code creation. It returns a continuation redirect for successful
login, not merely a provider identity. Configuration lives under `src/config`; provider modules
do not define a generic plugin or native-device login framework.
