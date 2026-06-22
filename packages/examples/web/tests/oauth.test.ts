import assert from "node:assert/strict";
import test from "node:test";

import { buildAuthorizeUrl, createPkcePair } from "../src/oauth.js";

test("buildAuthorizeUrl creates a password authorize request with PKCE", async () => {
  const pkce = await createPkcePair();
  const url = buildAuthorizeUrl({
    issuerUrl: "https://auth.example.com",
    clientId: "web",
    redirectUri: "https://web.example.com/auth/callback",
    state: "state-123",
    nonce: "nonce-123",
    scope: "openid email offline_access",
    codeChallenge: pkce.challenge,
  });

  assert.equal(url.origin, "https://auth.example.com");
  assert.equal(url.pathname, "/authorize");
  assert.equal(url.searchParams.get("response_type"), "code");
  assert.equal(url.searchParams.get("client_id"), "web");
  assert.equal(url.searchParams.get("redirect_uri"), "https://web.example.com/auth/callback");
  assert.equal(url.searchParams.get("provider"), "password");
  assert.equal(url.searchParams.get("code_challenge_method"), "S256");
  assert.equal(url.searchParams.get("code_challenge"), pkce.challenge);
  assert.equal(url.searchParams.get("state"), "state-123");
  assert.equal(url.searchParams.get("nonce"), "nonce-123");
});

test("buildAuthorizeUrl can create a Google authorize request with PKCE", async () => {
  const pkce = await createPkcePair();
  const url = buildAuthorizeUrl({
    issuerUrl: "https://auth.example.com",
    clientId: "web",
    redirectUri: "https://web.example.com/auth/callback",
    state: "state-123",
    nonce: "nonce-123",
    scope: "openid email offline_access",
    codeChallenge: pkce.challenge,
    provider: "google",
  });

  assert.equal(url.origin, "https://auth.example.com");
  assert.equal(url.pathname, "/authorize");
  assert.equal(url.searchParams.get("provider"), "google");
  assert.equal(url.searchParams.get("code_challenge_method"), "S256");
  assert.equal(url.searchParams.get("code_challenge"), pkce.challenge);
  assert.equal(url.searchParams.get("redirect_uri"), "https://web.example.com/auth/callback");
});

test("buildAuthorizeUrl can create an Apple authorize request with PKCE", async () => {
  const pkce = await createPkcePair();
  const url = buildAuthorizeUrl({
    issuerUrl: "https://auth.example.com",
    clientId: "web",
    redirectUri: "https://web.example.com/auth/callback",
    state: "state-123",
    nonce: "nonce-123",
    scope: "openid email offline_access",
    codeChallenge: pkce.challenge,
    provider: "apple",
  });

  assert.equal(url.origin, "https://auth.example.com");
  assert.equal(url.pathname, "/authorize");
  assert.equal(url.searchParams.get("provider"), "apple");
  assert.equal(url.searchParams.get("code_challenge_method"), "S256");
  assert.equal(url.searchParams.get("code_challenge"), pkce.challenge);
  assert.equal(url.searchParams.get("redirect_uri"), "https://web.example.com/auth/callback");
});
