#!/usr/bin/env node

import { spawn } from "node:child_process";
import { access, open, realpath, stat } from "node:fs/promises";
import { createRequire } from "node:module";
import { constants as osConstants } from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const cliArgs = process.argv.slice(2);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const frozenJsReferenceNote =
  "Rust is the canonical Maximus runtime. The bundled JS reference is frozen and only kept as a temporary compatibility bridge for legacy-compatible commands.";

try {
  const runtime = await resolveRuntime(cliArgs);

  if (runtime.kind === "binary") {
    await runBinary(runtime.command, cliArgs);
  } else if (runtime.kind === "compat-help") {
    console.log(formatCompatHelp());
    process.exitCode = 0;
  } else {
    await runFrozenJsReference(cliArgs);
  }
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Maximus failed: ${message}`);
  process.exitCode = process.exitCode || 1;
}

async function resolveRuntime(args) {
  const repoBinary = await resolveRepoBinary();
  if (repoBinary) {
    return { kind: "binary", command: repoBinary };
  }

  const platformPackage = resolvePlatformPackage();
  if (platformPackage) {
    const installedBinary = await resolveInstalledBinary(platformPackage.packageName);
    if (installedBinary) {
      return { kind: "binary", command: installedBinary };
    }
  }

  const fallback = await evaluateFrozenJsFallback(args);
  if (fallback.allowed) {
    return fallback.runtime;
  }
  if (fallback.reason) {
    throw new Error(
      `${fallback.reason} Build the Rust CLI with \`cargo build -p maximus-cli\`, or install a supported native runtime package before using this command.`,
    );
  }

  if (!platformPackage) {
    throw new Error(formatUnsupportedPlatformMessage());
  }

  throw new Error(formatMissingRuntimeMessage(platformPackage));
}

function resolvePlatformPackage() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return {
      packageName: "maximus-darwin-arm64",
      label: "darwin-arm64",
    };
  }

  if (process.platform === "darwin" && process.arch === "x64") {
    return {
      packageName: "maximus-darwin-x64",
      label: "darwin-x64",
    };
  }

  if (process.platform === "linux" && process.arch === "arm64") {
    if (!hasGlibcRuntime()) {
      return null;
    }

    return {
      packageName: "maximus-linux-arm64-gnu",
      label: "linux-arm64-gnu",
    };
  }

  if (process.platform === "linux" && process.arch === "x64") {
    if (!hasGlibcRuntime()) {
      return null;
    }

    return {
      packageName: "maximus-linux-x64-gnu",
      label: "linux-x64-gnu",
    };
  }

  return null;
}

function hasGlibcRuntime() {
  return Boolean(process.report?.getReport?.().header?.glibcVersionRuntime);
}

function formatUnsupportedPlatformMessage() {
  if (process.platform === "linux" && !hasGlibcRuntime()) {
    return `Linux musl is not supported yet. Maximus currently ships prebuilt Rust binaries only for Linux glibc and macOS. ${frozenJsReferenceNote}`;
  }

  return `Unsupported platform ${process.platform}-${process.arch}. Maximus currently ships prebuilt Rust binaries only for darwin-arm64, darwin-x64, linux-arm64-gnu, and linux-x64-gnu. ${frozenJsReferenceNote}`;
}

async function resolveInstalledBinary(packageName) {
  try {
    const packageJsonPath = require.resolve(`${packageName}/package.json`);
    const binaryPath = path.join(path.dirname(packageJsonPath), "bin", "maximus");

    await access(binaryPath);
    if (await isPlaceholderRuntime(binaryPath)) {
      return null;
    }
    return binaryPath;
  } catch {
    return null;
  }
}

async function resolveRepoBinary() {
  for (const candidate of [
    path.join(repoRoot, "target", "debug", "maximus"),
    path.join(repoRoot, "target", "release", "maximus"),
  ]) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      continue;
    }
  }

  return null;
}

async function evaluateFrozenJsFallback(args) {
  if (!(await hasFrozenJsReferenceRuntime())) {
    return { allowed: false, reason: null };
  }

  const parsed = parseCompatInvocation(args);

  if (parsed.mode === "compat-help") {
    return { allowed: true, runtime: { kind: "compat-help" } };
  }

  if (parsed.unsupportedFlags.length > 0) {
    return {
      allowed: false,
      reason: `A Rust runtime is required for options not supported by the frozen JS compatibility path: ${parsed.unsupportedFlags.join(", ")}. ${frozenJsReferenceNote}`,
    };
  }

  const configPath = await findNearestConfigPath(parsed.targetDir);
  if (configPath) {
    return {
      allowed: false,
      reason: `A Rust runtime is required when a Maximus config file is present (${configPath}). ${frozenJsReferenceNote}`,
    };
  }

  return { allowed: true, runtime: { kind: "js" } };
}

function parseCompatInvocation(args) {
  if (args.length === 0) {
    return {
      mode: "compat-help",
      targetDir: process.cwd(),
      unsupportedFlags: [],
    };
  }

  const [command, ...rest] = args;
  if (
    command === "help"
    || command === "--help"
    || command === "-h"
    || rest.includes("--help")
    || rest.includes("-h")
  ) {
    return {
      mode: "compat-help",
      targetDir: process.cwd(),
      unsupportedFlags: [],
    };
  }

  let pathArg;
  const unsupportedFlags = [];
  const valueFlags = new Set(["--only", "--skip", "--fail-on", "--fix-id", "--fix-prefix"]);
  const passthroughFlags = new Set(["--dry-run", "--json"]);
  const isFixCommand = command === "fix";
  const hasDryRun = rest.includes("--dry-run");

  if (isFixCommand && !hasDryRun) {
    unsupportedFlags.push("fix (without --dry-run)");
  }

  for (let index = 0; index < rest.length; index += 1) {
    const token = rest[index];

    if (valueFlags.has(token)) {
      unsupportedFlags.push(token);
      index += 1;
      continue;
    }

    if (token === "--diff") {
      unsupportedFlags.push(token);
      continue;
    }

    if (passthroughFlags.has(token) || token === "--help" || token === "-h") {
      continue;
    }

    if (pathArg === undefined) {
      pathArg = token;
    }
  }

  return {
    mode: "js",
    targetDir: path.resolve(pathArg ?? process.cwd()),
    unsupportedFlags,
  };
}

async function findNearestConfigPath(startDir) {
  const resolvedStartDir = path.resolve(startDir);
  const startDirs = [];
  try {
    startDirs.push(await realpath(resolvedStartDir));
  } catch {
    // Fall back to lexical ancestor search when the target is not realpath-able.
  }
  startDirs.push(resolvedStartDir);
  const searchedDirs = new Set();

  for (let currentDir of startDirs) {
    while (true) {
      if (!searchedDirs.has(currentDir)) {
        searchedDirs.add(currentDir);
        for (const name of ["maximus.config.json", ".maximusrc.json"]) {
          const candidate = path.join(currentDir, name);

          try {
            if ((await stat(candidate)).isFile()) {
              return candidate;
            }
          } catch {
            // continue
          }
        }
      }

      const parentDir = path.dirname(currentDir);
      if (parentDir === currentDir) {
        break;
      }

      currentDir = parentDir;
    }
  }

  return null;
}

async function hasFrozenJsReferenceRuntime() {
  try {
    await access(path.join(repoRoot, "src", "cli.js"));
    return true;
  } catch {
    return false;
  }
}

async function isPlaceholderRuntime(binaryPath) {
  const file = await open(binaryPath, "r");

  try {
    const buffer = Buffer.alloc(512);
    const { bytesRead } = await file.read(buffer, 0, buffer.length, 0);
    return buffer.subarray(0, bytesRead).includes("MAXIMUS_RUST_BINARY_PLACEHOLDER");
  } finally {
    await file.close();
  }
}

function formatMissingRuntimeMessage(platformPackage) {
  return [
    `No Rust runtime is available for ${platformPackage.label}.`,
    `Expected optional dependency "${platformPackage.packageName}" to be installed or a local Cargo build to exist in target/debug or target/release.`,
    frozenJsReferenceNote,
    "If you are developing inside the repository, build the Rust CLI with `cargo build -p maximus-cli` first.",
  ].join(" ");
}

async function runBinary(command, args) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
    });

    child.on("error", (error) => {
      reject(new Error(`Failed to launch Rust CLI at "${command}": ${error.message}`));
    });

    child.on("exit", (code, signal) => {
      if (signal) {
        process.exitCode = signalExitCode(signal);
        resolve();
        return;
      }

      process.exitCode = code ?? 1;
      resolve();
    });
  });
}

async function runFrozenJsReference(args) {
  const { runCli } = await import("../src/cli.js");
  await runCli(args);
  process.exitCode = process.exitCode ?? 0;
}

function formatCompatHelp() {
  return [
    "Maximus",
    "",
    "Bring order to chaotic configs.",
    "",
    "Usage",
    "  maximus audit [path] [--json]",
    "  maximus doctor [path] [--json]",
    "  maximus fix [path] --dry-run [--json]",
    "  maximus help",
    "",
    "Rust is the canonical Maximus runtime. When no Rust runtime is available, the bundled JS compatibility path stays as frozen reference-only fallback for legacy-compatible commands without Maximus config files or Rust-only flags. `--only`, `--skip`, `--fail-on`, `--diff`, `--fix-id`, and `--fix-prefix` require the Rust runtime, and `fix` is only available with `--dry-run`.",
  ].join("\n");
}

function signalExitCode(signal) {
  const signalNumber = osConstants.signals?.[signal];
  return typeof signalNumber === "number" ? 128 + signalNumber : 1;
}
