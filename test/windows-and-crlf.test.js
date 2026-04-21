import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { auditProject } from "../src/core/audit-project.js";

const fixtureRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "fixtures", "windows-crlf");

test("Windows CRLF fixture keeps env and tsconfig checks working", async () => {
  const audit = await auditProject(fixtureRoot);

  assert.equal(audit.summary.fixesAvailable, 1);
  assert.ok(
    audit.findings.some(
      (finding) =>
        finding.id.startsWith("env-example-missing:") &&
        finding.file === path.join(fixtureRoot, ".env"),
    ),
  );
  assert.ok(
    audit.findings.some(
      (finding) =>
        finding.id.startsWith("tsconfig-paths-missing:") &&
        finding.file === path.join(fixtureRoot, "tsconfig.json"),
    ),
  );
});
