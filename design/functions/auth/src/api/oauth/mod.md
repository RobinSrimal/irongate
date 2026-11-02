# OAuth API Exports

Source: [packages/functions/auth/src/api/oauth/mod.rs](../../../../../../packages/functions/auth/src/api/oauth/mod.rs).

## What

The OAuth API namespace used by the router.

## Why

Route imports remain grouped under api while protocol implementations live in oauth.

## How

Declares authorize, discovery, revoke, token and userinfo facade modules. These re-export handlers rather than implementing a second protocol path.
