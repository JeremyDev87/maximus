import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";

import { auditProject } from "../src/core/audit-project.js";

test("tsconfig audit reports deprecated options and missing alias targets", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "src"), { recursive: true });
  await writeFile(path.join(rootDir, "package.json"), '{"name":"fixture","imports":{"#app/*":"./src/runtime/*"}}', "utf8");
  await writeFile(
    path.join(rootDir, "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": ".",
        "importsNotUsedAsValues": "remove",
        "paths": {
          "#app/*": ["src/*"],
          "@missing/*": ["ghost/*"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);
  const findingTitles = audit.findings.map((finding) => finding.title);

  assert.ok(findingTitles.includes('Deprecated compiler option "importsNotUsedAsValues"'));
  assert.ok(findingTitles.includes("Path alias target does not exist"));
  assert.ok(findingTitles.includes('Alias "#app/*" differs between tsconfig and package imports'));
});

test("imports comparison respects tsconfig baseUrl", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-baseurl-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "config"), { recursive: true });
  await mkdir(path.join(rootDir, "src", "lib"), { recursive: true });
  await writeFile(
    path.join(rootDir, "package.json"),
    '{"name":"fixture","imports":{"#app/*":"./src/lib/*"}}',
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "config", "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": "../src",
        "paths": {
          "#app/*": ["lib/*"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.ok(!audit.findings.some((finding) => finding.id.startsWith("tsconfig-import-conflict:")));
});

test("wildcard alias targets require a matching concrete path when suffixes remain", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-wildcard-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "src", "generated"), { recursive: true });
  await writeFile(path.join(rootDir, "package.json"), '{"name":"fixture"}', "utf8");
  await writeFile(
    path.join(rootDir, "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": ".",
        "paths": {
          "@client/*": ["src/generated/*/client.ts"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.ok(
    audit.findings.some(
      (finding) =>
        finding.id.startsWith("tsconfig-paths-missing:") &&
        finding.detail.includes("src/generated/*/client.ts"),
    ),
  );
});

test("wildcard alias targets support extensionless matches", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-extensionless-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "src", "generated", "foo"), { recursive: true });
  await writeFile(path.join(rootDir, "package.json"), '{"name":"fixture"}', "utf8");
  await writeFile(path.join(rootDir, "src", "generated", "foo", "client.ts"), "export {};\n", "utf8");
  await writeFile(
    path.join(rootDir, "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": ".",
        "paths": {
          "@client/*": ["src/generated/*/client"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.ok(
    !audit.findings.some(
      (finding) =>
        finding.id.startsWith("tsconfig-paths-missing:") &&
        finding.detail.includes("src/generated/*/client"),
    ),
  );
});

test("imports comparison accepts matching conditional branches", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-conditional-match-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "src", "lib"), { recursive: true });
  await writeFile(
    path.join(rootDir, "package.json"),
    '{"name":"fixture","imports":{"#app/*":{"types":"./src/lib/*","default":"./dist/lib/*"}}}',
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": ".",
        "paths": {
          "#app/*": ["src/lib/*"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.ok(!audit.findings.some((finding) => finding.id.startsWith("tsconfig-import-conflict:")));
});

test("imports comparison still checks conditional objects when branches diverge", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-conditional-mismatch-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "src", "lib"), { recursive: true });
  await writeFile(
    path.join(rootDir, "package.json"),
    '{"name":"fixture","imports":{"#app/*":{"import":"./dist/lib/*"}}}',
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": ".",
        "paths": {
          "#app/*": ["src/lib/*"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.ok(audit.findings.some((finding) => finding.id.startsWith("tsconfig-import-conflict:")));
});

test("imports comparison catches conditional wildcard suffix drift", async (t) => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-conditional-suffix-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "src", "lib"), { recursive: true });
  await writeFile(
    path.join(rootDir, "package.json"),
    '{"name":"fixture","imports":{"#app/*":{"types":"./src/lib/*/client","default":"./dist/lib/*/client"}}}',
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "tsconfig.json"),
    `{
      "compilerOptions": {
        "baseUrl": ".",
        "paths": {
          "#app/*": ["src/lib/*/server"]
        }
      }
    }`,
    "utf8",
  );

  const audit = await auditProject(rootDir);

  assert.ok(audit.findings.some((finding) => finding.id.startsWith("tsconfig-import-conflict:")));
});
