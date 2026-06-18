import type {
  DurableObjectNamespaceLike,
  DurableObjectStateLike,
  SessionRecord,
  SessionStore,
  WebEnv,
} from "./types.js";

const sessionStorageKey = "session";

export const appSessionCookieName = "__Host-irongate_web_session";
export const loginSessionCookieName = "__Host-irongate_web_login";

export class MemorySessionStore implements SessionStore {
  private readonly records = new Map<string, SessionRecord>();

  async get(id: string): Promise<SessionRecord | null> {
    return this.records.get(id) ?? null;
  }

  async put(id: string, record: SessionRecord): Promise<void> {
    this.records.set(id, record);
  }

  async delete(id: string): Promise<void> {
    this.records.delete(id);
  }
}

export class DurableObjectSessionStore implements SessionStore {
  constructor(private readonly namespace: DurableObjectNamespaceLike) {}

  async get(id: string): Promise<SessionRecord | null> {
    const response = await this.stub(id).fetch("https://session.local/session");
    if (response.status === 404) {
      return null;
    }
    if (!response.ok) {
      throw new Error(`session store read failed: ${response.status}`);
    }
    return (await response.json()) as SessionRecord;
  }

  async put(id: string, record: SessionRecord): Promise<void> {
    const response = await this.stub(id).fetch("https://session.local/session", {
      method: "PUT",
      body: JSON.stringify(record),
      headers: { "content-type": "application/json" },
    });
    if (!response.ok) {
      throw new Error(`session store write failed: ${response.status}`);
    }
  }

  async delete(id: string): Promise<void> {
    const response = await this.stub(id).fetch("https://session.local/session", {
      method: "DELETE",
    });
    if (!response.ok && response.status !== 404) {
      throw new Error(`session store delete failed: ${response.status}`);
    }
  }

  private stub(id: string) {
    return this.namespace.get(this.namespace.idFromName(id));
  }
}

export class WebSessionObject {
  constructor(private readonly state: DurableObjectStateLike) {}

  async fetch(request: Request): Promise<Response> {
    if (request.method === "GET") {
      const record = await this.state.storage.get<SessionRecord>(sessionStorageKey);
      if (!record) {
        return new Response("not found", { status: 404 });
      }
      return Response.json(record);
    }

    if (request.method === "PUT") {
      const record = (await request.json()) as SessionRecord;
      await this.state.storage.put(sessionStorageKey, record);
      return new Response(null, { status: 204 });
    }

    if (request.method === "DELETE") {
      await this.state.storage.delete(sessionStorageKey);
      return new Response(null, { status: 204 });
    }

    return new Response("method not allowed", { status: 405 });
  }
}

export function getSessionStore(env: WebEnv): SessionStore {
  if (env.__LOCAL_SESSION_STORE) {
    return env.__LOCAL_SESSION_STORE;
  }
  if (env.SESSION_OBJECT) {
    return new DurableObjectSessionStore(env.SESSION_OBJECT);
  }
  throw new Error("SESSION_OBJECT binding is required");
}

export function buildSessionCookie(input: {
  name: string;
  value: string;
  maxAgeSeconds: number;
}): string {
  return [
    `${input.name}=${encodeURIComponent(input.value)}`,
    "Path=/",
    `Max-Age=${input.maxAgeSeconds}`,
    "HttpOnly",
    "Secure",
    "SameSite=Lax",
  ].join("; ");
}

export function buildClearCookie(name: string): string {
  return [
    `${name}=`,
    "Path=/",
    "Max-Age=0",
    "HttpOnly",
    "Secure",
    "SameSite=Lax",
  ].join("; ");
}

export function parseCookies(header: string | null): Map<string, string> {
  const cookies = new Map<string, string>();
  if (!header) {
    return cookies;
  }

  for (const part of header.split(";")) {
    const [rawName, ...rawValue] = part.trim().split("=");
    if (!rawName) {
      continue;
    }
    cookies.set(rawName, decodeURIComponent(rawValue.join("=")));
  }

  return cookies;
}
