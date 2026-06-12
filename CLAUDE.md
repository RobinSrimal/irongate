# Claude Code Guidelines

## Project Structure

Irongate is an SST v3 AWS app with one Rust Lambda.

```
.
├── infra/                  # SST infrastructure
│   ├── api.ts              # API Gateway HTTP API + Rust Lambda route
│   └── storage.ts          # DynamoDB table
├── packages/
│   └── functions/          # Rust Lambda crate
│       ├── Cargo.toml
│       └── src/
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
cargo test --manifest-path packages/functions/Cargo.toml
npx sst install
npm run typecheck
npm run deploy
```
