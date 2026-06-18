import { examplesConfig } from "./config.js";
import { api } from "../auth/api.js";

const webConfig = examplesConfig.web;

export const webWorker =
  examplesConfig.enabled && webConfig.enabled
    ? new sst.cloudflare.Worker("ExampleWebWorker", {
        handler: "packages/examples/web/src/worker.ts",
        url: true,
        domain: webConfig.domain,
        environment: {
          IRONGATE_ISSUER_URL: api.url,
          IRONGATE_CLIENT_ID: webConfig.clientId,
          ...(webConfig.baseUrl ? { WEB_BASE_URL: webConfig.baseUrl } : {}),
        },
        migrations: [
          {
            tag: "v1",
            newSqliteClasses: ["WebSessionObject"],
          },
        ],
        transform: {
          worker(args: { bindings?: unknown }) {
            args.bindings = $output(args.bindings ?? []).apply((bindings) => [
              ...(bindings as unknown[]),
              {
                type: "durable_object_namespace",
                name: "SESSION_OBJECT",
                className: "WebSessionObject",
              },
            ]);
          },
        },
      })
    : undefined;

export const exampleOutputs = {
  ...(webWorker
    ? {
        ExampleWebUrl: webWorker.url,
        ...(webConfig.baseUrl ? { ExampleWebBaseUrl: webConfig.baseUrl } : {}),
      }
    : {}),
};
