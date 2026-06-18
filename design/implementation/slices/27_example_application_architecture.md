# 27_example_application_architecture

## Status

Superseded by:

- `30_best_practice_examples_restructure`
- `31_cloudflare_web_example_foundation`

## Original Purpose

This slice originally defined a broader optional example architecture before implementation work started.

## Current Direction

The active example architecture is intentionally narrower:

```text
packages/examples/
  web/
  app/
```

The `web` example is a Cloudflare Worker BFF that also owns the initial protected `/api/*` routes. The `app` example is desktop-first Tauri with OS keychain storage and mobile-specific guidance.

No separate shared protected API package is part of the current example plan.
