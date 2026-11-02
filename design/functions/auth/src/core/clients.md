# Client Registry

Source: [packages/functions/auth/src/core/clients.rs](../../../../../packages/functions/auth/src/core/clients.rs).

## What

Immutable configured-client lookup, authentication, redirect and grant policy.

## Why

All routes enforce one registry rather than accepting arbitrary client IDs or callbacks.

## How

Public profiles require PKCE; confidential profiles validate the configured secret digest in constant time. Redirects match exactly except native_desktop loopback ports, with scheme, host, path and other URI components constrained. Browser origins are collected for CORS; public/confidential parsing aliases remain in the current enum.
