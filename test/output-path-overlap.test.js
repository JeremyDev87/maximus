import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDir, "..");

test("fixture-backed output path overlap audits stay wired through the CLI", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const overlap = runAudit("./test/fixtures/output-path-overlap/outdir-src");
  assert.equal(overlap.status, 1, overlap.stderr);
  assert.ok(overlap.stdout.includes("출력 디렉터리가 TypeScript source root와 겹침"));
  assert.ok(overlap.stdout.includes('outDir "src"이 source root "src"와 겹칩니다.'));

  const safe = runAudit("./test/fixtures/output-path-overlap/outdir-dist");
  assert.equal(safe.status, 0, safe.stderr);
  assert.ok(safe.stdout.includes("설정 차이가 감지되지 않았습니다."));
  assert.ok(!safe.stdout.includes("출력 디렉터리가 TypeScript source root와 겹침"));
  assert.ok(!safe.stdout.includes("출력 디렉터리가 TypeScript source root 안에 있음"));
});

test("CLI audit handles rootDir-dot, mixed inputs, and unmatched-include overlap boundaries", async (t) => {
  if (!(await shouldRunRustCliAssertions(t))) {
    return;
  }

  const rootDir = await mkdtemp(path.join(os.tmpdir(), "maximus-output-path-overlap-"));
  t.after(async () => {
    await rm(rootDir, { recursive: true, force: true });
  });

  await mkdir(path.join(rootDir, "rootdir-dot"), { recursive: true });
  await writeFile(
    path.join(rootDir, "rootdir-dot", "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: { rootDir: ".", outDir: "./src" },
        files: ["index.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(path.join(rootDir, "rootdir-dot", "index.ts"), "export const root = true;\n", "utf8");

  await mkdir(path.join(rootDir, "outdir-dot", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "outdir-dot", "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: { outDir: "." },
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(path.join(rootDir, "outdir-dot", "src", "index.ts"), "export const source = true;\n", "utf8");

  await mkdir(path.join(rootDir, "mixed", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "mixed", "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: { outDir: "./src" },
        files: ["src/index.ts", "config.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(path.join(rootDir, "mixed", "src", "index.ts"), "export const emitted = true;\n", "utf8");
  await writeFile(path.join(rootDir, "mixed", "config.ts"), "export const config = true;\n", "utf8");

  await mkdir(path.join(rootDir, "safe", "src"), { recursive: true });
  await writeFile(
    path.join(rootDir, "safe", "tsconfig.json"),
    JSON.stringify(
      {
        compilerOptions: { outDir: "./src/generated" },
        files: ["index.ts"],
        include: ["src/**/*.ts"],
      },
      null,
      2,
    ),
    "utf8",
  );
  await writeFile(path.join(rootDir, "safe", "index.ts"), "export const root = true;\n", "utf8");

  const rootdirDot = runAudit(path.join(rootDir, "rootdir-dot"));
  assert.equal(rootdirDot.status, 1, rootdirDot.stderr);
  assert.ok(rootdirDot.stdout.includes("출력 디렉터리가 TypeScript source root 안에 있음"));
  assert.ok(rootdirDot.stdout.includes('outDir "src"이 source root "." 안에 있습니다.'));

  const outdirDot = runAudit(path.join(rootDir, "outdir-dot"));
  assert.equal(outdirDot.status, 1, outdirDot.stderr);
  assert.ok(outdirDot.stdout.includes("출력 디렉터리에 TypeScript 입력 파일이 포함됨"));
  assert.ok(outdirDot.stdout.includes('outDir "."에 TypeScript 입력 "src/index.ts"이 포함됩니다.'));

  const mixed = runAudit(path.join(rootDir, "mixed"));
  assert.equal(mixed.status, 1, mixed.stderr);
  assert.ok(mixed.stdout.includes("출력 디렉터리가 TypeScript source root와 겹침"));
  assert.ok(mixed.stdout.includes('outDir "src"이 source root "src"와 겹칩니다.'));

  const safe = runAudit(path.join(rootDir, "safe"));
  assert.equal(safe.status, 1, safe.stderr);
  assert.ok(!safe.stdout.includes("출력 디렉터리가 TypeScript source root와 겹침"));
  assert.ok(!safe.stdout.includes("출력 디렉터리가 TypeScript source root 안에 있음"));
  assert.ok(safe.stdout.includes("include pattern이 어떤 파일과도 일치하지 않음"));
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

  t.skip("Rust canonical runtime build is not available; skip CLI output-path assertions on the frozen JS compatibility path.");
  return false;
}
