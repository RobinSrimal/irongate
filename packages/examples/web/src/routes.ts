import { loadConfig } from "./config.js";
import { randomSessionId, randomString } from "./crypto.js";
import {
  exchangeAuthorizationCode,
  fetchUserInfo,
  irongateFetch,
  postForm,
  postJson,
  revokeRefreshToken,
} from "./irongate.js";
import {
  buildAuthorizeUrl,
  callbackUrl,
  createPkcePair,
} from "./oauth.js";
import {
  appSessionCookieName,
  buildClearCookie,
  buildSessionCookie,
  getSessionStore,
  loginSessionCookieName,
  parseCookies,
} from "./session.js";
import type { AppSession, LoginTransaction, WebEnv } from "./types.js";
import {
  errorView,
  homeView,
  loginView,
  messageView,
  page,
  registerView,
  signedInView,
} from "./views.js";

const loginMaxAgeSeconds = 600;
const appSessionMaxAgeSeconds = 60 * 60 * 24 * 30;

export async function handleRequest(request: Request, env: WebEnv): Promise<Response> {
  const url = new URL(request.url);
  const requestOrigin = url.origin;

  try {
    if (request.method === "GET" && url.pathname === "/health") {
      return Response.json({ status: "ok" });
    }
    if (request.method === "GET" && url.pathname === "/") {
      return homeView();
    }
    if (request.method === "GET" && url.pathname === "/auth/register") {
      return registerView();
    }
    if (request.method === "POST" && url.pathname === "/auth/register") {
      return handleRegister(request, env, requestOrigin);
    }
    if (request.method === "GET" && url.pathname === "/auth/verify-email") {
      return handleVerifyEmail(url, env, requestOrigin);
    }
    if (request.method === "GET" && url.pathname === "/auth/login") {
      return handleLoginStart(env, requestOrigin);
    }
    if (request.method === "POST" && url.pathname === "/auth/login") {
      return handlePasswordLogin(request, env, requestOrigin);
    }
    if (
      (request.method === "GET" || request.method === "POST") &&
      url.pathname === "/auth/login/google"
    ) {
      return handleExternalProviderLogin(env, requestOrigin, "google");
    }
    if (request.method === "GET" && url.pathname === "/auth/login/apple") {
      return handleExternalProviderLogin(env, requestOrigin, "apple");
    }
    if (request.method === "GET" && url.pathname === "/auth/callback") {
      return handleCallback(request, url, env, requestOrigin);
    }
    if (request.method === "GET" && url.pathname === "/app") {
      return handleApp(request, env, requestOrigin);
    }
    if (request.method === "POST" && url.pathname === "/auth/logout") {
      return handleLogout(request, env, requestOrigin);
    }

    return new Response("not found", { status: 404 });
  } catch (error) {
    const message = error instanceof Error ? error.message : "unexpected error";
    return errorView(message, 500);
  }
}

async function handleRegister(
  request: Request,
  env: WebEnv,
  requestOrigin: string,
): Promise<Response> {
  const form = await request.formData();
  const email = readFormString(form, "email");
  const password = readFormString(form, "password");
  const config = loadConfig(env, requestOrigin);
  const response = await postJson(env, new URL("/password/register", config.issuerUrl).toString(), {
    email,
    password,
  });

  if (!response.ok) {
    return errorView("Registration failed", response.status);
  }

  return messageView("Check your email", "Open the verification link to finish registration.");
}

async function handleVerifyEmail(
  url: URL,
  env: WebEnv,
  requestOrigin: string,
): Promise<Response> {
  const token = url.searchParams.get("token");
  if (!token) {
    return errorView("Missing verification token");
  }

  const config = loadConfig(env, requestOrigin);
  const response = await postJson(env, new URL("/password/verify", config.issuerUrl).toString(), {
    token,
  });

  if (!response.ok) {
    return errorView("Email verification failed", response.status);
  }

  return messageView("Email verified", "You can now sign in.");
}

async function handleLoginStart(env: WebEnv, requestOrigin: string): Promise<Response> {
  const config = loadConfig(env, requestOrigin);
  const transaction = await createLoginTransaction();
  const authorizeUrl = buildAuthorizeUrl({
    issuerUrl: config.issuerUrl,
    clientId: config.clientId,
    redirectUri: callbackUrl(config.webBaseUrl),
    scope: config.scope,
    state: transaction.record.state,
    nonce: transaction.record.nonce,
    codeChallenge: transaction.pkceChallenge,
  });
  const authorizeResponse = await irongateFetch(env, authorizeUrl, { redirect: "manual" });
  const location = authorizeResponse.headers.get("location");
  if (!location || authorizeResponse.status < 300 || authorizeResponse.status >= 400) {
    return errorView("Could not start Irongate authorize flow", 502);
  }

  const authorizeSession = readAuthorizeSessionFromLocation(location, config.issuerUrl);
  await getSessionStore(env).put(transaction.loginId, {
    ...transaction.record,
    authorizeSession,
  });

  return page(
    "Sign in",
    loginView({
      googleLoginEnabled: config.googleLoginEnabled,
      appleLoginEnabled: config.appleLoginEnabled,
    }),
    {
      headers: {
        "set-cookie": buildSessionCookie({
          name: loginSessionCookieName,
          value: transaction.loginId,
          maxAgeSeconds: loginMaxAgeSeconds,
        }),
      },
    },
  );
}

async function handleExternalProviderLogin(
  env: WebEnv,
  requestOrigin: string,
  provider: "google" | "apple",
): Promise<Response> {
  const config = loadConfig(env, requestOrigin);
  if (provider === "google" && !config.googleLoginEnabled) {
    return errorView("Google login is not enabled", 404);
  }
  if (provider === "apple" && !config.appleLoginEnabled) {
    return errorView("Apple login is not enabled", 404);
  }

  const transaction = await createLoginTransaction();
  await getSessionStore(env).put(transaction.loginId, transaction.record);
  const authorizeUrl = buildAuthorizeUrl({
    issuerUrl: config.issuerUrl,
    clientId: config.clientId,
    redirectUri: callbackUrl(config.webBaseUrl),
    scope: config.scope,
    state: transaction.record.state,
    nonce: transaction.record.nonce,
    codeChallenge: transaction.pkceChallenge,
    provider,
  });

  return new Response(null, {
    status: 303,
    headers: {
      location: authorizeUrl.toString(),
      "set-cookie": buildSessionCookie({
        name: loginSessionCookieName,
        value: transaction.loginId,
        maxAgeSeconds: loginMaxAgeSeconds,
      }),
    },
  });
}

async function handlePasswordLogin(
  request: Request,
  env: WebEnv,
  requestOrigin: string,
): Promise<Response> {
  const cookies = parseCookies(request.headers.get("cookie"));
  const loginId = cookies.get(loginSessionCookieName);
  if (!loginId) {
    return errorView("Missing login transaction");
  }

  const store = getSessionStore(env);
  const record = await store.get(loginId);
  if (!record || record.kind !== "login") {
    return errorView("Invalid login transaction");
  }
  if (!record.authorizeSession) {
    return errorView("Invalid password login transaction");
  }

  const form = await request.formData();
  const config = loadConfig(env, requestOrigin);
  const response = await postForm(
    env,
    new URL("/password/login", config.issuerUrl).toString(),
    new URLSearchParams({
      session: record.authorizeSession,
      email: readFormString(form, "email"),
      password: readFormString(form, "password"),
    }),
  );
  const location = response.headers.get("location");
  if (!location || response.status < 300 || response.status >= 400) {
    return errorView("Sign in failed", response.status || 400);
  }

  return Response.redirect(location, 303);
}

async function handleCallback(
  request: Request,
  url: URL,
  env: WebEnv,
  requestOrigin: string,
): Promise<Response> {
  const cookies = parseCookies(request.headers.get("cookie"));
  const loginId = cookies.get(loginSessionCookieName);
  if (!loginId) {
    return errorView("Missing login transaction");
  }

  const store = getSessionStore(env);
  const record = await store.get(loginId);
  if (!record || record.kind !== "login") {
    return errorView("Invalid login transaction");
  }

  const state = url.searchParams.get("state");
  const code = url.searchParams.get("code");
  if (!state || state !== record.state || !code) {
    return errorView("Invalid OAuth callback");
  }

  const config = loadConfig(env, requestOrigin);
  const tokenResponse = await exchangeAuthorizationCode({
    env,
    issuerUrl: config.issuerUrl,
    clientId: config.clientId,
    redirectUri: callbackUrl(config.webBaseUrl),
    code,
    codeVerifier: record.codeVerifier,
  });
  const userinfo = await fetchUserInfo({
    env,
    issuerUrl: config.issuerUrl,
    accessToken: tokenResponse.access_token,
  });
  const sessionId = randomSessionId();
  const appSession: AppSession = {
    kind: "app",
    accessToken: tokenResponse.access_token,
    refreshToken: tokenResponse.refresh_token,
    idToken: tokenResponse.id_token,
    tokenType: tokenResponse.token_type,
    scope: tokenResponse.scope,
    createdAt: Date.now(),
    expiresAt: Date.now() + tokenResponse.expires_in * 1000,
    userinfo,
  };

  await store.put(sessionId, appSession);
  await store.delete(loginId);

  const headers = new Headers({
    location: new URL("/app", config.webBaseUrl).toString(),
  });
  headers.append(
    "set-cookie",
    buildSessionCookie({
      name: appSessionCookieName,
      value: sessionId,
      maxAgeSeconds: appSessionMaxAgeSeconds,
    }),
  );
  headers.append("set-cookie", buildClearCookie(loginSessionCookieName));

  return new Response(null, { status: 303, headers });
}

async function handleApp(
  request: Request,
  env: WebEnv,
  requestOrigin: string,
): Promise<Response> {
  const cookies = parseCookies(request.headers.get("cookie"));
  const sessionId = cookies.get(appSessionCookieName);
  if (!sessionId) {
    return Response.redirect(new URL("/auth/login", loadConfig(env, requestOrigin).webBaseUrl), 303);
  }

  const record = await getSessionStore(env).get(sessionId);
  if (!record || record.kind !== "app") {
    return Response.redirect(new URL("/auth/login", loadConfig(env, requestOrigin).webBaseUrl), 303);
  }

  return signedInView(record.userinfo);
}

async function handleLogout(
  request: Request,
  env: WebEnv,
  requestOrigin: string,
): Promise<Response> {
  const config = loadConfig(env, requestOrigin);
  const cookies = parseCookies(request.headers.get("cookie"));
  const sessionId = cookies.get(appSessionCookieName);
  const store = getSessionStore(env);
  const record = sessionId ? await store.get(sessionId) : null;
  if (record?.kind === "app" && record.refreshToken) {
    await revokeRefreshToken({
      env,
      issuerUrl: config.issuerUrl,
      clientId: config.clientId,
      refreshToken: record.refreshToken,
    });
  }
  if (sessionId) {
    await store.delete(sessionId);
  }

  return new Response(null, {
    status: 303,
    headers: {
      location: new URL("/", config.webBaseUrl).toString(),
      "set-cookie": buildClearCookie(appSessionCookieName),
    },
  });
}

function readFormString(form: FormData, name: string): string {
  const value = form.get(name);
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${name} is required`);
  }
  return value;
}

async function createLoginTransaction(): Promise<{
  loginId: string;
  pkceChallenge: string;
  record: LoginTransaction;
}> {
  const pkce = await createPkcePair();
  const now = Date.now();
  const record: LoginTransaction = {
    kind: "login",
    state: randomString(32),
    nonce: randomString(32),
    codeVerifier: pkce.verifier,
    createdAt: now,
    expiresAt: now + loginMaxAgeSeconds * 1000,
  };
  const loginId = randomSessionId();

  return {
    loginId,
    pkceChallenge: pkce.challenge,
    record,
  };
}

function readAuthorizeSessionFromLocation(location: string, issuerUrl: string): string {
  const url = new URL(location, issuerUrl);
  const session = url.searchParams.get("session");
  if (!session) {
    throw new Error("authorize redirect did not include password session");
  }
  return session;
}
