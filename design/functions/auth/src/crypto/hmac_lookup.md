# Lookup Digests

Source: [packages/functions/auth/src/crypto/hmac_lookup.rs](../../../../../packages/functions/auth/src/crypto/hmac_lookup.rs).

## What

Purpose-separated HMAC-SHA256 lookup keys.

## Why

Bearer values and reusable identity attributes need exact lookup without appearing raw in DynamoDB keys.

## How

The input is a fixed LookupFamily label, a zero separator and the raw value. Hex digests depend on the runtime lookup secret; changing that secret changes existing lookup addresses.

Rate-limit source, normalized email, client/source and provider/source have distinct family labels.
The limiter canonicalizes and frames inputs before hashing; this primitive does not normalize them.
