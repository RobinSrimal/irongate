# Irongate

Irongate is a security-first Rust implementation of an OpenAuth-style OAuth 2.0 authorization server for AWS.

The repository is intentionally small:

- `packages/functions` contains the Rust Lambda.
- `infra` contains the SST v3 infrastructure.
- `sst.config.ts` wires the AWS app.

The default deployment is one Lambda behind API Gateway HTTP API, backed by DynamoDB.

## Prerequisites

- Rust stable
- Node.js 22+
- AWS credentials configured for SST

## Configure

By default, SST passes the generated API Gateway URL to the Lambda as `ISSUER_URL`.
Set `ISSUER_URL` yourself only when the issuer will be reached through a custom domain.

```bash
export ISSUER_URL=https://auth.example.com
```

Provider configuration is passed through from environment variables:

```bash
export PROVIDERS=password
export PROVIDER_PASSWORD_TYPE=password
```

OAuth/OIDC providers use the existing `PROVIDER_{NAME}_*` variables from the Rust Lambda.

## Deploy

```bash
npm install
npm run deploy
```

SST outputs:

- `ApiUrl`
- `TableName`

After deploy, bootstrap the admin API key once:

```bash
curl -X POST "<ApiUrl>/admin/bootstrap"
```

Save the returned key. It is only shown once.

## Verify

```bash
cargo test --manifest-path packages/functions/Cargo.toml
npx sst install
npm run typecheck
```

Smoke-test a deployed issuer:

```bash
curl "<ApiUrl>/.well-known/oauth-authorization-server"
curl "<ApiUrl>/.well-known/jwks.json"
```
