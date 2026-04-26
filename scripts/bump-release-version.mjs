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

export const rustCrateManifestPaths = [
  "crates/maximus-cli/Cargo.toml",
  "crates/maximus-core/Cargo.toml",
  "crates/maximus-checks/Cargo.toml",
];

export const cargoLockPath = "Cargo.lock";

export const rustCrateNames = [
  "maximus-cli",
  "maximus-core",
  "maximus-checks",
];

export const versionFilePaths = [
  ...packageManifestPaths,
  ...rustCrateManifestPaths,
  cargoLockPath,
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

export function readCargoTomlPackageVersion(contents, relativePath = "Cargo.toml") {
  const lines = contents.split("\n");
  let inPackageSection = false;
  let sawPackageSection = false;

  for (const line of lines) {
    if (/^\s*\[package\]\s*(?:#.*)?$/.test(line)) {
      inPackageSection = true;
      sawPackageSection = true;
      continue;
    }

    if (inPackageSection && /^\s*\[/.test(line)) {
      inPackageSection = false;
    }

    if (!inPackageSection || !/^\s*version\s*=/.test(line)) {
      continue;
    }

    const match = line.match(/^\s*version\s*=\s*"([^"]+)"/);
    if (!match) {
      throw new Error(`${relativePath} [package] version must be a quoted string`);
    }
    return match[1];
  }

  if (!sawPackageSection) {
    throw new Error(`${relativePath} is missing a [package] section`);
  }
  throw new Error(`${relativePath} is missing [package] version`);
}

export function updateCargoTomlPackageVersion(contents, version, relativePath = "Cargo.toml") {
  const lines = contents.split("\n");
  let inPackageSection = false;
  let sawPackageSection = false;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (/^\s*\[package\]\s*(?:#.*)?$/.test(line)) {
      inPackageSection = true;
      sawPackageSection = true;
      continue;
    }

    if (inPackageSection && /^\s*\[/.test(line)) {
      inPackageSection = false;
    }

    if (!inPackageSection || !/^\s*version\s*=/.test(line)) {
      continue;
    }

    const nextLine = line.replace(
      /^(\s*version\s*=\s*)"[^"]+"(\s*(?:#.*)?)$/,
      `$1"${version}"$2`,
    );
    if (nextLine === line) {
      throw new Error(`${relativePath} [package] version must be a quoted string`);
    }

    lines[index] = nextLine;
    return lines.join("\n");
  }

  if (!sawPackageSection) {
    throw new Error(`${relativePath} is missing a [package] section`);
  }
  throw new Error(`${relativePath} is missing [package] version`);
}

export function readCargoLockWorkspacePackageVersions(contents, packageNames = rustCrateNames) {
  return updateCargoLockWorkspacePackageVersions(contents, undefined, packageNames).versions;
}

export function updateCargoLockWorkspacePackageVersions(
  contents,
  version,
  packageNames = rustCrateNames,
) {
  const lines = contents.split("\n");
  const expectedNames = new Set(packageNames);
  const seenNames = new Set();
  const versions = {};

  for (let start = 0; start < lines.length; start += 1) {
    if (!/^\s*\[\[package\]\]\s*$/.test(lines[start])) {
      continue;
    }

    let end = lines.length;
    for (let index = start + 1; index < lines.length; index += 1) {
      if (/^\s*\[\[package\]\]\s*$/.test(lines[index])) {
        end = index;
        break;
      }
    }

    const nameLine = lines
      .slice(start + 1, end)
      .find((line) => /^\s*name\s*=/.test(line));
    const packageName = nameLine?.match(/^\s*name\s*=\s*"([^"]+)"/)?.[1];
    if (!packageName || !expectedNames.has(packageName)) {
      continue;
    }

    let versionLineIndex = -1;
    for (let index = start + 1; index < end; index += 1) {
      if (/^\s*version\s*=/.test(lines[index])) {
        versionLineIndex = index;
        break;
      }
    }

    if (versionLineIndex === -1) {
      throw new Error(`Cargo.lock package block for ${packageName} is missing version`);
    }

    const versionLineMatch = lines[versionLineIndex].match(
      /^(\s*version\s*=\s*)"([^"]+)"(\s*(?:#.*)?)$/,
    );
    if (!versionLineMatch) {
      throw new Error(`Cargo.lock package block for ${packageName} has an invalid version`);
    }

    seenNames.add(packageName);
    versions[packageName] = versionLineMatch[2];

    if (version) {
      lines[versionLineIndex] = `${versionLineMatch[1]}"${version}"${versionLineMatch[3]}`;
    }

    start = end - 1;
  }

  const missingNames = packageNames.filter((packageName) => !seenNames.has(packageName));
  if (missingNames.length > 0) {
    throw new Error(`Cargo.lock package block not found for ${missingNames.join(", ")}`);
  }

  return {
    contents: lines.join("\n"),
    versions,
  };
}

export async function bumpPackageVersion(input, repoRoot = process.cwd()) {
  const normalizedTag = normalizeReleaseTag(input);
  const plan = resolveReleasePlan(normalizedTag);
  const rootManifestPath = path.join(repoRoot, "package.json");
  const rootManifest = JSON.parse(await fs.readFile(rootManifestPath, "utf8"));
  assertReleaseUpgrade(rootManifest.version, plan.version);

  const updatedManifestPaths = [];
  const updatedVersionPaths = [];
  const pendingWrites = [];

  for (const relativePath of packageManifestPaths) {
    const absolutePath = path.join(repoRoot, relativePath);
    const manifest = JSON.parse(await fs.readFile(absolutePath, "utf8"));
    const nextManifest = updatePackageManifest(manifest, plan.version);
    pendingWrites.push([absolutePath, `${JSON.stringify(nextManifest, null, 2)}\n`]);
    updatedManifestPaths.push(absolutePath);
    updatedVersionPaths.push(absolutePath);
  }

  for (const relativePath of rustCrateManifestPaths) {
    const absolutePath = path.join(repoRoot, relativePath);
    const contents = await fs.readFile(absolutePath, "utf8");
    pendingWrites.push([
      absolutePath,
      updateCargoTomlPackageVersion(contents, plan.version, relativePath),
    ]);
    updatedVersionPaths.push(absolutePath);
  }

  const cargoLockAbsolutePath = path.join(repoRoot, cargoLockPath);
  const cargoLockContents = await fs.readFile(cargoLockAbsolutePath, "utf8");
  const nextCargoLock = updateCargoLockWorkspacePackageVersions(
    cargoLockContents,
    plan.version,
  ).contents;
  pendingWrites.push([cargoLockAbsolutePath, nextCargoLock]);
  updatedVersionPaths.push(cargoLockAbsolutePath);

  for (const [absolutePath, contents] of pendingWrites) {
    await fs.writeFile(absolutePath, contents);
  }

  return {
    version: plan.version,
    tag: plan.tag,
    isPrerelease: plan.isPrerelease,
    manifestPaths: updatedManifestPaths,
    versionFilePaths: updatedVersionPaths,
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
  console.log(`versionFileCount=${result.versionFilePaths.length}`);
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
