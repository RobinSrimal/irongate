# Design

This directory describes the target shape of the repository before code is moved.

The rule is symmetry: the design tree should match the code tree we intend to create. Each folder explains what the corresponding code folder should own, what it should not own, and the security invariants it must preserve.

The current implementation still contains legacy/general-purpose pieces. These docs describe the narrower AWS-first auth template we want to refactor toward.

## Target Shape

```text
design/
  auth/
  infra/
  samples.md
```

Target code symmetry:

```text
packages/functions/auth/src/
  api/
  core/
  store/
  crypto/
  providers/
  email/
  config/
  observability/

infra/
  api.ts
  storage.ts
sst.config.ts
```

Infra stays small because SST owns most AWS wiring. Auth is more detailed because it carries the security model.

## Existing Notes

The root-level docs are source material for this design tree:

- `AUTH_FLOWS.md`
- `AWS_INFRASTRUCTURE_OPTIMIZATION.md`
- `STORAGE_SECURITY.md`
- `SECURITY_SCAN.md`

They can be split or moved after the design tree settles.

## Cross-Cutting Docs

- `scope.md`: in-scope and out-of-scope product boundaries.
- `samples.md`: frontend-agnostic sample app boundary.
- `migration.md`: refactor sequence from current code to target design.
- `security-scan-coverage.md`: mapping from scan findings to design decisions.
- `storage-security.md`: mapping from storage security notes to design decisions.
