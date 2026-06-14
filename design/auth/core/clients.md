# Clients

Target code: `packages/functions/auth/src/core/clients.rs`

## Owns

- OAuth client model.
- Redirect URI validation.
- Grant type validation.
- Client secret verification rules.

## Target Behavior

The first narrow template should prefer deployment-defined OAuth clients. Runtime client management can come later if needed.

## Security Invariants

- Redirect URI matching is exact.
- Public clients cannot use client secrets.
- Confidential clients store only secret hashes.
- Client credentials grant is optional and limited to confidential clients.
- Disabled clients cannot receive tokens.
