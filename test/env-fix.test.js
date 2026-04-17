import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";

import { auditProject } from "../src/core/audit-project.js";
import { applyFixes } from "../src/core/fixers.js";

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
