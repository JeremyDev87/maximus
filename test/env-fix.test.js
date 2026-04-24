import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { promisify } from "node:util";

import { auditProject } from "../src/core/audit-project.js";
import { applyFixes } from "../src/core/fixers.js";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(testDir, "fixtures");
const execFileAsync = promisify(execFile);

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

test("env gitignore protection honors negation and ancestor gitignore files", async (t) => {
  const negatedRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-negated-"));
  const ancestorRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-ancestor-"));
  const anchoredNestedRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-anchored-nested-"));
  const anchoredRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-anchored-root-"));
  const directoryOnlyRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-directory-only-"));
  const subdirAuditRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-subdir-"));
  const globRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-glob-"));
  const globstarRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-globstar-"));
  const leadingSpaceRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-leading-space-"));
  const directoryRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-directory-"));
  const bareDirectoryRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-bare-directory-"));
  const trackedRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-env-gitignore-tracked-"));

  t.after(async () => {
    await rm(negatedRoot, { recursive: true, force: true });
    await rm(ancestorRoot, { recursive: true, force: true });
    await rm(anchoredNestedRoot, { recursive: true, force: true });
    await rm(anchoredRoot, { recursive: true, force: true });
    await rm(directoryOnlyRoot, { recursive: true, force: true });
    await rm(subdirAuditRoot, { recursive: true, force: true });
    await rm(globRoot, { recursive: true, force: true });
    await rm(globstarRoot, { recursive: true, force: true });
    await rm(leadingSpaceRoot, { recursive: true, force: true });
    await rm(directoryRoot, { recursive: true, force: true });
    await rm(bareDirectoryRoot, { recursive: true, force: true });
    await rm(trackedRoot, { recursive: true, force: true });
  });

  await writeFile(path.join(negatedRoot, ".env"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(negatedRoot, ".gitignore"), ".env\n!.env\n", "utf8");

  const negatedAudit = await auditProject(negatedRoot);
  assert.ok(
    negatedAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "later negation should make .env unprotected",
  );

  await writeFile(path.join(leadingSpaceRoot, ".env"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(leadingSpaceRoot, ".gitignore"), " .env\n", "utf8");

  const leadingSpaceAudit = await auditProject(leadingSpaceRoot);
  assert.ok(
    leadingSpaceAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "leading-space .gitignore pattern should not protect .env",
  );

  await writeFile(path.join(globRoot, ".env.local"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(globRoot, ".env.example"), "API_TOKEN=\n", "utf8");
  await writeFile(path.join(globRoot, ".gitignore"), ".env*\n!.env.example\n", "utf8");

  const globAudit = await auditProject(globRoot);
  assert.ok(
    !globAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "glob .gitignore pattern should protect .env.local",
  );

  await mkdir(path.join(globstarRoot, "apps/web"), { recursive: true });
  await writeFile(path.join(globstarRoot, "apps/web/.env.local"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(globstarRoot, ".gitignore"), "**/.env.local\n", "utf8");

  const globstarAudit = await auditProject(globstarRoot);
  assert.ok(
    !globstarAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "globstar .gitignore pattern should protect nested .env.local",
  );

  await mkdir(path.join(directoryRoot, "apps/web"), { recursive: true });
  await writeFile(path.join(directoryRoot, "apps/web/.env.local"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(directoryRoot, ".gitignore"), "apps/\n", "utf8");

  const directoryAudit = await auditProject(directoryRoot);
  assert.ok(
    !directoryAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "directory-only .gitignore pattern should protect files under that directory",
  );

  await mkdir(path.join(bareDirectoryRoot, "secrets"), { recursive: true });
  await writeFile(path.join(bareDirectoryRoot, "secrets/.env"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(bareDirectoryRoot, ".gitignore"), "secrets\n", "utf8");

  const bareDirectoryAudit = await auditProject(bareDirectoryRoot);
  assert.ok(
    !bareDirectoryAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "bare directory .gitignore pattern should protect files under that directory",
  );

  await writeFile(path.join(trackedRoot, ".env"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(trackedRoot, ".gitignore"), ".env\n", "utf8");
  await execFileAsync("git", ["init"], { cwd: trackedRoot });
  await execFileAsync("git", ["add", "-f", ".env"], { cwd: trackedRoot });

  const trackedAudit = await auditProject(trackedRoot);
  assert.ok(
    trackedAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "tracked concrete env files should not be treated as protected by .gitignore",
  );

  await mkdir(path.join(ancestorRoot, "apps/web"), { recursive: true });
  await writeFile(
    path.join(ancestorRoot, "apps/web/.env.local"),
    "API_TOKEN=abcdef1234567890\n",
    "utf8",
  );
  await writeFile(path.join(ancestorRoot, "apps/.gitignore"), ".env.local\n", "utf8");

  const ancestorAudit = await auditProject(ancestorRoot);
  assert.ok(
    !ancestorAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "ancestor .gitignore should protect nested .env.local",
  );

  await mkdir(path.join(subdirAuditRoot, ".git"), { recursive: true });
  await mkdir(path.join(subdirAuditRoot, "packages/app"), { recursive: true });
  await writeFile(path.join(subdirAuditRoot, ".git/HEAD"), "ref: refs/heads/main\n", "utf8");
  await writeFile(path.join(subdirAuditRoot, ".gitignore"), "packages/app/.env.local\n", "utf8");
  await writeFile(
    path.join(subdirAuditRoot, "packages/app/.env.local"),
    "API_TOKEN=abcdef1234567890\n",
    "utf8",
  );

  const subdirAudit = await auditProject(path.join(subdirAuditRoot, "packages/app"));
  assert.ok(
    !subdirAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "repo root .gitignore should protect subdir audit targets",
  );

  await mkdir(path.join(anchoredNestedRoot, "apps/web"), { recursive: true });
  await writeFile(
    path.join(anchoredNestedRoot, "apps/web/.env.local"),
    "API_TOKEN=abcdef1234567890\n",
    "utf8",
  );
  await writeFile(path.join(anchoredNestedRoot, ".gitignore"), "/.env.local\n", "utf8");

  const anchoredNestedAudit = await auditProject(anchoredNestedRoot);
  assert.ok(
    anchoredNestedAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "anchored root .gitignore pattern should not protect nested .env.local",
  );

  await writeFile(path.join(anchoredRoot, ".env.local"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(anchoredRoot, ".gitignore"), "/.env.local\n", "utf8");

  const anchoredAudit = await auditProject(anchoredRoot);
  assert.ok(
    !anchoredAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "anchored root .gitignore pattern should protect root .env.local",
  );

  await writeFile(path.join(directoryOnlyRoot, ".env.local"), "API_TOKEN=abcdef1234567890\n", "utf8");
  await writeFile(path.join(directoryOnlyRoot, ".gitignore"), ".env.local/\n", "utf8");

  const directoryOnlyAudit = await auditProject(directoryOnlyRoot);
  assert.ok(
    directoryOnlyAudit.findings.some((finding) => finding.id.startsWith("env-gitignore:")),
    "directory-only .gitignore pattern should not protect env files",
  );
});

async function copyFixtureToTemp(relativeFixturePath) {
  const sourceDir = path.join(fixturesDir, relativeFixturePath);
  const tempParent = await mkdtemp(path.join(os.tmpdir(), "maximus-env-fixture-"));
  const targetDir = path.join(tempParent, path.basename(relativeFixturePath));

  await cp(sourceDir, targetDir, { recursive: true });

  return targetDir;
}
