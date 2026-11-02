# 01_repository_design_alignment

## Goal

Bring the current Irongate repository's design coverage and source-file layout into alignment with
`AGENTS.md` before transferring any downstream authentication improvements.

## Scope

- Inventory tracked hand-maintained files under `packages/`, `infra/` and `scripts/`.
- Add exact file-level design mirrors, preserving intermediate source and test directories.
- Describe current behaviour, rationale and implementation, including current limitations; do not
  describe reference-repository behaviour as already implemented here.
- Reconcile affected topic overviews and operator docs with the existing implementation.
- Split the oversized API route integration test by responsibility without changing its assertions.
- Replace the native example's obsolete deployed issuer default with a reserved placeholder and
  align its run instructions. Remove deployment-specific values from current design examples.
- Check mirror coverage, local links, source lengths and the affected tests.

Lockfiles, generated output, binary assets and third-party reference material are excluded.
Extensionless configuration files retain their name and gain `.md`. Configuration basenames such as
`tsconfig.test` and `tauri.conf` are retained. Where source files map to the same design path, such as
`App.tsx` and `App.css`, that document explicitly covers both files.

## Boundaries

No authentication behaviour changes, dependency upgrades, deployments, hosting changes or reference
repository edits. No downstream transfer checklist or future slice documents are part of this work.
Existing user edits to `AGENTS.md`, the design index and the handoff remain intact.

## Acceptance

- Every in-scope hand-maintained file has its required design mirror.
- Mirrors identify their source and explain its current what, why and how.
- Topic overviews supplement rather than substitute for file mirrors.
- Current implementation limitations are not hidden behind intended security guarantees.
- Source files are below 1,000 lines, including tests.
- The route-test split preserves existing observable contract coverage.
- Verification results and unperformed checks are recorded below.

## Status

Completed for the documentation/layout scope. No downstream implementation was transferred.

## Result

- All 158 in-scope hand-maintained files have file mirrors across 156 documents. The two shared
  paths explicitly cover the source pairs that map to them.
- Existing topic docs and affected setup instructions describe current implementation rather than
  unimplemented guarantees. The design index explains how topic overviews and file mirrors relate.
- The historical AWS smoke record lives under `design/research/`, with deployment outputs replaced
  by placeholders; it is not current validation evidence.
- The API route suite is split into a parent and password child module. An exact-text comparison
  confirmed the moved test bodies and remaining parent body were unchanged.
- The native issuer default is a reserved placeholder, with explicit setup instructions.
- User-provided guidance/handoff edits are preserved. No cloud resources or reference repositories
  were changed, and no future slice or transfer checklist was created.

## Verification

Run on 2026-11-02 for this slice:

| Check | Result |
| --- | --- |
| Source inventory, exact mirror coverage and what/why/how/source references | Pass: 158 source files, 156 mirrors |
| Source length audit | Pass: all in-scope files below 1,000 lines |
| Local Markdown link and trailing-whitespace audit | Pass: 307 local links checked before final result notes |
| `cargo test --locked --offline --manifest-path packages/functions/auth/Cargo.toml --test api_routes --test startup_config` | Pass: 22 route tests and 10 startup tests |
| `rustfmt --edition 2021 --check --config skip_children=true` on both route-test files | Pass |
| `npm run test:infra` | Pass |
| `npm run typecheck` | Pass |
| `npm run test:setup` | Pass |
| `git diff --check` | Pass |
| `cargo check --locked --offline --manifest-path packages/examples/app/src-tauri/Cargo.toml` | Blocked: `bitflags 2.13.0` is not cached; no native build success claimed |

No deployment, UI/device testing, live-provider testing or destructive storage testing was run.
The current slice does not certify all security behavior: documented gaps include memory/DynamoDB
semantic differences, recovery/lifecycle race boundaries, audit storage and example-session limits.
Those implementation gaps were not silently changed as part of documentation alignment.
