import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { readFile } from "node:fs/promises";

test("linux runtime packages declare glibc metadata", async () => {
  for (const [packageName, expectedCpu] of [
    ["maximus-linux-arm64-gnu", "arm64"],
    ["maximus-linux-x64-gnu", "x64"],
  ]) {
    const manifest = JSON.parse(
      await readFile(path.join(process.cwd(), "npm", packageName, "package.json"), "utf8"),
    );

    assert.deepEqual(manifest.os, ["linux"]);
    assert.deepEqual(manifest.cpu, [expectedCpu]);
    assert.deepEqual(manifest.libc, ["glibc"]);
  }
});
