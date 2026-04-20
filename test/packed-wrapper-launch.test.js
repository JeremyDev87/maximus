import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { chmod, mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import process from "node:process";
import { resolvePackedWrapperLaunch } from "../scripts/lib/packed-wrapper-launch.mjs";

test("packed wrapper launch prefers the npm .bin shim when it exists", async (t) => {
  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-packed-launch-bin-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", ".bin"), { recursive: true });
  await mkdir(path.join(installRoot, "node_modules", "maximus", "bin"), {
    recursive: true,
  });

  const packageEntrypoint = path.join(installRoot, "node_modules", "maximus", "bin", "maximus.js");
  await writeFile(packageEntrypoint, "#!/usr/bin/env node\n", "utf8");
  await chmod(packageEntrypoint, 0o755);
  await symlink("../maximus/bin/maximus.js", path.join(installRoot, "node_modules", ".bin", "maximus"));

  const launch = await resolvePackedWrapperLaunch(installRoot);

  assert.deepEqual(launch, {
    command: path.join(installRoot, "node_modules", ".bin", "maximus"),
    args: [],
  });
});

test("packed wrapper launch falls back to the installed package entrypoint when .bin is missing", async (t) => {
  const installRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-packed-launch-entrypoint-"));
  t.after(async () => {
    await rm(installRoot, { recursive: true, force: true });
  });

  await mkdir(path.join(installRoot, "node_modules", "maximus", "bin"), {
    recursive: true,
  });

  const packageEntrypoint = path.join(installRoot, "node_modules", "maximus", "bin", "maximus.js");
  await writeFile(packageEntrypoint, "#!/usr/bin/env node\n", "utf8");
  await chmod(packageEntrypoint, 0o755);

  const launch = await resolvePackedWrapperLaunch(installRoot);

  assert.deepEqual(launch, {
    command: process.execPath,
    args: [packageEntrypoint],
  });
});
