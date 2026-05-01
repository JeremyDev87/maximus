#!/usr/bin/env node

import { spawn } from "node:child_process";
import { constants as fsConstants } from "node:fs";
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
  "Rust가 Maximus의 canonical runtime입니다. 포함된 JS reference는 frozen 상태이며 legacy 호환 명령을 위한 임시 compatibility bridge로만 유지됩니다.";

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
  console.error(`Maximus 실패: ${message}`);
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
      `${fallback.reason} 이 명령을 사용하기 전에 \`cargo build -p maximus-cli\`로 Rust CLI를 빌드하거나 지원되는 native runtime package를 설치하세요.`,
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
      packageName: "@jeremyfellaz/maximus-darwin-arm64",
      label: "darwin-arm64",
    };
  }

  if (process.platform === "darwin" && process.arch === "x64") {
    return {
      packageName: "@jeremyfellaz/maximus-darwin-x64",
      label: "darwin-x64",
    };
  }

  if (process.platform === "linux" && process.arch === "arm64") {
    if (!hasGlibcRuntime()) {
      return null;
    }

    return {
      packageName: "@jeremyfellaz/maximus-linux-arm64-gnu",
      label: "linux-arm64-gnu",
    };
  }

  if (process.platform === "linux" && process.arch === "x64") {
    if (!hasGlibcRuntime()) {
      return null;
    }

    return {
      packageName: "@jeremyfellaz/maximus-linux-x64-gnu",
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
    return `Linux musl은 아직 지원하지 않습니다. Maximus는 현재 Linux glibc와 macOS용 prebuilt Rust binary만 제공합니다. ${frozenJsReferenceNote}`;
  }

  return `지원하지 않는 플랫폼입니다: ${process.platform}-${process.arch}. Maximus는 현재 darwin-arm64, darwin-x64, linux-arm64-gnu, linux-x64-gnu용 prebuilt Rust binary만 제공합니다. ${frozenJsReferenceNote}`;
}

async function resolveInstalledBinary(packageName) {
  try {
    const packageJsonPath = require.resolve(`${packageName}/package.json`);
    const binaryPath = path.join(path.dirname(packageJsonPath), "bin", "maximus");

    await access(binaryPath, fsConstants.X_OK);
    if (!(await isAccessible(binaryPath, fsConstants.R_OK))) {
      if (!(await probeExecutable(binaryPath))) {
        return null;
      }

      return binaryPath;
    }

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
      reason: `frozen JS compatibility path에서 지원하지 않는 옵션에는 Rust runtime이 필요합니다: ${parsed.unsupportedFlags.join(", ")}. ${frozenJsReferenceNote}`,
    };
  }

  const configPath = await findNearestConfigPath(parsed.targetDir);
  if (configPath) {
    return {
      allowed: false,
      reason: `Maximus config file이 있을 때는 Rust runtime이 필요합니다 (${configPath}). ${frozenJsReferenceNote}`,
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

  if (
    args[0] === "help"
    || args.includes("--help")
    || args.includes("-h")
  ) {
    return {
      mode: "compat-help",
      targetDir: process.cwd(),
      unsupportedFlags: [],
    };
  }

  let command;
  let pathArg;
  const unsupportedFlags = [];
  const commandNames = new Set(["audit", "doctor", "fix"]);
  const valueFlags = new Set(["--only", "--skip", "--fail-on", "--fix-id", "--fix-prefix", "--format", "--output"]);
  const passthroughFlags = new Set(["--dry-run", "--json"]);

  for (let index = 0; index < args.length; index += 1) {
    const token = args[index];
    const valueFlagWithEquals = Array.from(valueFlags).find((flag) => token.startsWith(`${flag}=`));

    if (valueFlags.has(token)) {
      unsupportedFlags.push(token);
      index += 1;
      continue;
    }

    if (valueFlagWithEquals) {
      unsupportedFlags.push(valueFlagWithEquals);
      continue;
    }

    if (token === "--diff" || token.startsWith("--diff=")) {
      unsupportedFlags.push("--diff");
      continue;
    }

    if (passthroughFlags.has(token) || token === "--help" || token === "-h") {
      continue;
    }

    if (command === undefined && commandNames.has(token)) {
      command = token;
      continue;
    }

    if (pathArg === undefined) {
      pathArg = token;
    }
  }

  const isFixCommand = command === "fix";
  const hasDryRun = args.includes("--dry-run");

  if (isFixCommand && !hasDryRun) {
    unsupportedFlags.push("fix (without --dry-run)");
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
    `${platformPackage.label}에서 사용할 수 있는 Rust runtime이 없습니다.`,
    `optional dependency "${platformPackage.packageName}"가 설치되어 있거나 target/debug 또는 target/release에 local Cargo build가 있어야 합니다.`,
    frozenJsReferenceNote,
    "repository 안에서 개발 중이라면 먼저 `cargo build -p maximus-cli`로 Rust CLI를 빌드하세요.",
  ].join(" ");
}

async function runBinary(command, args) {
  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: "inherit",
    });

    child.on("error", (error) => {
      reject(new Error(`"${command}"에서 Rust CLI를 실행하지 못했습니다: ${error.message}`));
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
    "혼란스러운 설정을 정리합니다.",
    "",
    "사용법",
    "  maximus audit [path] [--json]",
    "  maximus doctor [path] [--json]",
    "  maximus fix [path] --dry-run [--json]",
    "  maximus help",
    "",
    "Rust가 Maximus의 canonical runtime입니다. Rust runtime이 없을 때 포함된 JS compatibility path는 Maximus config file이나 Rust 전용 flag가 없는 legacy 호환 명령을 위한 frozen reference fallback으로만 동작합니다. `--only`, `--skip`, `--fail-on`, `--diff`, `--fix-id`, `--fix-prefix`, `--format`, `--output`에는 Rust runtime이 필요하고, `fix`는 `--dry-run`과 함께 사용할 때만 가능합니다.",
  ].join("\n");
}

function signalExitCode(signal) {
  const signalNumber = osConstants.signals?.[signal];
  return typeof signalNumber === "number" ? 128 + signalNumber : 1;
}

async function isAccessible(targetPath, mode) {
  try {
    await access(targetPath, mode);
    return true;
  } catch {
    return false;
  }
}

async function probeExecutable(binaryPath) {
  return await new Promise((resolve) => {
    const child = spawn(binaryPath, ["--help"], {
      stdio: "ignore",
    });
    let settled = false;
    const settle = (result) => {
      if (!settled) {
        settled = true;
        resolve(result);
      }
    };

    const timeout = setTimeout(() => {
      child.kill("SIGKILL");
      settle(false);
    }, 1000);

    child.on("error", () => {
      clearTimeout(timeout);
      settle(false);
    });

    child.on("exit", (code, signal) => {
      clearTimeout(timeout);
      settle(!signal && code === 0);
    });
  });
}
