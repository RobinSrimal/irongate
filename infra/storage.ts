import { infraConfig } from "./config.js";

const tableKmsKey =
  infraConfig.tableKmsMode === "customer"
    ? new aws.kms.Key("AuthTableKmsKey", {
        description: `${$app.name} ${$app.stage} auth DynamoDB table encryption key`,
        deletionWindowInDays: 30,
        enableKeyRotation: true,
      })
    : undefined;

if (tableKmsKey) {
  new aws.kms.Alias("AuthTableKmsAlias", {
    name: `alias/${$app.name}/auth-table-${$app.stage}`,
    targetKeyId: tableKmsKey.keyId,
  });
}

export const table = new sst.aws.Dynamo("AuthTable", {
  fields: {
    pk: "string",
    sk: "string",
  },
  primaryIndex: { hashKey: "pk", rangeKey: "sk" },
  ttl: "expiry",
  transform: {
    table: {
      serverSideEncryption: tableKmsKey
        ? {
            enabled: true,
            kmsKeyArn: tableKmsKey.arn,
          }
        : undefined,
    },
  },
});

export const tableKmsKeyArn = tableKmsKey?.arn ?? "aws-owned";
