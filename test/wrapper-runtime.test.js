import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { chmod, cp, mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
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
  await mkdir(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage.packageName, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage.packageName,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"),
    "#!/bin/sh\nkill -s INT $$\n",
    "utf8",
  );
  await chmod(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"), 0o755);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 130);
  assert.equal(result.stdout.trim(), "");
  assert.equal(result.stderr.trim(), "");
});

test("wrapper falls back to the JS reference runtime on unsupported platforms for legacy-compatible commands", async (t) => {
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

test("wrapper ignores placeholder native packages and falls back to the JS reference runtime for legacy-compatible commands", async (t) => {
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
  await mkdir(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await cp(
    path.join(process.cwd(), "npm", runtimePackage.directoryName, "bin", "maximus"),
    path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"),
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
    path.join(rootDir, "node_modules", runtimePackage.packageName, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage.packageName,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await chmod(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"), 0o755);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.stdout.trim()), {
    runtime: "js",
    args: ["audit", "."],
  });
  assert.equal(result.stderr.trim(), "");
});

test("wrapper accepts execute-only installed runtime binaries", async (t) => {
  if (process.platform === "win32") {
    t.skip("Windows does not model execute-only file permissions");
    return;
  }

  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the wrapper");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-exec-only-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage.packageName, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage.packageName,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await cp("/usr/bin/true", path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"));
  await chmod(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"), 0o111);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 0);
  assert.equal(result.stdout.trim(), "");
  assert.equal(result.stderr.trim(), "");
});

test("wrapper ignores execute-only placeholder runtimes and falls back to the JS reference runtime", async (t) => {
  if (process.platform === "win32") {
    t.skip("Windows does not model execute-only file permissions");
    return;
  }

  const runtimePackage = currentRuntimePackage();
  if (!runtimePackage) {
    t.skip("current platform is intentionally unsupported by the wrapper");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-exec-only-placeholder-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await mkdir(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await cp(
    path.join(process.cwd(), "npm", runtimePackage.directoryName, "bin", "maximus"),
    path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"),
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
    path.join(rootDir, "node_modules", runtimePackage.packageName, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage.packageName,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await chmod(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"), 0o111);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.stdout.trim()), {
    runtime: "js",
    args: ["audit", "."],
  });
  assert.equal(result.stderr.trim(), "");
});

test("wrapper blocks the frozen JS fallback when Rust-only flags are requested", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-rust-only-flag-"));
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
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "audit",
    ".",
    "--only",
    "env",
  ]);

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required/);
  assert.match(result.stderr, /--only/);
});

test("wrapper blocks output format flags on the frozen JS fallback", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-format-flag-"));
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
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "audit",
    ".",
    "--format",
    "json",
  ]);

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required/);
  assert.match(result.stderr, /--format/);

  const leadingResult = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "--format",
    "json",
    "audit",
    ".",
  ]);

  assert.equal(leadingResult.code, 1);
  assert.equal(leadingResult.stdout.trim(), "");
  assert.match(leadingResult.stderr, /A Rust runtime is required/);
  assert.match(leadingResult.stderr, /--format/);
});

test("wrapper treats dash-prefixed paths as positionals on the frozen JS fallback", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-dash-path-"));
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
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "audit",
    "-repo",
  ]);

  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.stdout.trim()), {
    runtime: "js",
    args: ["audit", "-repo"],
  });
  assert.equal(result.stderr.trim(), "");
});

test("wrapper blocks the frozen JS fallback when a Maximus config file is present", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-config-present-"));
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
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
      "}",
      "",
    ].join("\n"),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
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

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required when a Maximus config file is present/);
  assert.match(result.stderr, /maximus\.config\.json/);
});

test("wrapper blocks the frozen JS fallback for doctor target config files", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-doctor-config-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  const targetDir = path.join(rootDir, "target-project");
  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await mkdir(targetDir, { recursive: true });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
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
    path.join(targetDir, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "doctor",
    targetDir,
  ]);

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required when a Maximus config file is present/);
  assert.match(result.stderr, /maximus\.config\.json/);
});

test("wrapper blocks the frozen JS fallback when config is found through a symlinked target", async (t) => {
  if (process.platform === "win32") {
    t.skip("symlink privileges vary on Windows");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-symlink-config-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  const realProject = path.join(rootDir, "real");
  const realTarget = path.join(realProject, "apps", "web");
  const aliasTarget = path.join(rootDir, "alias-web");
  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await mkdir(realTarget, { recursive: true });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
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
    path.join(realProject, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
    "utf8",
  );
  await symlink(realTarget, aliasTarget);
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "audit",
    aliasTarget,
  ]);

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required when a Maximus config file is present/);
  assert.match(result.stderr, /maximus\.config\.json/);
});

test("wrapper prefers the real project config over a lexical mount config", async (t) => {
  if (process.platform === "win32") {
    t.skip("symlink privileges vary on Windows");
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-realpath-config-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  const realProject = path.join(rootDir, "real");
  const realTarget = path.join(realProject, "apps", "web");
  const mountRoot = path.join(rootDir, "mount");
  const aliasTarget = path.join(mountRoot, "web");
  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await mkdir(realTarget, { recursive: true });
  await mkdir(mountRoot, { recursive: true });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
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
    path.join(realProject, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
    "utf8",
  );
  await writeFile(
    path.join(mountRoot, "maximus.config.json"),
    '{ "checks": { "only": ["tsconfig"] } }\n',
    "utf8",
  );
  await symlink(realTarget, aliasTarget);
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "audit",
    aliasTarget,
  ]);

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required when a Maximus config file is present/);
  assert.match(result.stderr, /\/real\/maximus\.config\.json/);
  assert.doesNotMatch(result.stderr, /\/mount\/maximus\.config\.json/);
});

test("wrapper ignores directory-shaped config paths when evaluating the frozen JS fallback", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-config-dir-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "bin"), { recursive: true });
  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await mkdir(path.join(rootDir, "maximus.config.json"), { recursive: true });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
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
  assert.deepEqual(JSON.parse(result.stdout.trim()), {
    runtime: "js",
    args: ["audit", "."],
  });
  assert.equal(result.stderr.trim(), "");
});

test("wrapper prints help before checking config files on the frozen JS fallback", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-help-with-config-"));
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
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
      "}",
      "",
    ].join("\n"),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, ["--help"]);

  assert.equal(result.code, 0);
  assert.match(result.stdout, /Usage/);
  assert.match(result.stdout, /maximus fix \[path\] --dry-run \[--json\]/);
  assert.doesNotMatch(result.stdout, /--only <checks>/);
  assert.match(result.stdout, /--format.*require the Rust runtime/);
  assert.match(result.stdout, /require the Rust runtime/);
  assert.equal(result.stderr.trim(), "");
});

test("wrapper blocks the frozen JS fallback for fix without dry-run", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-wrapper-fix-write-"));
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
      "  console.log(JSON.stringify({ runtime: 'js', args }));",
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

  const result = await runWrapper(path.join(rootDir, "bootstrap.mjs"), rootDir, [
    "fix",
    ".",
  ]);

  assert.equal(result.code, 1);
  assert.equal(result.stdout.trim(), "");
  assert.match(result.stderr, /A Rust runtime is required/);
  assert.match(result.stderr, /fix \(without --dry-run\)/);
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
  await mkdir(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin"), {
    recursive: true,
  });

  await cp(path.join(process.cwd(), "bin", "maximus.js"), path.join(rootDir, "bin", "maximus.js"));
  await writeFile(path.join(rootDir, "target", "debug", "maximus"), "#!/bin/sh\necho debug-runtime\n", "utf8");
  await writeFile(path.join(rootDir, "target", "release", "maximus"), "#!/bin/sh\necho release-runtime\n", "utf8");
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage.packageName, "package.json"),
    JSON.stringify(
      {
        name: runtimePackage.packageName,
        version: "0.1.0",
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"),
    "#!/bin/sh\necho installed-runtime\n",
    "utf8",
  );
  await chmod(path.join(rootDir, "target", "debug", "maximus"), 0o755);
  await chmod(path.join(rootDir, "target", "release", "maximus"), 0o755);
  await chmod(path.join(rootDir, "node_modules", runtimePackage.packageName, "bin", "maximus"), 0o755);

  const result = await runWrapper(path.join(rootDir, "bin", "maximus.js"), rootDir);

  assert.equal(result.code, 0);
  assert.equal(result.stdout.trim(), "debug-runtime");
  assert.equal(result.stderr.trim(), "");
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

async function runWrapper(wrapperPath, cwd, args = ["audit", "."]) {
  return await new Promise((resolve, reject) => {
    const stdout = [];
    const stderr = [];
    const child = spawn(process.execPath, [wrapperPath, ...args], {
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
