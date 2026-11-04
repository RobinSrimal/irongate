import type { LogRetention } from "./config.js";

/** Existing SST log group/role, finite shared retention and unfiltered INFO audit output. */
export function auditFunctionLogging(retention: LogRetention) {
  return {
    logging: { retention, format: "json" },
    transform: {
      function(args: aws.lambda.FunctionArgs) {
        // SST exposes format/retention but not Lambda's application filter directly.
        // Preserve SST's resolved log group and any other logging configuration.
        args.loggingConfig = $util.output(args.loggingConfig).apply((config) => ({
          ...config,
          logFormat: "JSON",
          applicationLogLevel: "INFO",
        }));
      },
    },
  } satisfies Pick<sst.aws.FunctionArgs, "logging" | "transform">;
}
