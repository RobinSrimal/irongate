# AGENTS.md

## Operating Rule

Changes to the repo should keep code, docs, and design in sync.

When changing implementation, also check whether a matching design doc needs to change. The design
tree describes what exists, why it exists, and how it works. Do not use design docs for historical
notes, migration plans, postponed ideas, or revised decisions.

## Design Docs

Use the design tree as the architectural source of truth:

- `design/overview.md`
- `design/functions/`
- `design/infra/`
- `design/examples/`

Design docs should be current-state documentation only.

## Validation

Before finishing a change, run the smallest relevant checks.

Common checks:

```bash
npm run typecheck
npm run test:infra
npm run test:setup
cargo test --manifest-path packages/functions/auth/Cargo.toml
git diff --check
```

Do not claim a check passed unless it was run.

## Secrets And Deployment

Do not commit secrets, `.env`, signing keys, generated build output, or deployment state.

Do not deploy or remove AWS/Cloudflare resources unless explicitly asked.
