import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const apiTs = readFileSync(resolve(root, "infra/auth/api.ts"), "utf8");

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
  /export\s+const\s+publicAuthFunction\s*=\s*new\s+sst\.aws\.Function\(\s*"PublicAuthFunction",\s*publicAuthHandler,\s*\);/s,
  "public auth Lambda must be a named shared Function component",
);

assertContains(
  /export\s+const\s+adminFunction\s*=\s*new\s+sst\.aws\.Function\(\s*"AdminFunction",\s*adminHandler\s*\);/s,
  "admin Lambda must be a named shared Function component",
);

assertContains(
  /api\.route\("\$default",\s*publicAuthFunction\.arn\);/,
  "$default must route to the public auth Lambda without IAM options",
);

for (const route of [
  "GET /_admin/users/{subject}",
  "POST /_admin/users/{subject}/disable",
  "POST /_admin/users/{subject}/enable",
  "POST /_admin/users/{subject}/delete",
  "POST /_admin/users/{subject}/revoke-sessions",
]) {
  assertContains(
    new RegExp(`api\\.route\\("${route.replaceAll("/", "\\/")}",\\s*adminFunction\\.arn,\\s*adminRouteOptions\\);`),
    `${route} must route to the admin Lambda with IAM options`,
  );
}

if (/api\.route\([^,]+,\s*(publicAuthHandler|adminHandler)/.test(apiTs)) {
  failures.push("routes must target shared Function ARNs, not per-route FunctionArgs");
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
  "AUTH_HMAC_LOOKUP_SECRET",
  "RESEND_API_KEY",
  "AUTH_EMAIL_FROM",
  "AUTH_EMAIL_VERIFY_URL_BASE",
  "AUTH_EMAIL_RESET_URL_BASE",
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
