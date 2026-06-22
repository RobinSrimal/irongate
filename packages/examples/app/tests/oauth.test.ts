import assert from "node:assert/strict";
import test from "node:test";

import { buildAuthorizeUrl, parseAuthorizeSessionRedirect } from "../src/auth/oauth.js";

test("buildAuthorizeUrl creates native desktop provider authorize requests", () => {
  const url = buildAuthorizeUrl({
    issuerUrl: "https://auth.example.com",
    clientId: "app",
    redirectUri: "http://127.0.0.1:49152/oauth/callback",
    scope: "openid email offline_access",
    state: "state-value",
    nonce: "nonce-value",
    codeChallenge: "challenge-value",
    provider: "apple",
  });

  assert.equal(url.origin, "https://auth.example.com");
  assert.equal(url.pathname, "/authorize");
  assert.equal(url.searchParams.get("response_type"), "code");
  assert.equal(url.searchParams.get("client_id"), "app");
  assert.equal(url.searchParams.get("redirect_uri"), "http://127.0.0.1:49152/oauth/callback");
  assert.equal(url.searchParams.get("scope"), "openid email offline_access");
  assert.equal(url.searchParams.get("state"), "state-value");
  assert.equal(url.searchParams.get("nonce"), "nonce-value");
  assert.equal(url.searchParams.get("provider"), "apple");
  assert.equal(url.searchParams.get("code_challenge"), "challenge-value");
  assert.equal(url.searchParams.get("code_challenge_method"), "S256");
});

test("parseAuthorizeSessionRedirect extracts the API-only session handoff", () => {
  const session = parseAuthorizeSessionRedirect(
    "/password/login?session=raw-session-value",
    "https://auth.example.com",
  );

  assert.equal(session, "raw-session-value");
});

test("parseAuthorizeSessionRedirect rejects non-session redirects", () => {
  assert.throws(
    () => parseAuthorizeSessionRedirect("https://evil.example.com/callback", "https://auth.example.com"),
    /authorize session/,
  );
});
