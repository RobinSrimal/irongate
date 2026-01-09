# OpenAuth Rust Implementation Proposal

## Overview

This document provides a comprehensive guide for implementing OpenAuth in Rust with AWS CDK infrastructure-as-code in TypeScript. The goal is to create a production-ready, secure OAuth 2.0 authorization server that runs on AWS Lambda with all application logic written in Rust.

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│               AWS CDK (TypeScript)                      │
│  - Infrastructure definition (lib/openauth-stack.ts)   │
│  - DynamoDB table provisioning                         │
│  - Lambda function via cargo-lambda-cdk                │
│  - API Gateway setup                                    │
│  - IAM roles and permissions                           │
└─────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│              AWS Lambda (Rust Runtime)                  │
│  ┌──────────────────────────────────────────────────┐  │
│  │  OpenAuth Issuer (Rust)                          │  │
│  │  - HTTP handler (lambda_http)                    │  │
│  │  - OAuth 2.0 endpoints                           │  │
│  │  - JWT signing/verification                      │  │
│  │  - PKCE validation                               │  │
│  │  - Provider integrations                         │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│                    DynamoDB                             │
│  - Signing keys (signing:key)                           │
│  - Encryption keys (encryption:key)                     │
│  - Refresh tokens (oauth:refresh)                       │
│  - Authorization codes (oauth:code)                      │
│  - Password hashes (oauth:password)                      │
└─────────────────────────────────────────────────────────┘
```

## What Stays in TypeScript

### AWS CDK Infrastructure (`lib/openauth-stack.ts`)

The infrastructure definition uses AWS CDK with `cargo-lambda-cdk` for Rust support:

```typescript
import * as cdk from "aws-cdk-lib"
import { Construct } from "constructs"
import * as dynamodb from "aws-cdk-lib/aws-dynamodb"
import * as apigateway from "aws-cdk-lib/aws-apigatewayv2"
import * as integrations from "aws-cdk-lib/aws-apigatewayv2-integrations"
import { RustFunction } from "cargo-lambda-cdk"

export class OpenAuthStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props)

    // DynamoDB Table
    const table = new dynamodb.Table(this, "AuthTable", {
      partitionKey: { name: "pk", type: dynamodb.AttributeType.STRING },
      sortKey: { name: "sk", type: dynamodb.AttributeType.STRING },
      billingMode: dynamodb.BillingMode.PAY_PER_REQUEST,
      timeToLiveAttribute: "expiry",
      removalPolicy: cdk.RemovalPolicy.RETAIN,
    })

    // Rust Lambda Function (using cargo-lambda-cdk)
    const authFunction = new RustFunction(this, "AuthFunction", {
      manifestPath: "../rust/Cargo.toml",
      architecture: cdk.aws_lambda.Architecture.ARM_64,
      memorySize: 256,
      timeout: cdk.Duration.seconds(30),
      environment: {
        DYNAMODB_TABLE: table.tableName,
        RUST_LOG: "info",
      },
    })

    // Grant DynamoDB permissions
    table.grantReadWriteData(authFunction)

    // API Gateway
    const api = new apigateway.HttpApi(this, "AuthApi", {
      apiName: "OpenAuthApi",
      corsPreflight: {
        allowOrigins: ["*"],
        allowMethods: [apigateway.CorsHttpMethod.ANY],
        allowHeaders: ["*"],
      },
    })

    // Default route to Lambda
    api.addRoutes({
      path: "/{proxy+}",
      methods: [apigateway.HttpMethod.ANY],
      integration: new integrations.HttpLambdaIntegration(
        "AuthIntegration",
        authFunction,
      ),
    })

    // Root route
    api.addRoutes({
      path: "/",
      methods: [apigateway.HttpMethod.ANY],
      integration: new integrations.HttpLambdaIntegration(
        "RootIntegration",
        authFunction,
      ),
    })

    // Outputs
    new cdk.CfnOutput(this, "ApiUrl", {
      value: api.url ?? "undefined",
      description: "OpenAuth API URL",
    })

    new cdk.CfnOutput(this, "TableName", {
      value: table.tableName,
      description: "DynamoDB Table Name",
    })
  }
}
```

### CDK App Entry Point (`bin/openauth.ts`)

```typescript
#!/usr/bin/env node
import "source-map-support/register"
import * as cdk from "aws-cdk-lib"
import { OpenAuthStack } from "../lib/openauth-stack"

const app = new cdk.App()

new OpenAuthStack(app, "OpenAuthStack", {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: process.env.CDK_DEFAULT_REGION ?? "us-east-1",
  },
})
```

### CDK Dependencies (`package.json`)

```json
{
  "name": "openauth-infra",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "cdk": "cdk",
    "deploy": "cdk deploy",
    "destroy": "cdk destroy",
    "diff": "cdk diff",
    "synth": "cdk synth"
  },
  "devDependencies": {
    "@types/node": "^20",
    "typescript": "^5.6",
    "aws-cdk": "^2.170"
  },
  "dependencies": {
    "aws-cdk-lib": "^2.170",
    "cargo-lambda-cdk": "^0.0.30",
    "constructs": "^10.0.0",
    "source-map-support": "^0.5"
  }
}
```

## What Needs to be Rewritten in Rust

### Core Modules to Implement

1. **HTTP Server & Lambda Handler**
   - Lambda runtime integration
   - HTTP request/response handling
   - Route definitions (authorize, token, userinfo, well-known endpoints)

2. **Storage Adapter (DynamoDB)**
   - DynamoDB client integration
   - Key encoding/decoding
   - TTL handling
   - Atomic operations (for refresh token rotation)

3. **JWT Operations**
   - JWT signing (ES256)
   - JWT verification
   - JWKS endpoint implementation
   - Key generation and rotation

4. **OAuth 2.0 Core**
   - Authorization code generation and validation
   - Token exchange endpoint
   - Refresh token rotation (with atomic operations)
   - PKCE validation (with constant-time comparison)
   - Client credentials grant (with secret validation)

5. **Cryptography**
   - Key pair generation (ES256 for signing, RSA-OAEP-512 for encryption)
   - Cookie encryption/decryption
   - PKCE challenge generation and validation
   - Password hashing (PBKDF2 or Argon2)

6. **Provider System**
   - Provider trait/interface
   - OAuth2 provider implementation
   - OIDC provider implementation
   - Password provider
   - Code provider

7. **Subject Validation**
   - Schema validation (replace valibot with Rust equivalent)
   - Subject resolution and hashing

8. **UI Components** (Optional - can be simplified)
   - HTML form generation
   - Provider selection UI
   - Password/Code input forms

## Rust Dependencies

### Core Dependencies

```toml
[dependencies]
# HTTP Server & Lambda (cargo-lambda compatible)
lambda_http = "0.13"  # Lambda HTTP adapter
lambda_runtime = "0.13"  # Core Lambda runtime
tokio = { version = "1", features = ["full"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors"] }

# Optional: Use axum for routing
axum = { version = "0.7", optional = true }

# AWS SDK
aws-sdk-dynamodb = "1.54"
aws-config = { version = "1.5", features = ["behavior-version-latest"] }

# JWT & Cryptography
jsonwebtoken = "9.3"  # JWT signing/verification
p256 = "0.13"  # ES256 signing (ECDSA with P-256)
rsa = "0.9"  # RSA encryption (RSA-OAEP-512)
sha2 = "0.10"  # SHA-256 for PKCE and subject hashing
base64 = "0.22"
data-encoding = "2.6"  # base64url encoding

# CRITICAL: Constant-time comparison
subtle = "2.6"  # For timing-safe comparisons

# Password Hashing
argon2 = "0.5"  # Recommended over PBKDF2
rand = "0.8"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Validation
validator = "0.18"

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Utilities
url = "2.5"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.10", features = ["v4"] }
http = "1.1"
cookie = "0.18"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[features]
default = []
axum = ["dep:axum"]
```

### Build Configuration

```toml
[profile.release]
strip = true
lto = true
codegen-units = 1
opt-level = "z"  # Optimize for size

[target.aarch64-unknown-linux-musl]
linker = "aarch64-linux-musl-gcc"
```

## Implementation Phases

### Phase 1: Project Setup & Infrastructure

1. **Create Project Structure**

   ```
   openauth-rust/
   ├── rust/                      # Rust Lambda code
   │   ├── Cargo.toml
   │   ├── .cargo/
   │   │   └── config.toml
   │   ├── src/
   │   │   ├── main.rs           # Lambda entry point
   │   │   ├── lib.rs            # Library exports
   │   │   ├── config.rs         # Configuration
   │   │   ├── error.rs          # Error types
   │   │   ├── routes.rs         # HTTP route definitions
   │   │   ├── storage/
   │   │   │   ├── mod.rs
   │   │   │   ├── adapter.rs    # StorageAdapter trait
   │   │   │   └── dynamo.rs     # DynamoDB implementation
   │   │   ├── jwt/
   │   │   │   ├── mod.rs
   │   │   │   ├── sign.rs       # JWT signing
   │   │   │   ├── verify.rs     # JWT verification
   │   │   │   └── keys.rs       # Key management
   │   │   ├── oauth/
   │   │   │   ├── mod.rs
   │   │   │   ├── authorize.rs  # /authorize endpoint
   │   │   │   ├── token.rs      # /token endpoint
   │   │   │   ├── userinfo.rs   # /userinfo endpoint
   │   │   │   ├── well_known.rs # /.well-known/* endpoints
   │   │   │   └── pkce.rs       # PKCE implementation
   │   │   ├── crypto/
   │   │   │   ├── mod.rs
   │   │   │   ├── encrypt.rs    # Cookie encryption
   │   │   │   ├── password.rs   # Password hashing
   │   │   │   └── random.rs     # Secure random generation
   │   │   ├── provider/
   │   │   │   ├── mod.rs
   │   │   │   ├── traits.rs     # Provider trait
   │   │   │   ├── oauth2.rs     # Generic OAuth2
   │   │   │   ├── oidc.rs       # Generic OIDC
   │   │   │   ├── password.rs   # Password provider
   │   │   │   ├── code.rs       # OTP code provider
   │   │   │   ├── github.rs     # GitHub
   │   │   │   ├── google.rs     # Google
   │   │   │   └── ...           # Other providers
   │   │   ├── subject/
   │   │   │   ├── mod.rs
   │   │   │   └── schema.rs     # Subject validation
   │   │   └── ui/
   │   │       ├── mod.rs
   │   │       ├── select.rs     # Provider selection
   │   │       ├── password.rs   # Password forms
   │   │       └── code.rs       # Code input forms
   │   └── tests/
   │       ├── integration/
   │       └── unit/
   ├── infra/                     # CDK infrastructure
   │   ├── bin/
   │   │   └── openauth.ts       # CDK app entry
   │   ├── lib/
   │   │   └── openauth-stack.ts # Stack definition
   │   ├── cdk.json
   │   ├── package.json
   │   └── tsconfig.json
   └── README.md
   ```

2. **Set up SST Configuration**
   - Define DynamoDB table with TTL
   - Configure Lambda function with custom runtime
   - Set up API Gateway
   - Configure IAM permissions

3. **Build System Setup**
   - Create build script for Rust binary
   - Configure Lambda deployment package
   - Set up CI/CD pipeline

### Phase 2: Core Infrastructure

1. **Storage Adapter (DynamoDB)**
   - Implement `StorageAdapter` trait:
     ```rust
     #[async_trait]
     pub trait StorageAdapter: Send + Sync {
         async fn get(&self, key: &[&str]) -> Result<Option<Value>>;
         async fn set(&self, key: &[&str], value: Value, expiry: Option<DateTime>) -> Result<()>;
         async fn remove(&self, key: &[&str]) -> Result<()>;
         async fn scan(&self, prefix: &[&str]) -> Result<Vec<(Vec<String>, Value)>>;
     }
     ```
   - Key encoding/decoding (using separator `\x1f` - Unit Separator)
   - Key structure examples:
     - Signing keys: `["signing:key", "{uuid}"]`
     - Encryption keys: `["encryption:key", "{uuid}"]`
     - Auth codes: `["oauth:code", "{code}"]`
     - Refresh tokens: `["oauth:refresh", "{subject}", "{token}"]`
     - Password hashes: `["oauth:password", "{email}"]`
   - For keys with 2 parts: `pk = key[0]`, `sk = key[1]`
   - For keys with >2 parts: `pk = join(key[0..2])`, `sk = join(key[2..])`
   - TTL handling (both manual check and DynamoDB-native)
   - **CRITICAL**: Implement atomic operations using DynamoDB Transactions for refresh token rotation

2. **AWS Credentials & Client**
   - IAM role-based credentials (automatic in Lambda)
   - Use `aws-config` for credential resolution
   - DynamoDB client initialization with retry config
   - Region from environment (`AWS_REGION`)

3. **CORS Configuration**
   - Well-known endpoints: `origin: *`, methods: `GET`
   - Token endpoint: `origin: *`, methods: `POST`
   - No credentials for CORS endpoints

### Phase 3: Cryptography & JWT

1. **Key Management**
   - Key pair generation:
     - Signing: ES256 (P-256 ECDSA)
     - Encryption: RSA-OAEP-512 (for cookie encryption)
   - Key storage in DynamoDB:
     - New keys: `["signing:key", "{uuid}"]`
     - Legacy keys: `["oauth:key", "{uuid}"]` (RS512, for backward compatibility)
   - Key rotation logic (generate new if no unexpired keys)
   - JWKS endpoint returns all keys (including expired, for verification)
   - Keys sorted by creation date (newest first)

2. **JWT Operations**
   - JWT signing with ES256
   - JWT verification
   - Token expiration handling
   - Subject encoding in JWT payload

3. **PKCE Implementation**
   - Code verifier generation (43-128 chars, base64url)
   - Challenge generation (SHA-256 hash)
   - **CRITICAL**: Constant-time comparison for validation

   ```rust
   use subtle::ConstantTimeEq;
   // Use constant_time_eq for challenge comparison
   ```

4. **Cookie Encryption & Management**
   - RSA-OAEP-512 encryption for cookie values
   - Compact JWE format (A256GCM content encryption)
   - Cookie settings:
     - `HttpOnly: true`
     - `Secure: true` (when HTTPS)
     - `SameSite: None` for cross-origin (or `Lax`/`Strict` for same-site)
     - `Path: /`
   - Cookies used:
     - `authorization` - Stores auth state during flow (24h TTL)
     - `provider` - Provider-specific state (10 min TTL)

### Phase 4: OAuth 2.0 Core

1. **Authorization Endpoint (`/authorize`)**
   - Query parameter parsing:
     - `response_type` (required): `code` or `token`
     - `redirect_uri` (required)
     - `client_id` (required)
     - `state` (optional but recommended)
     - `provider` (optional - selects provider directly)
     - `audience` (optional)
     - `code_challenge` / `code_challenge_method` (for PKCE)
   - Redirect URI validation via `allow` callback
   - Authorization state storage in encrypted cookie (24h TTL)
   - Provider selection logic (redirect to `/{provider}/authorize`)
   - If single provider configured, auto-redirect

2. **Token Endpoint (`/token`)**
   - **Authorization Code Grant** (`grant_type=authorization_code`):
     - Code validation (single-use enforcement)
     - Redirect URI matching
     - Client ID validation
     - PKCE verifier validation (constant-time)
     - **CRITICAL**: Delete code BEFORE token generation to prevent race conditions
     - Code TTL: 60 seconds
   - **Refresh Token Grant** (`grant_type=refresh_token`):
     - Token validation
     - **CRITICAL**: Use DynamoDB Transactions for atomic refresh token rotation
     - Reuse detection with configurable time window (default: 60s)
     - Token invalidation on reuse past window
     - Pre-generate next refresh token to avoid race conditions
     - Token format: `{subject}:{token_uuid}`
   - **Client Credentials Grant** (`grant_type=client_credentials`):
     - **CRITICAL**: Validate client_secret (currently missing in TS version)
     - Provider-specific client authentication
     - Token generation

3. **Response Type `token` (Implicit Flow)**
   - Tokens returned in URL hash fragment
   - Format: `#access_token=...&refresh_token=...&state=...`
   - Used for SPA apps without backend

4. **Well-Known Endpoints**
   - `/.well-known/oauth-authorization-server` - Returns issuer metadata
   - `/.well-known/jwks.json` - Returns public signing keys (include expired keys for verification)

5. **UserInfo Endpoint (`/userinfo`)**
   - Bearer token validation in Authorization header
   - Subject extraction and return
   - Validates `mode=access` in token payload

6. **Default TTL Configuration**
   - Access token: 30 days (`60 * 60 * 24 * 30`)
   - Refresh token: 1 year (`60 * 60 * 24 * 365`)
   - Refresh reuse window: 60 seconds
   - Refresh retention: 0 seconds (configurable)

### Phase 5: Provider System

1. **Provider Trait**

   ```rust
   pub trait Provider: Send + Sync {
       fn name(&self) -> &str;
       fn provider_type(&self) -> &str;  // For UI display
       fn init(&self, routes: &mut Router, ctx: ProviderContext);
       // Optional: client credentials support
       fn client(&self, _input: ClientInput) -> Option<Result<ClientResponse>> {
           None
       }
   }
   ```

2. **Provider Routing**
   - Each provider is mounted at `/{provider_name}/`
   - Standard routes: `/{provider}/authorize`, `/{provider}/callback`
   - Callback supports both GET and POST

3. **OAuth2 Provider (Base)**
   - Authorization URL generation with state
   - Token exchange
   - PKCE support (optional per provider)
   - Configurable scopes and query params

4. **OIDC Provider**
   - Extends OAuth2
   - ID token validation via JWKS
   - Standard OIDC claims

5. **Specific OAuth Providers to Implement**
   - Apple
   - Cognito
   - Discord
   - Facebook
   - GitHub
   - Google
   - JumpCloud
   - Keycloak
   - LinkedIn
   - Microsoft
   - Slack
   - Spotify
   - Twitch
   - X (Twitter)
   - Yahoo

6. **Password Provider**
   - Password hashing (Argon2 recommended, PBKDF2 as fallback)
   - Password verification with timing-safe comparison
   - Registration/login/change password flows
   - Email verification code (6 digits, unbiased generation)
   - Code TTL: 10 minutes

7. **Code Provider**
   - OTP code generation (6 digits by default)
   - Code delivery via callback (email/SMS)
   - Code verification with timing-safe comparison

### Phase 6: Subject System

1. **Subject Schema Definition**
   - Replace valibot with Rust validation
   - Schema definition and validation
   - Type-safe subject properties

2. **Subject Resolution**
   - Hash generation (use SHA-256, not SHA-1, and don't truncate to 16 chars)
   - Subject ID format: `{type}:{hash}`

### Phase 7: UI Components (Simplified)

1. **Provider Selection UI**
   - HTML form generation
   - Provider list rendering

2. **Password/Code Forms**
   - Form HTML generation
   - Error message display

### Phase 8: Error Handling

1. **OAuth Error Types**

   ```rust
   pub enum OAuthErrorCode {
       InvalidRequest,
       InvalidGrant,
       UnauthorizedClient,
       AccessDenied,
       UnsupportedGrantType,
       ServerError,
       TemporarilyUnavailable,
   }
   ```

2. **Application Error Types**
   - `MissingParameterError` - Required parameter not provided
   - `UnauthorizedClientError` - Client not allowed for redirect URI
   - `UnknownStateError` - Browser state lost (cookies expired)
   - `InvalidSubjectError` - Subject validation failed
   - `InvalidRefreshTokenError` - Refresh token invalid/expired
   - `InvalidAccessTokenError` - Access token invalid/expired
   - `InvalidAuthorizationCodeError` - Auth code invalid/expired

3. **Error Redirect Handling**
   - On OAuth errors, redirect to `redirect_uri` with:
     - `?error={error_code}&error_description={description}`
   - On unknown state errors, display error page

### Phase 9: Security Hardening

1. **Fix All Security Issues Identified**
   - ✅ Client credentials secret validation
   - ✅ Atomic refresh token operations (DynamoDB Transactions)
   - ✅ Constant-time PKCE comparison
   - ✅ Authorization code deletion before token generation
   - ✅ Proper SameSite cookie settings
   - ✅ DynamoDB TTL enabled
   - ✅ Rate limiting (add per-endpoint limits)
   - ✅ State validation in authorization flow
   - ✅ Subject hash using SHA-256 (not SHA-1)

2. **Additional Security Measures**
   - Input validation and sanitization
   - CSRF protection
   - Rate limiting middleware
   - Security headers
   - Logging and monitoring

3. **Redirect URI Validation (`allow` callback)**
   - Default behavior:
     - Allow if redirect URI is `localhost` or `127.0.0.1`
     - Compare redirect URI hostname to request hostname (via `x-forwarded-host` or direct)
     - Allow if same domain or subdomain match
   - Customizable via configuration

## Critical Security Fixes to Implement

### 1. Authorization Code Race Condition

**Problem**: Code is deleted AFTER token generation, allowing reuse.

**Fix**:

```rust
// Validate code
let payload = storage.get(&code_key).await?;
// Delete code IMMEDIATELY
storage.remove(&code_key).await?;
// THEN generate tokens
let tokens = generate_tokens(payload).await?;
```

### 2. Refresh Token Atomic Rotation

**Problem**: Non-atomic operations allow multiple valid tokens.

**Fix**: Use DynamoDB Transactions

```rust
use aws_sdk_dynamodb::types::TransactWriteItem;

// Atomic transaction:
// 1. Check token exists and not used
// 2. Mark as used
// 3. Generate new token
// 4. Store new token
let transaction = vec![
    // Condition check
    TransactWriteItem::builder()
        .condition_check(...)
        .build()?,
    // Update old token
    TransactWriteItem::builder()
        .update(...)
        .build()?,
    // Put new token
    TransactWriteItem::builder()
        .put(...)
        .build()?,
];
dynamodb_client.transact_write_items()
    .transact_items(transaction)
    .send()
    .await?;
```

### 3. PKCE Constant-Time Comparison

**Problem**: String equality is timing-attack vulnerable.

**Fix**:

```rust
use subtle::ConstantTimeEq;

fn validate_pkce(verifier: &str, challenge: &str) -> bool {
    let computed = generate_challenge(verifier);
    computed.as_bytes().ct_eq(challenge.as_bytes()).into()
}
```

### 4. Client Credentials Secret Validation

**Problem**: Client secret is not validated.

**Fix**:

```rust
// Store client secrets in DynamoDB or environment
let stored_secret = get_client_secret(client_id).await?;
if !constant_time_compare(&provided_secret, &stored_secret) {
    return Err(OAuthError::InvalidClient);
}
```

## Testing Strategy

### Unit Tests

1. **Cryptography**
   - PKCE generation and validation
   - JWT signing and verification
   - Key generation

2. **Storage**
   - DynamoDB operations
   - Key encoding/decoding
   - TTL handling

3. **OAuth Flows**
   - Authorization code flow
   - Refresh token flow
   - Client credentials flow

### Integration Tests

1. **End-to-End OAuth Flow**
   - Full authorization → token exchange → token refresh

2. **Provider Integration**
   - OAuth2 provider flow
   - Password provider flow

### Security Tests

1. **Race Condition Tests**
   - Concurrent authorization code exchange
   - Concurrent refresh token rotation

2. **Timing Attack Tests**
   - PKCE comparison timing
   - Secret comparison timing

### Local Development

1. **Local DynamoDB**

   ```bash
   docker run -p 8000:8000 amazon/dynamodb-local

   # Create the table locally
   aws dynamodb create-table \
     --table-name openauth-local \
     --attribute-definitions AttributeName=pk,AttributeType=S AttributeName=sk,AttributeType=S \
     --key-schema AttributeName=pk,KeyType=HASH AttributeName=sk,KeyType=RANGE \
     --billing-mode PAY_PER_REQUEST \
     --endpoint-url http://localhost:8000
   ```

2. **Local Lambda Testing with cargo-lambda**

   ```bash
   cd rust

   # Terminal 1: Start the Lambda emulator with hot reload
   cargo lambda watch

   # Terminal 2: Test with curl
   curl http://localhost:9000/.well-known/oauth-authorization-server

   # Or invoke directly with a test event
   cargo lambda invoke --data-ascii '{"httpMethod": "GET", "path": "/"}'
   ```

3. **Environment for Local Testing**
   Create `rust/.env` file:

   ```bash
   DYNAMODB_TABLE=openauth-local
   DYNAMODB_ENDPOINT=http://localhost:8000
   AWS_ACCESS_KEY_ID=local
   AWS_SECRET_ACCESS_KEY=local
   AWS_REGION=us-east-1
   RUST_LOG=debug
   ISSUER_URL=http://localhost:9000
   ```

4. **CDK Watch (for infrastructure changes)**
   ```bash
   cd infra
   cdk watch
   ```
   This watches for infrastructure changes and redeploys automatically.

## Deployment Process

### cargo-lambda Setup (Required)

**cargo-lambda** is the recommended tool for building Rust Lambda functions. It handles cross-compilation, proper binary naming, and Lambda runtime compatibility.

1. **Install cargo-lambda**

   ```bash
   # Using Homebrew (macOS)
   brew tap cargo-lambda/cargo-lambda
   brew install cargo-lambda

   # Or using pip
   pip3 install cargo-lambda

   # Or using cargo (requires Zig for cross-compilation)
   cargo install cargo-lambda
   ```

2. **Install AWS CDK**
   ```bash
   npm install -g aws-cdk
   ```

### Build & Package

1. **Build with cargo-lambda**

   ```bash
   cd rust

   # Build for ARM64 (Graviton2 - recommended for cost/performance)
   cargo lambda build --release --arm64

   # Or for x86_64
   cargo lambda build --release --x86-64
   ```

   **Output location**: `target/lambda/{package-name}/bootstrap`

2. **Local Testing with cargo-lambda**

   ```bash
   # Start local Lambda emulator
   cargo lambda watch

   # In another terminal, invoke the function
   cargo lambda invoke --data-file event.json
   ```

### CDK Deployment

The `cargo-lambda-cdk` construct handles building automatically during `cdk deploy`.

1. **Bootstrap CDK (first time only)**

   ```bash
   cd infra
   npm install
   cdk bootstrap
   ```

2. **Deploy**

   ```bash
   cd infra
   cdk deploy
   ```

3. **View Outputs**
   After deployment, CDK outputs:

   ```
   Outputs:
   OpenAuthStack.ApiUrl = https://abc123.execute-api.us-east-1.amazonaws.com/
   OpenAuthStack.TableName = OpenAuthStack-AuthTable12345-ABCDEF
   ```

4. **Destroy (if needed)**
   ```bash
   cdk destroy
   ```

### CDK Configuration Files

**`infra/cdk.json`**

```json
{
  "app": "npx ts-node --prefer-ts-exts bin/openauth.ts",
  "watch": {
    "include": ["**"],
    "exclude": [
      "README.md",
      "cdk*.json",
      "**/*.d.ts",
      "**/*.js",
      "tsconfig.json",
      "node_modules",
      "../rust/target"
    ]
  },
  "context": {
    "@aws-cdk/aws-apigateway:usagePlanKeyOrderInsensitiveId": true,
    "@aws-cdk/aws-lambda:recognizeLayerVersion": true
  }
}
```

**`infra/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "declaration": true,
    "strict": true,
    "noImplicitAny": true,
    "strictNullChecks": true,
    "noImplicitThis": true,
    "alwaysStrict": true,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "noImplicitReturns": true,
    "noFallthroughCasesInSwitch": false,
    "inlineSourceMap": true,
    "inlineSources": true,
    "experimentalDecorators": true,
    "strictPropertyInitialization": false,
    "outDir": "./dist",
    "rootDir": "."
  },
  "exclude": ["node_modules", "cdk.out"]
}
```

### Environment Variables

| Variable            | Required    | Description                                                |
| ------------------- | ----------- | ---------------------------------------------------------- |
| `DYNAMODB_TABLE`    | Yes         | DynamoDB table name (set by CDK)                           |
| `AWS_REGION`        | Auto        | Set by Lambda runtime                                      |
| `ISSUER_URL`        | Recommended | Base URL for JWT `iss` claim (defaults to API Gateway URL) |
| `RUST_LOG`          | No          | Log level (debug, info, warn, error)                       |
| `DYNAMODB_ENDPOINT` | No          | Override endpoint (for local testing)                      |

These are automatically configured by the CDK stack via the `RustFunction` environment property.

### CI/CD Pipeline (GitHub Actions Example)

```yaml
name: Deploy
on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install cargo-lambda
        uses: jaxxstorm/action-install-gh-release@v1
        with:
          repo: cargo-lambda/cargo-lambda
          tag: v1.4.0
          platform: linux
          arch: x86_64

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "20"

      - name: Install CDK dependencies
        run: |
          cd infra
          npm ci

      - name: Deploy with CDK
        run: |
          cd infra
          npx cdk deploy --require-approval never
        env:
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          AWS_DEFAULT_REGION: us-east-1
```

### Multi-Environment Deployment

For staging/production environments, modify the CDK app:

```typescript
// bin/openauth.ts
const app = new cdk.App()

// Staging
new OpenAuthStack(app, "OpenAuthStaging", {
  env: { account: "123456789", region: "us-east-1" },
})

// Production
new OpenAuthStack(app, "OpenAuthProd", {
  env: { account: "123456789", region: "us-east-1" },
})
```

Deploy specific stack:

```bash
cdk deploy OpenAuthStaging
cdk deploy OpenAuthProd
```

## File Structure Reference

### TypeScript Reference Files (from original OpenAuth repo)

These files in the original TypeScript implementation should be used as reference:

- `packages/openauth/src/issuer.ts` - Main issuer implementation
- `packages/openauth/src/storage/dynamo.ts` - DynamoDB storage adapter
- `packages/openauth/src/jwt.ts` - JWT operations
- `packages/openauth/src/pkce.ts` - PKCE implementation
- `packages/openauth/src/keys.ts` - Key management
- `packages/openauth/src/provider/` - Provider implementations
- `packages/openauth/src/error.ts` - Error types
- `packages/openauth/src/random.ts` - Secure random generation

## Success Criteria

1. ✅ All OAuth 2.0 endpoints functional
2. ✅ All security vulnerabilities fixed
3. ✅ DynamoDB storage working with TTL
4. ✅ JWT signing and verification working
5. ✅ PKCE flow working with constant-time comparison
6. ✅ Refresh token rotation atomic
7. ✅ Client credentials grant validates secrets
8. ✅ Authorization codes single-use
9. ✅ Rate limiting implemented
10. ✅ Comprehensive test coverage

## Notes for AI Agent

### Critical Security Requirements

- **Always use constant-time comparisons** for secrets, PKCE challenges, OTP codes, passwords
- **Use DynamoDB Transactions** for any operation that must be atomic
- **Delete authorization codes immediately** after validation, before token generation
- **Validate client secrets** in client credentials grant
- **Enable DynamoDB TTL** on the `expiry` attribute
- **Use SHA-256** for subject hashing, don't truncate
- **Implement rate limiting** on all endpoints
- **Validate state parameter** in authorization callback
- **Set secure cookie flags** (HttpOnly, Secure, SameSite)

### Code Generation

- **Unbiased random digit generation** for OTP codes:
  ```rust
  fn generate_unbiased_digits(length: usize) -> String {
      let mut result = Vec::with_capacity(length);
      let mut rng = rand::thread_rng();
      while result.len() < length {
          let byte: u8 = rng.gen();
          if byte < 250 {  // Avoid modulo bias
              result.push((byte % 10) as char + '0');
          }
      }
      result.into_iter().collect()
  }
  ```

### Lambda Optimization

- Use `once_cell::sync::Lazy` or `std::sync::OnceLock` for DynamoDB client initialization
- Cache JWKS locally with TTL
- cargo-lambda automatically optimizes for Lambda, but you can further minimize cold start:
  ```toml
  # Cargo.toml
  [profile.release]
  strip = true
  lto = true
  codegen-units = 1
  opt-level = "z"  # Optimize for size
  ```

### Compatibility

- Maintain same DynamoDB key structure as TypeScript version
- Use same JWT claims structure
- Use same cookie names and encryption format
- Ensure backward compatibility with existing refresh tokens

## Lambda Handler Example

```rust
// src/main.rs
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use std::sync::OnceLock;

// Global DynamoDB client (initialized once)
static DYNAMO_CLIENT: OnceLock<aws_sdk_dynamodb::Client> = OnceLock::new();

async fn get_dynamo_client() -> &'static aws_sdk_dynamodb::Client {
    DYNAMO_CLIENT.get_or_init(|| {
        let config = tokio::runtime::Handle::current()
            .block_on(aws_config::load_defaults(aws_config::BehaviorVersion::latest()));
        aws_sdk_dynamodb::Client::new(&config)
    })
}

async fn function_handler(event: Request) -> Result<Response<Body>, Error> {
    // Your routing logic here (using axum or manual routing)
    let path = event.uri().path();

    match path {
        "/.well-known/oauth-authorization-server" => handle_well_known(event).await,
        "/.well-known/jwks.json" => handle_jwks(event).await,
        "/authorize" => handle_authorize(event).await,
        "/token" => handle_token(event).await,
        "/userinfo" => handle_userinfo(event).await,
        _ => handle_provider_routes(event).await,
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    run(service_fn(function_handler)).await
}
```

## Client Library Considerations

The existing TypeScript client (`@openauthjs/openauth/client`) can be used as-is with the Rust issuer since they communicate via standard OAuth 2.0 HTTP endpoints. No Rust client is required.

However, if a Rust client is needed (e.g., for Rust backend services):

```rust
pub struct OpenAuthClient {
    issuer: String,
    client_id: String,
    http_client: reqwest::Client,
}

impl OpenAuthClient {
    pub async fn authorize(&self, redirect_uri: &str, response_type: &str) -> AuthorizeResult;
    pub async fn exchange(&self, code: &str, redirect_uri: &str, verifier: Option<&str>) -> Result<Tokens>;
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens>;
    pub async fn verify(&self, token: &str) -> Result<Claims>;
}
```

## JWT Payload Structure

Access tokens contain:

```json
{
  "mode": "access",
  "type": "user",           // Subject type
  "properties": { ... },    // Subject properties
  "aud": "client-id",       // Audience (client_id)
  "iss": "https://auth...", // Issuer URL
  "sub": "user:abc123...",  // Subject identifier
  "exp": 1234567890         // Expiration timestamp
}
```

## Additional Resources

- [OAuth 2.0 RFC 6749](https://tools.ietf.org/html/rfc6749)
- [PKCE RFC 7636](https://tools.ietf.org/html/rfc7636)
- [JWT RFC 7519](https://tools.ietf.org/html/rfc7519)
- [JWE RFC 7516](https://tools.ietf.org/html/rfc7516) - For cookie encryption
- [AWS Lambda Rust Runtime](https://github.com/awslabs/aws-lambda-rust-runtime)
- [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust)
- [cargo-lambda](https://www.cargo-lambda.info/) - Build tool for Rust Lambda
- [cargo-lambda-cdk](https://github.com/cargo-lambda/cargo-lambda-cdk) - CDK construct for Rust
- [AWS CDK Documentation](https://docs.aws.amazon.com/cdk/v2/guide/home.html)
- [DynamoDB Transactions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transactions.html)
- [Subtle crate](https://docs.rs/subtle) - Constant-time operations
