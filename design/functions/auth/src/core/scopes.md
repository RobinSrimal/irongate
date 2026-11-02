# Supported Scopes

Source: [packages/functions/auth/src/core/scopes.rs](../../../../../packages/functions/auth/src/core/scopes.rs).

## What

Constants for openid, profile, email and offline_access.

## Why

Configuration and discovery share a fixed vocabulary.

## How

DEFAULT_SUPPORTED_SCOPES lists the four scopes. Request validation and claim selection are implemented by the registry, authorize endpoint and token builders, not by these constants.
