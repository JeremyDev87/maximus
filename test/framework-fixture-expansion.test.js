import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { readFile } from "node:fs/promises";

import { auditProject } from "../src/core/audit-project.js";
import { discoverProject, getFiles } from "../src/core/discover.js";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(testDir, "..");

test("next app clean fixture keeps a realistic app manifest and stays clean", async () => {
  const fixtureDir = path.join(repoRoot, "test", "fixtures", "next-app-clean");
  const packageJson = JSON.parse(await readFile(path.join(fixtureDir, "package.json"), "utf8"));
  const audit = await auditProject(fixtureDir);

  assert.equal(packageJson.name, "next-app-clean");
  assert.equal(packageJson.private, true);
  assert.equal(packageJson.scripts.dev, "next dev");
  assert.equal(packageJson.scripts.build, "next build");
  assert.equal(packageJson.scripts.start, "next start");
  assert.equal(packageJson.dependencies.next, "^15.3.0");
  assert.equal(packageJson.dependencies.react, "^19.0.0");
  assert.equal(packageJson.dependencies["react-dom"], "^19.0.0");
  assert.equal(audit.summary.blockingFindings, 0);
  assert.equal(audit.summary.warningFindings, 0);
  assert.equal(audit.fixes.length, 0);
});

test("vite app clean fixture keeps a realistic app manifest and stays clean", async () => {
  const fixtureDir = path.join(repoRoot, "test", "fixtures", "vite-app-clean");
  const packageJson = JSON.parse(await readFile(path.join(fixtureDir, "package.json"), "utf8"));
  const audit = await auditProject(fixtureDir);

  assert.equal(packageJson.name, "vite-app-clean");
  assert.equal(packageJson.private, true);
  assert.equal(packageJson.scripts.dev, "vite");
  assert.equal(packageJson.scripts.build, "vite build");
  assert.equal(packageJson.scripts.preview, "vite preview");
  assert.equal(packageJson.devDependencies.vite, "^6.3.0");
  assert.equal(audit.summary.blockingFindings, 0);
  assert.equal(audit.summary.warningFindings, 0);
  assert.equal(audit.fixes.length, 0);
});

test("pnpm workspace clean fixture is discovered as workspace config and stays clean", async () => {
  const fixtureDir = path.join(repoRoot, "test", "fixtures", "pnpm-workspace-clean");
  const project = await discoverProject(fixtureDir);
  const audit = await auditProject(fixtureDir);
  const workspaceFiles = getFiles(project, "workspace").map((file) => file.name);
  const content = await readFile(path.join(fixtureDir, "pnpm-workspace.yaml"), "utf8");

  assert.deepEqual(workspaceFiles, ["pnpm-workspace.yaml"]);
  assert.match(content, /packages:/u);
  assert.match(content, /apps\/\*/u);
  assert.match(content, /packages\/\*/u);
  assert.equal(audit.summary.blockingFindings, 0);
  assert.equal(audit.summary.warningFindings, 0);
  assert.equal(audit.fixes.length, 0);
});

test("turborepo clean fixture is discovered as workspace config and stays clean", async () => {
  const fixtureDir = path.join(repoRoot, "test", "fixtures", "turborepo-clean");
  const project = await discoverProject(fixtureDir);
  const audit = await auditProject(fixtureDir);
  const workspaceFiles = getFiles(project, "workspace").map((file) => file.name);
  const turboJson = JSON.parse(await readFile(path.join(fixtureDir, "turbo.json"), "utf8"));

  assert.deepEqual(workspaceFiles, ["turbo.json"]);
  assert.equal(turboJson.$schema, "https://turbo.build/schema.json");
  assert.equal(turboJson.tasks.build.dependsOn[0], "^build");
  assert.deepEqual(turboJson.tasks.build.outputs, [".next/**", "dist/**"]);
  assert.equal(audit.summary.blockingFindings, 0);
  assert.equal(audit.summary.warningFindings, 0);
  assert.equal(audit.fixes.length, 0);
});
