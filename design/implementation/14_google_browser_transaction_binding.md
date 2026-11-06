# 14_google_browser_transaction_binding

## Status

Implemented and locally verified on 2026-11-06.

Builds on `f5a3f63` (atomic Google email confirmation storage), following the user's implementation
request. No deployment, cloud changes, hosted auth pages or reference-repository edits were performed.

## Goal

Require Google's callback to come from the browser that started that provider transaction:

```text
browser -> /google/authorize
  -> short-lived host-only cookie + browser HMAC in provider state
  -> Google -> /google/callback
  -> matching browser proof + one-use state
  -> existing validated Google identity/code issuance
```

This establishes the browser proof needed alongside slice 13's durable mailbox-confirmation
record. It does not expose email confirmation or send its codes. New Google identities without
direct email authority continue to reject in the public callback.

## Why This Slice

Slice 13 verifies browser and email proofs atomically, but its fixtures supply the browser proof
directly. Current Google HTTP state has nonce and upstream PKCE, but no separate per-transaction
browser binding. Those checks remain necessary; this slice adds proof of possession of a cookie
established at provider start, before consuming the provider/session state.

The reference sets this cookie and checks it when starting email confirmation. Adapt the small
browser/state boundary, not its hosted confirmation page. Require the binding for all Google
browser callbacks instead of retaining bound and unbound branches.

This keeps the next change smaller than delivery, attempt limits, presentation and example forms
together. It preserves the API-only core without deciding where confirmation screens live. A
transaction cookie is not a hosted UI, account session, access token or refresh token.

## Scope

In scope:

- Google provider-start cookie generation and provider-state binding.
- Typed state with a required Google binding and an explicit Apple variant.
- Proof-aware single-use state consumption before session consumption or upstream work.
- Google callback cookie validation, safe response headers and transaction-cookie cleanup.
- Affected fixtures, HTTP rejection tests and shared memory/DynamoDB Local state contracts.
- Exact design mirrors and setup notes for the browser-navigation contract.

Out of scope:

- Connecting Google callbacks to email confirmation, delivery or confirmation forms.
- New email templates, admission policies, audit events, public routes or SDKs.
- Hosted pages, domains, DNS, certificates, hosting migrations or application sessions/SSO.
- Apple's browser/form-post binding, native Google/Apple token flows or password behavior.
- Provider JWT/JWKS changes, downstream PKCE, token claims or account-resolution changes.
- Removing the existing authorize cookie or cleaning up unrelated encryption helpers.
- New dependencies, tables, indexes, IAM permissions, stage settings or secrets.
- Unbound Google fallback, compatibility readers, migrations or deployed-data cleanup.
- UI automation, live provider testing, cloud deployment or reference-repository changes.

The existing Cloudflare BFF and Tauri app already navigate the external browser to Irongate's
authorize path. No example runtime change is expected; the core remains independent of both.

## Reviewed Reference

Read-only repository: `/Users/robinsrimal/Desktop/64x64`, pinned to
`4e7207887f86aacba19d1bcf7ffea312055d5620` from the
[handoff](../research/irongate_auth_improvements_handoff.md).

Relevant pinned files:

- `packages/functions/auth/src/api/providers/google.rs`: browser binding at provider start.
- `packages/functions/auth/src/api/providers/email_confirmation.rs`: cookie parsing and browser
  proof comparison. Its HTML, email delivery, cancellation and route design are not imported here.
- `packages/functions/auth/src/store/records.rs` and `src/store/provider_states.rs`.

Reference HEAD when checked was `1985d096506c782fde36666361a3a047fbe4bcc8`, with a clean worktree.
Recheck status/HEAD before implementation; keep the pinned comparison. Do not copy an optional
Google binding or check it only after consuming state/session.

Current Irongate callers reviewed:

- `packages/examples/web/src/routes.ts`, `handleExternalProviderLogin`, redirects the browser to
  the issuer's authorize endpoint rather than fetching provider start server-side.
- `packages/examples/app/src-tauri/src/auth.rs`, `login_with_provider`, opens that endpoint in the
  system browser and waits for the registered loopback callback.

Both keep the new cookie on the auth issuer, not the Worker or loopback callback host.

## Design To Honor

- [Overview](../overview.md) and [threat model](../functions/auth/threat-model.md).
- [Google API](../functions/auth/api/providers/google.md) and
  [HTTP mirror](../functions/auth/src/api/providers/google.md).
- [Provider state](../functions/auth/store/provider-states.md),
  [state implementation](../functions/auth/src/store/provider_states.md) and
  [records](../functions/auth/src/store/records.md).
- [Cookies](../functions/auth/crypto/cookies.md) and
  [HMAC families](../functions/auth/src/crypto/hmac_lookup.md).
- [Provider issuance](../functions/auth/src/store/provider_issuance.md),
  [confirmation storage](../functions/auth/src/store/sign_in_confirmation.md) and
  [rate limits](../functions/auth/src/store/rate_limits.md).
- [Test guide](../../packages/functions/auth/tests/README.md),
  [Google setup](../../docs/setup/07-google-login.md),
  [web setup](../../docs/setup/11-cloudflare-web-example.md),
  [app setup](../../docs/setup/12-tauri-app-example.md) and
  [smoke checks](../../docs/operations/smoke-test.md).

Authoritative docs continue describing current behavior until implementation. Update them with
what actually ships, not this definition as an already-working guarantee.

## Provider Start

Preserve configuration validation, trusted-source/provider-session admission and Google session
lookup. Reject expired/wrong-provider sessions before creating provider state or its cookie.

Generate independent high-entropy 32-character base64url values for state and browser proof using
the existing random helper. Nonce and upstream PKCE remain independent. Store only the browser
HMAC using `LookupFamily::SignInBrowser` and the existing secret. Neither state, an incoming cookie,
the authorize-session key nor caller input can substitute for a freshly generated proof.

Set the cookie only after confirmed conditional state creation, on the `/google/authorize`
redirect response. Scope its deterministic name to the full state lookup digest:

```text
__Host-ig-google-<state-digest>=<random-browser-proof>
Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=<remaining-lifetime>
```

There is no Domain attribute. Lax accommodates Google's cross-site top-level GET callback; do not
apply this assumption to Apple's POST callback. The host prefix requires Secure, Path=/ and no
Domain. See [MDN's cookie attributes and prefixes](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie).

Cap state expiry at the original authorize-session expiry using checked arithmetic and the existing
provider-state TTL. Cookie lifetime must not extend that deadline; runtime record expiry remains
authoritative. No new TTL setting or hard-coded 600-second override. Use no-store and no-referrer
on Google provider-start/callback responses.

Independent names permit two pending transactions to finish in either order. Do not introduce one
global browser cookie that overwrites concurrent attempts, or a server-side cookie inventory.

## Callback And State Consumption

Validate the generated state-handle shape before deriving a cookie name or accessing storage.
Parse only that transaction's cookie with the existing cookie dependency. Missing, empty, malformed
or duplicate matching cookies reject; do not arbitrarily choose the first/last duplicate. Ignore
unrelated valid cookies. Require the generated base64url shape. Query/body parameters, forwarded
headers and the old authorize cookie cannot supply this proof.

The typed state operation must:

1. Strongly read the exact provider-state key without consuming it.
2. Require the expected provider, a valid Google browser digest and live logical expiry.
3. Compare the supplied browser HMAC with the stored digest in constant time.
4. Recheck expiry and conditionally delete the exact captured record once.
5. Return the consumed record only after confirmed success.

Wrong/missing browser proof or provider mismatch must not consume provider/session state, exchange
an upstream code, fetch JWKS, send email or mutate identity/code state. Matching proof is required
even on a provider-declined/error callback. A valid bound decline follows the existing client-error
redirect contract after consuming its intended state/session.

Concurrent deletion/replacement rejects as invalid/replayed state without rereading a newer record.
Operational errors, conflicts and uncertain outcomes remain safe server errors through the existing
mapper. A lost response can follow a committed deletion; never restore state or retry exchange
automatically. Expired state need not be eagerly removed; TTL is cleanup, not authorization.

After successful consumption, retain existing session/provider checks, code exchange, complete
ID-token validation, bounded JWKS refresh and atomic identity/code issuance. Preserve client,
exact redirect, scope, nonce, PKCE and generation checks. State/session consumption still precedes
issuance and is not part of its transaction.

Clear this transaction's cookie on success and handled terminal failure after confirmed state
consumption, using the same name/path/security attributes with Max-Age=0. Never clear another
transaction's cookies. Abandoned/unavailable state remains bounded by cookie expiry. Do not log
cookie/proof digests, state, codes or tokens, or add them to audit events.

This supplements OAuth defenses, not client state, PKCE, nonce or issuer validation; do not claim
it solves every login CSRF scenario. See [RFC 9700, Section 4.7](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.7).

## Typed Record Contract

Use one current shape with provider-specific required data:

```text
Google: provider=google, required browser_digest, session/PKCE/nonce/timestamps
Apple:  provider=apple, no Google browser proof, session/PKCE/nonce/timestamps
```

A small tagged provider-binding enum is appropriate. Keep discriminator and binding in agreement
by construction, rather than a permissive optional Google field with a default. Missing/malformed
Google bindings fail closed. An explicitly unbound Apple variant represents its current flow,
not an unbound fallback for Google.

Replace unchecked generic state consumption with a typed expectation/proof, such as Google browser
digest versus Apple. An Apple expectation must not consume Google state. Update production callers
and fixtures; do not retain the unchecked method only for tests. Ordinary Apple callback behavior
otherwise remains unchanged. Pre-change Google state requires a fresh sign-in, not migration.

Keep cookie parsing in a small HTTP helper and record/proof checking in the typed store. Do not
expose raw-state reads plus unconditional deletion to routes. Creation validates shape and writes
matching TTL; consumption guards the exact original value. No partial updates, scans or aliases.

Do not change slice 13's confirmation schema or activate it from callbacks. Keep the validated
browser proof in callback scope without adding speculative public DTOs or endpoints.

## Change Surface And Docs

Expected runtime files:

```text
packages/functions/auth/src/api/providers/google.rs
packages/functions/auth/src/api/providers/browser_binding.rs  (small new HTTP helper)
packages/functions/auth/src/api/providers/mod.rs
packages/functions/auth/src/api/providers/apple.rs             (typed state call only)
packages/functions/auth/src/store/records.rs
packages/functions/auth/src/store/provider_states.rs
```

Reuse the provider clock or a narrowly injected state clock for deterministic expiry; do not add
sleeps or change unrelated clocks. Touch `store/mod.rs` only if clock wiring is needed.

Extend Google start/callback tests and update Apple state fixtures. Add domain-named shared
provider-state contracts to memory and the existing DynamoDB Local harness, reusing transaction
gates and signed fixtures. Do not duplicate issuance suites. Split files before 1,000 lines.

Mirror every changed/new source/test under its exact `design/functions/...` path, preserving `src/`
and `tests/`. Update cookie/provider-state/Google overviews, threat model and setup/smoke docs.
Explain that the browser visits the issuer's provider-start route; a server-side fetch followed by
transplanting its cookie onto an app host is not the supported contract.

Read and verify the existing web/native navigation paths; do not rewrite their runtime, hosting
or UI without a concrete mismatch. Document that public mailbox confirmation is still unavailable.

## Verification

Run the current Google start/callback and Apple callback baseline before implementation. Cover
observable behavior rather than construction counts or source-string checks:

- Start stores only the browser HMAC, sets the scoped secure cookie with bounded lifetime, and
  never sends the proof to Google or exposes it in code claims/logs.
- Independent starts work in either order with their own cookies; swapping proofs fails.
- Missing/wrong/malformed/duplicate cookies, unbound Google records and provider mismatch reject
  before session consumption or upstream calls. Other inputs cannot replace the cookie.
- Correct-cookie success produces an exchangeable code. Existing signed-token issuer, audience,
  nonce, signature and unknown-key rejections still run beyond cookie admission.
- A bound provider error consumes only its intended transaction and redirects safely with state;
  an error callback without proof cannot bypass admission.
- Check exact expiry, cookie clearing, no-store/no-referrer and another transaction remaining usable.
  These are HTTP/security contracts, not UI copy or screenshot tests.
- Shared memory/Local contracts exercise wrong proof/provider, replay, exact-record replacement,
  competing valid consumers and final expiry checks. One state consumer wins; the loser cannot
  consume another session or start exchange.
- Controlled pre-dispatch failure and a lost response after real commit remain distinct from
  confirmed conditional failure. Inspect actual stored outcomes; do not infer rollback from errors.
- Raw Local reads verify HMAC storage and session-capped TTL without scans or TTL cleanup waits.
- Apple still completes its form-post path without Google's cookie. Existing Google authoritative
  email/returning identity and first-party token behavior remain otherwise unchanged.

Use the documented loopback-only disposable Local emulator, dummy credentials and unique tables;
stop it afterward. No deployed tables or live accounts. Ignored Local tests are not passing evidence.

```sh
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test oidc_google_start --test oidc_google_callback --test oidc_apple_callback
cargo test --locked --manifest-path packages/functions/auth/Cargo.toml
DYNAMODB_LOCAL_ENDPOINT=http://127.0.0.1:18023 \
  cargo test --locked --manifest-path packages/functions/auth/Cargo.toml \
  --test dynamodb_storage -- --ignored
cargo check --locked --manifest-path packages/functions/admin/Cargo.toml
cargo clippy --locked --manifest-path packages/functions/auth/Cargo.toml --all-targets
npm run test:infra
git diff --check
```

Check touched Rust formatting, relative links, mirrors and source sizes. Run example checks only
if their code changes. Record actual results, warnings and skipped checks. HTTP fixtures do not
certify browser cookie behavior, live Google login or native-device acceptance; those need separate
authorization. Do not use MCP UI tools for this slice.

## Acceptance Criteria

- Every Google browser transaction requires its own proof, separate from state/nonce/PKCE.
- Invalid proof cannot consume provider/session state or reach upstream exchange.
- Typed consumption checks provider, proof, expiry and the exact captured record.
- No unbound Google fallback or unchecked compatibility/test method remains.
- Google and Apple retain their validated identity/code and token contracts.
- App-owned UI and example hosting remain independent; confirmation is not exposed.
- Mirrors/setup docs and memory/Local verification are complete and recorded.
- No new route, infrastructure, dependency, credentials or deployment is introduced.

## Implementation Results

- Google start generates an independent browser proof and sets its state-scoped host cookie only
  after confirmed creation. Provider TTL is capped by the original session; the remaining cookie
  lifetime is rounded down. Handler responses use no-store/no-referrer.
- Google callbacks require one matching cookie before consuming state/session or contacting the
  provider, including declined sign-ins. Confirmed state consumption clears only that cookie on
  success or handled terminal failure. Old/unbound Google state fails closed and needs fresh sign-in.
- `ProviderStateBinding` requires Google's browser HMAC and represents Apple explicitly. The
  unchecked take method is replaced by a provider/proof-aware operation that validates shape,
  compares proof, rechecks expiry and conditionally deletes the original raw record once.
  Conditional loss does not adopt a replacement; operational/uncertain errors remain distinct.
- Apple uses the typed expectation without gaining a Google cookie. Existing signed-token,
  authoritative-email, returning identity and token-exchange tests still reach their own boundaries.
- Shared memory/Local state contracts replace two duplicate start-suite tests. HTTP contracts cover
  rejected proofs leaving state usable, independent starts in both orders, bounded cookies, terminal
  cleanup and competing callbacks with one upstream exchange. Local raw reads check matching TTL.
- Exact source/test mirrors, cookie/provider-state/Google overviews, threat model, test guide and
  Google/web/Tauri/smoke setup notes describe the shipped contract. Example source was unchanged:
  existing browser navigation already meets it. Public mailbox confirmation remains disconnected.

The pinned reference comparison remained `4e7207887f86aacba19d1bcf7ffea312055d5620`.
Reference HEAD was still `1985d096506c782fde36666361a3a047fbe4bcc8`; unrelated marketing edits and
a later slice present during implementation were left untouched. No product IDs or credentials
were imported.

### Verification Evidence

| Check | Result |
| --- | --- |
| Pre-change Google start/callback and Apple callback baseline | 45 passed |
| Focused provider/callback/state checks | 50 passed |
| Full locked auth test suite after runtime changes | 343 passed; 80 Local cases ignored in this run |
| Final Google callback rerun after correcting the unknown-state fixture | 23 passed |
| All explicitly enabled DynamoDB Local contracts | 80 passed, including four provider-state cases |
| Locked auth and admin Cargo checks | Passed |
| Locked auth Clippy, all targets | Passed with existing warnings; no new warning kind/file pairs against slice 13's baseline |
| `npm run test:infra` | Passed |
| Touched Rust formatting, source sizes, exact mirrors and relative links | Passed |
| `git diff --check` | Passed |

The Local run used dummy credentials, unique disposable tables and an in-memory emulator verified
to listen only on `127.0.0.1:18023`. The process was stopped afterward and its listener was absent.
The ordinary suite's loopback HTTP fixtures required local socket permission; no deployed endpoint
or real account was used. Early test runs exposed obsolete no-cookie assertions and a controlled
expiry-fixture timestamp mismatch; both were corrected before the passing runs above.

No UI automation, live Google/Apple acceptance, deployed AWS/Cloudflare validation or native-device
test was run. These local results do not certify browser cookie enforcement. TypeScript/setup and
example builds were not rerun because their runtime/configuration files did not change. No new
architectural decision or compatibility path was introduced.
