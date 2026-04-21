import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { readFile } from "node:fs/promises";

const runtimePackages = [
  {
    directoryName: "maximus-darwin-arm64",
    packageName: "@jeremyfellaz/maximus-darwin-arm64",
    expectedCpu: "arm64",
  },
  {
    directoryName: "maximus-darwin-x64",
    packageName: "@jeremyfellaz/maximus-darwin-x64",
    expectedCpu: "x64",
  },
  {
    directoryName: "maximus-linux-arm64-gnu",
    packageName: "@jeremyfellaz/maximus-linux-arm64-gnu",
    expectedCpu: "arm64",
  },
  {
    directoryName: "maximus-linux-x64-gnu",
    packageName: "@jeremyfellaz/maximus-linux-x64-gnu",
    expectedCpu: "x64",
  },
];

test("platform runtime packages declare expected install metadata", async () => {
  for (const { directoryName, packageName, expectedCpu } of runtimePackages) {
    const manifest = JSON.parse(
      await readFile(path.join(process.cwd(), "npm", directoryName, "package.json"), "utf8"),
    );

    assert.equal(manifest.name, packageName);
    assert.deepEqual(manifest.cpu, [expectedCpu]);
    assert.deepEqual(manifest.files, ["bin/maximus"]);
    assert.deepEqual(manifest.bin, { maximus: "./bin/maximus" });
    assert.deepEqual(manifest.publishConfig, { access: "public" });

    if (directoryName.startsWith("maximus-darwin-")) {
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
  assert.equal(rootManifest.name, "@jeremyfellaz/maximus");
  assert.deepEqual(rootManifest.publishConfig, { access: "public" });

  for (const { directoryName, packageName } of runtimePackages) {
    const platformManifest = JSON.parse(
      await readFile(path.join(process.cwd(), "npm", directoryName, "package.json"), "utf8"),
    );

    assert.equal(platformManifest.version, rootManifest.version);
    assert.equal(rootManifest.optionalDependencies[packageName], rootManifest.version);
  }
});
