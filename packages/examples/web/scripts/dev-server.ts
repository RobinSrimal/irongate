import { createServer } from "node:http";

import { MemorySessionStore } from "../src/session.js";
import worker from "../src/worker.js";
import type { WebEnv } from "../src/types.js";

const port = Number.parseInt(process.env.PORT ?? "3000", 10);
const env: WebEnv = {
  IRONGATE_ISSUER_URL: requiredEnv("IRONGATE_ISSUER_URL"),
  IRONGATE_CLIENT_ID: process.env.IRONGATE_CLIENT_ID ?? "web",
  WEB_BASE_URL: process.env.WEB_BASE_URL,
  __LOCAL_SESSION_STORE: new MemorySessionStore(),
};

const server = createServer(async (incoming, outgoing) => {
  const chunks: Buffer[] = [];
  for await (const chunk of incoming) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }

  const request = new Request(new URL(incoming.url ?? "/", env.WEB_BASE_URL), {
    method: incoming.method,
    headers: incoming.headers as HeadersInit,
    body:
      incoming.method === "GET" || incoming.method === "HEAD"
        ? undefined
        : Buffer.concat(chunks),
  });
  const response = await worker.fetch(request, env);
  outgoing.writeHead(response.status, Object.fromEntries(response.headers.entries()));
  if (response.body) {
    const body = Buffer.from(await response.arrayBuffer());
    outgoing.end(body);
  } else {
    outgoing.end();
  }
});

server.listen(port, () => {
  console.log(`Irongate web example listening on http://localhost:${port}`);
});

function requiredEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}
