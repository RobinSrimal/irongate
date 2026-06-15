import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const failures = [];

function read(path) {
  return readFileSync(path, "utf8");
}

function filesUnder(dir) {
  if (!existsSync(dir)) {
    return [];
  }

  const out = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      out.push(...filesUnder(path));
    } else if (entry.endsWith(".rs")) {
      out.push(path);
    }
  }
  return out;
}

function failIfContains(path, pattern, message) {
  const contents = read(path).split(/\n#\[cfg\(test\)\]\s*\nmod tests\b/)[0];
  if (pattern.test(contents)) {
    failures.push(`${path}: ${message}`);
  }
}

const routeBoundaryFiles = [
  "packages/functions/auth/src/routes.rs",
  "packages/functions/auth/src/api/admin.rs",
  ...filesUnder("packages/functions/auth/src/api/oauth"),
  ...filesUnder("packages/functions/auth/src/api/providers"),
  ...filesUnder("packages/functions/auth/src/oauth").filter(
    (path) => !path.endsWith("/pkce.rs"),
  ),
  ...filesUnder("packages/functions/auth/src/providers"),
];

for (const path of routeBoundaryFiles) {
  failIfContains(path, /\buse\s+crate::storage::StorageAdapter\b/, "auth route code must not import StorageAdapter");
  failIfContains(path, /\bStorageAdapter\b/, "auth route code must not expose the raw storage adapter");
  failIfContains(path, /\.storage\b/, "auth route code must use state.store, not raw storage");
  failIfContains(path, /\b(?:AppState|AdminAppState)\s*<\s*S\s*>/, "route state must not be generic over raw storage");
  failIfContains(path, /<\s*S\s*:\s*StorageAdapter\b/, "auth route handler signatures must not be generic over StorageAdapter");
  failIfContains(path, /\bAuthStore::new\s*\(/, "route handlers must receive AuthStore through state, not construct it from a backend");
}

failIfContains(
  "packages/functions/auth/src/lib.rs",
  /pub\s+use\s+storage::\{[^}]*StorageAdapter|pub\s+use\s+storage::StorageAdapter/,
  "StorageAdapter must not be re-exported as a public runtime API",
);

if (failures.length > 0) {
  console.error("Store boundary validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Store boundary validation passed");
