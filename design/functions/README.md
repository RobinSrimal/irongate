# Functions

This folder mirrors `packages/functions`.

```text
packages/functions/
  admin/
  auth/
```

## Boundaries

- `auth/` documents the public auth Lambda and the shared auth library modules it owns.
- `admin/` documents the IAM-protected account lifecycle Lambda.

The admin function may reuse shared modules from the auth crate, but it has a separate deployed
entrypoint, route surface, runtime environment, and IAM boundary.
