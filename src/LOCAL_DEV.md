# Local Development Guide

## Prerequisites

- Rust toolchain (`rustup`)
- `cargo-lambda` (`brew install cargo-lambda`)
- Python 3 (for serving the test client)

## 1. Start the OAuth Server

```bash
cd src/rust
DEV_MODE=true \
ISSUER_URL=http://localhost:9000 \
PROVIDERS=password \
PROVIDER_PASSWORD_TYPE=password \
cargo lambda watch
```

The server runs at `http://localhost:9000` with in-memory storage (no DynamoDB needed).

## 2. Bootstrap & Register Test Client (once per server restart)

```bash
cd src/test-client
bash setup.sh
```

This creates an admin API key and registers a public OAuth client (`test-app`) with redirect URI `http://localhost:3000/`.

## 3. Start the Test Client

```bash
cd src/test-client
python3 -m http.server 3000
```

Open **http://localhost:3000** in your browser.

## Flow

1. Click **Login with Irongate**
2. You're redirected to the password login form (served by the OAuth server)
3. Register with any email/password, then log in
4. You're redirected back to `localhost:3000` with your JWT displayed

## Notes

- In-memory storage resets on server restart — re-run `setup.sh` each time.
- The test client uses PKCE (S256) and state validation.
- Tokens are stored in `localStorage`; click **Logout** to clear them.
