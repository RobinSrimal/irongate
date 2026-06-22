type AuditLogMode = "cloudwatch" | "none";
type DeletedIdentityReuse = "immediate" | "after_retention" | "never";
type SigningMode = "local-es256" | "kms-es256";
type TableKmsMode = "aws-owned" | "customer";

interface AppleProviderStageConfig {
  enabled: boolean;
  clientId: string;
  teamId: string;
  keyId: string;
  clientSecretTtlSeconds?: number;
}

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
    issuerUrl: string;
    accessTokenAudience: string;
    googleClientId: string;
    apple: AppleProviderStageConfig;
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
  examples: {
    enabled: boolean;
    web: {
      enabled: boolean;
      clientId: string;
      baseUrl: string;
      domain: string;
    };
    app: {
      enabled: boolean;
    };
  };
}

const stageConfigs = {
  dev: {
    infra: {
      tableKmsMode: "aws-owned", // Other option: "customer"
      auditLogMode: "cloudwatch", // Other option: "none"
      logRetentionDays: 30,
    },
    runtime: {
      rustLog: "info",
      clientConfigPath: "auth.clients.toml",
      deletedIdentityReuse: "after_retention", // Other options: "immediate", "never"
      deletedIdentityRetentionDays: 30,
    },
    auth: {
      issuerUrl: "",
      accessTokenAudience: "",
      googleClientId: "",
      apple: {
        enabled: false,
        clientId: "",
        teamId: "",
        keyId: "",
      },
    },
    email: {
      from: "",
      verifyUrlBase: "",
      resetUrlBase: "",
      brandName: "Irongate Dev",
    },
    signing: {
      mode: "local-es256", // Other option: "kms-es256"
      keyId: "dev-local-es256-1",
    },
    examples: {
      enabled: false,
      web: {
        enabled: false,
        clientId: "web",
        baseUrl: "",
        domain: "",
      },
      app: {
        enabled: false,
      },
    },
  },
  production: {
    infra: {
      tableKmsMode: "customer", // Other option: "aws-owned"
      auditLogMode: "cloudwatch", // Other option: "none"
      logRetentionDays: 30,
    },
    runtime: {
      rustLog: "info",
      clientConfigPath: "auth.clients.toml",
      deletedIdentityReuse: "after_retention", // Other options: "immediate", "never"
      deletedIdentityRetentionDays: 30,
    },
    auth: {
      issuerUrl: "",
      accessTokenAudience: "",
      googleClientId: "",
      apple: {
        enabled: false,
        clientId: "",
        teamId: "",
        keyId: "",
      },
    },
    email: {
      from: "",
      verifyUrlBase: "",
      resetUrlBase: "",
      brandName: "Irongate",
    },
    signing: {
      mode: "kms-es256", // Other option: "local-es256"
      keyId: "prod-kms-es256-1",
    },
    examples: {
      enabled: false,
      web: {
        enabled: false,
        clientId: "web",
        baseUrl: "",
        domain: "",
      },
      app: {
        enabled: false,
      },
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
