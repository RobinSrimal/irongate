import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

interface RustLambdaBundleArgs {
  name: string;
  manifestPath: string;
  watchPaths: string[];
  copyFiles?: Array<{
    from: string;
    to?: string;
  }>;
}

const root = process.cwd();

export function rustLambdaBundle(args: RustLambdaBundleArgs): string {
  const bundlePath = path.join(".sst", "rust", $app.stage, args.name);
  const bundleDir = path.join(root, bundlePath);
  const bootstrapPath = path.join(bundleDir, "bootstrap");

  if (isFresh(bootstrapPath, args.watchPaths)) {
    copyBundleFiles(bundleDir, args.copyFiles ?? []);
    return bundlePath;
  }

  execFileSync(
    "cargo",
    [
      "lambda",
      "build",
      "--release",
      "--locked",
      "--arm64",
      "--manifest-path",
      args.manifestPath,
      "--lambda-dir",
      bundleDir,
      "--flatten",
      "bootstrap",
      "--bin",
      "bootstrap",
    ],
    {
      cwd: root,
      env: { ...process.env, CARGO_TERM_COLOR: "always" },
      stdio: "inherit",
    },
  );

  if (!existsSync(bootstrapPath)) {
    throw new Error(`cargo-lambda did not create ${bootstrapPath}`);
  }

  copyBundleFiles(bundleDir, args.copyFiles ?? []);

  return bundlePath;
}

function copyBundleFiles(
  bundleDir: string,
  copyFiles: NonNullable<RustLambdaBundleArgs["copyFiles"]>,
) {
  for (const file of copyFiles) {
    const source = path.join(root, file.from);
    const destination = path.join(bundleDir, file.to ?? path.basename(file.from));
    mkdirSync(path.dirname(destination), { recursive: true });
    copyFileSync(source, destination);
  }
}

function isFresh(outputPath: string, inputPaths: string[]): boolean {
  if (!existsSync(outputPath)) {
    return false;
  }

  const outputMtime = statSync(outputPath).mtimeMs;
  return latestInputMtime(inputPaths) <= outputMtime;
}

function latestInputMtime(inputPaths: string[]): number {
  let latest = 0;
  for (const inputPath of inputPaths) {
    latest = Math.max(latest, latestPathMtime(path.join(root, inputPath)));
  }
  return latest;
}

function latestPathMtime(inputPath: string): number {
  if (!existsSync(inputPath)) {
    return 0;
  }

  const stat = statSync(inputPath);
  if (!stat.isDirectory()) {
    return stat.mtimeMs;
  }

  let latest = stat.mtimeMs;
  for (const entry of readdirSync(inputPath)) {
    if (entry === "target") {
      continue;
    }
    latest = Math.max(latest, latestPathMtime(path.join(inputPath, entry)));
  }
  return latest;
}
