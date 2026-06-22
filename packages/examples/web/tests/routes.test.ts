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

test("stylesheet route serves the desktop app visual system", async () => {
  const env = createTestEnv();
  const response = await worker.fetch(
    new Request("http://localhost:3000/styles.css", { method: "HEAD" }),
    env,
  );

  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /text\/css/);
});

test("logo route is not part of the web UI", async () => {
  const env = createTestEnv();
  const response = await worker.fetch(
    new Request("http://localhost:3000/irongate-logo.png?v=test"),
    env,
  );

  assert.equal(response.status, 404);
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

test("home page is the same usable sign-in surface as the app", async () => {
  const env = createTestEnv({ googleLoginEnabled: true, appleLoginEnabled: true });
  const response = await worker.fetch(new Request("http://localhost:3000/"), env);

  assert.equal(response.status, 200);
  const body = await response.text();
  assert.match(body, /<h1>Web sign in<\/h1>/);
  assert.match(body, /Continue with Google/);
  assert.match(body, /Continue with Apple/);
  assert.match(body, /name="email"/);
  assert.match(body, /Sign in with password/);
  assert.match(body, /Create account/);
  assert.match(response.headers.get("set-cookie") ?? "", /__Host-irongate_web_login=/);
  assert.equal(env.irongateRequests[0]?.url.startsWith("https://auth.example.com/authorize?"), true);
});

test("home page does not repeat signed out in the bottom status line", async () => {
  const env = createTestEnv({ googleLoginEnabled: true, appleLoginEnabled: true });
  const response = await worker.fetch(new Request("http://localhost:3000/"), env);

  assert.equal(response.status, 200);
  const body = await response.text();
  assert.equal(body.match(/Signed out/g)?.length, 1);
});

test("login page uses the desktop app visual shell", async () => {
  const env = createTestEnv({ googleLoginEnabled: true, appleLoginEnabled: true });
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  const body = await response.text();
  assert.match(body, /class="shell"/);
  assert.match(body, /class="panel"/);
  assert.match(body, /href="\/styles\.css\?v=/);
  assert.doesNotMatch(body, /brand-mark/);
  assert.doesNotMatch(body, /irongate-logo\.png/);
  assert.match(body, /Irongate web example/);
  assert.match(body, /class="provider-grid"/);
  assert.match(body, /class="password-form"/);
  assert.doesNotMatch(body, /class="status">Signed out/);
});

test("login page hides Google login when Google is disabled", async () => {
  const env = createTestEnv({ googleLoginEnabled: false });
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  assert.doesNotMatch(await response.text(), /Continue with Google/);
});

test("login page hides Apple login when Apple is disabled", async () => {
  const env = createTestEnv({ appleLoginEnabled: false });
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  assert.doesNotMatch(await response.text(), /Continue with Apple/);
});

test("login page shows Google login when Google is enabled", async () => {
  const env = createTestEnv({ googleLoginEnabled: true });
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  const body = await response.text();
  assert.match(body, /Continue with Google/);
  assert.match(body, /href="\/auth\/login\/google"/);
});

test("login page shows Apple login when Apple is enabled", async () => {
  const env = createTestEnv({ appleLoginEnabled: true });
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  const body = await response.text();
  assert.match(body, /Continue with Apple/);
  assert.match(body, /href="\/auth\/login\/apple"/);
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

test("Google login GET redirects to Irongate authorize with PKCE and a transaction cookie", async () => {
  const env = createTestEnv({ googleLoginEnabled: true });

  const response = await worker.fetch(
    new Request("http://localhost:3000/auth/login/google"),
    env,
  );

  assert.equal(response.status, 303);
  const loginCookie = extractCookie(response, "__Host-irongate_web_login");
  assert.match(loginCookie, /__Host-irongate_web_login=/);
  assert.match(response.headers.get("set-cookie") ?? "", /HttpOnly/);

  const location = response.headers.get("location");
  assert.ok(location);
  const authorize = new URL(location);
  assert.equal(authorize.origin, "https://auth.example.com");
  assert.equal(authorize.pathname, "/authorize");
  assert.equal(authorize.searchParams.get("provider"), "google");
  assert.equal(authorize.searchParams.get("response_type"), "code");
  assert.equal(authorize.searchParams.get("client_id"), "web");
  assert.equal(authorize.searchParams.get("redirect_uri"), "http://localhost:3000/auth/callback");
  assert.equal(authorize.searchParams.get("code_challenge_method"), "S256");
  assert.ok(authorize.searchParams.get("code_challenge"));
  assert.ok(authorize.searchParams.get("state"));
  assert.ok(authorize.searchParams.get("nonce"));
});

test("Apple login GET redirects to Irongate authorize with PKCE and a transaction cookie", async () => {
  const env = createTestEnv({ appleLoginEnabled: true });

  const response = await worker.fetch(
    new Request("http://localhost:3000/auth/login/apple"),
    env,
  );

  assert.equal(response.status, 303);
  const loginCookie = extractCookie(response, "__Host-irongate_web_login");
  assert.match(loginCookie, /__Host-irongate_web_login=/);
  assert.match(response.headers.get("set-cookie") ?? "", /HttpOnly/);

  const location = response.headers.get("location");
  assert.ok(location);
  const authorize = new URL(location);
  assert.equal(authorize.origin, "https://auth.example.com");
  assert.equal(authorize.pathname, "/authorize");
  assert.equal(authorize.searchParams.get("provider"), "apple");
  assert.equal(authorize.searchParams.get("response_type"), "code");
  assert.equal(authorize.searchParams.get("client_id"), "web");
  assert.equal(authorize.searchParams.get("redirect_uri"), "http://localhost:3000/auth/callback");
  assert.equal(authorize.searchParams.get("code_challenge_method"), "S256");
  assert.ok(authorize.searchParams.get("code_challenge"));
  assert.ok(authorize.searchParams.get("state"));
  assert.ok(authorize.searchParams.get("nonce"));
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
  const appBody = await app.text();
  assert.match(appBody, /user@example.com/);
  assert.match(appBody, /class="shell"/);
  assert.match(appBody, /class="panel"/);
  assert.match(appBody, /class="session"/);
  assert.match(appBody, /class="badge active"/);

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
