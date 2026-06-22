import assert from "node:assert/strict";
import test from "node:test";

import {
  base64UrlEncode,
  createPkcePair,
  randomUrlSafeString,
} from "../src/auth/pkce.js";

test("base64UrlEncode removes padding and URL-unsafe characters", () => {
  const encoded = base64UrlEncode(new Uint8Array([251, 255, 238]));

  assert.equal(encoded, "-__u");
  assert(!encoded.includes("="));
  assert(!encoded.includes("+"));
  assert(!encoded.includes("/"));
});

test("createPkcePair creates an S256 challenge for the verifier", async () => {
  const pair = await createPkcePair();

  assert.match(pair.verifier, /^[A-Za-z0-9._~-]{43,128}$/);
  assert.match(pair.challenge, /^[A-Za-z0-9_-]+$/);
  assert.notEqual(pair.challenge, pair.verifier);
});

test("randomUrlSafeString returns URL-safe state material", () => {
  const value = randomUrlSafeString(32);

  assert.match(value, /^[A-Za-z0-9_-]+$/);
  assert(value.length >= 32);
});
