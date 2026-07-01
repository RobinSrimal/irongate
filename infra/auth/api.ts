import { authTablePermissions, infraConfig } from "./config.js";
import { authSecrets } from "./secrets.js";
import { signingEnvironment, signingKmsPermissions } from "./signing.js";
import { table } from "./storage.js";
import { rustLambdaBundle } from "../shared/rust-bundle.js";
import { stageConfig } from "../shared/stage-config.js";

export const api = new sst.aws.ApiGatewayV2("AuthApi", {
  accessLog: {
    retention: infraConfig.logRetention,
  },
});

const issuerUrl = optionalStageValue(stageConfig.auth.issuerUrl) ?? api.url;

const optionalPublicAuthEnvironment = Object.fromEntries(
  Object.entries({
    AUTH_ACCESS_TOKEN_AUDIENCE: optionalStageValue(stageConfig.auth.accessTokenAudience),
    AUTH_EMAIL_REPLY_TO: stageConfig.email.replyTo,
    AUTH_EMAIL_BRAND_NAME: stageConfig.email.brandName,
    AUTH_EMAIL_SUPPORT_EMAIL: stageConfig.email.supportEmail,
    AUTH_EMAIL_VERIFY_SUBJECT: stageConfig.email.verifySubject,
    AUTH_EMAIL_RESET_SUBJECT: stageConfig.email.resetSubject,
    AUTH_EMAIL_VERIFY_TEMPLATE_PATH: stageConfig.email.verifyTemplatePath,
    AUTH_EMAIL_RESET_TEMPLATE_PATH: stageConfig.email.resetTemplatePath,
  }).filter(([, value]) => optionalStageValue(value) !== undefined),
) as Record<string, string>;

const googleClientId = optionalStageValue(stageConfig.auth.googleClientId);

const googleProviderEnvironment = googleClientId
  ? {
      AUTH_GOOGLE_CLIENT_ID: googleClientId,
      AUTH_GOOGLE_CLIENT_SECRET: authSecrets.googleClientSecret.value,
    }
  : {};

const appleProviderEnvironment = appleEnvironment();

function appleEnvironment(): Record<string, string> {
  const apple = stageConfig.auth.apple;
  if (!apple.enabled) {
    return {};
  }

  const clientId = optionalStageValue(apple.clientId);
  const teamId = optionalStageValue(apple.teamId);
  const keyId = optionalStageValue(apple.keyId);

  if (!clientId || !teamId || !keyId) {
    throw new Error(
      "Apple login is enabled but auth.apple.clientId, auth.apple.teamId, or auth.apple.keyId is missing.",
    );
  }

  return {
    AUTH_APPLE_CLIENT_ID: clientId,
    AUTH_APPLE_TEAM_ID: teamId,
    AUTH_APPLE_KEY_ID: keyId,
    AUTH_APPLE_PRIVATE_KEY_SECRET: "AUTH_APPLE_PRIVATE_KEY",
    AUTH_APPLE_PRIVATE_KEY: authSecrets.applePrivateKey.value,
    ...(apple.clientSecretTtlSeconds
      ? { AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS: String(apple.clientSecretTtlSeconds) }
      : {}),
  };
}

function optionalStageValue(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

const publicAuthHandler = {
  runtime: "provided.al2023",
  handler: "bootstrap",
  bundle: rustLambdaBundle({
    name: "auth",
    manifestPath: "packages/functions/auth/Cargo.toml",
    watchPaths: [
      "packages/functions/auth/Cargo.toml",
      "packages/functions/auth/Cargo.lock",
      "packages/functions/auth/src",
      "auth.clients.toml",
    ],
    copyFiles: [{ from: "auth.clients.toml" }],
  }),
  architecture: "arm64",
  memory: "256 MB",
  timeout: "30 seconds",
  permissions: [authTablePermissions(table.arn), ...signingKmsPermissions],
  logging: {
    retention: infraConfig.logRetention,
    format: "json",
  },
  environment: {
    DYNAMODB_TABLE: table.name,
    ISSUER_URL: issuerUrl,
    DEV_MODE: "false",
    RUST_LOG: stageConfig.runtime.rustLog,
    AUTH_CLIENT_CONFIG_PATH: stageConfig.runtime.clientConfigPath,
    AUTH_AUDIT_LOG_MODE: infraConfig.auditLogMode,
    AUTH_HMAC_LOOKUP_SECRET: authSecrets.hmacLookupSecret.value,
    RESEND_API_KEY: authSecrets.resendApiKey.value,
    AUTH_EMAIL_FROM: stageConfig.email.from,
    AUTH_EMAIL_VERIFY_URL_BASE: stageConfig.email.verifyUrlBase,
    AUTH_EMAIL_RESET_URL_BASE: stageConfig.email.resetUrlBase,
    ...optionalPublicAuthEnvironment,
    ...googleProviderEnvironment,
    ...appleProviderEnvironment,
    ...signingEnvironment,
  },
} as const;

const adminHandler = {
  runtime: "provided.al2023",
  handler: "bootstrap",
  bundle: rustLambdaBundle({
    name: "admin",
    manifestPath: "packages/functions/admin/Cargo.toml",
    watchPaths: [
      "packages/functions/admin/Cargo.toml",
      "packages/functions/admin/Cargo.lock",
      "packages/functions/admin/src",
      "packages/functions/auth/Cargo.toml",
      "packages/functions/auth/Cargo.lock",
      "packages/functions/auth/src",
    ],
  }),
  architecture: "arm64",
  memory: "128 MB",
  timeout: "30 seconds",
  permissions: [authTablePermissions(table.arn)],
  logging: {
    retention: infraConfig.logRetention,
    format: "json",
  },
  environment: {
    DYNAMODB_TABLE: table.name,
    RUST_LOG: stageConfig.runtime.rustLog,
    AUTH_AUDIT_LOG_MODE: infraConfig.auditLogMode,
    AUTH_DELETED_IDENTITY_REUSE: stageConfig.runtime.deletedIdentityReuse,
    AUTH_DELETED_IDENTITY_RETENTION_DAYS: String(
      stageConfig.runtime.deletedIdentityRetentionDays,
    ),
  },
} as const;

const adminRouteOptions = {
  auth: { iam: true },
} as const;

export const publicAuthFunction = new sst.aws.Function(
  "PublicAuthFunction",
  publicAuthHandler,
);

export const adminFunction = new sst.aws.Function("AdminFunction", adminHandler);

api.route("$default", publicAuthFunction.arn);
api.route("GET /_admin/users/{subject}", adminFunction.arn, adminRouteOptions);
api.route("POST /_admin/users/{subject}/disable", adminFunction.arn, adminRouteOptions);
api.route("POST /_admin/users/{subject}/enable", adminFunction.arn, adminRouteOptions);
api.route("POST /_admin/users/{subject}/delete", adminFunction.arn, adminRouteOptions);
api.route("POST /_admin/users/{subject}/revoke-sessions", adminFunction.arn, adminRouteOptions);
