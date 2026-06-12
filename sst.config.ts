/// <reference path="./.sst/platform/config.d.ts" />

const appName = "irongate";
const productionStage = "production";

function awsProfileForStage(stage?: string) {
  if (stage === productionStage) {
    return process.env.SST_PROD_AWS_PROFILE ?? `${appName}-prod`;
  }

  return process.env.SST_DEV_AWS_PROFILE ?? `${appName}-dev`;
}

export default $config({
  app(input) {
    return {
      name: appName,
      home: "aws",
      providers: {
        aws: {
          profile: awsProfileForStage(input?.stage),
        },
      },
      removal: input?.stage === productionStage ? "retain" : "remove",
      protect: input?.stage === productionStage,
    };
  },
  async run() {
    const storage = await import("./infra/storage.js");
    const api = await import("./infra/api.js");

    return {
      ApiUrl: api.api.url,
      TableName: storage.table.name,
    };
  },
});
