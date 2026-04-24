#!/usr/bin/env node

import crypto from "node:crypto";
import { realpathSync } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { assertReleaseUpgrade, resolveReleasePlan } from "./lib/release.mjs";

export const packageManifestPaths = [
  "package.json",
  "npm/maximus-darwin-arm64/package.json",
  "npm/maximus-darwin-x64/package.json",
  "npm/maximus-linux-arm64-gnu/package.json",
  "npm/maximus-linux-x64-gnu/package.json",
];

export function normalizeReleaseTag(input) {
  return input.startsWith("v") ? input : `v${input}`;
}

export function createManualBumpBranchName(input, baseRef = "master") {
  const normalizedTag = normalizeReleaseTag(input);
  const branchBase = baseRef
    .replace(/[./+]/g, "-")
    .replace(/[^0-9A-Za-z-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  const branchVersion = normalizedTag
    .slice(1)
    .replace(/[.+]/g, "-")
    .replace(/[^0-9A-Za-z-]/g, "-");
  const branchHash = crypto
    .createHash("sha256")
    .update(`${baseRef}\0${normalizedTag}`)
    .digest("hex")
    .slice(0, 8);

  return `codex/manual-bump-${branchBase}-v${branchVersion}-${branchHash}`;
}

export function updatePackageManifest(pkg, version) {
  const nextPkg = {
    ...pkg,
    version,
  };

  if (pkg.optionalDependencies && typeof pkg.optionalDependencies === "object") {
    nextPkg.optionalDependencies = Object.fromEntries(
      Object.entries(pkg.optionalDependencies).map(([name]) => [name, version]),
    );
  }

  return nextPkg;
}

export async function bumpPackageVersion(input, repoRoot = process.cwd()) {
  const normalizedTag = normalizeReleaseTag(input);
  const plan = resolveReleasePlan(normalizedTag);
  const rootManifestPath = path.join(repoRoot, "package.json");
  const rootManifest = JSON.parse(await fs.readFile(rootManifestPath, "utf8"));
  assertReleaseUpgrade(rootManifest.version, plan.version);

  const updatedManifestPaths = [];
  for (const relativePath of packageManifestPaths) {
    const absolutePath = path.join(repoRoot, relativePath);
    const manifest = JSON.parse(await fs.readFile(absolutePath, "utf8"));
    const nextManifest = updatePackageManifest(manifest, plan.version);
    await fs.writeFile(absolutePath, `${JSON.stringify(nextManifest, null, 2)}\n`);
    updatedManifestPaths.push(absolutePath);
  }

  return {
    version: plan.version,
    tag: plan.tag,
    isPrerelease: plan.isPrerelease,
    manifestPaths: updatedManifestPaths,
  };
}

async function main() {
  const input = process.argv[2];
  const repoRoot = process.argv[3] ?? process.cwd();

  if (!input) {
    console.error("Usage: node ./scripts/bump-release-version.mjs <tag-or-version> [repo-root]");
    process.exit(1);
  }

  const result = await bumpPackageVersion(input, repoRoot);
  console.log(`tag=${result.tag}`);
  console.log(`version=${result.version}`);
  console.log(`isPrerelease=${result.isPrerelease}`);
  console.log(`manifestCount=${result.manifestPaths.length}`);
}

function isDirectExecutionEntry() {
  if (!process.argv[1]) {
    return false;
  }

  try {
    return (
      realpathSync(fileURLToPath(import.meta.url))
      === realpathSync(path.resolve(process.argv[1]))
    );
  } catch {
    return false;
  }
}

const isDirectExecution = isDirectExecutionEntry();

if (isDirectExecution) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
