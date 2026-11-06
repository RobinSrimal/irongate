# Confirmation Completion Contracts

Source: [packages/functions/auth/tests/google_confirmation_completion.rs](../../../../packages/functions/auth/tests/google_confirmation_completion.rs).

## What

Internal completion admission, rejection, expiry and commit-outcome tests on the real typed store.

## Why

Browser binding and a correct code cannot bypass attempt admission or justify a rollback claim
after uncertain completion.

## How

Uses existing confirmation/provider fixtures, TestStorage and rate/transaction controls. Varying
requests, emails and clients cannot evade source limits; varying sources and newly created requests
cannot evade the normalized-email budget. Unknown/expired/wrong-browser inputs allocate no email
budget. Malformed and wrong codes with a valid browser consume attempts but leave pending state.

Missing/invalid policies, corrupt counters, read faults and committed counter response loss stop
issuance without refunds. Explicit disable is checked separately. Paused email admission crosses
the provider deadline deterministically. Corrupt pending records, read outages and invalid stored
redirects are unavailable, not proof mismatches. Exact row checks preserve pending state and codes.

Ordered competing completions return one code. Pre-dispatch failure, real conditional conflict,
lost response after commit and held success after commit have distinct observed row outcomes.
A delayed success returns its issued continuation after confirmation expiry; retry after consumed
state cannot issue a second code. The [protocol child](google_confirmation_completion/protocol.md)
checks the complete delivered-proof/token path. No public route, live provider or UI test is implied.
