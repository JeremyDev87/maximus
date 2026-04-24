import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import {
  bumpPackageVersion,
  createManualBumpBranchName,
  packageManifestPaths,
} from "../scripts/bump-release-version.mjs";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));

async function writeJson(filePath, value) {
  await mkdir(path.dirname(filePath), { recursive: true });
  await writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

test("bumpPackageVersion updates root and platform package manifests together", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-bump-version-"));

  await writeJson(path.join(repoRoot, "package.json"), {
    name: "@jeremyfellaz/maximus",
    version: "0.1.0",
    optionalDependencies: {
      "@jeremyfellaz/maximus-darwin-arm64": "0.1.0",
      "@jeremyfellaz/maximus-darwin-x64": "0.1.0",
      "@jeremyfellaz/maximus-linux-arm64-gnu": "0.1.0",
      "@jeremyfellaz/maximus-linux-x64-gnu": "0.1.0",
    },
  });

  for (const relativePath of packageManifestPaths.slice(1)) {
    await writeJson(path.join(repoRoot, relativePath), {
      name: `pkg-${relativePath}`,
      version: "0.1.0",
    });
  }

  const result = await bumpPackageVersion("v0.1.1", repoRoot);
  assert.equal(result.version, "0.1.1");
  assert.equal(result.tag, "v0.1.1");
  assert.equal(result.manifestPaths.length, packageManifestPaths.length);

  for (const relativePath of packageManifestPaths) {
    const manifest = JSON.parse(await readFile(path.join(repoRoot, relativePath), "utf8"));
    assert.equal(manifest.version, "0.1.1");
  }

  const rootManifest = JSON.parse(await readFile(path.join(repoRoot, "package.json"), "utf8"));
  assert.deepEqual(Object.values(rootManifest.optionalDependencies), [
    "0.1.1",
    "0.1.1",
    "0.1.1",
    "0.1.1",
  ]);
});

test("createManualBumpBranchName is deterministic per tag and base", () => {
  assert.equal(
    createManualBumpBranchName("v0.1.1", "master"),
    createManualBumpBranchName("v0.1.1", "master"),
  );
  assert.notEqual(
    createManualBumpBranchName("v0.1.1", "master"),
    createManualBumpBranchName("v0.1.2", "master"),
  );
});

test("bump module can be imported from node eval with tag argv", () => {
  const output = execFileSync(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      "import { createManualBumpBranchName } from './scripts/bump-release-version.mjs'; console.log(createManualBumpBranchName(process.argv[1]));",
      "v0.1.1",
    ],
    { cwd: projectRoot, encoding: "utf8" },
  );

  assert.match(output, /^codex\/manual-bump-master-v0-1-1-[0-9a-f]{8}\n$/);
});
