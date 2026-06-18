/// <reference path="./.sst/platform/config.d.ts" />

const appName = "irongate";
const developmentStage = "dev";
const productionStage = "production";

function assertConfiguredStage(stage?: string) {
  if (stage === "prod") {
    throw new Error('Unsupported stage "prod". Use "--stage production" for production deploys.');
  }

  const resolvedStage = stage ?? developmentStage;

  if (resolvedStage === developmentStage || resolvedStage === productionStage) {
    return resolvedStage;
  }

  throw new Error(
    `Unsupported stage "${resolvedStage}". Supported stages are "${developmentStage}" and "${productionStage}".`,
  );
}

function awsProfileForStage(stage?: string) {
  const configuredStage = assertConfiguredStage(stage);

  if (configuredStage === productionStage) {
    return process.env.SST_PROD_AWS_PROFILE ?? `${appName}-prod`;
  }

  return process.env.SST_DEV_AWS_PROFILE ?? `${appName}-dev`;
}

export default $config({
  app(input) {
    const stage = assertConfiguredStage(input?.stage);

    return {
      name: appName,
      home: "aws",
      providers: {
        aws: {
          profile: awsProfileForStage(stage),
        },
      },
      removal: stage === productionStage ? "retain" : "remove",
      protect: stage === productionStage,
    };
  },
  async run() {
    const storage = await import("./infra/auth/storage.js");
    const signing = await import("./infra/auth/signing.js");
    const api = await import("./infra/auth/api.js");
    const { stageConfig } = await import("./infra/shared/stage-config.js");
    const examples = stageConfig.examples.enabled
      ? await import("./infra/examples/index.js")
      : undefined;

    return {
      ApiUrl: api.api.url,
      ApiId: api.api.nodes.api.id,
      AdminRouteArnPattern: $interpolate`${api.api.nodes.api.executionArn}/*/*/_admin/users/*`,
      TableName: storage.table.name,
      TableKmsKeyArn: storage.tableKmsKeyArn,
      SigningKmsKeyArn: signing.signingKmsKeyArn,
      ...(examples?.exampleOutputs ?? {}),
    };
  },
});
