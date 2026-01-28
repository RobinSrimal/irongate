# Irongate - Security-First OAuth 2.0 Server

Rust implementation of a security-first OAuth 2.0 authorization server with AWS CDK infrastructure.

## Structure

```
src/
├── rust/           # Rust Lambda code
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # Lambda entry point
│       ├── lib.rs          # Library exports
│       ├── config.rs       # Configuration
│       ├── error.rs        # Error types
│       ├── routes.rs       # Axum routes
│       ├── admin/          # Management API
│       ├── client/         # Client registry
│       ├── crypto/         # Cryptography
│       ├── jwt/            # JWT operations
│       ├── oauth/          # OAuth 2.0 core
│       ├── provider/       # Identity providers
│       ├── ratelimit/      # Rate limiting
│       ├── storage/        # DynamoDB storage
│       ├── subject/        # Subject handling
│       └── ui/             # HTML forms
└── infra/          # AWS CDK infrastructure
    ├── bin/
    │   └── irongate.ts
    └── lib/
        └── irongate-stack.ts
```

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [cargo-lambda](https://www.cargo-lambda.info/) for Lambda builds
- [Node.js](https://nodejs.org/) 20+
- [AWS CDK](https://docs.aws.amazon.com/cdk/) CLI

### Install cargo-lambda

```bash
# macOS
brew tap cargo-lambda/cargo-lambda
brew install cargo-lambda

# Or via pip
pip3 install cargo-lambda

# Or via cargo (requires Zig)
cargo install cargo-lambda
```

## Development

### Local Rust Development

```bash
cd src/rust

# Check compilation
cargo check

# Run tests
cargo test

# Start local Lambda emulator
cargo lambda watch

# Test endpoints
curl http://localhost:9000/.well-known/oauth-authorization-server
```

### Local DynamoDB

```bash
# Start local DynamoDB
docker run -p 8000:8000 amazon/dynamodb-local

# Create table
aws dynamodb create-table \
  --table-name irongate-local \
  --attribute-definitions AttributeName=pk,AttributeType=S AttributeName=sk,AttributeType=S \
  --key-schema AttributeName=pk,KeyType=HASH AttributeName=sk,KeyType=RANGE \
  --billing-mode PAY_PER_REQUEST \
  --endpoint-url http://localhost:8000
```

### Environment Variables

Create `src/rust/.env` for local development:

```bash
DYNAMODB_TABLE=irongate-local
DYNAMODB_ENDPOINT=http://localhost:8000
AWS_ACCESS_KEY_ID=local
AWS_SECRET_ACCESS_KEY=local
AWS_REGION=us-east-1
RUST_LOG=debug
ISSUER_URL=http://localhost:9000
DEV_MODE=true
TRUSTED_PROXIES=none
```

## Deployment

### CDK Setup

```bash
cd src/infra

# Install dependencies
npm install

# Bootstrap CDK (first time only)
cdk bootstrap

# Synthesize CloudFormation
cdk synth

# Deploy
cdk deploy
```

### Post-Deployment

```bash
# Get the API URL from CDK output
API_URL=https://xxx.execute-api.us-east-1.amazonaws.com

# Bootstrap admin key (only works once!)
curl -X POST $API_URL/admin/bootstrap
# Save the returned API key!

# Register your first client
curl -X POST $API_URL/admin/clients \
  -H "X-Admin-API-Key: YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "client_id": "my-app",
    "client_type": "public",
    "redirect_uris": ["https://app.example.com/callback"],
    "allowed_grant_types": ["authorization_code", "refresh_token"]
  }'
```

## Security Features

- **Mandatory client registration** - No anonymous clients
- **Explicit redirect URI allowlist** - Exact match only, no patterns
- **PKCE required by default** - Can be disabled per-client
- **Rate limiting enabled** - DynamoDB-based counters
- **Constant-time comparisons** - Prevents timing attacks
- **Argon2 password hashing** - For passwords and client secrets
- **Atomic token rotation** - DynamoDB transactions

## License

MIT
