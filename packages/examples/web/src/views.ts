import type { UserInfo } from "./types.js";

export function page(title: string, body: string, init: ResponseInit = {}): Response {
  const headers = new Headers(init.headers);
  headers.set("content-type", "text/html; charset=utf-8");
  headers.set("cache-control", "no-store");
  headers.set("x-content-type-options", "nosniff");
  headers.set("referrer-policy", "no-referrer");
  headers.set(
    "content-security-policy",
    "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
  );

  return new Response(
    `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)}</title>
</head>
<body>
  <main>
    ${body}
  </main>
</body>
</html>`,
    { ...init, headers },
  );
}

export function homeView(): Response {
  return page(
    "Irongate Web Example",
    `<h1>Irongate Web Example</h1>
    <p>Password-auth BFF example.</p>
    <p><a href="/auth/register">Register</a> <a href="/auth/login">Sign in</a></p>`,
  );
}

export function registerView(): Response {
  return page(
    "Register",
    `<h1>Register</h1>
    <form method="post" action="/auth/register">
      <label>Email <input name="email" type="email" autocomplete="email" required></label>
      <label>Password <input name="password" type="password" autocomplete="new-password" required></label>
      <button type="submit">Register</button>
    </form>`,
  );
}

export function loginView(options: {
  googleLoginEnabled: boolean;
  appleLoginEnabled: boolean;
}): string {
  const googleLogin = options.googleLoginEnabled
    ? `<p><a href="/auth/login/google">Continue with Google</a></p>`
    : "";
  const appleLogin = options.appleLoginEnabled
    ? `<p><a href="/auth/login/apple">Continue with Apple</a></p>`
    : "";

  return `<h1>Sign in</h1>
    <form method="post" action="/auth/login">
      <label>Email <input name="email" type="email" autocomplete="email" required></label>
      <label>Password <input name="password" type="password" autocomplete="current-password" required></label>
      <button type="submit">Sign in</button>
    </form>
    ${googleLogin}
    ${appleLogin}`;
}

export function signedInView(userinfo?: UserInfo): Response {
  const email = typeof userinfo?.email === "string" ? userinfo.email : "signed-in user";
  const subject = typeof userinfo?.sub === "string" ? userinfo.sub : "unknown subject";
  return page(
    "Signed in",
    `<h1>Signed in</h1>
    <p>Email: ${escapeHtml(email)}</p>
    <p>Subject: ${escapeHtml(subject)}</p>
    <form method="post" action="/auth/logout">
      <button type="submit">Logout</button>
    </form>`,
  );
}

export function messageView(title: string, message: string): Response {
  return page(title, `<h1>${escapeHtml(title)}</h1><p>${escapeHtml(message)}</p>`);
}

export function errorView(message: string, status = 400): Response {
  return page("Request failed", `<h1>Request failed</h1><p>${escapeHtml(message)}</p>`, {
    status,
  });
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
