import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { runCli } from "../src/cli.js";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

test("JS audit --json output includes schema metadata", async () => {
  const targetDir = path.join(repoRoot, "test", "fixtures", "clean-project");
  const output = await captureJsonOutput(["audit", targetDir, "--json"]);

  assert.equal(output.schemaVersion, "1");
  assert.equal(output.generator, "maximus");
  assert.equal(output.rootDir, targetDir);
});

test("JS fix --json dry-run output includes schema metadata on initial and final payloads", async () => {
  const targetDir = path.join(repoRoot, "test", "fixtures", "reference-env");
  const output = await captureJsonOutput(["fix", targetDir, "--dry-run", "--json"]);

  assert.equal(output.initial.schemaVersion, "1");
  assert.equal(output.initial.generator, "maximus");
  assert.equal(output.final.schemaVersion, "1");
  assert.equal(output.final.generator, "maximus");
});

async function captureJsonOutput(argv) {
  const logs = [];
  const originalLog = console.log;
  const originalExitCode = process.exitCode;
  console.log = (...args) => {
    logs.push(args.join(" "));
  };

  try {
    process.exitCode = 0;
    await runCli(argv);
  } finally {
    console.log = originalLog;
    process.exitCode = originalExitCode;
  }

  assert.equal(logs.length, 1, "CLI should emit a single JSON payload");
  return JSON.parse(logs[0]);
}
