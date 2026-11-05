# Early Admission Tests

Source: [packages/functions/auth/tests/api_routes/admission.rs](../../../../../packages/functions/auth/tests/api_routes/admission.rs).

## What

Router-level checks for early rejection, trusted sources and OAuth/provider identity changes.

## Why

A quota response alone does not prove password checks, mail or state changes were prevented.

## How

Uses real Axum routes, email-only password registration, proof-plus-new-password completion and
a memory-backed observed adapter. Normal fixtures complete the same flow before login. Valid
request shapes reach admission rather than failing JSON extraction. Checks denial before
password/session/code work, generic 503 for counter read failures in
each password dimension, and shared unknown/canonical source budgets despite spoofed headers.
Invalid emails use source only. OAuth tests rotate registered/unknown clients, keep unknown names
in one secondary bucket and allow an independent source. Google/Apple starts rotate sessions and
share a source ceiling without creating state after denial. Helpers support the sibling
[rotation tests](rate_limits.md); no deployed accounts or provider calls are used.
