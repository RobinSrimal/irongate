import { handleRequest } from "./routes.js";
import { WebSessionObject } from "./session.js";
import type { WebEnv } from "./types.js";

export { WebSessionObject };

export default {
  fetch(request: Request, env: WebEnv): Promise<Response> {
    return handleRequest(request, env);
  },
};
