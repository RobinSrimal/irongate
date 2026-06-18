import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const files = {
  api: "infra/auth/api.ts",
  config: "packages/functions/auth/src/config.rs",
  lib: "packages/functions/auth/src/lib.rs",
  main: "packages/functions/auth/src/main.rs",
  token: "packages/functions/auth/src/oauth/token.rs",
  routes: "packages/functions/auth/src/routes.rs",
};

const source = Object.fromEntries(
  Object.entries(files).map(([name, rel]) => [
    name,
    readFileSync(resolve(root, rel), "utf8"),
  ]),
);

const failures = [];

function assertAbsentPath(rel, description) {
  if (existsSync(resolve(root, rel))) {
    failures.push(description);
  }
}

function assertNotContains(text, pattern, description) {
  if (pattern.test(text)) {
    failures.push(description);
  }
}

assertAbsentPath(
  "packages/functions/auth/src/provider",
  "legacy singular provider module tree must be removed",
);
assertAbsentPath(
  "packages/functions/auth/src/ui",
  "built-in auth UI module tree must be removed",
);
assertAbsentPath(
  "packages/functions/auth/src/admin",
  "legacy custom-key admin module tree must be removed",
);
assertAbsentPath(
  "packages/functions/auth/src/client/registry.rs",
  "legacy runtime client CRUD registry must be removed",
);
assertAbsentPath(
  "packages/functions/auth/src/jwt/keys.rs",
  "legacy DynamoDB signing-key helper must be removed",
);
assertAbsentPath(
  "packages/functions/auth/src/jwt",
  "legacy JWT storage/signing module tree must be removed",
);

assertNotContains(source.routes, /\/:provider\/authorize/, "dynamic provider authorize route must not be mounted");
assertNotContains(source.routes, /\/:provider\/callback/, "dynamic provider callback route must not be mounted");
assertNotContains(source.routes, /\/admin\/bootstrap/, "public bootstrap route must not be mounted");
assertNotContains(source.routes, /\/admin\/clients/, "runtime client-management routes must not be mounted");
assertNotContains(source.routes, /provider_authorize_handler/, "legacy provider authorize handler must be removed");
assertNotContains(source.routes, /provider_callback_/, "legacy provider callback handlers must be removed");
assertNotContains(source.routes, /CallbackForm/, "legacy provider callback form must be removed");
assertNotContains(source.routes, /crate::ui/, "routes must not depend on built-in auth UI modules");
assertNotContains(source.config, /enum\s+ProviderConfig/, "ProviderConfig must be removed from app config");
assertNotContains(source.config, /providers:\s*Arc<[^>]*ProviderConfig/, "AppState must not keep a provider registry");
assertNotContains(source.main, /load_providers_from_env/, "main must not load generic providers from env");
assertNotContains(source.main, /PROVIDERS|PROVIDER_/, "main must not parse generic provider env vars");
assertNotContains(source.main, /\bmod\s+provider\b/, "main must not compile the legacy provider module");
assertNotContains(source.main, /\bmod\s+ui\b/, "main must not compile built-in auth UI modules");
assertNotContains(source.main, /\bmod\s+admin\b/, "main must not compile the legacy custom-key admin module");
assertNotContains(source.main, /\bmod\s+jwt\b/, "main must not compile the legacy JWT storage module");
assertNotContains(source.lib, /pub\s+mod\s+provider\b/, "lib must not export the legacy provider module");
assertNotContains(source.lib, /pub\s+mod\s+ui\b/, "lib must not export built-in auth UI modules");
assertNotContains(source.lib, /pub\s+mod\s+admin\b/, "lib must not export the legacy custom-key admin module");
assertNotContains(source.lib, /pub\s+mod\s+jwt\b/, "lib must not export the legacy JWT storage module");
assertNotContains(source.token, /handle_refresh_token_grant/, "legacy raw-refresh-token grant helper must be removed");
assertNotContains(source.token, /rotate_refresh_record/, "legacy raw-refresh-token rotation helper must be removed");
assertNotContains(source.token, /revoke_refresh_tokens/, "legacy refresh-token scan revocation helper must be removed");
assertNotContains(source.token, /\[\s*"oauth:refresh"\s*,\s*refresh_token_str\s*\]/, "token endpoint must not look up refresh records by raw refresh token");
assertNotContains(source.token, /get_or_create_signing_key|get_all_signing_keys/, "token endpoint must not use DynamoDB signing-key helpers");
assertNotContains(source.api, /PROVIDERS|PROVIDER_/, "infra must not forward generic provider env vars");
assertNotContains(source.api, /providerEnvironment/, "infra must not build a generic provider env block");

if (failures.length > 0) {
  console.error("Legacy removal validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Legacy removal validation passed");
