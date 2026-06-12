# Irongate

Irongate is a template for starting an AWS app with a security-first Rust auth service.

It gives you:

- A Rust implementation of an OpenAuth-style OAuth 2.0 authorization server.
- A simple SST deployment to API Gateway, Lambda, and DynamoDB.
- A `packages/functions` area for adding your own business-logic functions beside auth.

The default deployment is one auth Lambda behind API Gateway HTTP API, backed by DynamoDB.

## Use This Template

Before you start, install:

- Rust stable
- Node.js 22+
- AWS CLI credentials for the target account

1. On GitHub, click **Use this template** and create a new repository.

   GitHub creates the new repository with these files and a fresh history. See GitHub's guide: <https://docs.github.com/en/repositories/creating-and-managing-repositories/creating-a-repository-from-a-template>

2. Clone your new repository.

   ```bash
   git clone <REPO_URL> my-app
   cd my-app
   ```

3. Run the template setup script.

   ```bash
   npm run setup -- my-app
   ```

   If you omit `my-app`, the script uses the checkout folder name. It rewrites the app/package names, the Rust crate name, and the default AWS profile names.

   By default, deployments use:

   - `my-app-dev` for non-production stages
   - `my-app-prod` for the `production` stage

   You can change those names later in `sst.config.ts`, or override them with `SST_DEV_AWS_PROFILE` and `SST_PROD_AWS_PROFILE`.

   The main files changed by the script are:

   - `package.json`
   - `package-lock.json`
   - `sst.config.ts`
   - `packages/functions/package.json`
   - `packages/functions/auth/Cargo.toml`

4. Install dependencies.

   ```bash
   npm install
   ```

5. Configure AWS credentials for SST.

   ```bash
   aws configure sso --profile my-app-dev
   aws configure sso --profile my-app-prod
   ```

   If `AWS_PROFILE` is set in your shell, unset it before deploying so SST can use the stage-specific profile from `sst.config.ts`.

6. Configure auth providers.

   Provider configuration is passed through from environment variables:

   ```bash
   export PROVIDERS=password
   export PROVIDER_PASSWORD_TYPE=password
   ```

   OAuth/OIDC providers use `PROVIDER_{NAME}_*` variables from the Rust auth Lambda.

7. Deploy to dev or production.

   ```bash
   npm run deploy -- --stage dev
   npm run deploy -- --stage production
   ```

   SST outputs:

   - `ApiUrl`
   - `TableName`

8. Bootstrap the admin API key once.

   ```bash
   curl -X POST "<ApiUrl>/admin/bootstrap"
   ```

   Save the returned key. It is only shown once.

## Repository Layout

```text
.
├── infra/
│   ├── api.ts              # API Gateway + auth Lambda route
│   └── storage.ts          # DynamoDB table
├── packages/
│   └── functions/
│       ├── auth/           # Rust auth Lambda crate
│       └── package.json    # Functions workspace package
├── sst.config.ts           # SST app entry point
├── package.json            # Root scripts and tooling
└── tsconfig.json
```

Add additional Lambda/function code under `packages/functions/<name>` and wire it from `infra/`.

## Configuration

By default, SST passes the generated API Gateway URL to the Lambda as `ISSUER_URL`.
Set `ISSUER_URL` yourself only when the issuer will be reached through a custom domain.

```bash
export ISSUER_URL=https://auth.example.com
```

## Verify

```bash
npm test
npx sst install
npm run typecheck
```

Smoke-test a deployed issuer:

```bash
curl "<ApiUrl>/.well-known/oauth-authorization-server"
curl "<ApiUrl>/.well-known/jwks.json"
```

## Maintainers

To publish this repository as a template, enable **Template repository** in the GitHub repository settings.
GitHub documents the setting here: <https://docs.github.com/en/repositories/creating-and-managing-repositories/creating-a-template-repository>
