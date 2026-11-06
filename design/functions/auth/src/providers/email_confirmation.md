# Google Confirmation Domain

Source: [packages/functions/auth/src/providers/email_confirmation.rs](../../../../../packages/functions/auth/src/providers/email_confirmation.rs).

## What

Internal `begin_google_confirmation` and `complete_google_confirmation` operations: deliver a
mailbox code, then admit and verify its proof before issuing the original OAuth continuation.
No public callback or endpoint calls them.

## Why

A signed third-party Google address is a delivery candidate, not email authority. Delivery must
preserve the original browser/session and cannot be treated as atomic with storage.
Short-code comparison needs separate online admission from email delivery.

## How

The caller supplies an opaque candidate after full Google validation, a consumed Google authorize
session, the admitted browser proof and trusted source identity. The operation checks handle/session
shape and expiry, then charges independent source/email budgets through `ProviderEmailSend`.
It generates a 32-character request handle and unbiased eight-digit code, derives purpose-separated
request/browser/request-bound code HMACs, and calls the guarded confirmation store once.

Only confirmed creation permits delivery to the candidate's normalized address. The store returns
the actual deadline: the lesser of the original session expiry and the configured provider-state
lifetime. Rendering uses existing brand/support settings and the existing email sender. The shared
provider clock checks expiry before and after sending; network waits never extend the deadline.
Success returns only the request handle and expiry, with no Debug/Serialize derivation. It does not
return an email code, subject, OAuth code or tokens.

The shared `ConfirmationError` distinguishes conditional/existence rejection, expiry, storage
unavailability, delivery unavailability and admission errors. Backend/delivery strings are discarded; admission preserves
existing retry metadata. An uncertain storage response stops sending but may leave a committed
pending row. Delivery failure may follow acceptance: pending state and charges remain, with no
automatic resend, refund or compensating deletion. Expiry during delivery can leave an email in
flight but cannot return a usable continuation. Pre-send checks cannot control remote delivery time.

Completion charges `ProviderEmailAttempt` before pending lookup, including invalid handles. The
typed store admits the browser into a private attempt value. Only its stored normalized email
selects `ProviderEmailConfirm` admission; code comparison happens afterward. Exactly eight ASCII
digits are accepted, retaining leading zeros without trimming. Source limits survive changing
handles, and email limits survive new requests, clients or source addresses. Limits are fixed-window,
not a lifetime lockout. Explicit global limiter disable retains its configured meaning.

Successful proof verification creates a private issuance-ready value. Completion generates a fresh
authorization code, stores only its HMAC and builds the continuation from the pinned redirect/state
with `url::Url` before consumption. A malformed stored redirect is unavailable, not a new destination.
Only confirmed finish returns `CompletedGoogleConfirmation { redirect_uri }`, without Debug/Serialize,
email, subject or tokens. No email is sent during completion. Unknown/expired/bad proofs return
invalid confirmation; corrupt records and operational errors return unavailable. Admission retains
its retry metadata. Neither conditional failures nor uncertain commits retry, re-resolve, refund or
restore proof. Confirmation expiry is checked after admission and before dispatch, not after a
confirmed commit; a delayed success still returns the issued code, with its own expiry.

HTTP completion and confirmation browser/CSRF transport remain absent. Internal attempt admission
does not make this an exposed flow or establish cookie provenance for a caller.
