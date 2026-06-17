import type { AuditLogMode, SigningMode, TableKmsMode } from "./config.js";

type DeletedIdentityReuse = "immediate" | "after_retention" | "never";

export interface StageConfig {
  infra: {
    tableKmsMode: TableKmsMode;
    auditLogMode: AuditLogMode;
    logRetentionDays: number;
  };
  runtime: {
    rustLog: string;
    clientConfigPath: string;
    deletedIdentityReuse: DeletedIdentityReuse;
    deletedIdentityRetentionDays: number;
  };
  auth: {
    issuerUrl?: string;
    accessTokenAudience?: string;
  };
  email: {
    from: string;
    verifyUrlBase: string;
    resetUrlBase: string;
    replyTo?: string;
    brandName?: string;
    supportEmail?: string;
    verifySubject?: string;
    resetSubject?: string;
    verifyTemplatePath?: string;
    resetTemplatePath?: string;
  };
  signing: {
    mode: SigningMode;
    keyId: string;
  };
}

const stageConfigs = {
  dev: {
    infra: {
      tableKmsMode: "aws-owned",
      auditLogMode: "cloudwatch",
      logRetentionDays: 30,
    },
    runtime: {
      rustLog: "info",
      clientConfigPath: "auth.clients.toml",
      deletedIdentityReuse: "after_retention",
      deletedIdentityRetentionDays: 30,
    },
    auth: {},
    email: {
      from: "Irongate Dev <auth@verify.raim.app>",
      verifyUrlBase: "http://localhost:3000/auth/verify-email",
      resetUrlBase: "http://localhost:3000/auth/reset-password",
      brandName: "Irongate Dev",
    },
    signing: {
      mode: "kms-es256",
      keyId: "dev-kms-es256-1",
    },
  },
  production: {
    infra: {
      tableKmsMode: "customer",
      auditLogMode: "cloudwatch",
      logRetentionDays: 30,
    },
    runtime: {
      rustLog: "info",
      clientConfigPath: "auth.clients.toml",
      deletedIdentityReuse: "after_retention",
      deletedIdentityRetentionDays: 30,
    },
    auth: {},
    email: {
      from: "Irongate <auth@verify.raim.app>",
      verifyUrlBase: "https://app.example.com/auth/verify-email",
      resetUrlBase: "https://app.example.com/auth/reset-password",
      brandName: "Irongate",
    },
    signing: {
      mode: "kms-es256",
      keyId: "prod-kms-es256-1",
    },
  },
} as const satisfies Record<string, StageConfig>;

export type ConfiguredStage = keyof typeof stageConfigs;

function resolveStage(stage: string): ConfiguredStage {
  if (stage === "dev") {
    return "dev";
  }

  if (stage === "production") {
    return "production";
  }

  if (stage === "prod") {
    throw new Error('Unsupported stage "prod". Use "--stage production" for production deploys.');
  }

  throw new Error(
    `Unsupported stage "${stage}". Supported stages are "dev" and "production".`,
  );
}

export const stageName = resolveStage($app.stage);
export const stageConfig: StageConfig = stageConfigs[stageName];
