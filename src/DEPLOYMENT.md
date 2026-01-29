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

### Start the Auth Server

```bash
cd src/rust
AWS_ENDPOINT_URL=http://localhost:8000 \
AWS_ACCESS_KEY_ID=local \
AWS_SECRET_ACCESS_KEY=local \
AWS_DEFAULT_REGION=us-east-1 \
DYNAMODB_TABLE=irongate \
DEV_MODE=true \
ISSUER_URL=http://localhost:9000 \
cargo lambda watch -p 9000
```

The server runs on `http://localhost:9000` with hot-reload on code changes.

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
