# Web Views

Source: [packages/examples/web/src/views.ts](../../../../packages/examples/web/src/views.ts).

## What

HTML/CSS responses for login, email-only registration, password setup, messages and the signed-in
view.

## Why

The optional example owns presentation; auth Lambdas remain API-only. Email ownership is followed
by deliberate password choice rather than activation from opening a link.

## How

View helpers escape dynamic HTML, render provider controls from flags and add no-store, CSP,
nosniff and default no-referrer headers. Registration/completion routes override only the referrer
policy to strict-origin. Styling is example-specific and no logo asset is served by this module.

Registration collects only email. Password setup uses a hidden, HTML-escaped proof field and empty
password/confirmation inputs with new-password autocomplete. Both forms submit to clean relative
Worker paths without a query string. Routes bound the proof length before rendering. Passwords are
never supplied as view values or redisplayed after errors; the proof appears only in the necessary
hidden form field, not in diagnostics or browser storage. `messageView` is used only by registration
and completion success responses and includes a sign-in link; it does not initiate login itself.
