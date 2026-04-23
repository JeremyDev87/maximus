import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";

import { auditProject } from "../src/core/audit-project.js";

test("JS config duplicate check preserves the ESLint mixed-mode migration guidance", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-config-duplicates-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await writeFile(path.join(rootDir, ".eslintrc.json"), '{ "root": true }\n', "utf8");
  await writeFile(path.join(rootDir, "eslint.config.js"), "export default [];\n", "utf8");

  const audit = await auditProject(rootDir);
  const finding = audit.findings.find((candidate) => candidate.id.startsWith("eslint-mixed-modes:"));

  assert.ok(finding, "mixed-mode finding should be reported");
  assert.equal(finding.title, "Legacy and flat ESLint configs coexist");
  assert.equal(
    finding.detail,
    "This directory contains both legacy .eslintrc.* files and flat eslint.config.* files, so ESLint can resolve different rule sets depending on the entry point.",
  );
  assert.equal(
    finding.hint,
    "Migrate to eslint.config.* as the single source of truth, then remove the legacy .eslintrc.* files after the new config fully replaces them.",
  );
});
