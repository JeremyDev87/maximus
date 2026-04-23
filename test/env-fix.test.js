import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";

import { auditProject } from "../src/core/audit-project.js";
import { applyFixes } from "../src/core/fixers.js";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(testDir, "fixtures");

test("maximus fix creates .env.example from concrete env files", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-env-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await writeFile(
    path.join(rootDir, ".env"),
    "API_URL=http://localhost:3000\nAUTH_TOKEN=supersecretvalue12345\n",
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.equal(audit.summary.fixesAvailable, 1);
  assert.ok(audit.findings.some((finding) => finding.id.startsWith("env-example-missing:")));

  await applyFixes(audit.fixes);

  const example = await readFile(path.join(rootDir, ".env.example"), "utf8");
  assert.equal(example, "API_URL=\nAUTH_TOKEN=\n");
});

test("env contract matrix fixtures keep template variants and duplicate chains stable", async (t) => {
  const matrixRoot = path.join(fixturesDir, "env-contract-matrix");
  const duplicateAudit = await auditProject(path.join(matrixRoot, "duplicate-chain"));

  assert.deepEqual(
    duplicateAudit.findings
      .filter((finding) => finding.id.includes(":A:"))
      .map((finding) => finding.detail),
    ["A is declared on lines 1 and 2.", "A is declared on lines 2 and 3."],
  );

  const sampleAudit = await auditProject(path.join(matrixRoot, "template-only-sample"));
  assert.equal(sampleAudit.summary.fixesAvailable, 0);
  assert.ok(!sampleAudit.findings.some((finding) => finding.id.startsWith("env-example-missing:")));

  const syncAudit = await auditProject(path.join(matrixRoot, "sync-template-like-example-local"));
  assert.ok(
    syncAudit.findings.some(
      (finding) =>
        finding.id.startsWith("env-example-sync:") &&
        finding.file === path.join(matrixRoot, "sync-template-like-example-local", ".env.example.local") &&
        finding.detail === "Missing keys: SECONDARY.",
    ),
  );

  const createFixtureDir = await copyFixtureToTemp("env-contract-matrix/create-from-env-local-only");
  t.after(async () => {
    await rm(createFixtureDir, { recursive: true, force: true });
  });

  const createAudit = await auditProject(createFixtureDir);
  await applyFixes(createAudit.fixes);

  const createdExample = await readFile(path.join(createFixtureDir, ".env.example"), "utf8");
  assert.equal(createdExample, "API_URL=\nAUTH_TOKEN=\n");
});

test("env template order preservation fixtures keep existing lines and JS sort order stable", async (t) => {
  const createFixtureDir = await copyFixtureToTemp("env-template-order-preservation/create-from-concrete");
  const syncFixtureDir = await copyFixtureToTemp("env-template-order-preservation/sync-existing-template");

  t.after(async () => {
    await rm(createFixtureDir, { recursive: true, force: true });
    await rm(syncFixtureDir, { recursive: true, force: true });
  });

  const createAudit = await auditProject(createFixtureDir);
  await applyFixes(createAudit.fixes);

  const createdExample = await readFile(path.join(createFixtureDir, ".env.example"), "utf8");
  assert.equal(
    createdExample,
    "API_URL=\nAPI-URL=\nAPI.URL=\nVAR_1=\nVAR_10=\nVAR_2=\n",
  );

  const syncAudit = await auditProject(syncFixtureDir);
  await applyFixes(syncAudit.fixes);

  const syncedExample = await readFile(path.join(syncFixtureDir, ".env.example"), "utf8");
  assert.equal(
    syncedExample,
    "VAR_2=\nAPI.URL=\nAPI_URL=\nAPI-URL=\nVAR_1=\nVAR_10=\n",
  );
});

test("template-like env files do not trigger .env.example creation", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-env-template-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await writeFile(path.join(rootDir, ".env.sample"), "API_URL=\nAUTH_TOKEN=\n", "utf8");

  const audit = await auditProject(rootDir);

  assert.equal(audit.summary.fixesAvailable, 0);
  assert.ok(!audit.findings.some((finding) => finding.id.startsWith("env-example-missing:")));
});

async function copyFixtureToTemp(relativeFixturePath) {
  const sourceDir = path.join(fixturesDir, relativeFixturePath);
  const tempParent = await mkdtemp(path.join(os.tmpdir(), "maximus-env-fixture-"));
  const targetDir = path.join(tempParent, path.basename(relativeFixturePath));

  await cp(sourceDir, targetDir, { recursive: true });

  return targetDir;
}
