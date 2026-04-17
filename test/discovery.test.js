import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";

import { auditProject } from "../src/core/audit-project.js";

test("discovery includes .prettierrc.toml for duplicate-source checks", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-prettier-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await writeFile(
    path.join(rootDir, "package.json"),
    '{"name":"fixture","prettier":{"semi":false}}',
    "utf8",
  );
  await writeFile(path.join(rootDir, ".prettierrc.toml"), "semi = false\n", "utf8");

  const audit = await auditProject(rootDir);

  assert.ok(
    audit.findings.some(
      (finding) =>
        finding.id.startsWith("duplicate-config:Prettier:") &&
        finding.title === "Prettier config is declared in multiple places",
    ),
  );
});
