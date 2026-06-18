# 26_infra_auth_examples_boundary

## Goal

Restructure the SST infrastructure code so Irongate core infrastructure is clearly separated from optional example infrastructure.

At the end of this slice, the default deploy should still create only the auth core:

```text
API Gateway HTTP API
  -> public Rust auth Lambda
  -> IAM-protected Rust admin Lambda
  -> DynamoDB AuthTable
  -> optional KMS resources
```

The repo should also have an explicit `infra/examples` boundary for future optional frontend/example deployments, but no example resources should deploy by default.

## Design Docs Followed

This slice should follow and update these design documents:

- `design/infra/README.md`
- `design/infra/api.md`
- `design/infra/auth-function.md`
- `design/infra/storage.md`
- `design/infra/secrets.md`
- `design/infra/stages.md`
- `design/infra/email.md`
- `design/infra/iam.md`
- `design/infra/performance.md`
- `design/auth/config/stages.md`
- `design/auth/config/environment.md`
- `design/scope.md`
- `design/migration.md`
- `design/implementation/ROADMAP.md`

The important design constraint is that frontend hosting and reference applications are optional example infrastructure, not Irongate core infrastructure.

## Why This Slice Next

The core auth rewrite has reached a stable deployed shape, and the next design direction is to demonstrate high-security web, mobile, and desktop integrations. That will likely require optional example infrastructure later, such as a hosted auth-web login surface, static web app, and protected sample API.

Before adding any of that, the infra tree should make the core/example boundary obvious:

```text
infra/auth      deploys Irongate core
infra/examples  reserved for opt-in examples
infra/shared    shared helpers used by auth and examples
```

This prevents future example work from leaking into the default deploy path.

## Scope Decision

This is a code organization and design-doc alignment slice. It should not add new AWS resources, auth flows, example applications, frontend hosting, or runtime behavior.

In scope:

- Move current core infra modules under `infra/auth`.
- Move shared helper modules under `infra/shared`.
- Add an `infra/examples` boundary with no deployed resources by default.
- Add stage config shape for examples with all examples disabled by default.
- Update `sst.config.ts` to import auth core modules from the new paths.
- Ensure example infra modules are not imported unless explicitly enabled.
- Keep SST component names stable so existing dev resources are not replaced only because files moved.
- Update design docs to reflect the new tree and the non-core status of frontend hosting.
- Update static infra validation if it checks old paths.

Out of scope:

- Building `auth-web`.
- Building a web SPA, mobile app, desktop app, or protected resource API.
- Deploying frontend hosting.
- Adding Cloudflare, S3, CloudFront, or custom-domain frontend resources.
- Changing auth API routes, token behavior, storage behavior, IAM semantics, or KMS behavior.
- Changing stage names or AWS profiles.
- Removing the current deployed dev stack.

## Target Infra Tree

The target tree for this slice is:

```text
infra/
  auth/
    api.ts
    config.ts
    secrets.ts
    signing.ts
    storage.ts

  examples/
    README.md
    config.ts
    index.ts

  shared/
    rust-bundle.ts
    stage-config.ts
```

`infra/examples/index.ts` may export an empty object or lightweight placeholders, but it must not create AWS resources unless examples are enabled.

## SST Import Boundary

SST creates resources at module import time. Because of that, `sst.config.ts` must keep imports deliberate.

Required default behavior:

```text
import infra/auth modules
do not import infra/examples resource modules
deploy auth core only
```

Required opt-in behavior:

```text
if stageConfig.examples.enabled:
  import infra/examples/index
```

Even after this slice, `examples.enabled` should default to `false` for every stage.

## Stage Config Shape

Add an examples section to stage config without enabling any resources:

```ts
examples: {
  enabled: false,
  authWeb: false,
  webSpa: false,
  resourceApi: false,
}
```

The exact TypeScript shape can differ if it follows local style, but the behavior must be clear:

- examples are disabled by default
- individual example resources can be enabled later
- auth core deploy does not depend on examples config
- production does not deploy examples unless explicitly configured

## Resource Stability Requirements

Moving files must not intentionally change existing SST component names.

Keep these logical component names stable:

```text
AuthApi
PublicAuthFunction
AdminFunction
AuthTable
AuthTableKey if customer table KMS is enabled
AuthSigningKey if KMS signing is enabled
```

The dev deploy should show no resource replacement caused only by the file move. If SST plans replacements, stop and inspect before applying.

## Design Doc Updates Required

Implementation must update the design docs, not only the code.

Required doc updates:

- `design/infra/README.md`: target tree becomes `infra/auth`, `infra/shared`, `infra/examples`.
- `design/infra/api.md`: target code path becomes `infra/auth/api.ts`.
- `design/infra/auth-function.md`: target code path becomes `infra/auth/api.ts` plus shared Rust bundle helper.
- `design/infra/storage.md`: target code path becomes `infra/auth/storage.ts`.
- `design/infra/secrets.md`: target code path becomes `infra/auth/secrets.ts`.
- `design/infra/stages.md`: stage config path becomes `infra/shared/stage-config.ts`, and examples are disabled by default.
- `design/infra/email.md`: email config remains auth-core config unless an example explicitly consumes it later.
- `design/scope.md`: clarify that frontend hosting is optional example infrastructure, not core.
- `design/migration.md`: keep reference/sample app decisions deferred; mention the infra boundary exists for future examples only.
- `design/implementation/ROADMAP.md`: add this slice to the sequence.

Optional doc updates if touched by implementation:

- `design/auth/config/stages.md`
- `design/auth/config/environment.md`
- `design/infra/performance.md`

## Expected Code Shape

Current repo paths should be moved rather than duplicated.

Target module imports:

```text
sst.config.ts
  -> ./infra/shared/stage-config.js
  -> ./infra/auth/storage.js
  -> ./infra/auth/signing.js
  -> ./infra/auth/api.js
  -> ./infra/examples/index.js only when examples are enabled
```

Auth infra imports:

```text
infra/auth/api.ts
  -> ./config.js
  -> ./secrets.js
  -> ./signing.js
  -> ./storage.js
  -> ../shared/rust-bundle.js
  -> ../shared/stage-config.js
```

Example infra should not import auth resources unless it is explicitly enabled and needs concrete auth outputs.

## Detailed Work Plan

Use frequent commits. A reasonable implementation split is:

1. Move `infra/api.ts` to `infra/auth/api.ts`.
2. Move `infra/config.ts` to `infra/auth/config.ts`.
3. Move `infra/secrets.ts` to `infra/auth/secrets.ts`.
4. Move `infra/signing.ts` to `infra/auth/signing.ts`.
5. Move `infra/storage.ts` to `infra/auth/storage.ts`.
6. Move `infra/rust-bundle.ts` to `infra/shared/rust-bundle.ts`.
7. Move `infra/stage-config.ts` to `infra/shared/stage-config.ts`.
8. Fix relative imports after the move.
9. Add `infra/examples/README.md`.
10. Add `infra/examples/config.ts` with examples disabled by default.
11. Add `infra/examples/index.ts` that creates no resources while disabled.
12. Update `sst.config.ts` imports and outputs.
13. Update infra validation scripts for the new paths.
14. Update design docs listed above.
15. Run TypeScript typecheck and infra validation.
16. Run a local SST preview/deploy only if needed to confirm no resource replacement.

## Tests And Validation

Required local validation:

```text
npm run typecheck
npm run test:infra
```

If the implementation changes bundling paths or SST imports in a way that typecheck cannot validate, also run:

```text
npm run deploy -- --stage dev
```

Only deploy after checking that the planned changes do not replace existing auth resources because of the file move.

## Acceptance Criteria

- Current auth infra lives under `infra/auth`.
- Shared helpers live under `infra/shared`.
- `infra/examples` exists and is explicitly opt-in.
- Examples are disabled by default for dev and production.
- Default SST deploy still creates only auth core resources.
- Existing SST component names remain stable.
- `sst.config.ts` outputs still include auth core outputs.
- Static infra validation understands the new paths.
- Design docs are updated to match the new infra tree.
- `npm run typecheck` passes.
- `npm run test:infra` passes.

## Manual Validation

After implementation, inspect the SST plan or deploy output for the dev stage.

Expected:

```text
no example resources created
no auth resources replaced only because files moved
AuthApi output still present
AuthTable output still present
AdminRouteArnPattern output still present
```

## Next Slice

After this slice, define the optional examples architecture slice.

Likely scope:

- document `packages/examples/auth-web`
- document `packages/examples/web-spa`
- document `packages/examples/mobile`
- document `packages/examples/desktop`
- document `packages/examples/resource-api`
- define browser, mobile, and desktop OAuth client profiles
- define frontend hosting as optional example infra

That later slice should remain design-only unless the example architecture is approved.
