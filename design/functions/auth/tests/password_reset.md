# Password Reset Tests

Source: [packages/functions/auth/tests/password_reset.rs](../../../../packages/functions/auth/tests/password_reset.rs).

## What

Reset-key storage, generic unknown-email responses and password replacement.

## Why

Reset proofs must be one use and bound to an active verified subject.

## How

Tests inspect escaped messages, generic forgot responses and eligible delivery. A verified-account
fixture proves preparation preserves the digest-keyed proof/index, completion removes them and
changes the actual password and generation once, and expired/reused proofs reject. Independent
hash-update/consume tests are replaced by the recovery contract. Real rollback and races are covered
in the local recovery modules, not inferred from this memory backend.
