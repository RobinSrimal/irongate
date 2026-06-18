import type { UserInfo, WebEnv } from "./types.js";

export interface TokenResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
  refresh_token?: string;
  id_token?: string;
  scope?: string;
}

export async function irongateFetch(
  env: WebEnv,
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  const fetcher = env.__IRONGATE_FETCH ?? fetch;
  return fetcher(input, init);
}

export async function postJson(
  env: WebEnv,
  url: string,
  body: unknown,
): Promise<Response> {
  return irongateFetch(env, url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

export async function postForm(
  env: WebEnv,
  url: string,
  body: URLSearchParams,
  redirect: RequestRedirect = "manual",
): Promise<Response> {
  return irongateFetch(env, url, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body,
    redirect,
  });
}

export async function exchangeAuthorizationCode(input: {
  env: WebEnv;
  issuerUrl: string;
  clientId: string;
  redirectUri: string;
  code: string;
  codeVerifier: string;
}): Promise<TokenResponse> {
  const response = await postForm(
    input.env,
    new URL("/token", input.issuerUrl).toString(),
    new URLSearchParams({
      grant_type: "authorization_code",
      client_id: input.clientId,
      redirect_uri: input.redirectUri,
      code: input.code,
      code_verifier: input.codeVerifier,
    }),
  );
  if (!response.ok) {
    throw new Error(`token exchange failed: ${response.status}`);
  }
  return (await response.json()) as TokenResponse;
}

export async function fetchUserInfo(input: {
  env: WebEnv;
  issuerUrl: string;
  accessToken: string;
}): Promise<UserInfo | undefined> {
  const response = await irongateFetch(input.env, new URL("/userinfo", input.issuerUrl), {
    headers: { authorization: `Bearer ${input.accessToken}` },
  });
  if (!response.ok) {
    return undefined;
  }
  return (await response.json()) as UserInfo;
}

export async function revokeRefreshToken(input: {
  env: WebEnv;
  issuerUrl: string;
  clientId: string;
  refreshToken: string;
}): Promise<void> {
  await postForm(
    input.env,
    new URL("/oauth/revoke", input.issuerUrl).toString(),
    new URLSearchParams({
      token: input.refreshToken,
      token_type_hint: "refresh_token",
      client_id: input.clientId,
    }),
  );
}
