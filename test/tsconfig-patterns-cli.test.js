import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDir, "..");

test("fixture-backed tsconfig pattern audits stay wired through the CLI", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const scenarios = [
    {
      name: "empty-include",
      expectedStatus: 1,
      expected: [
        "Include pattern does not match any files",
        'include pattern "src/missing/**/*.ts" matched 0 files',
      ],
      absent: ["Exclude pattern does not filter any included files"],
    },
    {
      name: "empty-exclude",
      expectedStatus: 0,
      expected: [
        "Exclude pattern does not filter any included files",
        'exclude pattern "generated/**/*.ts" removed 0 files from 1 included file(s)',
      ],
      absent: ["Include pattern does not match any files"],
    },
    {
      name: "exclude-only",
      expectedStatus: 0,
      expected: [
        "Exclude pattern does not filter any included files",
        'exclude pattern "missing/**/*.ts" removed 0 files from 1 included file(s)',
      ],
      absent: ["Include pattern does not match any files"],
    },
    {
      name: "useful-patterns",
      expectedStatus: 0,
      expected: ["No config drift detected."],
      absent: [
        "Include pattern does not match any files",
        "Exclude pattern does not filter any included files",
      ],
    },
  ];

  for (const scenario of scenarios) {
    const result = runAudit(`./test/fixtures/tsconfig-patterns/${scenario.name}`);

    assert.equal(result.status, scenario.expectedStatus, result.stderr);
    for (const snippet of scenario.expected) {
      assert.ok(
        result.stdout.includes(snippet),
        `expected ${scenario.name} output to include ${snippet}`,
      );
    }
    for (const snippet of scenario.absent) {
      assert.ok(
        !result.stdout.includes(snippet),
        `expected ${scenario.name} output to omit ${snippet}`,
      );
    }
  }
});

test("CLI audit respects allowJs when evaluating tsconfig include patterns", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-pattern-cli-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "js-without-allowjs", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "js-without-allowjs", "tsconfig.json"),
    JSON.stringify({ include: ["src"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "js-without-allowjs", "src", "index.js"),
    "export const ok = true;\n",
    "utf8",
  );

  await mkdir(path.join(rootDir, "js-with-allowjs", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "js-with-allowjs", "tsconfig.json"),
    JSON.stringify({ compilerOptions: { allowJs: true }, include: ["src"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "js-with-allowjs", "src", "index.js"),
    "export const ok = true;\n",
    "utf8",
  );

  const withoutAllowJs = runAudit(path.join(rootDir, "js-without-allowjs"));
  assert.equal(withoutAllowJs.status, 1, withoutAllowJs.stderr);
  assert.ok(withoutAllowJs.stdout.includes("Include pattern does not match any files"));

  const withAllowJs = runAudit(path.join(rootDir, "js-with-allowjs"));
  assert.equal(withAllowJs.status, 0, withAllowJs.stderr);
  assert.ok(withAllowJs.stdout.includes("No config drift detected."));
  assert.ok(!withAllowJs.stdout.includes("Include pattern does not match any files"));
});

test("CLI audit matches question-mark and zero-width star tsconfig include globs", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-pattern-glob-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "question-mark", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "question-mark", "tsconfig.json"),
    JSON.stringify({ include: ["src/file?.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "question-mark", "src", "file1.ts"),
    "export const ok = true;\n",
    "utf8",
  );

  await mkdir(path.join(rootDir, "star-zero", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "star-zero", "tsconfig.json"),
    JSON.stringify({ include: ["src/file*.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "star-zero", "src", "file.ts"),
    "export const ok = true;\n",
    "utf8",
  );

  for (const target of ["question-mark", "star-zero"]) {
    const result = runAudit(path.join(rootDir, target));

    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes("No config drift detected."));
    assert.ok(!result.stdout.includes("Include pattern does not match any files"));
  }
});

test("CLI audit respects inherited allowJs and outDir when evaluating tsconfig patterns", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-pattern-extends-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "inherited-allowjs", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "inherited-allowjs", "tsconfig.json"),
    JSON.stringify({ extends: "./tsconfig.base.json", include: ["src"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "inherited-allowjs", "tsconfig.base.json"),
    JSON.stringify({ compilerOptions: { allowJs: true } }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "inherited-allowjs", "src", "index.js"),
    "export const ok = true;\n",
    "utf8",
  );

  await mkdir(path.join(rootDir, "inherited-outdir", "src"), { recursive: true });
  await mkdir(path.join(rootDir, "inherited-outdir", "dist"), { recursive: true });
  await writeFile(
    path.join(rootDir, "inherited-outdir", "tsconfig.json"),
    JSON.stringify({ extends: "./tsconfig.base.json", exclude: ["dist/**/*.d.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "inherited-outdir", "tsconfig.base.json"),
    JSON.stringify({ compilerOptions: { outDir: "./dist" } }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "inherited-outdir", "src", "index.d.ts"),
    "export declare const source: true;\n",
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "inherited-outdir", "dist", "index.d.ts"),
    "export declare const built: true;\n",
    "utf8",
  );
  await mkdir(path.join(rootDir, "outdir-include", "dist"), { recursive: true });
  await writeFile(
    path.join(rootDir, "outdir-include", "tsconfig.json"),
    JSON.stringify({ extends: "./tsconfig.base.json", include: ["dist"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "outdir-include", "tsconfig.base.json"),
    JSON.stringify({ compilerOptions: { outDir: "./dist" } }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "outdir-include", "dist", "index.d.ts"),
    "export declare const built: true;\n",
    "utf8",
  );

  const inheritedAllowJs = runAudit(path.join(rootDir, "inherited-allowjs"));
  assert.equal(inheritedAllowJs.status, 0, inheritedAllowJs.stderr);
  assert.ok(inheritedAllowJs.stdout.includes("No config drift detected."));
  assert.ok(!inheritedAllowJs.stdout.includes("Include pattern does not match any files"));

  const inheritedOutDir = runAudit(path.join(rootDir, "inherited-outdir"));
  assert.equal(inheritedOutDir.status, 0, inheritedOutDir.stderr);
  assert.ok(inheritedOutDir.stdout.includes("Exclude pattern does not filter any included files"));
  assert.ok(inheritedOutDir.stdout.includes('exclude pattern "dist/**/*.d.ts" removed 0 files from 1 included file(s)'));

  const outdirInclude = runAudit(path.join(rootDir, "outdir-include"));
  assert.equal(outdirInclude.status, 1, outdirInclude.stderr);
  assert.ok(outdirInclude.stdout.includes("Include pattern does not match any files"));
  assert.ok(outdirInclude.stdout.includes('include pattern "dist" matched 0 files'));
});

test("CLI audit skips explicit empty inputs and default node_modules when evaluating tsconfig patterns", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-pattern-empty-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "include-empty", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "include-empty", "tsconfig.json"),
    JSON.stringify({ include: [], exclude: ["missing/**/*.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "include-empty", "src", "index.d.ts"),
    "export declare const ok: true;\n",
    "utf8",
  );

  await mkdir(path.join(rootDir, "files-empty", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "files-empty", "tsconfig.json"),
    JSON.stringify({ files: [], exclude: ["missing/**/*.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "files-empty", "src", "index.d.ts"),
    "export declare const ok: true;\n",
    "utf8",
  );

  await mkdir(path.join(rootDir, "default-excludes", "node_modules", "pkg"), { recursive: true });
  await writeFile(
    path.join(rootDir, "default-excludes", "tsconfig.json"),
    JSON.stringify({ exclude: ["missing/**/*.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "default-excludes", "node_modules", "pkg", "index.d.ts"),
    "export declare const ignored: true;\n",
    "utf8",
  );
  await mkdir(path.join(rootDir, "files-with-exclude", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "files-with-exclude", "tsconfig.json"),
    JSON.stringify({ files: ["src/index.d.ts"], exclude: ["src/**/*.d.ts"] }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "files-with-exclude", "src", "index.d.ts"),
    "export declare const explicit: true;\n",
    "utf8",
  );
  await mkdir(path.join(rootDir, "duplicate-excludes", "src", "generated"), { recursive: true });
  await writeFile(
    path.join(rootDir, "duplicate-excludes", "tsconfig.json"),
    JSON.stringify(
      {
        include: ["src/**/*.d.ts"],
        exclude: ["src/generated/**/*.d.ts", "src/generated/**/*.d.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "duplicate-excludes", "src", "generated", "index.d.ts"),
    "export declare const duplicated: true;\n",
    "utf8",
  );

  for (const target of ["include-empty", "files-empty", "default-excludes"]) {
    const result = runAudit(path.join(rootDir, target));

    assert.equal(result.status, 0, result.stderr);
    assert.ok(result.stdout.includes("No config drift detected."));
    assert.ok(!result.stdout.includes("Include pattern does not match any files"));
    assert.ok(!result.stdout.includes("Exclude pattern does not filter any included files"));
  }

  const filesWithExclude = runAudit(path.join(rootDir, "files-with-exclude"));
  assert.equal(filesWithExclude.status, 0, filesWithExclude.stderr);
  assert.ok(filesWithExclude.stdout.includes("Exclude pattern does not filter any included files"));
  assert.ok(filesWithExclude.stdout.includes('exclude pattern "src/**/*.d.ts" removed 0 files from 1 included file(s)'));

  const duplicateExcludes = runAudit(path.join(rootDir, "duplicate-excludes"));
  assert.equal(duplicateExcludes.status, 0, duplicateExcludes.stderr);
  assert.ok(duplicateExcludes.stdout.includes("Exclude pattern does not filter any included files"));
  assert.ok(duplicateExcludes.stdout.includes('exclude pattern "src/generated/**/*.d.ts" removed 0 files from 0 included file(s)'));
});

test("CLI audit inherits top-level pattern fields and reports invalid pattern entries", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-pattern-inherited-fields-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "shared"), { recursive: true });
  await writeFile(
    path.join(rootDir, "shared", "tsconfig.base.json"),
    JSON.stringify({ include: ["./src/**/*.ts"] }, null, 2),
    "utf8",
  );

  await mkdir(path.join(rootDir, "app-inherited-include", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "app-inherited-include", "tsconfig.json"),
    JSON.stringify({ extends: "../shared/tsconfig.base.json" }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "app-inherited-include", "src", "index.ts"),
    "export const ok = true;\n",
    "utf8",
  );
  await mkdir(path.join(rootDir, "app-missing-extends"), { recursive: true });
  await writeFile(
    path.join(rootDir, "app-missing-extends", "tsconfig.json"),
    JSON.stringify({ extends: "../shared-missing/tsconfig.base.json" }, null, 2),
    "utf8",
  );

  await mkdir(path.join(rootDir, "shared-empty"), { recursive: true });
  await writeFile(
    path.join(rootDir, "shared-empty", "tsconfig.base.json"),
    JSON.stringify({ files: [] }, null, 2),
    "utf8",
  );
  await mkdir(path.join(rootDir, "app-inherited-files-empty", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "app-inherited-files-empty", "tsconfig.json"),
    JSON.stringify(
      {
        extends: "../shared-empty/tsconfig.base.json",
        exclude: ["missing/**/*.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "app-inherited-files-empty", "src", "index.d.ts"),
    "export declare const ok: true;\n",
    "utf8",
  );

  await mkdir(path.join(rootDir, "invalid-pattern-entry"), { recursive: true });
  await writeFile(
    path.join(rootDir, "invalid-pattern-entry", "tsconfig.json"),
    JSON.stringify({ include: [42] }, null, 2),
    "utf8",
  );
  await mkdir(path.join(rootDir, "invalid-files-entry", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "invalid-files-entry", "tsconfig.json"),
    JSON.stringify(
      {
        files: ["src/*.ts"],
        exclude: ["missing/**/*.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "invalid-files-entry", "src", "index.d.ts"),
    "export declare const ok: true;\n",
    "utf8",
  );
  await mkdir(path.join(rootDir, "invalid-files-directory", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "invalid-files-directory", "tsconfig.json"),
    JSON.stringify(
      {
        files: ["src"],
        exclude: ["missing/**/*.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "invalid-files-directory", "src", "index.d.ts"),
    "export declare const ok: true;\n",
    "utf8",
  );
  await mkdir(path.join(rootDir, "invalid-files-missing"), { recursive: true });
  await writeFile(
    path.join(rootDir, "invalid-files-missing", "tsconfig.json"),
    JSON.stringify(
      {
        files: ["src/missing.ts"],
        exclude: ["missing/**/*.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );

  const inheritedInclude = runAudit(path.join(rootDir, "app-inherited-include"));
  assert.equal(inheritedInclude.status, 1, inheritedInclude.stderr);
  assert.ok(inheritedInclude.stdout.includes("Include pattern does not match any files"));
  assert.ok(inheritedInclude.stdout.includes('include pattern "./src/**/*.ts" matched 0 files'));

  const missingExtends = runAudit(path.join(rootDir, "app-missing-extends"));
  assert.equal(missingExtends.status, 1, missingExtends.stderr);
  assert.ok(missingExtends.stdout.includes("Inherited tsconfig could not be found"));
  assert.ok(missingExtends.stdout.includes("shared-missing/tsconfig.base.json"));

  const inheritedFilesEmpty = runAudit(path.join(rootDir, "app-inherited-files-empty"));
  assert.equal(inheritedFilesEmpty.status, 0, inheritedFilesEmpty.stderr);
  assert.ok(inheritedFilesEmpty.stdout.includes("No config drift detected."));
  assert.ok(!inheritedFilesEmpty.stdout.includes("Exclude pattern does not filter any included files"));

  const invalidPatternEntry = runAudit(path.join(rootDir, "invalid-pattern-entry"));
  assert.equal(invalidPatternEntry.status, 1, invalidPatternEntry.stderr);
  assert.ok(invalidPatternEntry.stdout.includes('"include" contains a non-string pattern'));
  assert.ok(invalidPatternEntry.stdout.includes("declares include[0], but TypeScript expects string patterns."));

  const invalidFilesEntry = runAudit(path.join(rootDir, "invalid-files-entry"));
  assert.equal(invalidFilesEntry.status, 1, invalidFilesEntry.stderr);
  assert.ok(invalidFilesEntry.stdout.includes('"files" entries must point to explicit files'));
  assert.ok(invalidFilesEntry.stdout.includes("declares files[0] as src/*.ts"));

  const invalidFilesDirectory = runAudit(path.join(rootDir, "invalid-files-directory"));
  assert.equal(invalidFilesDirectory.status, 1, invalidFilesDirectory.stderr);
  assert.ok(invalidFilesDirectory.stdout.includes('"files" entries must point to files'));
  assert.ok(invalidFilesDirectory.stdout.includes("declares files[0] as src, but that path resolves to a directory."));

  const invalidFilesMissing = runAudit(path.join(rootDir, "invalid-files-missing"));
  assert.equal(invalidFilesMissing.status, 1, invalidFilesMissing.stderr);
  assert.ok(invalidFilesMissing.stdout.includes('"files" entries must point to existing files'));
  assert.ok(invalidFilesMissing.stdout.includes("declares files[0] as src/missing.ts"));
});

test("CLI audit treats missing Next generated types include as info-only", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-tsconfig-next-types-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "next-app"), { recursive: true });
  await writeFile(
    path.join(rootDir, "next-app", "package.json"),
    JSON.stringify({ dependencies: { next: "15.0.0" } }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "next-app", "tsconfig.json"),
    JSON.stringify({ include: [".next/types/**/*.ts"] }, null, 2),
    "utf8",
  );

  await mkdir(path.join(rootDir, "plain-app"), { recursive: true });
  await writeFile(
    path.join(rootDir, "plain-app", "package.json"),
    JSON.stringify({ dependencies: { react: "19.0.0" } }, null, 2),
    "utf8",
  );
  await writeFile(
    path.join(rootDir, "plain-app", "tsconfig.json"),
    JSON.stringify({ include: [".next/types/**/*.ts"] }, null, 2),
    "utf8",
  );

  const nextResult = runAudit(path.join(rootDir, "next-app"));
  assert.equal(nextResult.status, 0, nextResult.stderr);
  assert.ok(nextResult.stdout.includes("Include pattern does not match any files"));
  assert.ok(nextResult.stdout.includes('include pattern ".next/types/**/*.ts" matched 0 files'));
  assert.ok(
    nextResult.stdout.includes("Next.js generates .next/types during development or build"),
  );
  assert.ok(
    !nextResult.stdout.includes(
      "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
    ),
  );

  const plainResult = runAudit(path.join(rootDir, "plain-app"));
  assert.equal(plainResult.status, 1, plainResult.stderr);
  assert.ok(plainResult.stdout.includes("Include pattern does not match any files"));
  assert.ok(
    plainResult.stdout.includes(
      "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
    ),
  );
  assert.ok(
    !plainResult.stdout.includes("Next.js generates .next/types during development or build"),
  );
});

function runAudit(target) {
  return spawnSync(process.execPath, ["./bin/maximus.js", "audit", target], {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

async function shouldRunRustCliAssertions(t) {
  for (const candidate of [path.join(repoRoot, "target", "debug", "maximus"), path.join(repoRoot, "target", "release", "maximus")]) {
    try {
      await access(candidate);
      return true;
    } catch {
      // try next candidate
    }
  }

  t.skip("Rust canonical runtime build is not available; skip CLI pattern assertions on the frozen JS compatibility path.");
  return false;
}
