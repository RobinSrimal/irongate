import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const files = {
  api: "infra/api.ts",
  config: "infra/config.ts",
  signing: "infra/signing.ts",
  storage: "infra/storage.ts",
  sst: "sst.config.ts",
  operatorPolicy: "design/infra/operator-iam-policy.md",
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
  /tableKmsMode:\s*"aws-owned"/,
  "infra config must default table KMS mode to aws-owned",
);
assertContains(
  source.config,
  /auditLogMode:\s*"cloudwatch"/,
  "infra config must default audit log mode to cloudwatch",
);
assertContains(
  source.config,
  /logRetentionDays:\s*30/,
  "infra config must default log retention to 30 days",
);
assertContains(
  source.config,
  /signingMode:\s*"local-es256"/,
  "infra config must default signing mode to local-es256",
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
  source.api,
  /import\s+\{\s*authTablePermissions,\s*infraConfig\s*\}\s+from\s+"\.\/config\.js"/,
  "api must use parsed infra config and explicit auth table permissions",
);
assertContains(
  source.api,
  /from\s+"\.\/signing\.js"/,
  "api must import KMS signing environment and permissions",
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
  /key\s*===\s*"RESEND_API_KEY"/,
  "public auth Lambda environment forwarding must include RESEND_API_KEY",
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
