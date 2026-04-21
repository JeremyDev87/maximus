import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import test from "node:test";
import { chmod, cp, mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import { resolvePackedWrapperLaunch } from "../scripts/lib/packed-wrapper-launch.mjs";

const cleanFixtureDir = path.join(process.cwd(), "test", "fixtures", "clean-project");

test("packed install without the optional runtime keeps legacy-compatible commands working", async (t) => {
  const installRoot = await createPackedFallbackInstall(t);

  for (const args of [
    ["audit", cleanFixtureDir],
    ["doctor", cleanFixtureDir],
    ["fix", cleanFixtureDir, "--dry-run"],
  ]) {
    const result = await runPackedWrapper(installRoot, args);

    assert.equal(result.code, 0, `expected maximus ${args.join(" ")} to succeed`);
    assert.equal(result.stderr.trim(), "");
  }
});

test("packed install without the optional runtime blocks config files, Rust-only flags, and fix without dry-run", async (t) => {
  const installRoot = await createPackedFallbackInstall(t);
  const configFixture = path.join(installRoot, "project-with-config");
  await mkdir(configFixture, { recursive: true });
  await writeFile(
    path.join(configFixture, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
    "utf8",
  );

  const configResult = await runPackedWrapper(installRoot, ["audit"], { cwd: configFixture });
  assert.equal(configResult.code, 1);
  assert.equal(configResult.stdout.trim(), "");
  assert.match(
    configResult.stderr,
    /A Rust runtime is required when a Maximus config file is present/,
  );

  const rustOnlyResult = await runPackedWrapper(installRoot, [
    "audit",
    cleanFixtureDir,
    "--only",
    "env",
  ]);
  assert.equal(rustOnlyResult.code, 1);
  assert.equal(rustOnlyResult.stdout.trim(), "");
  assert.match(
    rustOnlyResult.stderr,
    /A Rust runtime is required for options not supported by the frozen JS compatibility path/,
  );
  assert.match(rustOnlyResult.stderr, /--only/);

  const fixResult = await runPackedWrapper(installRoot, ["fix", cleanFixtureDir]);
  assert.equal(fixResult.code, 1);
  assert.equal(fixResult.stdout.trim(), "");
  assert.match(fixResult.stderr, /fix \(without --dry-run\)/);
});

async function createPackedFallbackInstall(t) {
  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-packed-fallback-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  const packageRoot = path.join(installRoot, "node_modules", "@jeremyfellaz", "maximus");
  await mkdir(path.join(packageRoot, "bin"), { recursive: true });
  await mkdir(path.join(installRoot, "node_modules", ".bin"), { recursive: true });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(packageRoot, "bin", "maximus.js"));
  await cp(path.join(process.cwd(), "package.json"), path.join(packageRoot, "package.json"));
  await cp(path.join(process.cwd(), "src"), path.join(packageRoot, "src"), {
    recursive: true,
  });
  await chmod(path.join(packageRoot, "bin", "maximus.js"), 0o755);
  await symlink(
    "../@jeremyfellaz/maximus/bin/maximus.js",
    path.join(installRoot, "node_modules", ".bin", "maximus"),
  );

  return installRoot;
}

async function runPackedWrapper(installRoot, args, options = {}) {
  const launch = await resolvePackedWrapperLaunch(installRoot);

  return await new Promise((resolve, reject) => {
    const stdout = [];
    const stderr = [];
    const child = spawn(launch.command, [...launch.args, ...args], {
      cwd: options.cwd ?? installRoot,
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
