import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { readFile } from "node:fs/promises";

const runtimePackages = [
  ["maximus-darwin-arm64", "arm64"],
  ["maximus-darwin-x64", "x64"],
  ["maximus-linux-arm64-gnu", "arm64"],
  ["maximus-linux-x64-gnu", "x64"],
];

test("platform runtime packages declare expected install metadata", async () => {
  for (const [packageName, expectedCpu] of runtimePackages) {
    const manifest = JSON.parse(
      await readFile(path.join(process.cwd(), "npm", packageName, "package.json"), "utf8"),
    );

    assert.deepEqual(manifest.cpu, [expectedCpu]);
    assert.deepEqual(manifest.files, ["bin/maximus"]);
    assert.deepEqual(manifest.bin, { maximus: "./bin/maximus" });

    if (packageName.startsWith("maximus-darwin-")) {
      assert.deepEqual(manifest.os, ["darwin"]);
      assert.equal("libc" in manifest, false);
      continue;
    }

    assert.deepEqual(manifest.os, ["linux"]);
    assert.deepEqual(manifest.libc, ["glibc"]);
  }
});

test("root optional dependency versions stay in sync with platform package versions", async () => {
  const rootManifest = JSON.parse(await readFile(path.join(process.cwd(), "package.json"), "utf8"));

  for (const [packageName] of runtimePackages) {
    const platformManifest = JSON.parse(
      await readFile(path.join(process.cwd(), "npm", packageName, "package.json"), "utf8"),
    );

    assert.equal(platformManifest.version, rootManifest.version);
    assert.equal(rootManifest.optionalDependencies[packageName], rootManifest.version);
  }
});
