# Web Views

Source: [packages/examples/web/src/views.ts](../../../../packages/examples/web/src/views.ts).

## What

HTML/CSS responses for login, registration, messages and the signed-in view.

## Why

The optional example owns presentation; auth Lambdas remain API-only.

## How

View helpers escape dynamic HTML, render provider controls from flags and add response/security headers. Forms call Worker routes. Styling is example-specific and no logo asset is served by this module.
