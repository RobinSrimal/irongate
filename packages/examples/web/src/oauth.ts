import { randomString, sha256Base64Url } from "./crypto.js";

export interface PkcePair {
  verifier: string;
  challenge: string;
}

export interface AuthorizeUrlInput {
  issuerUrl: string;
  clientId: string;
  redirectUri: string;
  state: string;
  nonce: string;
  scope: string;
  codeChallenge: string;
  provider?: "password" | "google" | "apple";
}

export async function createPkcePair(): Promise<PkcePair> {
  const verifier = randomString(64);
  return {
    verifier,
    challenge: await sha256Base64Url(verifier),
  };
}

export function buildAuthorizeUrl(input: AuthorizeUrlInput): URL {
  const url = new URL("/authorize", normalizeBaseUrl(input.issuerUrl));
  url.searchParams.set("response_type", "code");
  url.searchParams.set("client_id", input.clientId);
  url.searchParams.set("redirect_uri", input.redirectUri);
  url.searchParams.set("scope", input.scope);
  url.searchParams.set("state", input.state);
  url.searchParams.set("nonce", input.nonce);
  url.searchParams.set("provider", input.provider ?? "password");
  url.searchParams.set("code_challenge", input.codeChallenge);
  url.searchParams.set("code_challenge_method", "S256");
  return url;
}

export function callbackUrl(webBaseUrl: string): string {
  return new URL("/auth/callback", normalizeBaseUrl(webBaseUrl)).toString();
}

export function normalizeBaseUrl(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}
