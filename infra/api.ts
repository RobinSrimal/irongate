import { authTablePermissions, infraConfig } from "./config.js";
import { rustLambdaBundle } from "./rust-bundle.js";
import { authSecrets } from "./secrets.js";
import { signingEnvironment, signingKmsPermissions } from "./signing.js";
import { stageConfig } from "./stage-config.js";
import { table } from "./storage.js";

export const api = new sst.aws.ApiGatewayV2("AuthApi", {
  accessLog: {
    retention: infraConfig.logRetention,
  },
});

const issuerUrl = stageConfig.auth.issuerUrl ?? api.url;

const optionalPublicAuthEnvironment = Object.fromEntries(
  Object.entries({
    AUTH_ACCESS_TOKEN_AUDIENCE: stageConfig.auth.accessTokenAudience,
    AUTH_EMAIL_REPLY_TO: stageConfig.email.replyTo,
    AUTH_EMAIL_BRAND_NAME: stageConfig.email.brandName,
    AUTH_EMAIL_SUPPORT_EMAIL: stageConfig.email.supportEmail,
    AUTH_EMAIL_VERIFY_SUBJECT: stageConfig.email.verifySubject,
    AUTH_EMAIL_RESET_SUBJECT: stageConfig.email.resetSubject,
    AUTH_EMAIL_VERIFY_TEMPLATE_PATH: stageConfig.email.verifyTemplatePath,
    AUTH_EMAIL_RESET_TEMPLATE_PATH: stageConfig.email.resetTemplatePath,
  }).filter(([, value]) => value !== undefined),
) as Record<string, string>;

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
  memory: "256 MB",
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
