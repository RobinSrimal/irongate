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

- `auth/store/dynamodb.md`
- `auth/store/keys.md`
- `auth/store/records.md`

## DynamoDB Encryption At Rest

Decision: support two table encryption modes.

```text
AUTH_TABLE_KMS=aws-owned
AUTH_TABLE_KMS=customer
```

Default can remain AWS owned for the simplest template deployment. Production docs should recommend a customer managed KMS key.

Design coverage:

- `infra/storage.md`
- `infra/secrets.md`

## Bearer Secrets In Keys

Decision: raw bearer-style values never become DynamoDB `pk` or `sk`.

HMAC lookup digests are used for:

- OAuth session keys when treated as bearer-capable.
- Provider state.
- Authorization codes.
- Refresh tokens.
- Email verification codes or link tokens.
- Password reset codes or link tokens.
- Normalized email lookup for password users.

Design coverage:

- `auth/crypto/hmac-lookups.md`
- `auth/store/keys.md`
- `auth/store/authorization-codes.md`
- `auth/store/provider-states.md`
- `auth/store/password-secrets.md`
- `auth/store/refresh-tokens.md`

## Short Code Storage

Decision: verification and reset secrets use HMAC lookup digests, short TTLs, single-use consumption, and central attempt updates that preserve expiry.

Design coverage:

- `auth/store/password-secrets.md`
- `auth/store/password-users.md`
- `auth/core/passwords.md`

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

- `auth/crypto/signing.md`
- `infra/secrets.md`

## Operational Read Access

Decision: human/operator tooling should not read raw auth records by default.

Roles:

- Runtime Lambda role can read/write required auth records.
- Deploy role manages infra and keys.
- Break-glass role is audited and not standing access.

Design coverage:

- `infra/secrets.md`
- `infra/storage.md`
- `auth/observability/audit.md`

## Rewrite Checklist

- Keep one simple DynamoDB table unless a proven access pattern requires more.
- Use typed store methods instead of generic storage operations.
- Use HMAC lookup keys for token, code, state, session, and email lookup.
- Do not store raw bearer values in `pk`, `sk`, logs, or errors.
- Preserve expiry on every authorization-code, provider-state, verification, and reset update.
- Make customer managed KMS optional and recommended for production.
- Keep signing private keys out of ordinary AuthTable reads.
- Keep raw auth state behind runtime and audited break-glass access only.
