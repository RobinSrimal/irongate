export type Provider = "password" | "google" | "apple";

export interface AuthorizeUrlInput {
  issuerUrl: string;
  clientId: string;
  redirectUri: string;
  scope: string;
  state: string;
  nonce: string;
  codeChallenge: string;
  provider: Provider;
}

export function buildAuthorizeUrl(input: AuthorizeUrlInput): URL {
  const url = new URL("/authorize", input.issuerUrl);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("client_id", input.clientId);
  url.searchParams.set("redirect_uri", input.redirectUri);
  url.searchParams.set("scope", input.scope);
  url.searchParams.set("state", input.state);
  url.searchParams.set("nonce", input.nonce);
  url.searchParams.set("provider", input.provider);
  url.searchParams.set("code_challenge", input.codeChallenge);
  url.searchParams.set("code_challenge_method", "S256");
  return url;
}

export function parseAuthorizeSessionRedirect(location: string, issuerUrl: string): string {
  const url = new URL(location, issuerUrl);
  const session = url.searchParams.get("session");

  if (!session || !isIrongateAuthorizeSessionPath(url.pathname)) {
    throw new Error("authorize session redirect did not include an authorize session");
  }

  return session;
}

function isIrongateAuthorizeSessionPath(pathname: string): boolean {
  return pathname === "/password/login" || pathname === "/google/authorize" || pathname === "/apple/authorize";
}
