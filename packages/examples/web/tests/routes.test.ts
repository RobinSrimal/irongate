import assert from "node:assert/strict";
import test from "node:test";

import { createTestEnv } from "../src/testing.js";
import worker from "../src/worker.js";

test("health route returns ok", async () => {
  const env = createTestEnv();
  const response = await worker.fetch(new Request("http://localhost:3000/health"), env);

  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { status: "ok" });
});

test("register action forwards JSON registration to Irongate", async () => {
  const env = createTestEnv();
  const form = new FormData();
  form.set("email", "user@example.com");
  form.set("password", "correct horse battery staple");

  const response = await worker.fetch(
    new Request("http://localhost:3000/auth/register", {
      method: "POST",
      body: form,
    }),
    env,
  );

  assert.equal(response.status, 200);
  assert.equal(env.irongateRequests.length, 1);
  assert.equal(env.irongateRequests[0]?.url, "https://auth.example.com/password/register");
  assert.equal(env.irongateRequests[0]?.method, "POST");
  assert.deepEqual(JSON.parse(env.irongateRequests[0]?.body ?? "{}"), {
    email: "user@example.com",
    password: "correct horse battery staple",
  });
});

test("login page creates an authorize session and sets an HttpOnly transaction cookie", async () => {
  const env = createTestEnv();
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  assert.match(await response.text(), /name="email"/);
  assert.match(response.headers.get("set-cookie") ?? "", /__Host-irongate_web_login=/);
  assert.match(response.headers.get("set-cookie") ?? "", /HttpOnly/);
  assert.equal(env.irongateRequests[0]?.url.startsWith("https://auth.example.com/authorize?"), true);
});

test("login page derives callback URL from the request origin when no base URL is configured", async () => {
  const env = createTestEnv();
  delete env.WEB_BASE_URL;

  await worker.fetch(new Request("https://example-web.dev/auth/login"), env);

  const authorize = new URL(env.irongateRequests[0]?.url ?? "https://missing.example");
  assert.equal(
    authorize.searchParams.get("redirect_uri"),
    "https://example-web.dev/auth/callback",
  );
});

test("verify email action forwards the verification token to Irongate", async () => {
  const env = createTestEnv();
  const response = await worker.fetch(
    new Request("http://localhost:3000/auth/verify-email?token=verify-token"),
    env,
  );

  assert.equal(response.status, 200);
  assert.equal(env.irongateRequests[0]?.url, "https://auth.example.com/password/verify");
  assert.deepEqual(JSON.parse(env.irongateRequests[0]?.body ?? "{}"), {
    token: "verify-token",
  });
});

test("password login callback creates an app session and logout revokes refresh token", async () => {
  const env = createTestEnv();
  const start = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);
  const loginCookie = extractCookie(start, "__Host-irongate_web_login");
  const form = new FormData();
  form.set("email", "user@example.com");
  form.set("password", "correct horse battery staple");

  const login = await worker.fetch(
    new Request("http://localhost:3000/auth/login", {
      method: "POST",
      headers: { cookie: loginCookie },
      body: form,
    }),
    env,
  );

  assert.equal(login.status, 303);
  const callbackUrl = login.headers.get("location");
  assert.match(callbackUrl ?? "", /^http:\/\/localhost:3000\/auth\/callback\?code=code-123&state=/);

  const callback = await worker.fetch(
    new Request(callbackUrl ?? "http://localhost:3000/auth/callback", {
      headers: { cookie: loginCookie },
    }),
    env,
  );
  const appCookie = extractCookie(callback, "__Host-irongate_web_session");

  assert.equal(callback.status, 303);
  assert.equal(callback.headers.get("location"), "http://localhost:3000/app");
  assert.match(appCookie, /__Host-irongate_web_session=/);
  assert.equal(
    env.irongateRequests.some(
      (request) => request.url === "https://auth.example.com/token" && request.body?.includes("code=code-123"),
    ),
    true,
  );

  const app = await worker.fetch(
    new Request("http://localhost:3000/app", {
      headers: { cookie: appCookie },
    }),
    env,
  );
  assert.equal(app.status, 200);
  assert.match(await app.text(), /user@example.com/);

  const logout = await worker.fetch(
    new Request("http://localhost:3000/auth/logout", {
      method: "POST",
      headers: { cookie: appCookie },
    }),
    env,
  );

  assert.equal(logout.status, 303);
  assert.equal(
    env.irongateRequests.some(
      (request) =>
        request.url === "https://auth.example.com/oauth/revoke" &&
        request.body?.includes("token=refresh-token"),
    ),
    true,
  );
});

function extractCookie(response: Response, name: string): string {
  const setCookie = response.headers.get("set-cookie");
  assert.ok(setCookie, `missing set-cookie header for ${name}`);
  const match = setCookie.match(new RegExp(`${name}=[^;]+`));
  assert.ok(match, `missing ${name} cookie in ${setCookie}`);
  return match[0];
}
