import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("Usage: node scripts/sst-with-env.mjs <sst-command> [...args]");
  process.exit(1);
}

const nodeArgs = [];
if (existsSync(".env")) {
  nodeArgs.push("--env-file=.env");
}
nodeArgs.push("node_modules/sst/bin/sst.mjs", ...args);

const result = spawnSync(process.execPath, nodeArgs, {
  stdio: "inherit",
  env: process.env,
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
