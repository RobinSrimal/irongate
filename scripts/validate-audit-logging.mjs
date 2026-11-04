// Resolve the actual infra modules with inert resource constructors, never AWS credentials/calls.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
function resolveAuth(stage, env = {}, defaults = {}) {
  const functions = new Map();
  const modules = new Map();
  const table = { arn: "arn:aws:dynamodb:eu-west-1:123456789012:table/test", name: "test" };
  const fakeSecret = { value: "test-only-secret" };
  const stubs = new Map([
    [resolve(root, "infra/auth/secrets.ts"), {
      authSecrets: { hmacLookupSecret: fakeSecret, resendApiKey: fakeSecret,
        googleClientSecret: fakeSecret, applePrivateKey: fakeSecret },
    }],
    [resolve(root, "infra/auth/signing.ts"), { signingEnvironment: {}, signingKmsPermissions: [] }],
    [resolve(root, "infra/auth/storage.ts"), { table }],
    [resolve(root, "infra/shared/rust-bundle.ts"), { rustLambdaBundle: () => "/test/bundle" }],
  ]);
  const context = vm.createContext({
    process: { env },
    $app: { stage },
    $interpolate: (strings, ...values) => strings.reduce((all, text, i) => all + text + (values[i] ?? ""), ""),
    $util: { output: (value) => ({ apply: (fn) => fn(value) }) },
    sst: { aws: {
      ApiGatewayV2: class {
        url = "https://auth.test";
        route() {}
      },
      Function: class {
        constructor(name, args) { functions.set(name, args); this.arn = `arn:test:${name}`; }
      },
    } },
  });
  function load(path) {
    if (stubs.has(path)) return stubs.get(path);
    if (modules.has(path)) return modules.get(path);
    const code = ts.transpileModule(readFileSync(path, "utf8"), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    }).outputText;
    const exports = {};
    modules.set(path, exports);
    const execute = vm.runInContext(`(function(exports, require) { ${code}\n})`, context, { filename: path });
    execute(exports, (specifier) => {
      assert.ok(specifier.startsWith("."), "no external runtime imports in this infra harness");
      return load(resolve(dirname(path), specifier.replace(/\.js$/, ".ts")));
    });
    return exports;
  }
  // Mutate only this isolated VM copy to exercise invalid checked-in defaults.
  Object.assign(load(resolve(root, "infra/shared/stage-config.ts")).stageConfig.infra, defaults);
  const config = load(resolve(root, "infra/auth/config.ts")).infraConfig;
  load(resolve(root, "infra/auth/api.ts"));
  return { functions, config };
}

for (const stage of ["dev", "production"]) {
  for (const env of [{}, { AUTH_LOG_RETENTION_DAYS: "14", AUTH_AUDIT_LOG_MODE: "none" }]) {
    const { functions, config } = resolveAuth(stage, env);
    assert.equal(config.logRetentionDays, env.AUTH_LOG_RETENTION_DAYS ? 14 : 30);
    assert.equal(functions.size, 2);
    for (const name of ["PublicAuthFunction", "AdminFunction"]) {
      const args = functions.get(name);
      assert.ok(args, `${name} must exist`);
      assert.equal(args.runtime, "provided.al2023");
      assert.equal(args.logging.format, "json");
      assert.equal(args.logging.retention, env.AUTH_LOG_RETENTION_DAYS ? "2 weeks" : "1 month");
      assert.equal(args.environment.AUTH_AUDIT_LOG_MODE, env.AUTH_AUDIT_LOG_MODE ?? "cloudwatch");
      assert.equal(args.role, undefined, "keep SST's logging-enabled execution role");
      const lambda = { loggingConfig: { logGroup: `/aws/lambda/${name}`, logFormat: "JSON", systemLogLevel: "WARN" } };
      args.transform.function(lambda);
      assert.deepEqual(JSON.parse(JSON.stringify(lambda.loggingConfig)), {
        logGroup: `/aws/lambda/${name}`, logFormat: "JSON", systemLogLevel: "WARN", applicationLogLevel: "INFO",
      });
      assert.ok(!args.permissions.some((permission) => permission.actions.some((action) => action.startsWith("logs:"))), "audit must not add runtime CloudWatch SDK permissions");
    }
  }
  for (const days of ["0", "-1", "2", "1.5", "forever"]) {
    assert.throws(() => resolveAuth(stage, { AUTH_LOG_RETENTION_DAYS: days }), /AUTH_LOG_RETENTION_DAYS/);
  }
  assert.throws(() => resolveAuth(stage, { AUTH_AUDIT_LOG_MODE: "s3" }), /AUTH_AUDIT_LOG_MODE/);
  for (const days of [0, -1, 2, 1.5, NaN, Infinity, "forever"]) {
    assert.throws(() => resolveAuth(stage, {}, { logRetentionDays: days }), /AUTH_LOG_RETENTION_DAYS/);
  }
  assert.equal(resolveAuth(stage, {}, { logRetentionDays: 14 }).config.logRetention, "2 weeks");
  assert.throws(() => resolveAuth(stage, {}, { auditLogMode: "s3" }), /AUTH_AUDIT_LOG_MODE/);
}

// Adapter contract: the installed SST version attaches its standard logging policy when enabled.
// This is not an assertion about deployed IAM or CloudWatch delivery; those need release checks.
const sstFunction = readFileSync(resolve(root, ".sst/platform/src/components/aws/function.ts"), "utf8");
assert.match(sstFunction, /\.\.\.\(logging\s*\?\s*\[\s*interpolate`[^`]*AWSLambdaBasicExecutionRole`/);
assert.match(sstFunction, /retentionInDays:\s*RETENTION\[logging.retention\]/);
assert.match(sstFunction, /args\.transform\?\.function/);
console.log("Resolved auth/admin audit logging configuration passed (local, not deployed delivery)");
