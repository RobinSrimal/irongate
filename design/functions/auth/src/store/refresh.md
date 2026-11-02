# Refresh Families

Source: [packages/functions/auth/src/store/refresh.rs](../../../../../packages/functions/auth/src/store/refresh.rs).

## What

Issuance, rotation, reuse detection and family/subject revocation.

## Why

Long-lived sessions need single-use rotation without table scans.

## How

Issuance writes token, family and indexes transactionally. Rotation conditionally replaces the old token and advances the family, rejecting wrong clients and inactive accounts. Family revocation invalidates descendants. Current Update operations change logical family expiry but leave the existing DynamoDB TTL unchanged; subject cleanup queries indexed records.
