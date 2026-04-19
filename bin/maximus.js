#!/usr/bin/env node

import { spawn } from "node:child_process";
import { access } from "node:fs/promises";
import { createRequire } from "node:module";
import { constants as osConstants } from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const cliArgs = process.argv.slice(2);
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

try {
  const runtime = await resolveRuntime();

  if (runtime.kind === "binary") {
    await runBinary(runtime.command, cliArgs);
  } else {
    await runJsReference(cliArgs);
  }
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Maximus failed: ${message}`);
  process.exitCode = process.exitCode || 1;
}

async function resolveRuntime() {
  const platformPackage = resolvePlatformPackage();
  if (platformPackage) {
    const installedBinary = await resolveInstalledBinary(platformPackage.packageName);
    if (installedBinary) {
      return { kind: "binary", command: installedBinary };
    }
  }

  const repoBinary = await resolveRepoBinary();
  if (repoBinary) {
    return { kind: "binary", command: repoBinary };
  }

  if (await hasJsReferenceRuntime()) {
    return { kind: "js" };
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
    return "Linux musl is not supported yet. Maximus currently ships prebuilt Rust binaries only for Linux glibc and macOS.";
  }

  return `Unsupported platform ${process.platform}-${process.arch}. Maximus currently ships prebuilt Rust binaries only for darwin-arm64, darwin-x64, linux-arm64-gnu, and linux-x64-gnu.`;
}

async function resolveInstalledBinary(packageName) {
  try {
    const packageJsonPath = require.resolve(`${packageName}/package.json`);
    const binaryPath = path.join(path.dirname(packageJsonPath), "bin", "maximus");

    await access(binaryPath);
    return binaryPath;
  } catch {
    return null;
  }
}

async function resolveRepoBinary() {
  for (const candidate of [
    path.join(repoRoot, "target", "release", "maximus"),
    path.join(repoRoot, "target", "debug", "maximus"),
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

async function hasJsReferenceRuntime() {
  try {
    await access(path.join(repoRoot, "src", "cli.js"));
    return true;
  } catch {
    return false;
  }
}

function formatMissingRuntimeMessage(platformPackage) {
  return [
    `No runtime is available for ${platformPackage.label}.`,
    `Expected optional dependency "${platformPackage.packageName}" to be installed.`,
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

async function runJsReference(args) {
  const { runCli } = await import("../src/cli.js");
  await runCli(args);
  process.exitCode = process.exitCode ?? 0;
}

function signalExitCode(signal) {
  const signalNumber = osConstants.signals?.[signal];
  return typeof signalNumber === "number" ? 128 + signalNumber : 1;
}
