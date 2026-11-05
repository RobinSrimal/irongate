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

test("register GET and HEAD do not call Irongate or create a session", async () => {
  const env = createTestEnv();
  const sessionAccess = trackSessionAccess(env);
  for (const method of ["GET", "HEAD"]) {
    const response = await worker.fetch(new Request("http://localhost:3000/auth/register", { method }), env);
    assert.equal(response.status, 200);
    assertRegistrationHeaders(response);
    const body = await response.text();
    if (method === "HEAD") {
      assert.equal(body, "");
    } else {
      assert.match(body, /<form[^>]+method="post" action="\/auth\/register"/);
      assert.match(body, /<input name="email" type="email"/);
      assert.doesNotMatch(body, /type="password"/);
    }
  }
  assert.deepEqual(env.irongateRequests, []);
  assert.deepEqual(sessionAccess, []);
});

test("register action forwards only email to Irongate", async () => {
  const env = createTestEnv();
  const sessionAccess = trackSessionAccess(env);
  const form = new FormData();
  form.set("email", "user@example.com");
  form.set("password", "correct horse battery staple");

  const response = await worker.fetch(
    new Request("http://localhost:3000/auth/register", {
      method: "POST",
      headers: { origin: "http://localhost:3000" },
      body: form,
    }),
    env,
  );

  assert.equal(response.status, 200);
  assertRegistrationHeaders(response);
  assert.equal(env.irongateRequests.length, 1);
  assert.equal(env.irongateRequests[0]?.url, "https://auth.example.com/password/register");
  assert.equal(env.irongateRequests[0]?.method, "POST");
  assert.deepEqual(JSON.parse(env.irongateRequests[0]?.body ?? "{}"), {
    email: "user@example.com",
  });
  assert.doesNotMatch(await response.text(), /correct horse battery staple/);
  assert.deepEqual(sessionAccess, []);
});

test("login page creates an authorize session and sets an HttpOnly transaction cookie", async () => {
  const env = createTestEnv();
  const response = await worker.fetch(new Request("http://localhost:3000/auth/login"), env);

  assert.equal(response.status, 200);
  assert.match(response.headers.get("set-cookie") ?? "", /__Host-irongate_web_login=/);
  assert.match(response.headers.get("set-cookie") ?? "", /HttpOnly/);
  assert.equal(env.irongateRequests[0]?.url.startsWith("https://auth.example.com/authorize?"), true);
});

test("home page starts an authorize session and sets an HttpOnly transaction cookie", async () => {
  const env = createTestEnv({ googleLoginEnabled: true, appleLoginEnabled: true });
  const response = await worker.fetch(new Request("http://localhost:3000/"), env);

  assert.equal(response.status, 200);
  assert.match(response.headers.get("set-cookie") ?? "", /__Host-irongate_web_login=/);
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

test("verification GET, reload, prefetch and HEAD only render an escaped setup form", async () => {
  const env = createTestEnv();
  const sessionAccess = trackSessionAccess(env);
  const token = `proof-"<script>&'`;
  const url = new URL("http://localhost:3000/auth/verify-email");
  url.searchParams.set("token", token);
  for (const init of [
    {},
    {},
    { headers: { purpose: "prefetch", "sec-purpose": "prefetch" } },
    { method: "HEAD" },
  ]) {
    const response = await worker.fetch(new Request(url, init), env);
    assert.equal(response.status, 200);
    assertRegistrationHeaders(response);
    const body = await response.text();
    if (init.method === "HEAD") {
      assert.equal(body, "");
    } else {
      assert.match(body, /<form[^>]+method="post" action="\/auth\/verify-email"/);
      assert.match(body, /<input name="token" type="hidden" value="proof-&quot;&lt;script&gt;&amp;&#39;">/);
      assert.match(body, /<input name="new_password" type="password" autocomplete="new-password" required>/);
      assert.match(body, /<input name="password_confirmation" type="password" autocomplete="new-password" required>/);
      assert.equal(body.includes(token), false);
      assert.equal(body.split("proof-").length, 2);
      assert.doesNotMatch(body, /<script|[?&]token=/);
    }
    assert.deepEqual(env.irongateRequests, []);
    assert.deepEqual(sessionAccess, []);
  }
});

test("verification GET and HEAD reject absent proofs without calling Irongate", async () => {
  const env = createTestEnv();
  for (const method of ["GET", "HEAD"]) {
    for (const query of ["", "?token="]) {
      const response = await worker.fetch(
        new Request(`http://localhost:3000/auth/verify-email${query}`, { method }),
        env,
      );
      assert.equal(response.status, 400);
      assertRegistrationHeaders(response);
      if (method === "HEAD") {
        assert.equal(await response.text(), "");
      }
    }
  }
  assert.deepEqual(env.irongateRequests, []);
});

test("verification GET, HEAD and POST bound proofs before rendering or forwarding", async () => {
  for (const method of ["GET", "HEAD", "POST"]) {
    for (const length of [1024, 1025]) {
      const env = createTestEnv();
      const sessionAccess = trackSessionAccess(env);
      const token = "x".repeat(length);
      const url = new URL("http://localhost:3000/auth/verify-email");
      const form = registrationForm();
      form.set("token", token);
      if (method !== "POST") {
        url.searchParams.set("token", token);
      }
      const response = await worker.fetch(new Request(url, {
        method,
        headers: { origin: url.origin },
        body: method === "POST" ? form : undefined,
      }), env);
      assert.equal(response.status, length === 1024 ? 200 : 400);
      assertRegistrationHeaders(response);
      const body = await response.text();
      if (method === "HEAD") {
        assert.equal(body, "");
      }
      if (method === "GET" && length === 1024) {
        assert.ok(body.includes(`name="token" type="hidden" value="${token}"`));
      } else {
        assert.equal(body.includes(token), false);
      }
      if (method === "POST" && length === 1024) {
        assert.equal(env.irongateRequests.length, 1);
        assert.equal(JSON.parse(env.irongateRequests[0]?.body ?? "{}").token, token);
      } else {
        assert.deepEqual(env.irongateRequests, []);
      }
      assert.deepEqual(sessionAccess, []);
    }
  }
});

test("verification POST forwards only proof and chosen password without logging in", async () => {
  const env = createTestEnv();
  const sessionAccess = trackSessionAccess(env);
  const password = "  correct horse battery staple  ";
  const response = await worker.fetch(
    new Request("http://localhost:3000/auth/verify-email", {
      method: "POST",
      headers: { origin: "http://localhost:3000" },
      body: new URLSearchParams({
        token: "verify-token",
        new_password: password,
        password_confirmation: password,
        email: "ignored@example.com",
      }),
    }),
    env,
  );

  assert.equal(response.status, 200);
  assertRegistrationHeaders(response);
  assert.equal(env.irongateRequests.length, 1);
  assert.equal(env.irongateRequests[0]?.url, "https://auth.example.com/password/verify");
  assert.equal(env.irongateRequests[0]?.method, "POST");
  assert.deepEqual(JSON.parse(env.irongateRequests[0]?.body ?? "{}"), {
    token: "verify-token",
    new_password: password,
  });
  const body = await response.text();
  assert.match(body, /href="\/auth\/login"/);
  assert.doesNotMatch(body, /verify-token|correct horse battery staple|user_test/);
  assert.deepEqual(sessionAccess, []);
});

test("registration and verification POSTs require a single serialized same-origin Origin", async (t) => {
  const origins = [
    undefined, "", "null", "https://foreign.example", "https://localhost:3000",
    "http://localhost:3001", "not-an-origin", "localhost:3000", "http:localhost:3000",
    "http://localhost:3000/", "http://localhost:3000/path", "http://localhost:3000?query",
    "http://localhost:3000#fragment", "http://user:password@localhost:3000",
    "http://localhost:3000 http://localhost:3000", "http://localhost:3000, https://foreign.example",
    "http://localhost:3000\\", "http://localho\tst:3000",
  ];
  for (const path of ["/auth/register", "/auth/verify-email"]) {
    for (const origin of origins) {
      await t.test(`${path}: ${JSON.stringify(origin) ?? "missing"}`, async () => {
        const env = createTestEnv();
        const sessionAccess = trackSessionAccess(env);
        const response = await worker.fetch(
          new Request(`http://localhost:3000${path}`, {
            method: "POST",
            headers: origin === undefined ? {} : { origin },
            body: registrationForm(),
          }),
          env,
        );
        assert.equal(response.status, 403);
        assertRegistrationHeaders(response);
        assert.deepEqual(env.irongateRequests, []);
        assert.deepEqual(sessionAccess, []);
      });
    }
  }
});

test("configured web origin rejects alternate hosts, schemes and ports on both paths", async () => {
  const env = createTestEnv();
  const sessionAccess = trackSessionAccess(env);
  for (const path of ["/auth/register", "/auth/verify-email?token=verify-token"]) {
    for (const method of ["GET", "HEAD", "POST"]) {
      for (const origin of ["http://alternate.example:3000", "https://localhost:3000", "http://localhost:3001"]) {
        for (const submittedOrigin of [origin, "http://localhost:3000"]) {
          const response = await worker.fetch(
            new Request(`${origin}${path}`, {
              method,
              headers: { origin: submittedOrigin, "x-forwarded-host": "localhost:3000" },
              body: method === "POST" ? registrationForm() : undefined,
            }),
            env,
          );
          assert.equal(response.status, 403);
          assertRegistrationHeaders(response);
          assert.doesNotMatch(await response.text(), /verify-token/);
        }
      }
    }
  }
  assert.deepEqual(env.irongateRequests, []);
  assert.deepEqual(sessionAccess, []);
});

test("configured and inferred web origins accept only same-origin form POSTs", async () => {
  for (const { origin, configured } of [
    { origin: "https://web.example.test", configured: true },
    { origin: "https://preview.example.test:8443", configured: false },
    { origin: "http://localhost:3100", configured: false },
  ]) {
    for (const path of ["/auth/register", "/auth/verify-email"]) {
      const env = createTestEnv();
      if (configured) {
        env.WEB_BASE_URL = `${origin}/`;
      } else {
        delete env.WEB_BASE_URL;
      }
      const page = await worker.fetch(new Request(`${origin}${path}?token=verify-token`), env);
      assert.equal(page.status, 200);
      assertRegistrationHeaders(page);
      assert.deepEqual(env.irongateRequests, []);

      const rejected = await worker.fetch(new Request(`${origin}${path}`, {
        method: "POST",
        headers: { origin: "https://foreign.example" },
        body: registrationForm(),
      }), env);
      assert.equal(rejected.status, 403);
      assert.deepEqual(env.irongateRequests, []);

      const accepted = await worker.fetch(new Request(`${origin}${path}`, {
        method: "POST",
        headers: { origin },
        body: registrationForm(),
      }), env);
      assert.equal(accepted.status, 200);
      assertRegistrationHeaders(accepted);
      assert.equal(env.irongateRequests.length, 1);
    }
  }
});

test("invalid or mismatched form fields are rejected before upstream calls", async () => {
  for (const path of ["/auth/register", "/auth/verify-email"]) {
    const fields = path === "/auth/register" ? ["email"] : ["token", "new_password", "password_confirmation"];
    for (const field of fields) {
      for (const value of [undefined, "", new Blob(["private-file-value"])]) {
        const env = createTestEnv();
        const form = registrationForm();
        if (value === undefined) {
          form.delete(field);
        } else {
          form.set(field, value);
        }
        const response = await worker.fetch(new Request(`http://localhost:3000${path}?token=query-proof`, {
          method: "POST",
          headers: { origin: "http://localhost:3000" },
          body: form,
        }), env);
        assert.equal(response.status, 400);
        assertRegistrationHeaders(response);
        assert.doesNotMatch(await response.text(), /verify-token|chosen-password|query-proof|private-file-value/);
        assert.deepEqual(env.irongateRequests, []);
      }
    }
  }

  const env = createTestEnv();
  const form = registrationForm();
  form.set("password_confirmation", "different-secret");
  const response = await worker.fetch(new Request("http://localhost:3000/auth/verify-email", {
    method: "POST",
    headers: { origin: "http://localhost:3000" },
    body: form,
  }), env);
  assert.equal(response.status, 400);
  assertRegistrationHeaders(response);
  assert.doesNotMatch(await response.text(), /verify-token|chosen-password|different-secret/);
  assert.deepEqual(env.irongateRequests, []);
});

test("malformed form bodies fail safely and unsupported methods cannot complete setup", async () => {
  const env = createTestEnv();
  for (const path of ["/auth/register", "/auth/verify-email"]) {
    for (const method of ["POST", "PUT", "DELETE", "OPTIONS"]) {
      const response = await worker.fetch(new Request(`http://localhost:3000${path}`, {
        method,
        headers: { origin: "http://localhost:3000", "content-type": "text/plain" },
        body: "verify-token chosen-password",
      }), env);
      assert.equal(response.status, method === "POST" ? 400 : 405);
      assert.equal(response.headers.get("allow"), method === "POST" ? null : "GET, HEAD, POST");
      assertRegistrationHeaders(response);
      assert.doesNotMatch(await response.text(), /verify-token|chosen-password/);
    }
  }
  assert.deepEqual(env.irongateRequests, []);
});

test("upstream rejections and thrown failures never disclose proofs or passwords", async () => {
  for (const path of ["/auth/register", "/auth/verify-email"]) {
    for (const failure of [400, 429, 500, "throw", "throw-value"] as const) {
      const env = createTestEnv();
      const sessionAccess = trackSessionAccess(env);
      let calls = 0;
      env.__IRONGATE_FETCH = async () => {
        calls += 1;
        const secret = "verify-token chosen-password internal-error-secret";
        if (failure === "throw") {
          throw new Error(secret);
        }
        if (failure === "throw-value") {
          throw secret;
        }
        return new Response(secret, { status: failure });
      };
      const response = await worker.fetch(new Request(`http://localhost:3000${path}`, {
        method: "POST",
        headers: { origin: "http://localhost:3000" },
        body: registrationForm(),
      }), env);
      assert.equal(response.status, typeof failure === "number" ? failure : 500);
      assertRegistrationHeaders(response);
      assert.doesNotMatch(await response.text(), /verify-token|chosen-password|internal-error-secret/);
      assert.equal(calls, 1);
      assert.deepEqual(sessionAccess, []);
    }
  }
});

test("invalid configured URLs fail closed without disclosing their values", async () => {
  for (const [key, value] of [
    ["WEB_BASE_URL", "invalid-url-with-secret"],
    ["IRONGATE_ISSUER_URL", "invalid-url-with-secret"],
    ["WEB_BASE_URL", "ftp://localhost:3000/invalid-url-with-secret"],
    ["WEB_BASE_URL", "http://invalid-url-with-secret@localhost:3000"],
    ["WEB_BASE_URL", "http://:invalid-url-with-secret@localhost:3000"],
  ] as const) {
    for (const path of ["/auth/register", "/auth/verify-email"]) {
      const env = createTestEnv();
      env[key] = value;
      const response = await worker.fetch(new Request(`http://localhost:3000${path}`, {
        method: "POST",
        headers: { origin: "http://localhost:3000" },
        body: registrationForm(),
      }), env);
      assert.equal(response.status, 500);
      assertRegistrationHeaders(response);
      assert.doesNotMatch(await response.text(), /invalid-url-with-secret|verify-token|chosen-password/);
      assert.deepEqual(env.irongateRequests, []);
    }
  }
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

function registrationForm(): FormData {
  const form = new FormData();
  form.set("email", "user@example.com");
  form.set("token", "verify-token");
  form.set("new_password", "chosen-password");
  form.set("password_confirmation", "chosen-password");
  return form;
}

function assertRegistrationHeaders(response: Response): void {
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.equal(response.headers.get("referrer-policy"), "strict-origin");
  assert.equal(response.headers.get("x-content-type-options"), "nosniff");
  assert.match(response.headers.get("content-security-policy") ?? "", /form-action 'self'/);
  assert.match(response.headers.get("content-security-policy") ?? "", /base-uri 'none'/);
  assert.match(response.headers.get("content-security-policy") ?? "", /frame-ancestors 'none'/);
  assert.equal(response.headers.get("set-cookie"), null);
  assert.equal(response.headers.get("location"), null);
}

function trackSessionAccess(env: ReturnType<typeof createTestEnv>): string[] {
  const operations: string[] = [];
  env.__LOCAL_SESSION_STORE = {
    get: async () => { operations.push("get"); return null; },
    put: async () => { operations.push("put"); },
    delete: async () => { operations.push("delete"); },
  };
  return operations;
}

function extractCookie(response: Response, name: string): string {
  const setCookie = response.headers.get("set-cookie");
  assert.ok(setCookie, `missing set-cookie header for ${name}`);
  const match = setCookie.match(new RegExp(`${name}=[^;]+`));
  assert.ok(match, `missing ${name} cookie in ${setCookie}`);
  return match[0];
}
