# Infra IAM

Target code: `infra/*.ts` and SST-linked permissions.

## Owns

- Runtime Lambda permissions.
- API Gateway IAM authorization for admin lifecycle routes.
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
dynamodb:ConditionCheckItem
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

When `AUTH_SIGNING_MODE=kms-es256`, the Lambda needs only signing/public-key permissions on the configured asymmetric signing key:

```text
kms:Sign
kms:GetPublicKey
```

It should not receive broad `kms:*`.

## Role Boundaries

| Role | Target access |
| --- | --- |
| Auth Lambda role | Minimal DynamoDB actions, Resend secret read, HMAC secret read, KMS use only when configured |
| Admin Lambda role | Minimal DynamoDB lifecycle actions for accounts, identities, password users, reset/verification records, and refresh tokens; no provider/email/signing secrets by default |
| Deploy role | Creates and updates SST resources |
| Operator admin role | `execute-api:Invoke` only on explicit `/_admin/*` account lifecycle route ARNs |
| Break-glass role | Audited raw table access, no standing access |

## Admin Route IAM

Admin account lifecycle routes should be configured with API Gateway IAM authorization:

```ts
auth: { iam: true }
```

Operators call those routes with SigV4-signed requests. Their IAM policy should grant `execute-api:Invoke` only on the admin routes they need, for example:

```text
arn:aws:execute-api:<region>:<account>:<api-id>/<stage>/POST/_admin/users/*/disable
arn:aws:execute-api:<region>:<account>:<api-id>/<stage>/POST/_admin/users/*/delete
arn:aws:execute-api:<region>:<account>:<api-id>/<stage>/POST/_admin/users/*/revoke-sessions
arn:aws:execute-api:<region>:<account>:<api-id>/<stage>/GET/_admin/users/*
```

The public auth routes should not require IAM because browsers and mobile clients need the standard OAuth/OIDC flow. IAM is only for operator control-plane calls.

Admin routes should invoke a separate admin Lambda instead of the public auth Lambda. This keeps route authorization, runtime configuration, and IAM permissions easier to reason about. The admin Lambda should not receive Resend provider keys, Google/Apple provider secrets, or local JWT signing private keys unless a future lifecycle route explicitly requires them.

## Security Invariants

- Human operators do not get standing access to JWT private keys, password hashes, refresh records, or reset records.
- Runtime auth flows do not require `dynamodb:Scan`.
- Secrets are granted by exact secret/resource where SST supports it.
- KMS permissions are scoped to configured keys, not `kms:*`.
- Admin permissions are granted through IAM `execute-api:Invoke`, not custom application API keys.
- Admin lifecycle code is isolated in the admin Lambda, not mounted behind the public `$default` integration.
