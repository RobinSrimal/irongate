type AuditLogMode = "cloudwatch" | "none";
type DeletedIdentityReuse = "immediate" | "after_retention" | "never";
type SigningMode = "local-es256" | "kms-es256";
type TableKmsMode = "aws-owned" | "customer";

interface AppleProviderStageConfig {
  enabled: boolean;
  clientId?: string;
  teamId?: string;
  keyId?: string;
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
    issuerUrl?: string;
    accessTokenAudience?: string;
    googleClientId?: string;
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
      baseUrl?: string;
      domain?: string;
    };
    app: {
      enabled: boolean;
    };
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
    auth: {
      googleClientId:
        "791523530070-vb31r972fabkvcu4a0gbcajdjoannjro.apps.googleusercontent.com",
      apple: {
        enabled: true,
        clientId: "com.auth.irongate",
        teamId: "XUTMJDN8V6",
        keyId: "W4DMH8K6X2",
      },
    },
    email: {
      from: "Irongate Dev <auth@verify.raim.app>",
      verifyUrlBase: "https://irongate-dev-examplewebworkerscript.robin-srimal.workers.dev/auth/verify-email",
      resetUrlBase: "http://localhost:3000/auth/reset-password",
      brandName: "Irongate Dev",
    },
    signing: {
      mode: "local-es256",
      keyId: "dev-local-es256-1",
    },
    examples: {
      enabled: true,
      web: {
        enabled: true,
        clientId: "web",
        baseUrl: undefined,
        domain: undefined,
      },
      app: {
        enabled: false,
      },
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
    auth: {
      googleClientId: undefined,
      apple: {
        enabled: false,
      },
    },
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
    examples: {
      enabled: false,
      web: {
        enabled: false,
        clientId: "web",
        baseUrl: undefined,
        domain: undefined,
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
