# Token Claims

Source: [packages/functions/auth/src/core/tokens.rs](../../../../../packages/functions/auth/src/core/tokens.rs).

## What

Access-token and OIDC ID-token claim types and builders.

## Why

Access tokens address resource APIs; ID tokens address the OAuth client.

## How

Builders copy the persisted subject and granted scope, add issuer/audience/times and distinguish mode=access from mode=id. ID tokens require openid, include email only with email scope, and preserve the initial OIDC nonce. Access properties are copied from the stored authentication record.
