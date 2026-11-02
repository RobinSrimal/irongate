# Repository guidance

## Scope and boundaries

- Irongate is a reusable Rust/AWS authentication template. Keep product behaviour, branding,
  deployment identifiers and credentials out of shared auth code.
- Keep the default auth deployment independent of optional examples. The existing Cloudflare web
  example does not authorize a hosting migration or a new dependency for the auth core.
- Obtain the user's explicit approval before significant architecture or infrastructure changes.
  Explain the problem, trade-offs and simpler alternatives first. A general instruction to build,
  implement or deploy does not authorize unplanned architectural changes; writing a slice document
  does not itself grant approval.
- Do not deploy, remove or reconfigure AWS/Cloudflare resources unless explicitly asked.
- Do not perform UI testing through MCP tools unless explicitly requested.
- Prefer clean contract changes. Do not add legacy aliases, dual implementations or migration
  scaffolding solely for backwards compatibility unless the user requests it. Keep OAuth/OIDC
  interoperability and security guarantees intact, and update callers, examples and setup docs.

## Design and delivery

- Authoritative design lives in `design/overview.md`, `design/functions/`, `design/examples/`,
  `design/packages/`, `design/infra/`, `design/scripts/`, and `design/apps/` when applicable.
- Mirror every hand-maintained source, test, manifest, README and project-configuration file under
  `packages/`, `infra/`, `scripts/` and `apps/` at its relative path in the mapped design folder,
  replacing the complete source suffix with `.md`.
- Map `packages/functions/` to `design/functions/`, `packages/examples/` to `design/examples/`,
  other packages to `design/packages/`, `infra/` to `design/infra/`, `scripts/` to `design/scripts/`,
  and `apps/` to `design/apps/`. Preserve intermediate directories such as `src/` and `tests/`.
  For example, `packages/functions/auth/src/store/refresh.rs` maps to
  `design/functions/auth/src/store/refresh.md`; `routes.test.ts` maps to `routes.md`.
  Retain meaningful configuration basenames such as `tauri.conf` when replacing `.json`.
- Exclude lockfiles, generated output, third-party reference material and binary assets from
  source mirroring. Existing topic-level docs remain useful overviews, but do not replace a file
  mirror. Bring touched files into alignment in the same slice; avoid unrelated bulk doc rewrites.
- Each design document briefly states the current what, why and how. Remove superseded decisions
  instead of accumulating their history. Keep operator/setup docs in `docs/` consistent too.
- `design/research/` holds non-authoritative investigations, explicit handoffs and operational
  notes. `design/implementation/` holds numbered delivery slices. Both are exempt from mirroring.
- Before implementation, write the next numbered slice under `design/implementation/` as
  `NN_meaningful_slice_name.md`. If none exist, start at `01`.
- Write only one slice at a time. Complete and verify it before writing the next. Keep future
  plans in the conversation; do not create future slice documents, outlines or roadmaps.
- Keep source files below 1,000 lines, including inline tests. Split by responsibility before
  reaching the limit.

## Validation and tests

- Run the smallest checks that establish the changed behaviour. Report what ran, failures and
  skipped checks honestly; a previous project's passing result is not verification of this port.
- Prefer tests of observable contracts, real rejection paths, failures and races. Keep fake storage
  semantics aligned with DynamoDB, and use disposable local integration tests where atomicity or
  conditional writes matter. Never point destructive tests at deployed tables.
- Be critical of tests. Do not add assertions that merely restate implementation or test mocks
  against themselves. Consolidate or remove redundant touched tests without a blanket deletion pass.
  Test counts are not a quality target. Do not add tests just for reversible copy/style edits.
- Common checks, selected for the change:

  ```sh
  npm run typecheck
  npm run test:infra
  npm run test:setup
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
  git diff --check
  ```

- Check the current manifests for example/admin/native commands. Run affected builds and meaningful
  integration checks as needed; do not rebuild unrelated platforms to verify a backend-only change.

## Secrets and cross-repository work

- Never commit secrets, `.env`, private/signing keys, provisioning profiles, credentials, generated
  release output or deployment state. Do not print secrets in logs or handoff documents.
- Keep provider IDs, issuers, audiences, callbacks and sender/brand values in explicit template
  configuration. Use safe placeholders or disabled optional features in a fresh checkout.
- When importing work from another repository, inspect its current implementation and dependencies
  before copying. Keep reference repositories read-only unless changes there are separately requested.
  Do not infer permission to reuse deployment credentials from their presence on this machine.
- For the current transfer from Knights & Forks, read
  [the agent handoff](design/research/irongate_auth_improvements_handoff.md) before changing code.
