# HMAC Lookups

Implementation: [hmac_lookup.rs](../src/crypto/hmac_lookup.md).

## What And Why

HMAC-SHA256 derives deterministic lookup keys without placing raw tokens, codes, email addresses or
provider identities in DynamoDB keys. A deployment secret prevents offline guessing of low-entropy
email inputs by an attacker who only reads the table.

## How

```text
digest = hex(HMAC-SHA256(lookup_secret, family_label || NUL || raw_value))
```

The enum defines separate labels for authorize sessions, provider state, authorization codes, refresh
tokens, verification/reset tokens, email and password/Google/Apple identities. Separate rate-limit
families cover source, email, client/source and provider/source. The limiter canonicalizes IPs and
uses JSON tuple framing for compound identifiers. Callers normalize emails before hashing; this
primitive does not normalize arbitrary input.

The current runtime uses one lookup secret with no previous-key fallback. Replacing it makes
existing keyed records, including password and identity lookups, unreachable under the new digest.
It is not a transparent rotation operation. The secret lives outside the auth table.

HMAC lookup protection is not encryption of record values. Provider flow records still contain
sensitive continuation material; raw table read access remains privileged.
