# DynamoDB Provider Issuance

Source: [packages/functions/auth/tests/dynamodb_storage/provider_issuance.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/provider_issuance.rs).

## What

Shared provider-issuance and Google/Apple ownership contracts plus a raw TTL/metadata case on DynamoDB Local.

## Why

Real transaction cancellation and physical TTL retention cannot be established by the memory
runner or a success-only SDK stub.

## How

The existing `dynamo::run` harness creates and removes a unique fixture-owned table for each case.
Every shared case delegates conditional transactions to `DynamoStorage`. The Local-specific case
uses both Google and Apple for new and returning issuance, checks physical code TTL against JSON
expiry and confirms that account, identity, email owner and reverse index records remain persistent. A real
code collision must preserve all four raw records including TTL, and the prior code stays usable.

The target is ignored by default and accepts only the harness's explicit loopback endpoint and
dummy credentials. It uses exact-key reads, no scans or background-TTL sleeps, and does not contact
deployed tables or validate production provider/permission/network behavior.
