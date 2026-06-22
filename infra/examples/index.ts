import { examplesConfig } from "./config.js";
import { api } from "../auth/api.js";
import { stageConfig } from "../shared/stage-config.js";

const webConfig = examplesConfig.web;
const webDomain = optionalStageValue(webConfig.domain);
const webBaseUrl = optionalStageValue(webConfig.baseUrl);
const googleLoginEnabled = optionalStageValue(stageConfig.auth.googleClientId) ? "true" : "false";

export const webWorker =
  examplesConfig.enabled && webConfig.enabled
    ? new sst.cloudflare.Worker("ExampleWebWorker", {
        handler: "packages/examples/web/src/worker.ts",
        url: true,
        domain: webDomain,
        environment: {
          IRONGATE_ISSUER_URL: api.url,
          IRONGATE_CLIENT_ID: webConfig.clientId,
          IRONGATE_GOOGLE_LOGIN_ENABLED: googleLoginEnabled,
          IRONGATE_APPLE_LOGIN_ENABLED: stageConfig.auth.apple.enabled ? "true" : "false",
          ...(webBaseUrl ? { WEB_BASE_URL: webBaseUrl } : {}),
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
        ...(webBaseUrl ? { ExampleWebBaseUrl: webBaseUrl } : {}),
      }
    : {}),
};

function optionalStageValue(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}
