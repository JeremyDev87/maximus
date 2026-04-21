import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";
import { chmod, cp, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import {
  assertInstalledNativeRuntime,
  inspectInstalledNativeRuntime,
} from "../scripts/assert-installed-native-runtime.mjs";

test("installed native runtime assertion accepts a non-placeholder runtime package on supported hosts", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the runtime assertion");
    return;
  }

  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-installed-runtime-ok-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });
  const binaryPath = path.join(installRoot, "node_modules", runtimePackage.packageName, "bin", "maximus");
  await writeFile(binaryPath, "#!/bin/sh\necho native-runtime\n", "utf8");
  await chmod(binaryPath, 0o755);

  const result = await assertInstalledNativeRuntime(installRoot);

  assert.deepEqual(result, {
    packageName: runtimePackage.packageName,
    binaryPath,
  });
});

test("installed native runtime assertion rejects placeholder runtime binaries", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the runtime assertion");
    return;
  }

  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-installed-runtime-placeholder-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });
  const binaryPath = path.join(installRoot, "node_modules", runtimePackage.packageName, "bin", "maximus");
  await cp(path.join(process.cwd(), "npm", runtimePackage.directoryName, "bin", "maximus"), binaryPath);
  await chmod(binaryPath, 0o755);

  await assert.rejects(
    () => assertInstalledNativeRuntime(installRoot),
    /placeholder binary/,
  );
});

test("installed native runtime inspection reports a missing optional runtime package", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the runtime assertion");
    return;
  }

  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-installed-runtime-missing-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  const binaryPath = path.join(
    installRoot,
    "node_modules",
    runtimePackage.packageName,
    "bin",
    "maximus",
  );

  const result = await inspectInstalledNativeRuntime(installRoot);

  assert.deepEqual(result, {
    state: "missing",
    packageName: runtimePackage.packageName,
    binaryPath,
  });
  await assert.rejects(
    () => assertInstalledNativeRuntime(installRoot),
    /no binary was installed/,
  );
});

test("installed native runtime assertion rejects non-executable runtime binaries", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the runtime assertion");
    return;
  }

  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-installed-runtime-not-runnable-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });
  const binaryPath = path.join(installRoot, "node_modules", runtimePackage.packageName, "bin", "maximus");
  await writeFile(binaryPath, "#!/bin/sh\necho native-runtime\n", "utf8");
  await chmod(binaryPath, 0o644);

  const result = await inspectInstalledNativeRuntime(installRoot);

  assert.deepEqual(result, {
    state: "not-runnable",
    packageName: runtimePackage.packageName,
    binaryPath,
  });
  await assert.rejects(
    () => assertInstalledNativeRuntime(installRoot),
    /not executable/,
  );
});

test("installed native runtime assertion accepts execute-only runtime binaries", async (t) => {
  if (process.platform === "win32") {
    t.skip("Windows does not model execute-only file permissions");
    return;
  }

  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the runtime assertion");
    return;
  }

  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-installed-runtime-exec-only-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });
  const binaryPath = path.join(installRoot, "node_modules", runtimePackage.packageName, "bin", "maximus");
  await cp("/usr/bin/true", binaryPath);
  await chmod(binaryPath, 0o111);

  const result = await inspectInstalledNativeRuntime(installRoot);

  assert.deepEqual(result, {
    state: "installed",
    packageName: runtimePackage.packageName,
    binaryPath,
  });
  await assert.doesNotReject(
    () => assertInstalledNativeRuntime(installRoot),
  );
});

test("installed native runtime assertion rejects execute-only placeholder binaries", async (t) => {
  if (process.platform === "win32") {
    t.skip("Windows does not model execute-only file permissions");
    return;
  }

  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the runtime assertion");
    return;
  }

  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-installed-runtime-exec-only-placeholder-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });
  const binaryPath = path.join(installRoot, "node_modules", runtimePackage.packageName, "bin", "maximus");
  await cp(path.join(process.cwd(), "npm", runtimePackage.directoryName, "bin", "maximus"), binaryPath);
  await chmod(binaryPath, 0o111);

  const result = await inspectInstalledNativeRuntime(installRoot);

  assert.deepEqual(result, {
    state: "not-runnable",
    packageName: runtimePackage.packageName,
    binaryPath,
  });
  await assert.rejects(
    () => assertInstalledNativeRuntime(installRoot),
    /not executable/,
  );
});

function currentRuntimePackage() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return {
      packageName: "@jeremyfellaz/maximus-darwin-arm64",
      directoryName: "maximus-darwin-arm64",
    };
  }

  if (process.platform === "darwin" && process.arch === "x64") {
    return {
      packageName: "@jeremyfellaz/maximus-darwin-x64",
      directoryName: "maximus-darwin-x64",
    };
  }

  if (
    process.platform === "linux" &&
    process.arch === "arm64" &&
    process.report?.getReport?.().header?.glibcVersionRuntime
  ) {
    return {
      packageName: "@jeremyfellaz/maximus-linux-arm64-gnu",
      directoryName: "maximus-linux-arm64-gnu",
    };
  }

  if (
    process.platform === "linux" &&
    process.arch === "x64" &&
    process.report?.getReport?.().header?.glibcVersionRuntime
  ) {
    return {
      packageName: "@jeremyfellaz/maximus-linux-x64-gnu",
      directoryName: "maximus-linux-x64-gnu",
    };
  }

  return null;
}
