import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const apiTs = readFileSync(resolve(root, "infra/api.ts"), "utf8");

const failures = [];

function assertContains(pattern, description) {
  if (!pattern.test(apiTs)) {
    failures.push(description);
  }
}

function block(name) {
  const match = apiTs.match(new RegExp(`const ${name} = \\{([\\s\\S]*?)\\n\\} as const;`));
  return match?.[1] ?? "";
}

assertContains(
  /api\.route\("\$default",\s*publicAuthHandler\);/,
  "$default must route to the public auth Lambda without IAM options",
);

for (const route of [
  "GET /_admin/users/{subject}",
  "POST /_admin/users/{subject}/disable",
  "POST /_admin/users/{subject}/delete",
  "POST /_admin/users/{subject}/revoke-sessions",
]) {
  assertContains(
    new RegExp(`api\\.route\\("${route.replaceAll("/", "\\/")}",\\s*adminHandler,\\s*adminRouteOptions\\);`),
    `${route} must route to the admin Lambda with IAM options`,
  );
}

assertContains(
  /const adminRouteOptions = \{\s*auth:\s*\{\s*iam:\s*true\s*\},\s*\} as const;/s,
  "admin route options must require IAM auth",
);

const adminBlock = block("adminHandler");
for (const forbidden of [
  "RESEND_API_KEY",
  "PROVIDER_",
  "AUTH_SIGNING_PRIVATE_KEY",
  "AUTH_GOOGLE",
  "AUTH_APPLE",
  "AUTH_EMAIL_FROM",
  "AUTH_EMAIL_VERIFY_URL_BASE",
  "AUTH_EMAIL_RESET_URL_BASE",
]) {
  if (adminBlock.includes(forbidden)) {
    failures.push(`admin Lambda environment must not include ${forbidden}`);
  }
}

const publicBlock = block("publicAuthHandler");
for (const required of [
  "DYNAMODB_TABLE",
  "ISSUER_URL",
  "AUTH_CLIENT_CONFIG_PATH",
  "...authEnvironment",
  "...providerEnvironment",
]) {
  if (!publicBlock.includes(required)) {
    failures.push(`public auth Lambda environment must include ${required}`);
  }
}

if (failures.length > 0) {
  console.error("Infra route validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Infra route validation passed");
