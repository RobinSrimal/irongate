# UserInfo Endpoint

Source: [packages/functions/auth/src/oauth/userinfo.rs](../../../../../packages/functions/auth/src/oauth/userinfo.rs).

## What

Minimal identity claims for a validated access token.

## Why

Resource audience and active-account checks distinguish API authorization from ID-token presentation.

## How

Extracts Bearer credentials, verifies them using the runtime signer and checks the persisted account. Returns sub/type and scope-gated email fields from token properties. It does not fetch a fresh full profile or invalidate already-issued JWTs at other services.
