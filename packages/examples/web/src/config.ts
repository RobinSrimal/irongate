import type { WebEnv } from "./types.js";
import { normalizeBaseUrl } from "./oauth.js";

export interface WebConfig {
  issuerUrl: string;
  clientId: string;
  webBaseUrl: string;
  scope: string;
  googleLoginEnabled: boolean;
  appleLoginEnabled: boolean;
}

export function loadConfig(env: WebEnv, requestOrigin: string): WebConfig {
  return {
    issuerUrl: normalizeBaseUrl(env.IRONGATE_ISSUER_URL ?? "https://auth.example.com"),
    clientId: env.IRONGATE_CLIENT_ID ?? "web",
    webBaseUrl: normalizeBaseUrl(env.WEB_BASE_URL ?? requestOrigin),
    scope: "openid email offline_access",
    googleLoginEnabled: env.IRONGATE_GOOGLE_LOGIN_ENABLED === "true",
    appleLoginEnabled: env.IRONGATE_APPLE_LOGIN_ENABLED === "true",
  };
}
