export interface CapturedIrongateRequest {
  url: string;
  method: string;
  body?: string;
}

export interface DurableObjectIdLike {}

export interface DurableObjectStubLike {
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
}

export interface DurableObjectNamespaceLike {
  idFromName(name: string): DurableObjectIdLike;
  get(id: DurableObjectIdLike): DurableObjectStubLike;
}

export interface DurableObjectStorageLike {
  get<T>(key: string): Promise<T | undefined>;
  put<T>(key: string, value: T): Promise<void>;
  delete(key: string): Promise<boolean>;
}

export interface DurableObjectStateLike {
  storage: DurableObjectStorageLike;
}

export interface WebEnv {
  IRONGATE_ISSUER_URL?: string;
  IRONGATE_CLIENT_ID?: string;
  IRONGATE_GOOGLE_LOGIN_ENABLED?: string;
  IRONGATE_APPLE_LOGIN_ENABLED?: string;
  WEB_BASE_URL?: string;
  SESSION_OBJECT?: DurableObjectNamespaceLike;
  __LOCAL_SESSION_STORE?: SessionStore;
  __IRONGATE_FETCH?: typeof fetch;
  irongateRequests?: CapturedIrongateRequest[];
}

export interface LoginTransaction {
  kind: "login";
  state: string;
  nonce: string;
  codeVerifier: string;
  authorizeSession?: string;
  createdAt: number;
  expiresAt: number;
}

export interface UserInfo {
  sub?: string;
  email?: string;
  email_verified?: boolean;
  name?: string;
  [claim: string]: unknown;
}

export interface AppSession {
  kind: "app";
  accessToken: string;
  refreshToken?: string;
  idToken?: string;
  tokenType: string;
  scope?: string;
  expiresAt: number;
  createdAt: number;
  userinfo?: UserInfo;
}

export type SessionRecord = LoginTransaction | AppSession;

export interface SessionStore {
  get(id: string): Promise<SessionRecord | null>;
  put(id: string, record: SessionRecord): Promise<void>;
  delete(id: string): Promise<void>;
}
