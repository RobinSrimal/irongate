# Clients

Target code: `packages/functions/auth/src/core/clients.rs`

## Owns

- OAuth client model.
- Redirect URI validation.
- Grant type validation.
- Client secret verification rules.

## Target Behavior

The first narrow template uses config-only OAuth clients. The core receives a validated, read-only client registry from configuration and applies OAuth rules against that registry.

Runtime client management is out of v1. Adding it later would require a separate client-management control-plane design. The IAM-protected account lifecycle admin routes must not mutate OAuth clients.

## Security Invariants

- Redirect URI matching is exact.
- Public clients cannot use client secrets.
- Confidential clients store only secret hashes.
- Client credentials grant is not supported in v1.
- Disabled clients cannot receive tokens.
- Client definitions cannot be created or changed through public auth routes.
