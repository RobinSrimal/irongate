import { stageConfig } from "./stage-config.js";

export type TableKmsMode = "aws-owned" | "customer";
export type AuditLogMode = "cloudwatch" | "none";
export type SigningMode = "local-es256" | "kms-es256";
export type LogRetention =
  | "1 day"
  | "3 days"
  | "5 days"
  | "1 week"
  | "2 weeks"
  | "1 month"
  | "2 months"
  | "3 months"
  | "4 months"
  | "5 months"
  | "6 months"
  | "1 year"
  | "13 months"
  | "18 months"
  | "2 years"
  | "3 years"
  | "5 years"
  | "6 years"
  | "7 years"
  | "8 years"
  | "9 years"
  | "10 years";

type TableArn = string | $util.Output<string>;

export const allowedDynamoDbActions = [
  "dynamodb:GetItem",
  "dynamodb:PutItem",
  "dynamodb:UpdateItem",
  "dynamodb:DeleteItem",
  "dynamodb:Query",
  "dynamodb:TransactWriteItems",
  "dynamodb:ConditionCheckItem",
] as const;

export const allowedSigningKmsActions = ["kms:Sign", "kms:GetPublicKey"] as const;

const retentionByDays = {
  1: "1 day",
  3: "3 days",
  5: "5 days",
  7: "1 week",
  14: "2 weeks",
  30: "1 month",
  60: "2 months",
  90: "3 months",
  120: "4 months",
  150: "5 months",
  180: "6 months",
  365: "1 year",
  400: "13 months",
  545: "18 months",
  731: "2 years",
  1096: "3 years",
  1827: "5 years",
  2192: "6 years",
  2557: "7 years",
  2922: "8 years",
  3288: "9 years",
  3653: "10 years",
} as const satisfies Record<number, LogRetention>;

const infraDefaults = {
  tableKmsMode: stageConfig.infra.tableKmsMode,
  auditLogMode: stageConfig.infra.auditLogMode,
  logRetentionDays: stageConfig.infra.logRetentionDays,
  signingMode: stageConfig.signing.mode,
  signingKeyId: stageConfig.signing.keyId,
} as const;

export const infraConfig = {
  tableKmsMode: parseTableKmsMode(process.env.AUTH_TABLE_KMS),
  auditLogMode: parseAuditLogMode(process.env.AUTH_AUDIT_LOG_MODE),
  logRetentionDays: parseLogRetentionDays(process.env.AUTH_LOG_RETENTION_DAYS),
  signingMode: parseSigningMode(process.env.AUTH_SIGNING_MODE),
  signingKeyId: parseRequiredString(
    process.env.AUTH_SIGNING_KEY_ID,
    infraDefaults.signingKeyId,
    "AUTH_SIGNING_KEY_ID",
  ),
  get logRetention(): LogRetention {
    return retentionByDays[this.logRetentionDays];
  },
};

export function authTablePermissions(tableArn: TableArn) {
  return {
    actions: [...allowedDynamoDbActions],
    resources: [tableArn, $interpolate`${tableArn}/*`],
  };
}

export function authSigningKmsPermissions(keyArn: TableArn) {
  return {
    actions: [...allowedSigningKmsActions],
    resources: [keyArn],
  };
}

function parseTableKmsMode(value: string | undefined): TableKmsMode {
  if (value === undefined || value === "") {
    return infraDefaults.tableKmsMode;
  }

  if (value === "aws-owned" || value === "customer") {
    return value;
  }

  throw new Error("AUTH_TABLE_KMS must be one of: aws-owned, customer");
}

function parseAuditLogMode(value: string | undefined): AuditLogMode {
  if (value === undefined || value === "") {
    return infraDefaults.auditLogMode;
  }

  if (value === "cloudwatch" || value === "none") {
    return value;
  }

  throw new Error("AUTH_AUDIT_LOG_MODE must be one of: cloudwatch, none");
}

function parseSigningMode(value: string | undefined): SigningMode {
  if (value === undefined || value === "") {
    return infraDefaults.signingMode;
  }

  if (value === "local-es256" || value === "kms-es256") {
    return value;
  }

  throw new Error("AUTH_SIGNING_MODE must be one of: local-es256, kms-es256");
}

function parseLogRetentionDays(value: string | undefined): keyof typeof retentionByDays {
  if (value === undefined || value === "") {
    return infraDefaults.logRetentionDays as keyof typeof retentionByDays;
  }

  if (!/^\d+$/.test(value)) {
    throw new Error("AUTH_LOG_RETENTION_DAYS must be a supported positive day count");
  }

  const parsed = Number(value);
  if (!Object.hasOwn(retentionByDays, parsed)) {
    throw new Error(
      `AUTH_LOG_RETENTION_DAYS must be one of: ${Object.keys(retentionByDays).join(", ")}`,
    );
  }

  return parsed as keyof typeof retentionByDays;
}

function parseRequiredString(
  value: string | undefined,
  fallback: string,
  name: string,
): string {
  const resolved = value === undefined || value === "" ? fallback : value;
  if (resolved.trim() === "") {
    throw new Error(`${name} must not be empty`);
  }
  return resolved;
}
