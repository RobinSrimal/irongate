# DynamoDB Ownership Contracts

Source: [packages/functions/auth/tests/dynamodb_storage/email_ownership.rs](../../../../../packages/functions/auth/tests/dynamodb_storage/email_ownership.rs).

## What

Local runner for shared email-ownership contracts and physical persistence/corruption regressions.

## Why

Memory-only tests cannot establish DynamoDB transaction behavior, physical TTL absence or failure
when malformed physical rows would otherwise hide incomplete deletion.

## How

The loopback-only harness gives each case a uniquely owned disposable table with dummy credentials.
The shared runner uses real `DynamoStorage` for rejection, retention, cleanup and race contracts.
Raw exact-key reads verify that owners, reverse indexes and deletion tombstones have no TTL;
retention stays an eligibility timestamp, not an expiry attribute.

A Local-only matrix replaces an actual owner or subject index with a missing, non-string or
malformed-JSON `value` attribute using raw SDK writes. GetItem and both query methods must return
sanitized operational errors. Deletion must also fail, leaving the exact raw owner/index pair
unchanged despite the earlier account-deletion transition. Restoring only the deliberately corrupt
fixture permits retry, preserving the persisted deletion time and removing the subject linkage/index.
Raw SDK reads bypass the typed fixture decoder solely to inspect malformed attributes faithfully.

Cases are ignored unless `DYNAMODB_LOCAL_ENDPOINT` explicitly selects a disposable loopback service.
There are no scans, deployed tables, live mail or fake successful transaction responses. Malformed
physical keys, which real tables cannot store, are covered by the separate wire-contract target.
