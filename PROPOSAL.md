# OpenAuth Rust Implementation Proposal

## Overview

This document provides a comprehensive guide for implementing OpenAuth in Rust with AWS CDK infrastructure-as-code in TypeScript. The goal is to create a production-ready, **security-first** OAuth 2.0 authorization server that runs on AWS Lambda with all application logic written in Rust.

**Security Philosophy**: This implementation follows the principle of "secure by default". All permissive behaviors must be explicitly enabled. The default configuration denies all unauthenticated/unregistered access.

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│               AWS CDK (TypeScript)                          │
│  - Infrastructure definition (lib/openauth-stack.ts)        │
│  - DynamoDB table provisioning                              │
│  - Lambda function via cargo-lambda-cdk                     │
│  - API Gateway setup (with header sanitization)             │
│  - IAM roles and permissions                                │
└─────────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│              AWS Lambda (Rust Runtime)                      │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  OpenAuth Issuer (Rust)                              │  │
│  │  - HTTP handler (lambda_http)                        │  │
│  │  - OAuth 2.0 endpoints                               │  │
│  │  - JWT signing/verification                          │  │
│  │  - PKCE validation (REQUIRED by default)             │  │
│  │  - Provider integrations                             │  │
│  │  - Client Registry validation                        │  │
│  │  - Management API (authenticated)                    │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    DynamoDB                                 │
│  - Client registry (client:config)        [NEW]             │
│  - Admin credentials (admin:key)          [NEW]             │
│  - Signing keys (signing:key)                               │
│  - Encryption keys (encryption:key)                         │
│  - Refresh tokens (oauth:refresh)                           │
│  - Authorization codes (oauth:code)                         │
│  - Password hashes (oauth:password)                         │
│  - Rate limit counters (ratelimit:*)      [NEW]             │
└─────────────────────────────────────────────────────────────┘
```

## Security-First Defaults

### What's Different from the Original TypeScript Implementation

| Feature | Original (TS) | This Implementation (Rust) |
|---------|---------------|---------------------------|
| Client Registration | None (any client_id accepted) | **Mandatory** - clients must be pre-registered |
| Redirect URI Validation | Domain matching + localhost auto-allow | **Explicit allowlist only** |
| Client Secrets | Optional, provider-specific | **Required for confidential clients** |
| PKCE | Optional | **Required by default** (can be disabled per-client) |
| Localhost Access | Always allowed | **Denied by default** (dev mode opt-in) |
| x-forwarded-host Trust | Always trusted | **Explicit trusted proxy configuration** |
| Rate Limiting | Not implemented | **Enabled by default** |
| Management API | None | **API key authenticated** |

### Security Threat Model

```
┌─────────────────────────────────────────────────────────────┐
│                    THREAT VECTORS                           │
├─────────────────────────────────────────────────────────────┤
│ 1. Unauthorized Client Access                               │
│    Attack: Malicious app uses your auth server              │
│    Defense: Mandatory client registry + secret validation   │
│                                                             │
│ 2. Redirect URI Hijacking                                   │
│    Attack: Tokens redirected to attacker's server           │
│    Defense: Explicit URI allowlist (no pattern matching)    │
│                                                             │
│ 3. Token Interception (MITM)                                │
│    Attack: Attacker intercepts authorization code           │
│    Defense: PKCE required, codes single-use + short-lived   │
│                                                             │
│ 4. Subdomain Takeover                                       │
│    Attack: Attacker controls evil.yourdomain.com            │
│    Defense: No domain matching - explicit URIs only         │
│                                                             │
│ 5. Header Spoofing                                          │
│    Attack: Forge x-forwarded-host to bypass checks          │
│    Defense: Trusted proxy allowlist, header validation      │
│                                                             │
│ 6. Brute Force / DoS                                        │
│    Attack: Flood endpoints to guess codes/exhaust resources │
│    Defense: Rate limiting on all endpoints                  │
│                                                             │
│ 7. Refresh Token Theft                                      │
│    Attack: Stolen token used indefinitely                   │
│    Defense: Rotation + reuse detection + invalidation       │
└─────────────────────────────────────────────────────────────┘
```

---

## Client Registry (NEW - MANDATORY)

### Overview

Every OAuth client MUST be registered before it can use the authorization server. There are no anonymous or auto-approved clients.

### Client Types

1. **Confidential Clients** (backend apps, server-side)
   - MUST have a `client_secret`
   - Secret validated on token exchange
   - Can use all grant types

2. **Public Clients** (SPAs, mobile apps, CLIs)
   - No `client_secret` (cannot be kept secret)
   - MUST use PKCE
   - Cannot use `client_credentials` grant

### DynamoDB Schema

```
Client Registry:
┌─────────────────────────────────────────────────────────────┐
│ pk: "client:{client_id}"                                    │
│ sk: "config"                                                │
├─────────────────────────────────────────────────────────────┤
│ {                                                           │
│   "client_id": "my-frontend-app",                           │
│   "client_type": "public" | "confidential",                 │
│   "client_secret_hash": "argon2:...",  // null for public   │
│   "redirect_uris": [                                        │
│     "https://app.example.com/callback",                     │
│     "https://app.example.com/auth/callback"                 │
│   ],                                                        │
│   "allowed_grant_types": [                                  │
│     "authorization_code",                                   │
│     "refresh_token"                                         │
│   ],                                                        │
│   "allowed_scopes": ["openid", "profile", "email"],         │
│   "pkce_required": true,           // default: true         │
│   "token_endpoint_auth_method": "none" | "client_secret_post" | "client_secret_basic", │
│   "access_token_ttl": 86400,       // override default      │
│   "refresh_token_ttl": 31536000,   // override default      │
│   "created_at": "2024-01-01T00:00:00Z",                     │
│   "updated_at": "2024-01-01T00:00:00Z",                     │
│   "enabled": true                                           │
│ }                                                           │
└─────────────────────────────────────────────────────────────┘
```

### Client Validation Flow

```rust
/// Validates client on every /authorize and /token request
pub async fn validate_client(
    storage: &dyn StorageAdapter,
    client_id: &str,
    redirect_uri: Option<&str>,
    client_secret: Option<&str>,
    grant_type: Option<&str>,
) -> Result<Client, OAuthError> {
    // 1. Fetch client from registry
    let client = storage
        .get(&["client", client_id, "config"])
        .await?
        .ok_or(OAuthError::InvalidClient("Client not registered"))?;
    
    // 2. Check if client is enabled
    if !client.enabled {
        return Err(OAuthError::InvalidClient("Client is disabled"));
    }
    
    // 3. Validate redirect_uri against allowlist (EXACT MATCH)
    if let Some(uri) = redirect_uri {
        if !client.redirect_uris.contains(&uri.to_string()) {
            return Err(OAuthError::InvalidRedirectUri(
                "Redirect URI not in allowlist"
            ));
        }
    }
    
    // 4. Validate client_secret for confidential clients
    if client.client_type == ClientType::Confidential {
        let secret = client_secret
            .ok_or(OAuthError::InvalidClient("Client secret required"))?;
        
        let hash = client.client_secret_hash
            .as_ref()
            .ok_or(OAuthError::ServerError("Client misconfigured"))?;
        
        if !verify_secret_constant_time(secret, hash) {
            return Err(OAuthError::InvalidClient("Invalid client secret"));
        }
    }
    
    // 5. Validate grant_type is allowed
    if let Some(grant) = grant_type {
        if !client.allowed_grant_types.contains(&grant.to_string()) {
            return Err(OAuthError::UnauthorizedClient(
                "Grant type not allowed for this client"
            ));
        }
    }
    
    Ok(client)
}
```

### Client Secret Handling

```rust
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use subtle::ConstantTimeEq;

/// Hash client secret for storage (during registration)
pub fn hash_client_secret(secret: &str) -> Result<String, Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(secret.as_bytes(), &salt)?
        .to_string();
    Ok(hash)
}

/// Verify client secret (constant-time)
pub fn verify_secret_constant_time(provided: &str, stored_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    
    Argon2::default()
        .verify_password(provided.as_bytes(), &parsed_hash)
        .is_ok()
}
```

---

## Management API (NEW - AUTHENTICATED)

### Overview

Administrative endpoints for managing clients, viewing metrics, and revoking tokens. All management endpoints require authentication via API key.

### Authentication

```rust
/// API key stored in DynamoDB
/// pk: "admin:key"
/// sk: "{key_id}"
/// value: { "key_hash": "sha256:...", "name": "production", "created_at": "...", "permissions": [...] }

pub async fn authenticate_admin(
    req: &Request,
    storage: &dyn StorageAdapter,
) -> Result<AdminKey, AuthError> {
    // Extract API key from header
    let api_key = req
        .headers()
        .get("X-Admin-API-Key")
        .or_else(|| req.headers().get("Authorization"))
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingApiKey)?;
    
    // Parse key format: {key_id}:{secret}
    let (key_id, secret) = api_key
        .split_once(':')
        .ok_or(AuthError::InvalidKeyFormat)?;
    
    // Fetch key from storage
    let key = storage
        .get(&["admin:key", key_id])
        .await?
        .ok_or(AuthError::InvalidApiKey)?;
    
    // Verify secret (constant-time)
    let expected_hash = sha256(secret);
    if !constant_time_eq(expected_hash.as_bytes(), key.key_hash.as_bytes()) {
        return Err(AuthError::InvalidApiKey);
    }
    
    Ok(key)
}
```

### Management Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/admin/clients` | GET | List all registered clients |
| `/admin/clients` | POST | Register a new client |
| `/admin/clients/{id}` | GET | Get client details |
| `/admin/clients/{id}` | PUT | Update client configuration |
| `/admin/clients/{id}` | DELETE | Delete/disable client |
| `/admin/clients/{id}/rotate-secret` | POST | Rotate client secret |
| `/admin/tokens/revoke` | POST | Revoke tokens for a subject |
| `/admin/keys/rotate` | POST | Force key rotation |
| `/admin/metrics` | GET | Get auth metrics |

### Client Registration Request

```rust
#[derive(Deserialize, Validate)]
pub struct CreateClientRequest {
    #[validate(length(min = 3, max = 64), regex = "^[a-z0-9-]+$")]
    pub client_id: String,
    
    pub client_type: ClientType,
    
    #[validate(length(min = 1))]
    pub redirect_uris: Vec<String>,
    
    #[validate(length(min = 1))]
    pub allowed_grant_types: Vec<GrantType>,
    
    pub allowed_scopes: Option<Vec<String>>,
    
    pub pkce_required: Option<bool>,  // default: true
    
    pub access_token_ttl: Option<u64>,
    pub refresh_token_ttl: Option<u64>,
}

// Response includes generated client_secret for confidential clients
#[derive(Serialize)]
pub struct CreateClientResponse {
    pub client_id: String,
    pub client_secret: Option<String>,  // Only returned once!
    pub client_type: ClientType,
    pub created_at: DateTime<Utc>,
}
```

### Bootstrap Admin Key

On first deployment, generate an initial admin API key:

```rust
/// Called during CDK deployment or first Lambda invocation
pub async fn bootstrap_admin_key(storage: &dyn StorageAdapter) -> Result<String, Error> {
    // Check if any admin key exists
    let existing = storage.scan(&["admin:key"]).await?;
    if !existing.is_empty() {
        return Err(Error::AlreadyBootstrapped);
    }
    
    // Generate new admin key
    let key_id = generate_random_string(16);
    let secret = generate_random_string(32);
    let key_hash = sha256(&secret);
    
    storage.set(
        &["admin:key", &key_id],
        json!({
            "key_hash": key_hash,
            "name": "bootstrap",
            "created_at": Utc::now(),
            "permissions": ["*"]  // Full access
        }),
        None,  // No expiry
    ).await?;
    
    // Return full key (only shown once)
    Ok(format!("{}:{}", key_id, secret))
}
```

---

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
        // Security: Explicitly configure trusted proxies
        TRUSTED_PROXIES: "api-gateway",  // Only trust API Gateway headers
        // Security: Dev mode disabled by default
        DEV_MODE: "false",
      },
    })

    // Grant DynamoDB permissions
    table.grantReadWriteData(authFunction)

    // API Gateway with header sanitization
    const api = new apigateway.HttpApi(this, "AuthApi", {
      apiName: "OpenAuthApi",
      // CORS configured per-route, not globally
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

---

## What Needs to be Rewritten in Rust

### Core Modules to Implement

1. **HTTP Server & Lambda Handler**
   - Lambda runtime integration
   - HTTP request/response handling
   - Route definitions (authorize, token, userinfo, well-known endpoints)
   - **Management API routes (authenticated)**

2. **Client Registry (NEW)**
   - Client CRUD operations
   - Client validation on every request
   - Secret hashing and verification
   - Redirect URI exact-match validation

3. **Storage Adapter (DynamoDB)**
   - DynamoDB client integration
   - Key encoding/decoding
   - TTL handling
   - Atomic operations (for refresh token rotation)

4. **JWT Operations**
   - JWT signing (ES256)
   - JWT verification
   - JWKS endpoint implementation
   - Key generation and rotation

5. **OAuth 2.0 Core**
   - Authorization code generation and validation
   - Token exchange endpoint
   - Refresh token rotation (with atomic operations)
   - PKCE validation (with constant-time comparison)
   - Client credentials grant (with secret validation)

6. **Cryptography**
   - Key pair generation (ES256 for signing, RSA-OAEP-512 for encryption)
   - Cookie encryption/decryption
   - PKCE challenge generation and validation
   - Password hashing (Argon2)
   - **Client secret hashing (Argon2)**

7. **Provider System**
   - Provider trait/interface
   - OAuth2 provider implementation
   - OIDC provider implementation
   - Password provider
   - Code provider

8. **Subject Validation**
   - Schema validation (replace valibot with Rust equivalent)
   - Subject resolution and hashing

9. **Rate Limiting (NEW)**
   - Per-endpoint rate limits
   - Per-client rate limits
   - DynamoDB-based counter storage

10. **UI Components** (Optional - can be simplified)
    - HTML form generation
    - Provider selection UI
    - Password/Code input forms

---

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

# Password & Secret Hashing
argon2 = "0.5"  # For passwords AND client secrets
rand = "0.8"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Validation
validator = { version = "0.18", features = ["derive"] }

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
# Development mode: enables localhost redirect URIs (NEVER use in production)
dev-mode = []
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

---

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
   │   │   ├── client/           # NEW: Client registry
   │   │   │   ├── mod.rs
   │   │   │   ├── registry.rs   # Client CRUD
   │   │   │   ├── validation.rs # Client validation
   │   │   │   └── types.rs      # Client types
   │   │   ├── admin/            # NEW: Management API
   │   │   │   ├── mod.rs
   │   │   │   ├── auth.rs       # API key authentication
   │   │   │   ├── clients.rs    # Client management endpoints
   │   │   │   └── tokens.rs     # Token revocation
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
   │   │   │   ├── secrets.rs    # NEW: Client secret hashing
   │   │   │   └── random.rs     # Secure random generation
   │   │   ├── ratelimit/        # NEW: Rate limiting
   │   │   │   ├── mod.rs
   │   │   │   └── middleware.rs
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

2. **Set up CDK Configuration**
   - Define DynamoDB table with TTL
   - Configure Lambda function with custom runtime
   - Set up API Gateway
   - Configure IAM permissions

3. **Build System Setup**
   - Create build script for Rust binary
   - Configure Lambda deployment package
   - Set up CI/CD pipeline

### Phase 2: Client Registry & Management API

**This phase is NEW and must be completed before OAuth endpoints.**

1. **Client Registry Implementation**
   ```rust
   // src/client/types.rs
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Client {
       pub client_id: String,
       pub client_type: ClientType,
       pub client_secret_hash: Option<String>,
       pub redirect_uris: Vec<String>,
       pub allowed_grant_types: Vec<GrantType>,
       pub allowed_scopes: Vec<String>,
       pub pkce_required: bool,
       pub token_endpoint_auth_method: TokenEndpointAuthMethod,
       pub access_token_ttl: Option<u64>,
       pub refresh_token_ttl: Option<u64>,
       pub created_at: DateTime<Utc>,
       pub updated_at: DateTime<Utc>,
       pub enabled: bool,
   }

   #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
   #[serde(rename_all = "snake_case")]
   pub enum ClientType {
       Public,
       Confidential,
   }

   #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
   #[serde(rename_all = "snake_case")]
   pub enum GrantType {
       AuthorizationCode,
       RefreshToken,
       ClientCredentials,
   }

   #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
   #[serde(rename_all = "snake_case")]
   pub enum TokenEndpointAuthMethod {
       None,                  // Public clients
       ClientSecretPost,      // Secret in body
       ClientSecretBasic,     // Secret in Authorization header
   }
   ```

2. **Client Validation Middleware**
   ```rust
   // src/client/validation.rs
   
   /// Called on every /authorize request
   pub async fn validate_authorize_request(
       storage: &dyn StorageAdapter,
       client_id: &str,
       redirect_uri: &str,
       response_type: &str,
       code_challenge: Option<&str>,
   ) -> Result<Client, OAuthError> {
       let client = get_client(storage, client_id).await?;
       
       // Validate redirect URI (EXACT MATCH - no pattern matching!)
       if !client.redirect_uris.iter().any(|uri| uri == redirect_uri) {
           return Err(OAuthError::InvalidRedirectUri(format!(
               "Redirect URI '{}' not registered for client '{}'",
               redirect_uri, client_id
           )));
       }
       
       // Validate PKCE requirement
       if client.pkce_required && code_challenge.is_none() {
           return Err(OAuthError::InvalidRequest(
               "PKCE is required for this client"
           ));
       }
       
       // Validate response_type implies grant_type
       let required_grant = match response_type {
           "code" => GrantType::AuthorizationCode,
           "token" => GrantType::AuthorizationCode, // Implicit uses same
           _ => return Err(OAuthError::UnsupportedResponseType),
       };
       
       if !client.allowed_grant_types.contains(&required_grant) {
           return Err(OAuthError::UnauthorizedClient(
               "Response type not allowed for this client"
           ));
       }
       
       Ok(client)
   }
   
   /// Called on every /token request
   pub async fn validate_token_request(
       storage: &dyn StorageAdapter,
       client_id: &str,
       client_secret: Option<&str>,
       grant_type: &str,
       auth_header: Option<&str>,
   ) -> Result<Client, OAuthError> {
       let client = get_client(storage, client_id).await?;
       
       // Parse grant_type
       let grant = match grant_type {
           "authorization_code" => GrantType::AuthorizationCode,
           "refresh_token" => GrantType::RefreshToken,
           "client_credentials" => GrantType::ClientCredentials,
           _ => return Err(OAuthError::UnsupportedGrantType),
       };
       
       // Validate grant type is allowed
       if !client.allowed_grant_types.contains(&grant) {
           return Err(OAuthError::UnauthorizedClient(
               "Grant type not allowed for this client"
           ));
       }
       
       // Validate client authentication based on type
       match client.client_type {
           ClientType::Confidential => {
               validate_client_authentication(
                   &client,
                   client_secret,
                   auth_header,
               )?;
           }
           ClientType::Public => {
               // Public clients cannot use client_credentials
               if grant == GrantType::ClientCredentials {
                   return Err(OAuthError::UnauthorizedClient(
                       "Public clients cannot use client_credentials grant"
                   ));
               }
           }
       }
       
       Ok(client)
   }
   
   fn validate_client_authentication(
       client: &Client,
       client_secret: Option<&str>,
       auth_header: Option<&str>,
   ) -> Result<(), OAuthError> {
       let secret = match client.token_endpoint_auth_method {
           TokenEndpointAuthMethod::None => {
               return Err(OAuthError::InvalidClient(
                   "Confidential client must have auth method"
               ));
           }
           TokenEndpointAuthMethod::ClientSecretPost => {
               client_secret.ok_or(OAuthError::InvalidClient(
                   "client_secret required in request body"
               ))?
           }
           TokenEndpointAuthMethod::ClientSecretBasic => {
               parse_basic_auth(auth_header)?
           }
       };
       
       let hash = client.client_secret_hash.as_ref()
           .ok_or(OAuthError::ServerError("Client misconfigured"))?;
       
       if !verify_secret_constant_time(secret, hash) {
           // Use constant-time comparison to prevent timing attacks
           return Err(OAuthError::InvalidClient("Invalid client secret"));
       }
       
       Ok(())
   }
   ```

3. **Management API Authentication**
   ```rust
   // src/admin/auth.rs
   use subtle::ConstantTimeEq;
   
   pub async fn authenticate_admin_request(
       req: &Request,
       storage: &dyn StorageAdapter,
   ) -> Result<AdminContext, AuthError> {
       // Get API key from header
       let api_key = extract_api_key(req)?;
       
       // Parse key format: {key_id}:{secret}
       let (key_id, provided_secret) = api_key
           .split_once(':')
           .ok_or(AuthError::InvalidKeyFormat)?;
       
       // Fetch key from storage
       let key_data = storage
           .get(&["admin:key", key_id])
           .await?
           .ok_or(AuthError::InvalidApiKey)?;
       
       // Verify secret using constant-time comparison
       let expected_hash = &key_data.key_hash;
       let provided_hash = sha256(provided_secret);
       
       if !bool::from(expected_hash.as_bytes().ct_eq(provided_hash.as_bytes())) {
           return Err(AuthError::InvalidApiKey);
       }
       
       Ok(AdminContext {
           key_id: key_id.to_string(),
           permissions: key_data.permissions,
       })
   }
   
   fn extract_api_key(req: &Request) -> Result<&str, AuthError> {
       // Try X-Admin-API-Key header first
       if let Some(key) = req.headers().get("X-Admin-API-Key") {
           return key.to_str().map_err(|_| AuthError::InvalidKeyFormat);
       }
       
       // Fall back to Authorization: Bearer
       if let Some(auth) = req.headers().get("Authorization") {
           let auth_str = auth.to_str().map_err(|_| AuthError::InvalidKeyFormat)?;
           if let Some(key) = auth_str.strip_prefix("Bearer ") {
               return Ok(key);
           }
       }
       
       Err(AuthError::MissingApiKey)
   }
   ```

4. **Management Endpoints**
   ```rust
   // src/admin/clients.rs
   
   /// POST /admin/clients - Register a new client
   pub async fn create_client(
       storage: &dyn StorageAdapter,
       req: CreateClientRequest,
   ) -> Result<CreateClientResponse, Error> {
       // Validate request
       req.validate()?;
       
       // Check client doesn't already exist
       if storage.get(&["client", &req.client_id, "config"]).await?.is_some() {
           return Err(Error::ClientAlreadyExists);
       }
       
       // Generate client secret for confidential clients
       let (secret, secret_hash) = if req.client_type == ClientType::Confidential {
           let secret = generate_random_string(32);
           let hash = hash_client_secret(&secret)?;
           (Some(secret), Some(hash))
       } else {
           (None, None)
       };
       
       // Create client record
       let now = Utc::now();
       let client = Client {
           client_id: req.client_id.clone(),
           client_type: req.client_type,
           client_secret_hash: secret_hash,
           redirect_uris: req.redirect_uris,
           allowed_grant_types: req.allowed_grant_types,
           allowed_scopes: req.allowed_scopes.unwrap_or_default(),
           pkce_required: req.pkce_required.unwrap_or(true), // Default: PKCE required
           token_endpoint_auth_method: if req.client_type == ClientType::Confidential {
               TokenEndpointAuthMethod::ClientSecretPost
           } else {
               TokenEndpointAuthMethod::None
           },
           access_token_ttl: req.access_token_ttl,
           refresh_token_ttl: req.refresh_token_ttl,
           created_at: now,
           updated_at: now,
           enabled: true,
       };
       
       // Store client
       storage.set(
           &["client", &client.client_id, "config"],
           serde_json::to_value(&client)?,
           None, // No expiry
       ).await?;
       
       Ok(CreateClientResponse {
           client_id: client.client_id,
           client_secret: secret, // Only returned once!
           client_type: client.client_type,
           created_at: client.created_at,
       })
   }
   ```

### Phase 3: Core Infrastructure

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
     - **Client registry: `["client", "{client_id}", "config"]`** (NEW)
     - **Admin keys: `["admin:key", "{key_id}"]`** (NEW)
     - Signing keys: `["signing:key", "{uuid}"]`
     - Encryption keys: `["encryption:key", "{uuid}"]`
     - Auth codes: `["oauth:code", "{code}"]`
     - Refresh tokens: `["oauth:refresh", "{subject}", "{token}"]`
     - Password hashes: `["oauth:password", "{email}"]`
     - **Rate limit counters: `["ratelimit", "{endpoint}", "{identifier}"]`** (NEW)
   - For keys with 2 parts: `pk = key[0]`, `sk = key[1]`
   - For keys with >2 parts: `pk = join(key[0..2])`, `sk = join(key[2..])`
   - TTL handling (both manual check and DynamoDB-native)
   - **CRITICAL**: Implement atomic operations using DynamoDB Transactions for refresh token rotation

2. **Trusted Proxy Configuration (NEW)**
   ```rust
   // src/config.rs
   
   #[derive(Debug, Clone)]
   pub struct ProxyConfig {
       /// Which proxies to trust for X-Forwarded-* headers
       pub trusted_proxies: TrustedProxies,
   }
   
   #[derive(Debug, Clone)]
   pub enum TrustedProxies {
       /// Don't trust any proxy headers (safest)
       None,
       /// Trust API Gateway headers only
       ApiGateway,
       /// Trust specific IP ranges (CIDR notation)
       IpRanges(Vec<IpNet>),
   }
   
   impl ProxyConfig {
       pub fn from_env() -> Self {
           let trusted = std::env::var("TRUSTED_PROXIES")
               .unwrap_or_else(|_| "none".to_string());
           
           Self {
               trusted_proxies: match trusted.as_str() {
                   "none" => TrustedProxies::None,
                   "api-gateway" => TrustedProxies::ApiGateway,
                   ranges => TrustedProxies::IpRanges(
                       ranges.split(',')
                           .filter_map(|r| r.parse().ok())
                           .collect()
                   ),
               },
           }
       }
   }
   
   /// Get the real client info, only trusting configured proxies
   pub fn get_client_info(req: &Request, config: &ProxyConfig) -> ClientInfo {
       match &config.trusted_proxies {
           TrustedProxies::None => {
               // Don't trust any headers, use direct connection info
               ClientInfo {
                   host: req.uri().host().map(String::from),
                   protocol: req.uri().scheme_str().map(String::from),
                   ip: None, // Would need actual connection info
               }
           }
           TrustedProxies::ApiGateway => {
               // API Gateway sets these reliably
               ClientInfo {
                   host: req.headers()
                       .get("x-forwarded-host")
                       .or_else(|| req.headers().get("host"))
                       .and_then(|v| v.to_str().ok())
                       .map(String::from),
                   protocol: req.headers()
                       .get("x-forwarded-proto")
                       .and_then(|v| v.to_str().ok())
                       .map(String::from),
                   ip: req.headers()
                       .get("x-forwarded-for")
                       .and_then(|v| v.to_str().ok())
                       .and_then(|v| v.split(',').next())
                       .map(|s| s.trim().to_string()),
               }
           }
           TrustedProxies::IpRanges(_ranges) => {
               // Only trust headers if request came from trusted IP
               // This requires actual connection IP which Lambda provides
               // in the request context
               todo!("Implement IP range checking")
           }
       }
   }
   ```

3. **CORS Configuration**
   - Well-known endpoints: `origin: *`, methods: `GET`
   - Token endpoint: `origin: *`, methods: `POST`
   - **Admin endpoints: No CORS (server-to-server only)** (NEW)
   - No credentials for CORS endpoints

### Phase 4: Rate Limiting (NEW)

1. **Rate Limit Configuration**
   ```rust
   // src/ratelimit/mod.rs
   
   #[derive(Debug, Clone)]
   pub struct RateLimitConfig {
       pub enabled: bool,
       pub limits: HashMap<Endpoint, RateLimit>,
   }
   
   #[derive(Debug, Clone)]
   pub struct RateLimit {
       pub requests: u32,
       pub window_seconds: u64,
   }
   
   impl Default for RateLimitConfig {
       fn default() -> Self {
           let mut limits = HashMap::new();
           
           // Conservative defaults
           limits.insert(Endpoint::Authorize, RateLimit {
               requests: 100,
               window_seconds: 60,
           });
           limits.insert(Endpoint::Token, RateLimit {
               requests: 50,
               window_seconds: 60,
           });
           limits.insert(Endpoint::PasswordLogin, RateLimit {
               requests: 5,       // Very aggressive for password endpoints
               window_seconds: 60,
           });
           limits.insert(Endpoint::CodeVerify, RateLimit {
               requests: 5,
               window_seconds: 60,
           });
           limits.insert(Endpoint::AdminApi, RateLimit {
               requests: 100,
               window_seconds: 60,
           });
           
           Self {
               enabled: true,
               limits,
           }
       }
   }
   ```

2. **Rate Limit Middleware**
   ```rust
   // src/ratelimit/middleware.rs
   
   pub async fn check_rate_limit(
       storage: &dyn StorageAdapter,
       config: &RateLimitConfig,
       endpoint: Endpoint,
       identifier: &str,  // IP address or client_id
   ) -> Result<(), RateLimitError> {
       if !config.enabled {
           return Ok(());
       }
       
       let limit = config.limits.get(&endpoint)
           .ok_or(RateLimitError::NotConfigured)?;
       
       let key = ["ratelimit", endpoint.as_str(), identifier];
       let now = Utc::now();
       let window_start = now - Duration::seconds(limit.window_seconds as i64);
       
       // Get current count
       let current: Option<RateLimitCounter> = storage.get(&key).await?;
       
       let count = match current {
           Some(counter) if counter.window_start > window_start => {
               counter.count + 1
           }
           _ => 1,
       };
       
       if count > limit.requests {
           return Err(RateLimitError::Exceeded {
               limit: limit.requests,
               window_seconds: limit.window_seconds,
               retry_after: limit.window_seconds,
           });
       }
       
       // Update counter
       storage.set(
           &key,
           json!({
               "count": count,
               "window_start": now,
           }),
           Some(now + Duration::seconds(limit.window_seconds as i64 * 2)),
       ).await?;
       
       Ok(())
   }
   ```

### Phase 5: Cryptography & JWT

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
   - **PKCE is REQUIRED by default** (NEW)
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
     - `Secure: true` (ALWAYS in production)
     - `SameSite: Strict` (default, can be `Lax` for cross-origin flows)
     - `Path: /`
   - Cookies used:
     - `authorization` - Stores auth state during flow (24h TTL)
     - `provider` - Provider-specific state (10 min TTL)

### Phase 6: OAuth 2.0 Core

1. **Authorization Endpoint (`/authorize`)**
   - Query parameter parsing:
     - `response_type` (required): `code` or `token`
     - `redirect_uri` (required)
     - `client_id` (required)
     - `state` (required - **now mandatory**) (CHANGED)
     - `provider` (optional - selects provider directly)
     - `audience` (optional)
     - `code_challenge` / `code_challenge_method` (required by default) (CHANGED)
   - **Client validation against registry** (NEW)
   - **Redirect URI exact-match validation** (NEW)
   - Authorization state storage in encrypted cookie (24h TTL)
   - Provider selection logic (redirect to `/{provider}/authorize`)
   - If single provider configured, auto-redirect

2. **Token Endpoint (`/token`)**
   - **Client validation on every request** (NEW)
   - **Authorization Code Grant** (`grant_type=authorization_code`):
     - **Client authentication (for confidential clients)** (NEW)
     - Code validation (single-use enforcement)
     - Redirect URI matching
     - Client ID validation
     - PKCE verifier validation (constant-time)
     - **CRITICAL**: Delete code BEFORE token generation to prevent race conditions
     - Code TTL: 60 seconds
   - **Refresh Token Grant** (`grant_type=refresh_token`):
     - **Client authentication (for confidential clients)** (NEW)
     - Token validation
     - **CRITICAL**: Use DynamoDB Transactions for atomic refresh token rotation
     - Reuse detection with configurable time window (default: 60s)
     - Token invalidation on reuse past window
     - Pre-generate next refresh token to avoid race conditions
     - Token format: `{subject}:{token_uuid}`
   - **Client Credentials Grant** (`grant_type=client_credentials`):
     - **CRITICAL**: Validate client_secret (constant-time comparison)
     - **Only for confidential clients** (NEW)
     - Provider-specific client authentication
     - Token generation

3. **Response Type `token` (Implicit Flow)**
   - Tokens returned in URL hash fragment
   - Format: `#access_token=...&refresh_token=...&state=...`
   - Used for SPA apps without backend
   - **Requires PKCE** (NEW)

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

### Phase 7: Provider System

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
   - Password hashing (Argon2 - mandatory)
   - Password verification with timing-safe comparison
   - Registration/login/change password flows
   - Email verification code (6 digits, unbiased generation)
   - Code TTL: 10 minutes
   - **Rate limiting on all password endpoints** (NEW)

7. **Code Provider**
   - OTP code generation (6 digits by default)
   - Code delivery via callback (email/SMS)
   - Code verification with timing-safe comparison
   - **Rate limiting on verification endpoint** (NEW)

### Phase 8: Subject System

1. **Subject Schema Definition**
   - Replace valibot with Rust validation
   - Schema definition and validation
   - Type-safe subject properties

2. **Subject Resolution**
   - Hash generation (use SHA-256, not SHA-1, and don't truncate)
   - Subject ID format: `{type}:{hash}`

### Phase 9: UI Components (Simplified)

1. **Provider Selection UI**
   - HTML form generation
   - Provider list rendering

2. **Password/Code Forms**
   - Form HTML generation
   - Error message display

### Phase 10: Error Handling

1. **OAuth Error Types**

   ```rust
   pub enum OAuthErrorCode {
       InvalidRequest,
       InvalidClient,        // NEW: Client not registered or invalid secret
       InvalidGrant,
       UnauthorizedClient,
       AccessDenied,
       UnsupportedGrantType,
       UnsupportedResponseType,
       InvalidRedirectUri,   // NEW: Redirect URI not in allowlist
       ServerError,
       TemporarilyUnavailable,
       RateLimitExceeded,    // NEW: Too many requests
   }
   ```

2. **Application Error Types**
   - `MissingParameterError` - Required parameter not provided
   - `UnauthorizedClientError` - Client not registered or not allowed
   - `InvalidClientSecretError` - Client secret validation failed (NEW)
   - `UnknownStateError` - Browser state lost (cookies expired)
   - `InvalidSubjectError` - Subject validation failed
   - `InvalidRefreshTokenError` - Refresh token invalid/expired
   - `InvalidAccessTokenError` - Access token invalid/expired
   - `InvalidAuthorizationCodeError` - Auth code invalid/expired
   - `RateLimitExceededError` - Too many requests (NEW)
   - `ClientNotFoundError` - Client not registered (NEW)
   - `RedirectUriMismatchError` - Redirect URI not in allowlist (NEW)

3. **Error Redirect Handling**
   - On OAuth errors, redirect to `redirect_uri` with:
     - `?error={error_code}&error_description={description}`
   - **On unregistered redirect URI, DO NOT redirect - display error page** (NEW)
   - On unknown state errors, display error page

---

## Critical Security Implementation Details

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

### 4. Client Secret Validation (ALL FLOWS)

**NEW**: All confidential clients must provide valid secrets.

```rust
// Validate on every token request for confidential clients
pub async fn validate_client_secret(
    client: &Client,
    provided_secret: Option<&str>,
) -> Result<(), OAuthError> {
    if client.client_type != ClientType::Confidential {
        return Ok(());
    }
    
    let secret = provided_secret
        .ok_or(OAuthError::InvalidClient("Client secret required"))?;
    
    let hash = client.client_secret_hash.as_ref()
        .ok_or(OAuthError::ServerError("Client misconfigured"))?;
    
    if !verify_secret_constant_time(secret, hash) {
        return Err(OAuthError::InvalidClient("Invalid client secret"));
    }
    
    Ok(())
}
```

### 5. Redirect URI Exact Match (NO PATTERN MATCHING)

**NEW**: Redirect URIs must match exactly - no wildcards, no domain matching.

```rust
pub fn validate_redirect_uri(
    client: &Client,
    provided_uri: &str,
) -> Result<(), OAuthError> {
    // EXACT MATCH ONLY - no patterns, no domain matching
    if !client.redirect_uris.iter().any(|uri| uri == provided_uri) {
        return Err(OAuthError::InvalidRedirectUri(format!(
            "Redirect URI not registered. Allowed URIs: {:?}",
            client.redirect_uris
        )));
    }
    Ok(())
}
```

---

## Testing Strategy

### Unit Tests

1. **Client Registry**
   - Client CRUD operations
   - Secret hashing and verification
   - Validation logic

2. **Cryptography**
   - PKCE generation and validation
   - JWT signing and verification
   - Key generation
   - Client secret hashing

3. **Storage**
   - DynamoDB operations
   - Key encoding/decoding
   - TTL handling

4. **OAuth Flows**
   - Authorization code flow
   - Refresh token flow
   - Client credentials flow

5. **Rate Limiting**
   - Counter increment
   - Window expiration
   - Limit enforcement

### Integration Tests

1. **End-to-End OAuth Flow**
   - Client registration → authorization → token exchange → token refresh

2. **Provider Integration**
   - OAuth2 provider flow
   - Password provider flow

3. **Management API**
   - Client CRUD with authentication
   - Token revocation

### Security Tests

1. **Race Condition Tests**
   - Concurrent authorization code exchange
   - Concurrent refresh token rotation

2. **Timing Attack Tests**
   - PKCE comparison timing
   - Secret comparison timing

3. **Client Security Tests** (NEW)
   - Unregistered client rejection
   - Invalid redirect URI rejection
   - Invalid client secret rejection
   - Public client restrictions

4. **Rate Limit Tests** (NEW)
   - Limit enforcement
   - Window reset behavior

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
   # Development mode - allows localhost redirect URIs
   DEV_MODE=true
   TRUSTED_PROXIES=none
   ```

4. **Bootstrap Admin Key for Local Testing**
   ```bash
   # Generate initial admin key
   curl -X POST http://localhost:9000/admin/bootstrap
   # Returns: {"api_key": "abc123:xyz789..."}
   # Save this - it's only shown once!
   ```

5. **Register a Test Client**
   ```bash
   curl -X POST http://localhost:9000/admin/clients \
     -H "X-Admin-API-Key: abc123:xyz789..." \
     -H "Content-Type: application/json" \
     -d '{
       "client_id": "test-app",
       "client_type": "public",
       "redirect_uris": ["http://localhost:3000/callback"],
       "allowed_grant_types": ["authorization_code", "refresh_token"]
     }'
   ```

6. **CDK Watch (for infrastructure changes)**
   ```bash
   cd infra
   cdk watch
   ```
   This watches for infrastructure changes and redeploys automatically.

---

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

3. **Post-Deployment: Bootstrap Admin Key**
   ```bash
   # After deployment, generate the initial admin key
   curl -X POST https://your-api-url/admin/bootstrap
   # SAVE THIS KEY SECURELY - it's only shown once!
   ```

4. **Register Your First Client**
   ```bash
   curl -X POST https://your-api-url/admin/clients \
     -H "X-Admin-API-Key: YOUR_ADMIN_KEY" \
     -H "Content-Type: application/json" \
     -d '{
       "client_id": "my-web-app",
       "client_type": "public",
       "redirect_uris": [
         "https://app.example.com/callback",
         "https://app.example.com/auth/callback"
       ],
       "allowed_grant_types": ["authorization_code", "refresh_token"]
     }'
   ```

5. **View Outputs**
   After deployment, CDK outputs:

   ```
   Outputs:
   OpenAuthStack.ApiUrl = https://abc123.execute-api.us-east-1.amazonaws.com/
   OpenAuthStack.TableName = OpenAuthStack-AuthTable12345-ABCDEF
   ```

6. **Destroy (if needed)**
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

| Variable | Required | Description |
| ------------------- | ----------- | ---------------------------------------------------------- |
| `DYNAMODB_TABLE` | Yes | DynamoDB table name (set by CDK) |
| `AWS_REGION` | Auto | Set by Lambda runtime |
| `ISSUER_URL` | Recommended | Base URL for JWT `iss` claim (defaults to API Gateway URL) |
| `RUST_LOG` | No | Log level (debug, info, warn, error) |
| `DYNAMODB_ENDPOINT` | No | Override endpoint (for local testing) |
| `TRUSTED_PROXIES` | Yes | "none", "api-gateway", or comma-separated CIDRs (NEW) |
| `DEV_MODE` | No | "true" to allow localhost redirects (default: "false") (NEW) |

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

// Staging (with dev mode for testing)
new OpenAuthStack(app, "OpenAuthStaging", {
  env: { account: "123456789", region: "us-east-1" },
  devMode: true,  // Allow localhost redirects
})

// Production (locked down)
new OpenAuthStack(app, "OpenAuthProd", {
  env: { account: "123456789", region: "us-east-1" },
  devMode: false,  // No localhost redirects
})
```

Deploy specific stack:

```bash
cdk deploy OpenAuthStaging
cdk deploy OpenAuthProd
```

---

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

---

## Success Criteria

1. ✅ All OAuth 2.0 endpoints functional
2. ✅ **Client registry implemented and mandatory** (NEW)
3. ✅ **Management API with authentication** (NEW)
4. ✅ **Rate limiting enabled by default** (NEW)
5. ✅ All security vulnerabilities fixed
6. ✅ DynamoDB storage working with TTL
7. ✅ JWT signing and verification working
8. ✅ PKCE flow working with constant-time comparison (**required by default**)
9. ✅ Refresh token rotation atomic
10. ✅ **Client secrets validated for all confidential clients** (NEW)
11. ✅ Authorization codes single-use
12. ✅ **Redirect URI exact-match validation** (NEW)
13. ✅ **No permissive defaults (localhost, domain matching disabled)** (NEW)
14. ✅ Comprehensive test coverage

---

## Notes for AI Agent

### Critical Security Requirements

- **Client Registry is MANDATORY** - No unregistered clients allowed
- **Redirect URIs must EXACT MATCH** - No patterns, no domain matching
- **Always use constant-time comparisons** for secrets, PKCE challenges, OTP codes, passwords
- **Use DynamoDB Transactions** for any operation that must be atomic
- **Delete authorization codes immediately** after validation, before token generation
- **Validate client secrets** for ALL confidential client requests
- **Enable DynamoDB TTL** on the `expiry` attribute
- **Use SHA-256** for subject hashing, don't truncate
- **Implement rate limiting** on all endpoints (enabled by default)
- **Validate state parameter** in authorization callback (now required)
- **Set secure cookie flags** (HttpOnly, Secure, SameSite=Strict)
- **PKCE is required by default** - can only be disabled per-client
- **Management API requires authentication** - no anonymous admin access
- **Don't trust x-forwarded-* headers** unless explicitly configured

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

- Maintain same DynamoDB key structure as TypeScript version (except new keys for client registry)
- Use same JWT claims structure
- Use same cookie names and encryption format
- Ensure backward compatibility with existing refresh tokens
- **Existing deployments will need to register clients before upgrade**

---

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
    let path = event.uri().path();

    // Rate limiting check (before any processing)
    if let Err(e) = check_rate_limit(&event).await {
        return Ok(rate_limit_response(e));
    }

    match path {
        // Public endpoints
        "/.well-known/oauth-authorization-server" => handle_well_known(event).await,
        "/.well-known/jwks.json" => handle_jwks(event).await,
        "/authorize" => handle_authorize(event).await,  // Validates client
        "/token" => handle_token(event).await,          // Validates client + secret
        "/userinfo" => handle_userinfo(event).await,
        
        // Admin endpoints (require authentication)
        path if path.starts_with("/admin/") => {
            // Authenticate admin request
            match authenticate_admin_request(&event).await {
                Ok(admin_ctx) => handle_admin_routes(event, admin_ctx).await,
                Err(_) => Ok(unauthorized_response()),
            }
        }
        
        // Provider routes
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

---

## Client Library Considerations

The existing TypeScript client (`@openauthjs/openauth/client`) can be used as-is with the Rust issuer since they communicate via standard OAuth 2.0 HTTP endpoints. No Rust client is required.

However, if a Rust client is needed (e.g., for Rust backend services):

```rust
pub struct OpenAuthClient {
    issuer: String,
    client_id: String,
    client_secret: Option<String>,  // For confidential clients
    http_client: reqwest::Client,
}

impl OpenAuthClient {
    pub async fn authorize(&self, redirect_uri: &str, response_type: &str) -> AuthorizeResult;
    pub async fn exchange(&self, code: &str, redirect_uri: &str, verifier: Option<&str>) -> Result<Tokens>;
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens>;
    pub async fn verify(&self, token: &str) -> Result<Claims>;
}
```

---

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

---

## Migration Guide (from TypeScript OpenAuth)

If migrating from the original TypeScript implementation:

1. **Register All Existing Clients**
   - Document all client_ids currently in use
   - Register each with appropriate redirect URIs
   - Update client applications with any new secrets

2. **Update Redirect URIs**
   - Remove wildcard patterns
   - List ALL exact redirect URIs used

3. **Enable PKCE in Clients**
   - Update SPAs and mobile apps to use PKCE
   - Confidential clients should also use PKCE

4. **Test in Staging**
   - Deploy to staging with `DEV_MODE=true` first
   - Verify all clients work correctly
   - Then deploy to production with `DEV_MODE=false`

---

## Additional Resources

- [OAuth 2.0 RFC 6749](https://tools.ietf.org/html/rfc6749)
- [PKCE RFC 7636](https://tools.ietf.org/html/rfc7636)
- [JWT RFC 7519](https://tools.ietf.org/html/rfc7519)
- [JWE RFC 7516](https://tools.ietf.org/html/rfc7516) - For cookie encryption
- [OAuth 2.0 Security Best Current Practice](https://datatracker.ietf.org/doc/html/draft-ietf-oauth-security-topics) - Security recommendations
- [OAuth 2.0 for Browser-Based Apps](https://datatracker.ietf.org/doc/html/draft-ietf-oauth-browser-based-apps) - SPA security
- [AWS Lambda Rust Runtime](https://github.com/awslabs/aws-lambda-rust-runtime)
- [AWS SDK for Rust](https://github.com/awslabs/aws-sdk-rust)
- [cargo-lambda](https://www.cargo-lambda.info/) - Build tool for Rust Lambda
- [cargo-lambda-cdk](https://github.com/cargo-lambda/cargo-lambda-cdk) - CDK construct for Rust
- [AWS CDK Documentation](https://docs.aws.amazon.com/cdk/v2/guide/home.html)
- [DynamoDB Transactions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/transactions.html)
- [Subtle crate](https://docs.rs/subtle) - Constant-time operations
- [Argon2 crate](https://docs.rs/argon2) - Password hashing
