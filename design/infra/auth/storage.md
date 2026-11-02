# Auth Table

Source: [infra/auth/storage.ts](../../../infra/auth/storage.ts).

## What

An SST DynamoDB table with string `pk`/`sk` primary keys and TTL enabled on `expiry`.

## Why

Typed auth operations share one physical table. The Rust backend owns JSON `value` serialization
and logical record families; infrastructure owns encryption and cleanup configuration.

## How

`aws-owned` uses DynamoDB default encryption. `customer` creates a symmetric KMS key with rotation,
a 30-day deletion window and alias `alias/<project>/auth-table-<stage>`. Checked-in dev uses
AWS-owned encryption and production uses customer-managed encryption. The output is the key ARN or
`aws-owned`.

TTL is asynchronous cleanup, not authorization. Runtime code checks record expiry. Encryption at
rest does not conceal values from an authorized table reader. Runtime IAM excludes Scan; current
partition-query and transaction semantics are described in the
[backend mirror](../../functions/auth/src/storage/dynamo.md).
