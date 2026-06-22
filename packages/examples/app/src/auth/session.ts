export type Provider = "google" | "apple";

export interface UserInfo {
  sub?: string;
  email?: string;
  email_verified?: boolean;
  provider?: string;
  [claim: string]: unknown;
}

export interface AppSession {
  token_type: string;
  expires_in: number;
  scope?: string;
  access_token: string;
  id_token?: string;
  userinfo?: UserInfo;
}

export interface StoredSessionStatus {
  has_refresh_token: boolean;
}

export function displaySubject(session: AppSession): string {
  return session.userinfo?.sub ?? "unknown user";
}

export function displayEmail(session: AppSession): string {
  return session.userinfo?.email ?? "no email claim";
}
