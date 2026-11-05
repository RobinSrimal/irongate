# Web Route Tests

Source: [packages/examples/web/tests/routes.test.ts](../../../../packages/examples/web/tests/routes.test.ts).

## What

Worker request/response tests with the injected auth transport, including email-first registration
and scanner-safe password completion.

## Why

The BFF's cookie, callback and upstream-request contracts are observable without a browser. Email
links must not activate credentials merely by being opened, and secret-bearing form submissions
must stay within the intended origin and request body.

## How

- Assert email-only registration forwarding, escaped proof fields, password/confirmation fields and
  clean relative form actions. Verification GET, reload, prefetch and HEAD make no upstream calls
  or session-store accesses; HEAD has no body, including missing-proof errors.
- Check the 1,024-character proof boundary for GET/HEAD rendering and POST forwarding. Oversized
  proofs fail without reflection, upstream calls or session changes.
- Assert deliberate POST forwards only the proof and untrimmed chosen password, does not echo
  submitted secrets or the upstream subject, and offers sign-in without creating a session.
- Reject missing, null, malformed and foreign Origins, alternate configured request origins,
  non-string/absent fields, mismatched confirmation, malformed bodies and unsupported methods
  before upstream calls. Exercise configured HTTPS and inferred origins, including nondefault ports.
- Assert no-store, strict-origin, CSP form/base/frame restrictions, no cookies and no redirects on
  setup responses. Upstream rejection bodies, thrown errors/values and invalid configuration are
  replaced by fixed safe errors; invalid web URL schemes/credentials fail closed.

Existing tests cover health, password/provider authorize starts, callback/session creation and
logout revocation. Assertions use MemorySessionStore, session-access tracking and fake transport
responses, not deployed Cloudflare, a browser, actual proof consumption or cryptographically
verified provider tokens. Core password policy, atomicity and replay tests belong to the Rust suite.
