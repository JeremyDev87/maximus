import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { chmod, copyFile, mkdir, mkdtemp, readFile, rename, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");

const [packJsonPath, fixtureArg] = process.argv.slice(2);

if (!packJsonPath) {
  console.error(
    "Usage: node ./scripts/run-packed-wrapper-smoke.mjs <npm-pack-json-path> <target-dir>",
  );
  process.exit(1);
}

const fixtureDir = path.resolve(fixtureArg ?? path.join(repoRoot, "test/fixtures/clean-project"));
const platformPackage = resolvePlatformPackage();

if (!platformPackage) {
  throw new Error(formatUnsupportedPlatformMessage());
}

const tempRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-packed-wrapper-"));
try {
  const rootTarball = await resolveRootTarball(packJsonPath);
  const platformTarball = await packPlatformPackage(tempRoot, platformPackage.directory);
  const rustBinary = await ensureRustBinary();
  const installRoot = await installPackedPackages(
    tempRoot,
    rootTarball,
    platformTarball,
    platformPackage.packageName,
  );

  await patchInstalledBinary(installRoot, platformPackage.packageName, rustBinary);

  const wrapper = path.join(installRoot, "node_modules", "maximus", "bin", "maximus.js");
  runWrapper(wrapper, ["audit", fixtureDir], installRoot);
  runWrapper(wrapper, ["doctor", fixtureDir], installRoot);
  runWrapper(wrapper, ["fix", fixtureDir, "--dry-run"], installRoot);

  console.log(`Packed wrapper smoke passed via ${path.basename(rootTarball)}.`);
} finally {
  await rm(tempRoot, { recursive: true, force: true });
}

async function resolveRootTarball(jsonPath) {
  const absoluteJsonPath = path.resolve(jsonPath);
  const packResult = JSON.parse(await readFile(absoluteJsonPath, "utf8"));
  const filename = packResult[0]?.filename;

  assert.equal(typeof filename, "string", "npm pack JSON must include filename");

  return path.isAbsolute(filename) ? filename : path.join(repoRoot, filename);
}

function resolvePlatformPackage() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return {
      packageName: "maximus-darwin-arm64",
      directory: path.join(repoRoot, "npm", "maximus-darwin-arm64"),
    };
  }

  if (process.platform === "darwin" && process.arch === "x64") {
    return {
      packageName: "maximus-darwin-x64",
      directory: path.join(repoRoot, "npm", "maximus-darwin-x64"),
    };
  }

  if (process.platform === "linux" && process.arch === "arm64" && hasGlibcRuntime()) {
    return {
      packageName: "maximus-linux-arm64-gnu",
      directory: path.join(repoRoot, "npm", "maximus-linux-arm64-gnu"),
    };
  }

  if (process.platform === "linux" && process.arch === "x64" && hasGlibcRuntime()) {
    return {
      packageName: "maximus-linux-x64-gnu",
      directory: path.join(repoRoot, "npm", "maximus-linux-x64-gnu"),
    };
  }

  return null;
}

function hasGlibcRuntime() {
  return Boolean(process.report?.getReport?.().header?.glibcVersionRuntime);
}

function formatUnsupportedPlatformMessage() {
  if (process.platform === "linux" && !hasGlibcRuntime()) {
    return "Packed wrapper smoke does not support Linux musl yet.";
  }

  return `Packed wrapper smoke does not support ${process.platform}-${process.arch}.`;
}

async function packPlatformPackage(tempRoot, packageDir) {
  const packOutput = runCommand(
    "npm",
    ["pack", "--json", "--pack-destination", tempRoot],
    {
      cwd: packageDir,
      env: {
        ...process.env,
        npm_config_cache: path.join(tempRoot, ".npm-cache"),
      },
      encoding: "utf8",
    },
  );
  const packResult = JSON.parse(packOutput.stdout);
  const filename = packResult[0]?.filename;

  assert.equal(typeof filename, "string", "platform npm pack JSON must include filename");

  return path.join(tempRoot, filename);
}

async function installPackedPackages(tempRoot, rootTarball, platformTarball, platformPackageName) {
  const installRoot = path.join(tempRoot, "install");
  const nodeModulesRoot = path.join(installRoot, "node_modules");

  await mkdir(nodeModulesRoot, { recursive: true });

  await unpackTarball(rootTarball, path.join(tempRoot, "root-extract"));
  await unpackTarball(platformTarball, path.join(tempRoot, "platform-extract"));

  await rename(
    path.join(tempRoot, "root-extract", "package"),
    path.join(nodeModulesRoot, "maximus"),
  );
  await rename(
    path.join(tempRoot, "platform-extract", "package"),
    path.join(nodeModulesRoot, platformPackageName),
  );

  return installRoot;
}

async function ensureRustBinary() {
  const debugBinary = path.join(repoRoot, "target", "debug", "maximus");

  const build = spawnSync("cargo", ["build", "-q", "-p", "maximus-cli"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (build.status !== 0) {
    throw new Error(build.stderr || build.stdout || "cargo build failed");
  }

  return debugBinary;
}

async function patchInstalledBinary(installRoot, packageName, rustBinary) {
  const installedBinary = path.join(installRoot, "node_modules", packageName, "bin", "maximus");
  await copyFile(rustBinary, installedBinary);
  await chmod(installedBinary, 0o755);
}

async function unpackTarball(tarball, destination) {
  await mkdir(destination, { recursive: true });
  runCommand("tar", ["-xzf", tarball, "-C", destination], {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

function runWrapper(wrapper, args, cwd) {
  const result = spawnSync(process.execPath, [wrapper, ...args], {
    cwd,
    encoding: "utf8",
  });

  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `wrapper command failed: ${args.join(" ")}`);
  }
}

function runCommand(command, args, options) {
  const result = spawnSync(command, args, {
    ...options,
  });

  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `${command} ${args.join(" ")} failed`);
  }

  return result;
}
