# Google Confirmation Delivery

Source: [packages/functions/auth/src/providers/email_confirmation.rs](../../../../../packages/functions/auth/src/providers/email_confirmation.rs).

## What

Internal `begin_google_confirmation` operation: admit a send, create pending proof and deliver a
mailbox code. No public callback or endpoint calls it.

## Why

A signed third-party Google address is a delivery candidate, not email authority. Delivery must
preserve the original browser/session and cannot be treated as atomic with storage.

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

Conditional/existence rejection, expiry, storage unavailability, delivery unavailability and
admission errors remain distinct. Backend/delivery strings are discarded; admission preserves
existing retry metadata. An uncertain storage response stops sending but may leave a committed
pending row. Delivery failure may follow acceptance: pending state and charges remain, with no
automatic resend, refund or compensating deletion. Expiry during delivery can leave an email in
flight but cannot return a usable continuation. Pre-send checks cannot control remote delivery time.

This is not a public verification flow. Send limits do not bound code guesses; HTTP completion,
verification-attempt admission and confirmation browser/CSRF transport are not implemented here.
