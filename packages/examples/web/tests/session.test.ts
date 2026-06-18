import assert from "node:assert/strict";
import test from "node:test";

import {
  buildClearCookie,
  buildSessionCookie,
  parseCookies,
} from "../src/session.js";

test("buildSessionCookie creates an opaque HttpOnly Secure SameSite cookie", () => {
  const header = buildSessionCookie({
    name: "__Host-irongate_web_session",
    value: "opaque-session-id",
    maxAgeSeconds: 3600,
  });

  assert.match(header, /^__Host-irongate_web_session=opaque-session-id;/);
  assert.match(header, /HttpOnly/);
  assert.match(header, /Secure/);
  assert.match(header, /SameSite=Lax/);
  assert.match(header, /Path=\//);
  assert.match(header, /Max-Age=3600/);
});

test("buildClearCookie expires the session cookie", () => {
  const header = buildClearCookie("__Host-irongate_web_session");

  assert.match(header, /^__Host-irongate_web_session=;/);
  assert.match(header, /Max-Age=0/);
  assert.match(header, /HttpOnly/);
  assert.match(header, /Secure/);
  assert.match(header, /Path=\//);
});

test("parseCookies reads cookie values without decoding unrelated attributes", () => {
  const cookies = parseCookies("a=one; __Host-irongate_web_session=two%203; empty=");

  assert.equal(cookies.get("a"), "one");
  assert.equal(cookies.get("__Host-irongate_web_session"), "two 3");
  assert.equal(cookies.get("empty"), "");
});
