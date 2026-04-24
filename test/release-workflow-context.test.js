import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { assertReleaseWorkflowContext } from "../scripts/assert-release-workflow-context.mjs";

async function writePackageJson(dir, version) {
  await writeFile(
    path.join(dir, "package.json"),
    JSON.stringify({ name: "@jeremyfellaz/maximus", version }, null, 2),
    "utf8",
  );
}

test("release workflow context accepts matching release tags", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-context-stable-"));
  await writePackageJson(repoRoot, "1.2.3");

  const summary = await assertReleaseWorkflowContext({
    repoRoot,
    eventName: "push",
    githubRef: "refs/tags/v1.2.3",
    githubRefName: "v1.2.3",
    requestedReleaseTag: "v1.2.3",
  });

  assert.equal(summary.packageVersion, "1.2.3");
  assert.equal(summary.releaseTag, "v1.2.3");
});

test("release workflow context accepts workflow dispatch when the selected tag ref is provided", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-context-dispatch-"));
  await writePackageJson(repoRoot, "1.2.3-alpha.1");

  const summary = await assertReleaseWorkflowContext({
    repoRoot,
    eventName: "workflow_dispatch",
    githubRef: "refs/tags/v1.2.3-alpha.1",
    githubRefName: "v1.2.3-alpha.1",
    requestedReleaseTag: "v1.2.3-alpha.1",
  });

  assert.equal(summary.packageVersion, "1.2.3-alpha.1");
  assert.equal(summary.releaseTag, "v1.2.3-alpha.1");
});

test("release workflow context rejects non-tag refs", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-context-nontag-"));
  await writePackageJson(repoRoot, "1.2.3");

  await assert.rejects(
    assertReleaseWorkflowContext({
      repoRoot,
      eventName: "workflow_dispatch",
      githubRef: "refs/heads/master",
      githubRefName: "master",
      requestedReleaseTag: "v1.2.3",
    }),
    /must run from a tag ref/,
  );
});

test("release workflow context rejects mismatched selected tags", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-context-selected-mismatch-"));
  await writePackageJson(repoRoot, "1.2.3");

  await assert.rejects(
    assertReleaseWorkflowContext({
      repoRoot,
      eventName: "workflow_dispatch",
      githubRef: "refs/tags/v1.2.3",
      githubRefName: "v1.2.3",
      requestedReleaseTag: "v1.2.4",
    }),
    /does not match release tag/,
  );
});

test("release workflow context rejects package version mismatches", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-context-version-mismatch-"));
  await writePackageJson(repoRoot, "1.2.3");

  await assert.rejects(
    assertReleaseWorkflowContext({
      repoRoot,
      eventName: "push",
      githubRef: "refs/tags/v1.2.4",
      githubRefName: "v1.2.4",
      requestedReleaseTag: "v1.2.4",
    }),
    /does not match package\.json version/,
  );
});
