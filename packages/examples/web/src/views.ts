import type { UserInfo } from "./types.js";

const assetVersion = "2026-06-22-brand-refresh";

export function page(title: string, body: string, init: ResponseInit = {}): Response {
  const headers = new Headers(init.headers);
  headers.set("content-type", "text/html; charset=utf-8");
  headers.set("cache-control", "no-store");
  headers.set("x-content-type-options", "nosniff");
  headers.set("referrer-policy", "no-referrer");
  headers.set(
    "content-security-policy",
    "default-src 'self'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
  );

  return new Response(
    `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(title)}</title>
  <link rel="stylesheet" href="/styles.css?v=${assetVersion}">
</head>
<body>
  ${body}
</body>
</html>`,
    { ...init, headers },
  );
}

export function stylesView(): Response {
  return new Response(styleSheet, {
    headers: {
      "content-type": "text/css; charset=utf-8",
      "cache-control": "no-cache",
      "x-content-type-options": "nosniff",
    },
  });
}

export function registerView(): Response {
  return page(
    "Register",
    shell({
      eyebrow: "Irongate web example",
      title: "Create account",
      badge: "Signed out",
      content: `<section class="login">
        <form class="password-form standalone" method="post" action="/auth/register">
          <label>
            Email
            <input name="email" type="email" autocomplete="email" required>
          </label>
          <label>
            Password
            <input name="password" type="password" autocomplete="new-password" required>
          </label>
          <button type="submit">Register</button>
        </form>
        <a class="button secondary" href="/auth/login">Sign in instead</a>
      </section>`,
    }),
  );
}

export function loginView(options: {
  googleLoginEnabled: boolean;
  appleLoginEnabled: boolean;
}): string {
  const googleLogin = options.googleLoginEnabled
    ? `<a class="button" href="/auth/login/google">Continue with Google</a>`
    : "";
  const appleLogin = options.appleLoginEnabled
    ? `<a class="button" href="/auth/login/apple">Continue with Apple</a>`
    : "";

  return shell({
    eyebrow: "Irongate web example",
    title: "Web sign in",
    badge: "Signed out",
    content: `<section class="login">
      <div class="provider-grid">
        ${googleLogin}
        ${appleLogin}
      </div>

      <form class="password-form" method="post" action="/auth/login">
        <label>
          Email
          <input name="email" type="email" autocomplete="email" required>
        </label>
        <label>
          Password
          <input name="password" type="password" autocomplete="current-password" required>
        </label>
        <button type="submit">Sign in with password</button>
      </form>
      <a class="button secondary" href="/auth/register">Create account</a>
    </section>`,
  });
}

export function signedInView(userinfo?: UserInfo): Response {
  const email = typeof userinfo?.email === "string" ? userinfo.email : "signed-in user";
  const subject = typeof userinfo?.sub === "string" ? userinfo.sub : "unknown subject";
  return page(
    "Signed in",
    shell({
      eyebrow: "Irongate web example",
      title: "Signed in",
      badge: "Active",
      badgeActive: true,
      content: `<section class="session">
        <dl>
          <div>
            <dt>Subject</dt>
            <dd>${escapeHtml(subject)}</dd>
          </div>
          <div>
            <dt>Email</dt>
            <dd>${escapeHtml(email)}</dd>
          </div>
        </dl>

        <form class="actions" method="post" action="/auth/logout">
          <button type="submit" class="secondary">Logout</button>
        </form>
      </section>`,
    }),
  );
}

export function messageView(title: string, message: string): Response {
  return page(
    title,
    shell({
      eyebrow: "Irongate web example",
      title,
      badge: "Status",
      content: `<p class="status">${escapeHtml(message)}</p>`,
    }),
  );
}

export function errorView(message: string, status = 400): Response {
  return page(
    "Request failed",
    shell({
      eyebrow: "Irongate web example",
      title: "Request failed",
      badge: "Error",
      content: `<p class="status">${escapeHtml(message)}</p>`,
    }),
    { status },
  );
}

function shell(options: {
  eyebrow: string;
  title: string;
  badge: string;
  badgeActive?: boolean;
  content: string;
}): string {
  const badgeClass = options.badgeActive ? "badge active" : "badge";
  return `<main class="shell">
    <section class="panel">
      <header class="header">
        <div>
          <p class="eyebrow">${escapeHtml(options.eyebrow)}</p>
          <h1>${escapeHtml(options.title)}</h1>
        </div>
        <span class="${badgeClass}">${escapeHtml(options.badge)}</span>
      </header>

      ${options.content}
    </section>
  </main>`;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

const styleSheet = `:root {
  color: #242a2f;
  background: #f3f2ef;
  font-family:
    Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
}

button,
input,
.button {
  font: inherit;
}

button,
.button {
  align-items: center;
  background: #2f363c;
  border: 0;
  border-radius: 8px;
  color: white;
  cursor: pointer;
  display: inline-flex;
  font-weight: 650;
  justify-content: center;
  min-height: 44px;
  padding: 0 16px;
  text-decoration: none;
}

button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

button.secondary,
.button.secondary {
  background: #ededeb;
  color: #242a2f;
}

input {
  background: #ffffff;
  border: 1px solid #d2d1cc;
  border-radius: 8px;
  min-height: 42px;
  padding: 8px 10px;
  width: 100%;
}

.shell {
  display: grid;
  min-height: 100vh;
  padding: 24px;
  place-items: center;
}

.panel {
  background: #ffffff;
  border: 1px solid #deddd8;
  border-radius: 8px;
  box-shadow: 0 22px 64px rgba(36, 42, 47, 0.14);
  padding: 24px;
  width: min(100%, 620px);
}

.header {
  align-items: start;
  display: flex;
  gap: 16px;
  justify-content: space-between;
  margin-bottom: 24px;
}

.eyebrow {
  color: #6b6a66;
  font-size: 0.8rem;
  font-weight: 700;
  letter-spacing: 0;
  margin: 0 0 4px;
  text-transform: uppercase;
}

h1 {
  font-size: 2rem;
  line-height: 1.1;
  margin: 0;
}

.badge {
  background: #f0efeb;
  border-radius: 999px;
  color: #696865;
  font-size: 0.85rem;
  font-weight: 700;
  padding: 6px 10px;
  white-space: nowrap;
}

.badge.active {
  background: #e1f0e8;
  color: #1f6649;
}

.login,
.session,
.password-form {
  display: grid;
  gap: 14px;
}

.provider-grid {
  display: grid;
  gap: 10px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.provider-grid:empty {
  display: none;
}

.password-form {
  border-top: 1px solid #ecebe7;
  padding-top: 14px;
}

.password-form.standalone {
  border-top: 0;
  padding-top: 0;
}

label {
  color: #30363b;
  display: grid;
  font-size: 0.92rem;
  font-weight: 650;
  gap: 6px;
}

dl {
  display: grid;
  gap: 12px;
  margin: 0;
}

dl div {
  border-bottom: 1px solid #ecebe7;
  display: grid;
  gap: 4px;
  padding-bottom: 10px;
}

dt {
  color: #6b6a66;
  font-size: 0.82rem;
  font-weight: 750;
  text-transform: uppercase;
}

dd {
  margin: 0;
  overflow-wrap: anywhere;
}

.actions {
  display: flex;
  gap: 10px;
  justify-content: flex-end;
  margin: 0;
}

.status {
  color: #6b6a66;
  margin: 18px 0 0;
  min-height: 24px;
  overflow-wrap: anywhere;
}

@media (max-width: 520px) {
  .provider-grid {
    grid-template-columns: 1fr;
  }

  .header {
    display: grid;
  }
}
`;
