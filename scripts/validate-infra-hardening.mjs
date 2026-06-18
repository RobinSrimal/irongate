import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const files = {
  api: "infra/auth/api.ts",
  config: "infra/auth/config.ts",
  examplesConfig: "infra/examples/config.ts",
  examplesIndex: "infra/examples/index.ts",
  rustBundle: "infra/shared/rust-bundle.ts",
  secrets: "infra/auth/secrets.ts",
  signing: "infra/auth/signing.ts",
  stageConfig: "infra/shared/stage-config.ts",
  storage: "infra/auth/storage.ts",
  sst: "sst.config.ts",
  operatorPolicy: "design/infra/auth/operator-iam-policy.md",
};

const source = {};
for (const [name, rel] of Object.entries(files)) {
  const path = resolve(root, rel);
  if (!existsSync(path)) {
    source[name] = "";
  } else {
    source[name] = readFileSync(path, "utf8");
  }
}

const failures = [];

function requireFile(name) {
  if (!source[name]) {
    failures.push(`${files[name]} must exist`);
  }
}

function assertContains(text, pattern, description) {
  if (!pattern.test(text)) {
    failures.push(description);
  }
}

function assertNotContains(text, pattern, description) {
  if (pattern.test(text)) {
    failures.push(description);
  }
}

for (const name of Object.keys(files)) {
  requireFile(name);
}

assertContains(
  source.stageConfig,
  /dev:\s*\{/,
  "stage config must define checked-in dev settings",
);
assertContains(
  source.stageConfig,
  /production:\s*\{/,
  "stage config must define checked-in production settings",
);
assertContains(
  source.stageConfig,
  /if\s*\(\s*stage\s*===\s*"dev"\s*\)[\s\S]*return\s+"dev"/,
  "stage config must explicitly allow the dev stage",
);
assertContains(
  source.stageConfig,
  /if\s*\(\s*stage\s*===\s*"production"\s*\)[\s\S]*return\s+"production"/,
  "stage config must explicitly allow the production stage",
);
assertContains(
  source.stageConfig,
  /stage\s*===\s*"prod"[\s\S]*production/,
  "stage config must reject prod with a clear use-production message",
);
assertNotContains(
  source.stageConfig,
  /stage\s*===\s*"production"\s*\?\s*"production"\s*:\s*"dev"/,
  "stage config must not silently map unknown stages to dev",
);
assertContains(
  source.stageConfig,
  /email:\s*\{/,
  "stage config must define non-secret email settings",
);
assertContains(
  source.stageConfig,
  /signing:\s*\{/,
  "stage config must define non-secret signing settings",
);
assertContains(
  source.stageConfig,
  /examples:\s*\{/,
  "stage config must define example deployment settings",
);
assertContains(
  source.stageConfig,
  /examples:\s*\{[\s\S]*enabled:\s*false[\s\S]*authWeb:\s*false[\s\S]*webSpa:\s*false[\s\S]*resourceApi:\s*false/s,
  "examples must be disabled by default in checked-in stage config",
);
assertContains(
  source.examplesConfig,
  /stageConfig\.examples/,
  "example config must read from checked-in stage config",
);
assertNotContains(
  source.examplesIndex,
  /new\s+(sst|aws)\./,
  "example infra must not create resources in the boundary slice",
);
assertContains(
  source.secrets,
  /new\s+sst\.Secret\("AuthHmacLookupSecret"\)/,
  "infra secrets must define AuthHmacLookupSecret as an SST secret",
);
assertContains(
  source.secrets,
  /new\s+sst\.Secret\("ResendApiKey"\)/,
  "infra secrets must define ResendApiKey as an SST secret",
);

assertContains(
  source.config,
  /export\s+type\s+TableKmsMode\s*=\s*"aws-owned"\s*\|\s*"customer"/,
  "infra config must define exact AUTH_TABLE_KMS modes",
);
assertContains(
  source.config,
  /export\s+type\s+AuditLogMode\s*=\s*"cloudwatch"\s*\|\s*"none"/,
  "infra config must define exact AUTH_AUDIT_LOG_MODE modes",
);
assertContains(
  source.config,
  /export\s+type\s+SigningMode\s*=\s*"local-es256"\s*\|\s*"kms-es256"/,
  "infra config must define exact AUTH_SIGNING_MODE modes",
);
assertContains(
  source.config,
  /stageConfig\.infra\.tableKmsMode/,
  "infra config must read table KMS mode from checked-in stage config",
);
assertContains(
  source.config,
  /stageConfig\.infra\.auditLogMode/,
  "infra config must read audit log mode from checked-in stage config",
);
assertContains(
  source.config,
  /stageConfig\.infra\.logRetentionDays/,
  "infra config must read log retention from checked-in stage config",
);
assertContains(
  source.config,
  /stageConfig\.signing\.mode/,
  "infra config must read signing mode from checked-in stage config",
);
assertContains(
  source.config,
  /throw new Error\([^)]*AUTH_TABLE_KMS/s,
  "infra config must reject invalid AUTH_TABLE_KMS values",
);
assertContains(
  source.config,
  /throw new Error\([^)]*AUTH_AUDIT_LOG_MODE/s,
  "infra config must reject invalid AUTH_AUDIT_LOG_MODE values",
);
assertContains(
  source.config,
  /throw new Error\([^)]*AUTH_LOG_RETENTION_DAYS/s,
  "infra config must reject invalid AUTH_LOG_RETENTION_DAYS values",
);
assertContains(
  source.config,
  /throw new Error\([^)]*AUTH_SIGNING_MODE/s,
  "infra config must reject invalid AUTH_SIGNING_MODE values",
);
assertContains(
  source.config,
  /kms:Sign/,
  "infra config must define kms:Sign for token signing permissions",
);
assertContains(
  source.config,
  /kms:GetPublicKey/,
  "infra config must define kms:GetPublicKey for token signing permissions",
);
assertContains(
  source.sst,
  /const\s+developmentStage\s*=\s*"dev"/,
  "sst config must define an explicit development stage",
);
assertContains(
  source.sst,
  /assertConfiguredStage/,
  "sst config must validate the requested stage",
);
assertContains(
  source.sst,
  /stage\s*===\s*"prod"[\s\S]*production/,
  "sst config must reject prod with a clear use-production message",
);
assertNotContains(
  source.sst,
  /stage\s*===\s*productionStage[\s\S]*return\s+process\.env\.SST_DEV_AWS_PROFILE/s,
  "sst config must not default every non-production stage to the dev AWS profile",
);
assertContains(
  source.sst,
  /\.\/infra\/auth\/storage\.js/,
  "sst config must import auth storage from infra/auth",
);
assertContains(
  source.sst,
  /\.\/infra\/auth\/signing\.js/,
  "sst config must import auth signing from infra/auth",
);
assertContains(
  source.sst,
  /\.\/infra\/auth\/api\.js/,
  "sst config must import auth API from infra/auth",
);
assertContains(
  source.sst,
  /stageConfig\.examples\.enabled[\s\S]*\.\/infra\/examples\/index\.js/s,
  "sst config must import example infra only when examples are enabled",
);

assertContains(
  source.rustBundle,
  /cargo[\s\S]*lambda[\s\S]*build/,
  "Rust bundle helper must build Lambda artifacts with cargo-lambda",
);
assertContains(
  source.rustBundle,
  /"--arm64"/,
  "Rust bundle helper must build ARM64 Lambda binaries",
);
assertContains(
  source.rustBundle,
  /"--locked"/,
  "Rust bundle helper must use Cargo.lock for reproducible Lambda builds",
);
assertContains(
  source.rustBundle,
  /"--flatten"[\s\S]*"bootstrap"/,
  "Rust bundle helper must produce a root bootstrap binary for Lambda",
);
assertContains(
  source.rustBundle,
  /copyFileSync/,
  "Rust bundle helper must support copying runtime config files into Lambda bundles",
);

assertContains(
  source.storage,
  /import\s+\{\s*infraConfig\s*\}\s+from\s+"\.\/config\.js"/,
  "storage must use parsed infra config",
);
assertContains(
  source.storage,
  /new\s+aws\.kms\.Key\("AuthTableKmsKey"/,
  "customer table KMS mode must create a customer managed KMS key",
);
assertContains(
  source.storage,
  /enableKeyRotation:\s*true/,
  "customer table KMS key must enable rotation",
);
assertContains(
  source.storage,
  /new\s+aws\.kms\.Alias\("AuthTableKmsAlias"/,
  "customer table KMS mode must create a stage-specific alias",
);
assertContains(
  source.storage,
  /serverSideEncryption:\s*tableKmsKey\s*\?/s,
  "Dynamo table must configure customer-managed server-side encryption only when enabled",
);
assertContains(source.storage, /ttl:\s*"expiry"/, "DynamoDB TTL must remain expiry");
assertContains(source.storage, /pk:\s*"string"/, "DynamoDB pk string field must remain configured");
assertContains(source.storage, /sk:\s*"string"/, "DynamoDB sk string field must remain configured");

assertContains(
  source.signing,
  /new\s+aws\.kms\.Key\("AuthSigningKmsKey"/,
  "kms-es256 signing mode must create a managed asymmetric KMS signing key",
);
assertContains(
  source.signing,
  /customerMasterKeySpec:\s*"ECC_NIST_P256"/,
  "KMS signing key must use ECC_NIST_P256",
);
assertContains(
  source.signing,
  /keyUsage:\s*"SIGN_VERIFY"/,
  "KMS signing key must use SIGN_VERIFY",
);
assertContains(
  source.signing,
  /new\s+aws\.kms\.Alias\("AuthSigningKmsAlias"/,
  "KMS signing key must have a stage-specific alias",
);
assertContains(
  source.signing,
  /alias\/\$\{\$app\.name\}\/auth-signing-\$\{\$app\.stage\}/,
  "KMS signing key alias must include app and stage",
);
assertContains(
  source.signing,
  /infraConfig\.signingMode\s*===\s*"kms-es256"/,
  "KMS signing resources must be conditional on kms-es256 mode",
);
assertContains(
  source.signing,
  /new\s+sst\.Secret\("AuthSigningPrivateKey"\)/,
  "local-es256 signing mode must require AuthSigningPrivateKey as an SST secret",
);

assertContains(
  source.api,
  /import\s+\{\s*authTablePermissions,\s*infraConfig\s*\}\s+from\s+"\.\/config\.js"/,
  "api must use parsed infra config and explicit auth table permissions",
);
assertContains(
  source.api,
  /from\s+"\.\/secrets\.js"/,
  "api must import SST auth secrets",
);
assertContains(
  source.api,
  /from\s+"\.\.\/shared\/stage-config\.js"/,
  "api must import checked-in shared stage config",
);
assertContains(
  source.api,
  /from\s+"\.\/signing\.js"/,
  "api must import KMS signing environment and permissions",
);
assertContains(
  source.api,
  /from\s+"\.\.\/shared\/rust-bundle\.js"/,
  "api must import the shared Rust Lambda bundle helper",
);
assertContains(
  source.api,
  /runtime:\s*"provided\.al2023"/,
  "Rust Lambdas must deploy as AWS custom-runtime functions",
);
assertContains(
  source.api,
  /handler:\s*"bootstrap"/,
  "Rust Lambdas must use the cargo-lambda bootstrap handler",
);
assertContains(
  source.api,
  /rustLambdaBundle\(\{\s*name:\s*"auth"/s,
  "public auth Lambda must use the explicit cargo-lambda bundle",
);
assertContains(
  source.api,
  /copyFiles:\s*\[\s*\{\s*from:\s*"auth\.clients\.toml"\s*\}\s*\]/,
  "public auth Lambda bundle must include auth.clients.toml",
);
assertContains(
  source.api,
  /rustLambdaBundle\(\{\s*name:\s*"admin"/s,
  "admin Lambda must use the explicit cargo-lambda bundle",
);
assertContains(
  source.api,
  /accessLog:\s*\{\s*retention:\s*infraConfig\.logRetention/s,
  "API access log retention must come from infra config",
);
assertContains(
  source.api,
  /AUTH_AUDIT_LOG_MODE:\s*infraConfig\.auditLogMode/g,
  "public and admin Lambdas must receive AUTH_AUDIT_LOG_MODE",
);
assertContains(
  source.api,
  /AUTH_HMAC_LOOKUP_SECRET:\s*authSecrets\.hmacLookupSecret\.value/,
  "public auth Lambda environment must read AUTH_HMAC_LOOKUP_SECRET from SST secret",
);
assertContains(
  source.api,
  /RESEND_API_KEY:\s*authSecrets\.resendApiKey\.value/,
  "public auth Lambda environment must read RESEND_API_KEY from SST secret",
);
assertContains(
  source.api,
  /AUTH_EMAIL_FROM:\s*stageConfig\.email\.from/,
  "public auth Lambda environment must read AUTH_EMAIL_FROM from checked-in stage config",
);
assertContains(
  source.api,
  /AUTH_EMAIL_VERIFY_URL_BASE:\s*stageConfig\.email\.verifyUrlBase/,
  "public auth Lambda environment must read AUTH_EMAIL_VERIFY_URL_BASE from checked-in stage config",
);
assertContains(
  source.api,
  /AUTH_EMAIL_RESET_URL_BASE:\s*stageConfig\.email\.resetUrlBase/,
  "public auth Lambda environment must read AUTH_EMAIL_RESET_URL_BASE from checked-in stage config",
);
assertContains(
  source.api,
  /permissions:\s*\[\s*authTablePermissions\(table\.arn\),\s*\.{3}signingKmsPermissions/s,
  "public auth Lambda must include signing KMS permissions when enabled",
);
assertContains(
  source.api,
  /permissions:\s*\[\s*authTablePermissions\(table\.arn\)\s*\]/,
  "admin Lambda must keep only table permissions by default",
);
assertContains(
  source.api,
  /\.{3}signingEnvironment/,
  "public auth Lambda must receive managed KMS signing environment when enabled",
);
assertNotContains(
  source.api,
  /\.{3}authEnvironment/,
  "public auth Lambda must not depend on broad shell AUTH_* forwarding",
);
assertNotContains(
  source.api,
  /link:\s*\[\s*table\s*\]/,
  "public and admin Lambdas must not link table because SST Dynamo links grant dynamodb:*",
);

for (const forbidden of [
  "dynamodb:Scan",
  "dynamodb:*",
  "kms:*",
  "iam:*",
  "secretsmanager:*",
]) {
  assertNotContains(
    source.api,
    new RegExp(forbidden.replaceAll("*", "\\*")),
    `runtime Lambda permissions must not include ${forbidden}`,
  );
}

assertContains(
  source.sst,
  /AdminRouteArnPattern:/,
  "SST outputs must include an admin route ARN pattern",
);
assertContains(
  source.sst,
  /TableKmsKeyArn:/,
  "SST outputs must include the optional table KMS key ARN",
);
assertContains(
  source.sst,
  /SigningKmsKeyArn:/,
  "SST outputs must include the optional signing KMS key ARN",
);

assertContains(
  source.operatorPolicy,
  /execute-api:Invoke/,
  "operator IAM policy example must grant execute-api:Invoke",
);
assertContains(
  source.operatorPolicy,
  /\/_admin\/users\/\*/,
  "operator IAM policy example must be scoped to /_admin/users/*",
);
assertNotContains(
  source.operatorPolicy,
  /\$default|\/authorize|\/token|\/password|\/google|\/apple|\/oauth\/revoke/,
  "operator IAM policy example must not grant public auth route access",
);

if (failures.length > 0) {
  console.error("Infra hardening validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Infra hardening validation passed");
