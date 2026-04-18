import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDir, "..");

const scenarios = [
  {
    name: "clean-project audit output stays stable",
    args: ["audit", "./test/fixtures/clean-project"],
    goldenFile: "clean-project.audit.txt",
    expectedStatus: 0,
    targetDir: path.join(repoRoot, "test", "fixtures", "clean-project"),
  },
  {
    name: "clean-project doctor output stays stable",
    args: ["doctor", "./test/fixtures/clean-project"],
    goldenFile: "clean-project.doctor.txt",
    expectedStatus: 0,
    targetDir: path.join(repoRoot, "test", "fixtures", "clean-project"),
  },
  {
    name: "reference env audit output stays stable",
    args: ["audit", "./test/fixtures/reference-env"],
    goldenFile: "env-missing-example.audit.txt",
    expectedStatus: 1,
    targetDir: path.join(repoRoot, "test", "fixtures", "reference-env"),
  },
  {
    name: "reference env doctor output stays stable",
    args: ["doctor", "./test/fixtures/reference-env"],
    goldenFile: "env-missing-example.doctor.txt",
    expectedStatus: 1,
    targetDir: path.join(repoRoot, "test", "fixtures", "reference-env"),
  },
  {
    name: "reference env fix dry-run output stays stable",
    args: ["fix", "./test/fixtures/reference-env", "--dry-run"],
    goldenFile: "env-missing-example.fix-dry-run.txt",
    expectedStatus: 1,
    targetDir: path.join(repoRoot, "test", "fixtures", "reference-env"),
  },
  {
    name: "reference tsconfig audit output stays stable",
    args: ["audit", "./test/fixtures/reference-tsconfig"],
    goldenFile: "tsconfig-missing-alias.audit.txt",
    expectedStatus: 1,
    targetDir: path.join(repoRoot, "test", "fixtures", "reference-tsconfig"),
  },
];

for (const scenario of scenarios) {
  test(scenario.name, async () => {
    const result = spawnSync(process.execPath, ["./bin/maximus.js", ...scenario.args], {
      cwd: repoRoot,
      encoding: "utf8",
    });

    assert.equal(result.status, scenario.expectedStatus, result.stderr);

    const actual = normalizeOutput(result.stdout, scenario.targetDir);
    const golden = normalizeOutput(
      await readFile(path.join(repoRoot, "test", "golden-rust", scenario.goldenFile), "utf8"),
      scenario.targetDir,
    );

    assert.equal(actual, golden);
  });
}

function normalizeOutput(output, targetDir) {
  return output.replaceAll("\r\n", "\n").replaceAll(targetDir, "<TARGET>").trimEnd();
}
