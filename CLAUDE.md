# Claude Code Guidelines

## Project Structure

Irongate is an SST AWS app with one Rust auth Lambda and room for additional function code.

```
.
├── infra/                  # SST infrastructure
│   ├── api.ts              # API Gateway HTTP API + Rust Lambda route
│   └── storage.ts          # DynamoDB table
├── packages/
│   └── functions/          # Function workspace
│       ├── auth/           # Rust auth Lambda crate
│       │   ├── Cargo.toml
│       │   └── src/
│       └── package.json
├── sst.config.ts           # SST app entry point
├── package.json            # SST and TypeScript tooling
└── README.md
```

## Security Invariants

These must not be relaxed:

- Client registration is mandatory.
- Redirect URIs are exact-match only.
- PKCE is required by default.
- Secret comparisons use constant-time operations.
- Authorization codes are single-use.
- Refresh token rotation is atomic.
- Rate limiting is enabled by default.
- `x-forwarded-*` headers are only trusted when explicitly configured.

## Build & Test

```bash
cargo test --manifest-path packages/functions/auth/Cargo.toml
npx sst install
npm run typecheck
npm run deploy
```
