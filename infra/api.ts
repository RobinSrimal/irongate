import { table } from "./storage.js";

const providerEnvironment = Object.fromEntries(
  Object.entries(process.env).filter(([key]) => key === "PROVIDERS" || key.startsWith("PROVIDER_")),
) as Record<string, string>;

const authEnvironment = Object.fromEntries(
  Object.entries(process.env).filter(([key]) => key.startsWith("AUTH_")),
) as Record<string, string>;

export const api = new sst.aws.ApiGatewayV2("AuthApi", {
  accessLog: {
    retention: "1 month",
  },
});

const issuerUrl = process.env.ISSUER_URL ?? api.url;

const publicAuthHandler = {
  runtime: "rust",
  handler: "packages/functions/auth",
  architecture: "arm64",
  memory: "256 MB",
  timeout: "30 seconds",
  link: [table],
  environment: {
    DYNAMODB_TABLE: table.name,
    ISSUER_URL: issuerUrl,
    TRUSTED_PROXIES: "api-gateway",
    DEV_MODE: "false",
    RUST_LOG: process.env.RUST_LOG ?? "info",
    AUTH_CLIENT_CONFIG_PATH: process.env.AUTH_CLIENT_CONFIG_PATH ?? "auth.clients.toml",
    ...authEnvironment,
    ...providerEnvironment,
  },
} as const;

const adminHandler = {
  runtime: "rust",
  handler: "packages/functions/admin",
  architecture: "arm64",
  memory: "256 MB",
  timeout: "30 seconds",
  link: [table],
  environment: {
    DYNAMODB_TABLE: table.name,
    RUST_LOG: process.env.RUST_LOG ?? "info",
  },
} as const;

const adminRouteOptions = {
  auth: { iam: true },
} as const;

api.route("$default", publicAuthHandler);
api.route("GET /_admin/users/{subject}", adminHandler, adminRouteOptions);
api.route("POST /_admin/users/{subject}/disable", adminHandler, adminRouteOptions);
api.route("POST /_admin/users/{subject}/revoke-sessions", adminHandler, adminRouteOptions);
