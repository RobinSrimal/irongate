# Google Browser Cookies

Source: [packages/functions/auth/src/api/providers/browser_binding.rs](../../../../../../packages/functions/auth/src/api/providers/browser_binding.rs).

## What

Private Google transaction-cookie parsing, generation and response protection.

## Why

A callback must prove possession of a browser secret independent of its state query parameter.
Separate cookie names let concurrent transactions finish without overwriting each other's proof.

## How

State handles and proofs require exactly 32 base64url-alphabet ASCII characters. The cookie name is
`__Host-ig-google-<full-state-HMAC>`. Parsing examines all Cookie headers, rejects malformed input
and duplicate matching names, and ignores unrelated valid cookies. Only the matching cookie supplies
the proof; query parameters and the old authorize cookie do not.

Generated cookies have Path=/, Secure, HttpOnly, SameSite=Lax and no Domain. Lax permits Google's
top-level GET return; it is not Apple's POST policy. The caller supplies the bounded Max-Age, or
an empty value and zero Max-Age after consumption. This is not an application session/SSO cookie.
The response wrapper applies no-store and no-referrer to success and safe OAuth errors, including
query extraction failures. Invalid browser input gets a fixed error without reflecting secrets.
