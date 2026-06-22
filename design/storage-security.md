# Storage Security Coverage

This document maps `STORAGE_SECURITY.md` recommendations to rewrite decisions.

## Physical Table

Decision: keep the physical DynamoDB table simple.

```text
pk string
sk string
value string
expiry number
```

The security boundary moves into typed store operations and secret-aware key construction. The route/provider layers should not manipulate raw DynamoDB items or generic key arrays.

Design coverage:

- `functions/auth/store/dynamodb.md`
- `functions/auth/store/keys.md`
- `functions/auth/store/records.md`

## DynamoDB Encryption At Rest

Decision: support two table encryption modes.

```text
AUTH_TABLE_KMS=aws-owned
AUTH_TABLE_KMS=customer
```

Default can remain AWS owned for the simplest template deployment. Production docs should recommend a customer managed KMS key.

Design coverage:

- `infra/auth/storage.md`
- `infra/auth/secrets.md`

## Bearer Secrets In Keys

Decision: raw bearer-style values never become DynamoDB `pk` or `sk`.

HMAC lookup digests are used for:

- OAuth session keys when treated as bearer-capable.
- Provider state.
- Authorization codes.
- Refresh tokens.
- Email verification link tokens.
- Password reset link tokens.
- Normalized email lookup for password users.

Design coverage:

- `functions/auth/crypto/hmac-lookups.md`
- `functions/auth/store/keys.md`
- `functions/auth/store/authorization-codes.md`
- `functions/auth/store/provider-states.md`
- `functions/auth/store/password-secrets.md`
- `functions/auth/store/refresh-tokens.md`

## Verification And Reset Link Tokens

Decision: verification and reset secrets use high-entropy link tokens with HMAC lookup digests,
short TTLs, and single-use consumption.

Design coverage:

- `functions/auth/store/password-secrets.md`
- `functions/auth/store/password-users.md`
- `functions/auth/core/passwords.md`

## JWT Private Key Storage

Decision: raw JWT private keys should not be readable through ordinary AuthTable access.

Preferred hardened target:

```text
AWS KMS asymmetric signing
private key non-exportable
```

Acceptable transitional target:

```text
local ES256 signing
private key encrypted before storage
decrypt permission narrower than table read permission
```

Design coverage:

- `functions/auth/crypto/signing.md`
- `infra/auth/secrets.md`

## Operational Read Access

Decision: human/operator tooling should not read raw auth records by default.

Roles:

- Runtime Lambda role can read/write required auth records.
- Operator admin role invokes sanitized IAM-protected lifecycle APIs.
- Deploy role manages infra and keys.
- Break-glass role is audited and not standing access.

Design coverage:

- `infra/auth/secrets.md`
- `infra/auth/storage.md`
- `functions/auth/observability/audit.md`
- `functions/admin/api.md`

## Account Deletion

Decision: deletion behavior is fixed, not config-based.

The target auth core uses anonymized tombstones:

- Account tombstone keeps only subject, deleted status, and deletion timestamp.
- Identity tombstone keeps only provider, HMAC identity digest, deleted status, deletion timestamp, and optional reuse timestamp.
- Password hash material, contact metadata, refresh tokens, reset secrets, verification secrets, and raw bearer values are removed.

Deleted identity reuse timing is configurable, but it never reuses the old subject and never changes the deletion shape.

Design coverage:

- `functions/auth/core/account-lifecycle.md`
- `functions/auth/store/accounts.md`
- `functions/auth/store/identities.md`

## Rewrite Checklist

- Keep one simple DynamoDB table unless a proven access pattern requires more.
- Use typed store methods instead of generic storage operations.
- Use HMAC lookup keys for token, code, state, session, and email lookup.
- Do not store raw bearer values in `pk`, `sk`, logs, or errors.
- Preserve expiry on every authorization-code, provider-state, verification, and reset path.
- Make customer managed KMS optional and recommended for production.
- Keep signing private keys out of ordinary AuthTable reads.
- Keep raw auth state behind runtime and audited break-glass access only.
- Use IAM-protected sanitized admin APIs for account lifecycle instead of raw table access.
- Keep account deletion behavior fixed and anonymized.
