# Infra IAM

Target code: `infra/*.ts` and SST-linked permissions.

## Owns

- Runtime Lambda permissions.
- Deploy role assumptions.
- Break-glass access boundaries.

## Runtime Lambda Permissions

The auth Lambda should have only the DynamoDB permissions needed by typed store operations:

```text
dynamodb:GetItem
dynamodb:PutItem
dynamodb:UpdateItem
dynamodb:DeleteItem
dynamodb:Query
dynamodb:TransactWriteItems
```

Avoid runtime permission for:

```text
dynamodb:Scan
iam:*
kms:*
secretsmanager:* broadly
dynamodb:* broadly
```

If customer managed KMS is enabled for DynamoDB, the Lambda should receive only the key usage required by DynamoDB and any explicitly configured secrets/signing path.

## Role Boundaries

| Role | Target access |
| --- | --- |
| Auth Lambda role | Minimal DynamoDB actions, Resend secret read, HMAC secret read, KMS use only when configured |
| Deploy role | Creates and updates SST resources |
| Break-glass role | Audited raw table access, no standing access |

## Security Invariants

- Human operators do not get standing access to JWT private keys, password hashes, refresh records, or reset records.
- Runtime auth flows do not require `dynamodb:Scan`.
- Secrets are granted by exact secret/resource where SST supports it.
- KMS permissions are scoped to configured keys, not `kms:*`.
