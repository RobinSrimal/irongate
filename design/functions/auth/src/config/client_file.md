# Client File

Source: [packages/functions/auth/src/config/client_file.rs](../../../../../packages/functions/auth/src/config/client_file.rs).

## What

Loading and validation of the supported auth.clients.toml subset.

## Why

Client registration is deployment configuration rather than a public runtime control plane.

## How

A line-oriented parser accepts explicit client fields and rejects unknown/duplicate definitions. Validation checks profile, redirect, origin, scope, grant and PKCE rules. Confidential secret references are resolved and hashed in memory; this is not a general-purpose TOML parser.
