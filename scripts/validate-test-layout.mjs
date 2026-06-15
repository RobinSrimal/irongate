import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const testDir = "packages/functions/auth/tests";
const readmePath = join(testDir, "README.md");

const failures = [];

if (!existsSync(readmePath)) {
  failures.push(`${readmePath} is required to document integration test layout`);
}

const entries = existsSync(testDir) ? readdirSync(testDir) : [];
for (const entry of entries) {
  const fullPath = join(testDir, entry);
  if (!statSync(fullPath).isFile() || !entry.endsWith(".rs")) {
    continue;
  }

  if (entry.endsWith("_slice.rs")) {
    failures.push(`${fullPath} must be renamed by auth domain, not implementation slice`);
  }

  if (/^\d+[_-]/.test(entry)) {
    failures.push(`${fullPath} must not start with an implementation slice number`);
  }
}

if (failures.length > 0) {
  console.error("Test layout validation failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("Test layout validation passed");
