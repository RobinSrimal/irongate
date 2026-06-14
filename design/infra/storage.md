# Infra Storage

Target code: `infra/storage.ts`

## Owns

- DynamoDB table creation.
- Primary key shape.
- TTL attribute.
- Optional customer managed KMS key for table encryption.

## Target Table

The initial table can remain:

```text
pk string
sk string
expiry number
value string
```

The auth store code owns the logical record layout. Infra only creates the physical table.

## KMS

Default template mode can use DynamoDB's AWS owned key for low-friction deployment.

Production should be able to opt into a customer managed KMS key:

```text
AUTH_TABLE_KMS=aws-owned
AUTH_TABLE_KMS=customer
```

Mode behavior:

| Mode | Behavior | Intended use |
| --- | --- | --- |
| `aws-owned` | DynamoDB default encryption with AWS owned key | Lowest-friction dev/template deploy |
| `customer` | Create/use stage-specific customer managed KMS key | Recommended production setting |

Suggested aliases:

```text
alias/<project-name>/auth-table-dev
alias/<project-name>/auth-table-prod
```

## Security Invariants

- TTL must be enabled on `expiry`.
- Non-runtime tooling should not require raw reads from this table.
- Customer managed KMS must be stage/account specific when enabled.
- The Lambda role should not need table-wide `Scan` for the target auth core.
- At-rest encryption is not treated as a substitute for secret-aware key and record design.
- Raw bearer secrets must not appear in DynamoDB `pk` or `sk`.

## Access Patterns

The table should support exact key reads/writes, bounded partition queries, conditional writes, and transactions. Runtime auth flows should avoid unbounded scans.
