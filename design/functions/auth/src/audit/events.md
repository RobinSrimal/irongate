# Audit Events

Source: [packages/functions/auth/src/audit/events.rs](../../../../../packages/functions/auth/src/audit/events.rs).

## What

The ten allowlisted OAuth/admin outcome variants and typed revocation reasons.

## Why

Security events cannot accept arbitrary names, free-form detail or request payloads.

## How

Serde tags variants with fixed snake-case event_type. Counts are numeric; identifiers have bounded
ASCII alphabets, token_hash is 64 hex characters, and actor/subject remain separate. Invalid values
fail emission without truncation or changing auth validation. Event kinds and payload validation
live together. Admin actor comes from trusted IAM context, not submitted headers.
