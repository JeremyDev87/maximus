import assert from "node:assert/strict";
import path from "node:path";
import process from "node:process";
import { open } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const PLACEHOLDER_MARKER = "MAXIMUS_RUST_BINARY_PLACEHOLDER";

export async function assertInstalledNativeRuntime(installRoot) {
  const runtimePackage = resolveRuntimePackage();
  assert(runtimePackage, formatUnsupportedPlatformMessage());

  const binaryPath = path.join(
    installRoot,
    "node_modules",
    runtimePackage.packageName,
    "bin",
    "maximus",
  );

  if (await isPlaceholderRuntime(binaryPath)) {
    throw new Error(
      `Installed runtime for ${runtimePackage.packageName} is still the placeholder binary at "${binaryPath}".`,
    );
  }

  return {
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
