# Implementation Layers

## Layer 1: Foundation (no blockers) ✅

These have zero dependencies on other TODOs and unblock everything above.

- [x] `storage/dynamo.rs` — `scan()`, `compare_and_set()`, `transact()`
- [x] `jwt/keys.rs` — `generate_signing_key()` + `to_jwks()` (ES256 key generation)
- [x] `error.rs` — Added `StorageError::TransactionConflict` variant

## Layer 2: Core OAuth (depends on Layer 1) ✅

- [x] `crypto/encrypt.rs` — Cookie encrypt/decrypt (RSA-OAEP + AES-GCM)
- [x] `oauth/token.rs` — Authorization code, refresh token, and client credentials grant handlers
- [x] `oauth/authorize.rs` — Provider redirect and state storage
- [x] `client/validation.rs` — Fix `parse_basic_auth()`

## Layer 3: Identity Providers (depends on Layer 2) ✅

- [x] `provider/oauth2.rs` — `exchange_code()`, `fetch_userinfo()`, `build_authorization_url()` with PKCE
- [x] `provider/oidc.rs` — `validate_id_token()` via JWKS fetch, RS256/ES256 verification
- [x] `provider/password.rs` — Registration, login, verify, change, forgot password flows
- [x] `provider/code.rs` — OTP request and verify flows (constant-time, max attempts)

## Layer 4: Wiring + UI (depends on Layer 3) ✅

- [x] `routes.rs` — Wire all handlers (replacing `todo!()` stubs)
- [x] `ui/select.rs` — Provider selection page
- [x] `ui/password.rs` — Login/register forms
- [x] `ui/code.rs` — OTP input form

## Layer 5: Polish

- [x] `admin/tokens.rs` — Token revocation
- [x] `admin/clients.rs` — Pagination for list_clients
