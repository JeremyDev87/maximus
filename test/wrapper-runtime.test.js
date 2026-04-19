import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { chmod, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";

test("wrapper preserves conventional exit code when the child terminates by signal", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the wrapper");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-runtime-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "node_modules", runtimePackage, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage, "bin", "maximus"),
    "#!/bin/sh\nkill -s INT $$\n",
    "utf8",
  );
  await chmod(path.join(rootDir, "node_modules", runtimePackage, "bin", "maximus"), 0o755);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 130);
  assert.equal(result.stdout.trim(), "");
  assert.equal(result.stderr.trim(), "");
});

test("wrapper falls back to the JS reference runtime on unsupported platforms", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-unsupported-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await writeFile(
    path.join(rootDir, "src", "cli.js"),
    [
      "export async function runCli(args) {",
      "  console.log(JSON.stringify(args));",
      "}",
      "",
    ].join("\n"),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "bootstrap.mjs"),
    [
      'import path from "node:path";',
      'import { pathToFileURL } from "node:url";',
      "",
      'Object.defineProperty(process, "platform", { value: "win32" });',
      'Object.defineProperty(process, "arch", { value: "x64" });',
      "",
      'await import(pathToFileURL(path.join(process.cwd(), "bin", "maximus.js")));',
      "",
    ].join("\n"),
    "utf8",
  );

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir);

  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.stdout.trim()), ["audit", "."]);
  assert.equal(result.stderr.trim(), "");
});

test("wrapper ignores placeholder native packages and falls back to the JS reference runtime", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the wrapper");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-placeholder-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await mkdir(path.join(rootDir, "node_modules", runtimePackage, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await cp(
    path.join(process.cwd(), "npm", runtimePackage, "bin", "maximus"),
    path.join(rootDir, "node_modules", runtimePackage, "bin", "maximus"),
  );
  await writeFile(
    path.join(rootDir, "src", "cli.js"),
    [
      "export async function runCli(args) {",
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
      "}",
      "",
    ].join("\n"),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await chmod(path.join(rootDir, "node_modules", runtimePackage, "bin", "maximus"), 0o755);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.stdout.trim()), {
    runtime: "js",
    args: ["audit", "."],
  });
  assert.equal(result.stderr.trim(), "");
});

test("wrapper prefers repository debug binaries over installed packages and release binaries", async (t) => {
  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the wrapper");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-precedence-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "target", "debug"), { recursive: true });
  await mkdir(path.join(rootDir, "target", "release"), { recursive: true });
  await mkdir(path.join(rootDir, "node_modules", runtimePackage, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await writeFile(path.join(rootDir, "target", "debug", "maximus"), "#!/bin/sh\necho debug-runtime\n", "utf8");
  await writeFile(path.join(rootDir, "target", "release", "maximus"), "#!/bin/sh\necho release-runtime\n", "utf8");
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage, "bin", "maximus"),
    "#!/bin/sh\necho installed-runtime\n",
    "utf8",
  );
  await chmod(path.join(rootDir, "target", "debug", "maximus"), 0o755);
  await chmod(path.join(rootDir, "target", "release", "maximus"), 0o755);
  await chmod(path.join(rootDir, "node_modules", runtimePackage, "bin", "maximus"), 0o755);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 0);
  assert.equal(result.stdout.trim(), "debug-runtime");
  assert.equal(result.stderr.trim(), "");
});

function currentRuntimePackage() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return "maximus-darwin-arm64";
  }

  if (process.platform === "darwin" && process.arch === "x64") {
    return "maximus-darwin-x64";
  }

  if (
    process.platform === "linux" &&
    process.arch === "arm64" &&
    process.report?.getReport?.().header?.glibcVersionRuntime
  ) {
    return "maximus-linux-arm64-gnu";
  }

  if (
    process.platform === "linux" &&
    process.arch === "x64" &&
    process.report?.getReport?.().header?.glibcVersionRuntime
  ) {
    return "maximus-linux-x64-gnu";
  }

  return null;
}

async function runWrapper(wrapperPath, cwd) {
  return await new Promise((resolve, reject) => {
    const stdout = [];
    const stderr = [];
    const child = spawn(process.execPath, [wrapperPath, "audit", "."], {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
    });

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code, signal) => {
      resolve({
        code,
        signal,
        stdout: stdout.join(""),
        stderr: stderr.join(""),
      });
    });
  });
}
