# Deployment Guide

## Local Development

### Prerequisites

- [Rust](https://rustup.rs/)
- [cargo-lambda](https://www.cargo-lambda.info/guide/installation.html)
- [Docker](https://docs.docker.com/get-docker/)

### Start DynamoDB Local

```bash
cd src
docker compose up
```

This starts DynamoDB Local (in-memory, port 8000) and creates the `irongate` table automatically.

### Option A: In-Memory Storage (no Docker needed)

```bash
cd src/rust
DEV_MODE=true \
ISSUER_URL=http://localhost:9000 \
PROVIDERS=password \
PROVIDER_PASSWORD_TYPE=password \
cargo run
```

Runs as a plain Axum HTTP server on `http://localhost:9000` with in-memory storage. Data resets on restart.

### Option B: DynamoDB Local (persistent across restarts)

```bash
cd src
docker compose up
```

Then in another terminal:

```bash
cd src/rust
AWS_ENDPOINT_URL=http://localhost:8000 \
AWS_ACCESS_KEY_ID=local \
AWS_SECRET_ACCESS_KEY=local \
AWS_DEFAULT_REGION=us-east-1 \
DYNAMODB_TABLE=irongate \
ISSUER_URL=http://localhost:9000 \
PROVIDERS=password \
PROVIDER_PASSWORD_TYPE=password \
cargo run
```

### Bootstrap & Register a Test Client

```bash
cd src/test-client
bash setup.sh
```

This creates an admin API key and registers a public OAuth client (`test-app`) with redirect URI `http://localhost:3000/`. Re-run after each server restart if using in-memory storage.

### Test Client Web App

```bash
cd src/test-client
python3 -m http.server 3000
```

Open `http://localhost:3000` to test the full OAuth login flow.

### Connect Your App

Point your local website's OAuth config to `http://localhost:9000` as the issuer/authorization server URL.

### Useful Commands

```bash
# Run tests
cd src/rust && cargo test

# Check compilation
cd src/rust && cargo check

# View DynamoDB table contents
aws dynamodb scan \
  --endpoint-url http://localhost:8000 \
  --table-name irongate \
  --region us-east-1
```

## AWS Deployment

### Prerequisites

- [AWS CDK CLI](https://docs.aws.amazon.com/cdk/v2/guide/getting-started.html)
- [Docker](https://docs.docker.com/get-docker/) (required by cargo-lambda-cdk for cross-compilation)
- AWS credentials configured (`aws configure`)

### Deploy

```bash
cd src/infra
npm install
npx cdk bootstrap   # first time only
npx cdk deploy
```

CDK outputs the API Gateway URL and DynamoDB table name. The Lambda runs on ARM64 with the Rust binary built via cargo-lambda.

### Environment Variables (set by CDK)

| Variable | Local | AWS |
|---|---|---|
| `DYNAMODB_TABLE` | `irongate` | Set by CDK stack |
| `ISSUER_URL` | `http://localhost:9000` | API Gateway URL |
| `DEV_MODE` | `true` | `false` |
| `TRUSTED_PROXIES` | (not set) | `api-gateway` |
| `AWS_ENDPOINT_URL` | `http://localhost:8000` | (not set, uses default) |

No code changes between environments. The same Rust binary runs in both — only environment variables differ.
