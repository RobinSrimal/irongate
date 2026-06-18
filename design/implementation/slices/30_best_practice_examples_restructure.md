# 30_best_practice_examples_restructure

## Goal

Correct the optional example architecture so Irongate demonstrates only best-practice integration patterns.

At the end of this slice, the design docs should describe three example packages:

```text
packages/examples/
  web/
  app/
```

The web example is BFF-based and owns the initial protected API routes. The app example is desktop-first Tauri with OS keychain token storage and documented mobile-specific changes.

## Design Docs Followed

This slice follows and updates:

- `design/examples/README.md`
- `design/examples/client-profiles.md`
- `design/scope.md`
- `design/migration.md`
- `design/implementation/ROADMAP.md`

## Scope Decision

This is a design-correction slice only.

In scope:

- Replace the old broader example set.
- Define the new example set:
  - `web`
  - `app`
- Document web as BFF-only for the recommended browser example.
- Document web as the owner of the initial protected API routes.
- Document app as desktop-first Tauri using OS keychain storage.
- Document mobile-specific app changes in the app README/design, without creating a separate mobile implementation.
- Update roadmap and scope docs.

Out of scope:

- Implementing any example package.
- Deploying example infra.
- Removing implemented auth-core client profile support.
- Adding BFF routes, Tauri code, protected API implementation code, or keychain crates.
- Changing Irongate core runtime behavior.

## Architecture Decision

The best-practice example set is:

| Example | Purpose |
| --- | --- |
| `web` | Cloudflare Worker web app using a BFF. Browser receives only an HttpOnly Secure SameSite session cookie. Refresh tokens stay server-side. The Worker also owns the initial protected API routes. |
| `app` | Desktop-first Tauri native app using external browser login, PKCE, loopback redirect, OS keychain refresh-token storage, in-memory access tokens, and the web Worker's protected API routes. |

Direct browser token storage is not a showcased best-practice example. If documented later, it should be clearly marked educational or lower-assurance.

## Acceptance Criteria

- `design/examples/README.md` lists only `web` and `app`.
- Web docs require the BFF pattern and do not recommend browser refresh-token storage.
- Web docs own the initial protected API routes.
- App docs describe Tauri desktop-first behavior with OS keychain storage.
- App docs include mobile-specific redirect/storage notes without making mobile a separate first implementation.
- Roadmap references the corrected example direction.
- No runtime code or infra code changes are made.

## Manual Validation

Read the updated docs and confirm:

- No page describes a direct browser-token SPA as the recommended web example.
- No page describes a hosted auth UI as a required or planned core surface.
- No page splits the first native example into separate mobile and desktop packages.
- The examples remain optional and outside Irongate core.

## Next Slice

After this slice, choose the first implementation slice:

```text
31_cloudflare_web_example_foundation
```

Building the Cloudflare web Worker first gives the browser example a secure BFF boundary and creates the protected `/api/*` routes the app can call later.
