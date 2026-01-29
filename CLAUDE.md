# Claude Code Guidelines

## Project Structure

All new code goes in `src/`. Everything outside `src/` is the original TypeScript OpenAuth implementation and will be refactored or removed once the Rust implementation is complete. Do not modify files outside `src/` unless explicitly asked.

### Original Code (do not modify)

- `packages/` - Original TypeScript OpenAuth implementation (Hono, Arctic, Valibot)
- `examples/` - Client and issuer examples (Next.js, React, Lambda, Cloudflare)
- `www/` - Documentation site (Astro)
- `scripts/` - Formatting scripts
- `.changeset/` - Release management

### New Implementation (`src/`)

```
src/
├── rust/                      # Rust Lambda (OAuth 2.0 server)
│   ├── Cargo.toml
│   ├── .cargo/config.toml     # Cross-compilation for ARM64 Lambda
│   └── src/
│       ├── main.rs            # Lambda entry point, DynamoDB client init
│       ├── lib.rs             # Public exports
│       ├── config.rs          # Env-based config (proxy, rate limits, tokens)
│       ├── error.rs           # Error types (OAuth, Auth, Storage)
│       ├── routes.rs          # Axum router with all endpoint definitions
│       ├── client/            # Client registry (mandatory for all OAuth clients)
│       │   ├── types.rs       # Client, GrantType, ClientType structs
│       │   ├── registry.rs    # CRUD operations for client management
│       │   └── validation.rs  # Per-request client + redirect URI validation
│       ├── admin/             # Management API (API key authenticated)
│       │   ├── auth.rs        # Admin key verification, bootstrap
│       │   ├── clients.rs     # Client CRUD endpoints
│       │   └── tokens.rs      # Token revocation
│       ├── storage/           # Persistence layer
│       │   ├── adapter.rs     # StorageAdapter trait (get/set/remove/scan/transact)
│       │   └── dynamo.rs      # DynamoDB implementation with key encoding
│       ├── jwt/               # Token operations
│       │   ├── keys.rs        # ES256 key generation, rotation, JWKS
│       │   ├── sign.rs        # Access + refresh token signing
│       │   └── verify.rs      # Token verification
│       ├── oauth/             # OAuth 2.0 endpoints
│       │   ├── authorize.rs   # /authorize - client validation, PKCE, state
│       │   ├── token.rs       # /token - code exchange, refresh, client creds
│       │   ├── pkce.rs        # PKCE challenge/verify (constant-time)
│       │   ├── userinfo.rs    # /userinfo - bearer token validation
│       │   └── well_known.rs  # Discovery + JWKS endpoints
│       ├── crypto/            # Cryptographic operations
│       │   ├── secrets.rs     # Client secret hashing (Argon2)
│       │   ├── password.rs    # User password hashing (Argon2)
│       │   ├── encrypt.rs     # Cookie encryption (RSA-OAEP + AES-GCM)
│       │   └── random.rs      # Secure random strings, unbiased OTP digits
│       ├── ratelimit/         # DynamoDB-based rate limiting
│       │   └── middleware.rs   # Per-endpoint sliding window counters
│       ├── provider/          # Identity provider integrations
│       │   ├── traits.rs      # Provider trait definition
│       │   ├── oauth2.rs      # Base OAuth2 flow
│       │   ├── oidc.rs        # OIDC (extends OAuth2 with ID token)
│       │   ├── google.rs      # Google (OIDC)
│       │   ├── github.rs      # GitHub (OAuth2)
│       │   ├── apple.rs       # Apple Sign In (OIDC)
│       │   ├── password.rs    # Email/password with verification
│       │   └── code.rs        # OTP code (email/SMS)
│       ├── subject/           # Subject identity
│       │   └── schema.rs      # Subject hashing (SHA-256), validation
│       └── ui/                # HTML form generation
│           ├── select.rs      # Provider selection page
│           ├── password.rs    # Login/register forms
│           └── code.rs        # OTP input form
└── infra/                     # AWS CDK infrastructure (TypeScript)
    ├── bin/irongate.ts        # CDK app entry point
    ├── lib/irongate-stack.ts  # DynamoDB + Lambda + API Gateway stack
    ├── cdk.json
    ├── package.json           # aws-cdk-lib, cargo-lambda-cdk
    └── tsconfig.json
```

### Rationale

**Why Rust for the Lambda?** The original TypeScript implementation lacks security-first defaults. The Rust rewrite enforces mandatory client registration, explicit redirect URI allowlists, required PKCE, rate limiting, and constant-time comparisons by default.

**Why Axum?** Provides clean routing with extractors and middleware for a Lambda with 15+ endpoints. Minimal cold start overhead for an IO-bound OAuth server.

**Why CDK (not SST)?** The original uses SST. CDK with `cargo-lambda-cdk` gives direct control over the Rust Lambda build and deployment without the SST abstraction layer.

**Why separate `client/` and `admin/`?** Client registry is a new security requirement (not in the original). Admin API provides authenticated management. These are distinct from the OAuth flow itself.

**Why `storage/adapter.rs` trait?** Allows swapping DynamoDB for in-memory storage in tests without changing business logic.

## Security Invariants

These must never be relaxed:

- Client registration is mandatory (no anonymous clients)
- Redirect URIs are exact-match only (no patterns, no domain matching)
- PKCE is required by default (can only be disabled per-client)
- All secret comparisons use constant-time operations (`subtle` crate)
- Authorization codes are deleted before token generation (prevent race conditions)
- Refresh token rotation uses DynamoDB transactions (atomic)
- Rate limiting is enabled by default on all endpoints
- `x-forwarded-*` headers are only trusted when explicitly configured

## Build & Test

```bash
# Rust
cd src/rust
cargo check          # Verify compilation
cargo test           # Run tests
cargo lambda watch   # Local Lambda emulator

# CDK
cd src/infra
npm install
npx cdk synth        # Generate CloudFormation (needs Docker or cargo-lambda)
npx cdk deploy       # Deploy to AWS
```
