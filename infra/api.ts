import { authTablePermissions, infraConfig } from "./config.js";
import { signingEnvironment, signingKmsPermissions } from "./signing.js";
import { table } from "./storage.js";

const infraOnlyAuthEnvironment = new Set([
  "AUTH_AUDIT_LOG_MODE",
  "AUTH_LOG_RETENTION_DAYS",
  "AUTH_SIGNING_KMS_KEY_ID",
  "AUTH_SIGNING_MODE",
  "AUTH_TABLE_KMS",
]);

const authEnvironment = Object.fromEntries(
  Object.entries(process.env).filter(
    ([key]) =>
      key === "RESEND_API_KEY" ||
      (key.startsWith("AUTH_") && !infraOnlyAuthEnvironment.has(key)),
  ),
) as Record<string, string>;

export const api = new sst.aws.ApiGatewayV2("AuthApi", {
  accessLog: {
    retention: infraConfig.logRetention,
  },
});

const issuerUrl = process.env.ISSUER_URL ?? api.url;

const publicAuthHandler = {
  runtime: "rust",
  handler: "packages/functions/auth",
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
    RUST_LOG: process.env.RUST_LOG ?? "info",
    AUTH_CLIENT_CONFIG_PATH: process.env.AUTH_CLIENT_CONFIG_PATH ?? "auth.clients.toml",
    AUTH_AUDIT_LOG_MODE: infraConfig.auditLogMode,
    ...authEnvironment,
    ...signingEnvironment,
  },
} as const;

const adminHandler = {
  runtime: "rust",
  handler: "packages/functions/admin",
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
    RUST_LOG: process.env.RUST_LOG ?? "info",
    AUTH_AUDIT_LOG_MODE: infraConfig.auditLogMode,
    AUTH_DELETED_IDENTITY_REUSE: process.env.AUTH_DELETED_IDENTITY_REUSE ?? "after_retention",
    AUTH_DELETED_IDENTITY_RETENTION_DAYS:
      process.env.AUTH_DELETED_IDENTITY_RETENTION_DAYS ?? "30",
  },
} as const;

const adminRouteOptions = {
  auth: { iam: true },
} as const;

api.route("$default", publicAuthHandler);
api.route("GET /_admin/users/{subject}", adminHandler, adminRouteOptions);
api.route("POST /_admin/users/{subject}/disable", adminHandler, adminRouteOptions);
api.route("POST /_admin/users/{subject}/delete", adminHandler, adminRouteOptions);
api.route("POST /_admin/users/{subject}/revoke-sessions", adminHandler, adminRouteOptions);
