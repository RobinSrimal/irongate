# Password Route Tests

Source: [packages/functions/auth/tests/api_routes/password.rs](../../../../../packages/functions/auth/tests/api_routes/password.rs).

## What

Email-first password registration/completion, route response shapes, redirects and rejection.

## Why

Registration/verification/reset must remain distinct from authenticated code issuance.

## How

Imports the parent api_routes fixtures and invokes the real router against TestStorage. Normal
password fixtures use registration plus emailed-proof completion, not store-only activation.
Email-only registration delivers a proof and leaves a pending binding with no credential, subject,
account or OAuth session. Completion stores the chosen password, clears the binding and consumes
the proof without issuing login tokens or a cookie; replay cannot change the installed hash.

Legacy registration passwords, unknown fields and token-only verification reject before state
changes. Weak/oversized passwords return invalid_request while preserving a usable proof. Missing,
logically expired, wrong-purpose, mismatched-registration and missing-registration-binding proofs
return one safe invalid_grant response and leave credential state unchanged. Deliberately expired proofs remain physically
present so rejection does not depend on the memory adapter's TTL filtering.

Forgot/reset and login assertions retain their existing contracts. Source/secondary policies are
configured explicitly; privacy assertions query bounded endpoint partitions, require nonempty
rows and reject raw IPs and flow secrets. Verification source denial preserves a second valid
proof, which another source can complete. Rotating identities and early rejection are covered in sibling
[rate_limits](rate_limits.md) and [admission](admission.md) modules. This is not a separate Cargo target.
