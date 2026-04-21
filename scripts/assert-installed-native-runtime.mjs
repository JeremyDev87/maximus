import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { constants as fsConstants } from "node:fs";
import { access, open } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const PLACEHOLDER_MARKER = "MAXIMUS_RUST_BINARY_PLACEHOLDER";

export async function assertInstalledNativeRuntime(installRoot) {
  const runtime = await inspectInstalledNativeRuntime(installRoot);

  if (runtime.state === "unsupported") {
    throw new Error(runtime.message);
  }

  if (runtime.state === "missing") {
    throw new Error(
      `Expected installed runtime ${runtime.packageName} at "${runtime.binaryPath}", but no binary was installed.`,
    );
  }

  if (runtime.state === "not-runnable") {
    throw new Error(
      `Installed runtime for ${runtime.packageName} exists at "${runtime.binaryPath}", but it is not executable.`,
    );
  }

  if (runtime.state === "placeholder") {
    throw new Error(
      `Installed runtime for ${runtime.packageName} is still the placeholder binary at "${runtime.binaryPath}".`,
    );
  }

  return {
    packageName: runtime.packageName,
    binaryPath: runtime.binaryPath,
  };
}

export async function inspectInstalledNativeRuntime(installRoot) {
  const runtimePackage = resolveRuntimePackage();

  if (!runtimePackage) {
    return {
      state: "unsupported",
      packageName: null,
      binaryPath: null,
      message: formatUnsupportedPlatformMessage(),
    };
  }

  const binaryPath = path.join(
    installRoot,
    "node_modules",
    runtimePackage.packageName,
    "bin",
    "maximus",
  );

  if (!(await isAccessible(binaryPath, fsConstants.F_OK))) {
    return {
      state: "missing",
      packageName: runtimePackage.packageName,
      binaryPath,
    };
  }

  if (!(await isAccessible(binaryPath, fsConstants.X_OK))) {
    return {
      state: "not-runnable",
      packageName: runtimePackage.packageName,
      binaryPath,
    };
  }

  if (!(await isAccessible(binaryPath, fsConstants.R_OK))) {
    if (!(await probeExecutable(binaryPath))) {
      return {
        state: "not-runnable",
        packageName: runtimePackage.packageName,
        binaryPath,
      };
    }

    return {
      state: "installed",
      packageName: runtimePackage.packageName,
      binaryPath,
    };
  }

  if (await isPlaceholderRuntime(binaryPath)) {
    return {
      state: "placeholder",
      packageName: runtimePackage.packageName,
      binaryPath,
    };
  }

  return {
    state: "installed",
    packageName: runtimePackage.packageName,
    binaryPath,
  };
}

function resolveRuntimePackage() {
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

  if (process.platform === "linux" && process.arch === "arm64" && hasGlibcRuntime()) {
    return {
      packageName: "@jeremyfellaz/maximus-linux-arm64-gnu",
      label: "linux-arm64-gnu",
    };
  }

  if (process.platform === "linux" && process.arch === "x64" && hasGlibcRuntime()) {
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
    return "Installed native runtime assertion does not support Linux musl yet.";
  }

  return `Installed native runtime assertion does not support ${process.platform}-${process.arch}.`;
}

async function isPlaceholderRuntime(binaryPath) {
  const file = await open(binaryPath, "r");

  try {
    const buffer = Buffer.alloc(512);
    const { bytesRead } = await file.read(buffer, 0, buffer.length, 0);
    return buffer.subarray(0, bytesRead).includes(PLACEHOLDER_MARKER);
  } finally {
    await file.close();
  }
}

async function isAccessible(binaryPath, mode) {
  try {
    await access(binaryPath, mode);
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

const isDirectExecution =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isDirectExecution) {
  try {
    const installRoot = process.argv[2];
    assert.equal(typeof installRoot, "string", "usage: node ./scripts/assert-installed-native-runtime.mjs <install-root>");
    const result = await assertInstalledNativeRuntime(path.resolve(installRoot));
    console.log(`Verified native runtime ${result.packageName} at ${result.binaryPath}`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Native runtime assertion failed: ${message}`);
    process.exitCode = 1;
  }
}
