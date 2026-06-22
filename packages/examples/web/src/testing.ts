import { MemorySessionStore } from "./session.js";
import type { CapturedIrongateRequest, WebEnv } from "./types.js";

export interface TestEnv extends WebEnv {
  irongateRequests: CapturedIrongateRequest[];
}

export interface TestEnvOptions {
  googleLoginEnabled?: boolean;
  appleLoginEnabled?: boolean;
}

export function createTestEnv(options: TestEnvOptions = {}): TestEnv {
  const irongateRequests: CapturedIrongateRequest[] = [];
  let lastAuthorizeState = "state";
  return {
    IRONGATE_ISSUER_URL: "https://auth.example.com",
    IRONGATE_CLIENT_ID: "web",
    IRONGATE_GOOGLE_LOGIN_ENABLED: options.googleLoginEnabled ? "true" : "false",
    IRONGATE_APPLE_LOGIN_ENABLED: options.appleLoginEnabled ? "true" : "false",
    WEB_BASE_URL: "http://localhost:3000",
    __LOCAL_SESSION_STORE: new MemorySessionStore(),
    irongateRequests,
    __IRONGATE_FETCH: async (input, init) => {
      const request = new Request(input, init);
      const body = await request.clone().text();
      irongateRequests.push({
        url: request.url,
        method: request.method,
        body: body.length > 0 ? body : undefined,
      });

      const url = new URL(request.url);
      if (url.pathname === "/authorize") {
        lastAuthorizeState = url.searchParams.get("state") ?? lastAuthorizeState;
        return new Response(null, {
          status: 302,
          headers: { location: "/password/login?session=irongate-session-123" },
        });
      }
      if (url.pathname === "/password/register") {
        return Response.json({ status: "pending_verification" });
      }
      if (url.pathname === "/password/verify") {
        return Response.json({
          status: "verified",
          subject: "user_test",
        });
      }
      if (url.pathname === "/password/login") {
        return new Response(null, {
          status: 303,
          headers: {
            location: `http://localhost:3000/auth/callback?code=code-123&state=${lastAuthorizeState}`,
          },
        });
      }
      if (url.pathname === "/token") {
        return Response.json({
          access_token: "access-token",
          token_type: "Bearer",
          expires_in: 900,
          refresh_token: "refresh-token",
          id_token: "id-token",
          scope: "openid email offline_access",
        });
      }
      if (url.pathname === "/userinfo") {
        return Response.json({
          sub: "user_test",
          email: "user@example.com",
          email_verified: true,
        });
      }
      if (url.pathname === "/oauth/revoke") {
        return new Response(null, { status: 200 });
      }

      return new Response("not found", { status: 404 });
    },
  };
}
