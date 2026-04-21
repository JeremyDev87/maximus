import assert from "node:assert/strict";
import test from "node:test";
import { assertReleaseWorkflowContext } from "../scripts/assert-release-workflow-context.mjs";

test("release workflow context accepts matching release tags", async () => {
  const summary = await assertReleaseWorkflowContext({
    repoRoot: process.cwd(),
    eventName: "push",
    githubRef: "refs/tags/v0.1.0",
    githubRefName: "v0.1.0",
    requestedReleaseTag: "v0.1.0",
  });

  assert.equal(summary.packageVersion, "0.1.0");
  assert.equal(summary.releaseTag, "v0.1.0");
});

test("release workflow context accepts workflow dispatch when the selected tag ref is provided", async () => {
  const summary = await assertReleaseWorkflowContext({
    repoRoot: process.cwd(),
    eventName: "workflow_dispatch",
    githubRef: "refs/tags/v0.1.0",
    githubRefName: "v0.1.0",
    requestedReleaseTag: "v0.1.0",
  });

  assert.equal(summary.packageVersion, "0.1.0");
  assert.equal(summary.releaseTag, "v0.1.0");
});

test("release workflow context rejects non-tag refs", async () => {
  await assert.rejects(
    assertReleaseWorkflowContext({
      repoRoot: process.cwd(),
      eventName: "workflow_dispatch",
      githubRef: "refs/heads/master",
      githubRefName: "master",
      requestedReleaseTag: "v0.1.0",
    }),
    /must run from a tag ref/,
  );
});

test("release workflow context rejects mismatched selected tags", async () => {
  await assert.rejects(
    assertReleaseWorkflowContext({
      repoRoot: process.cwd(),
      eventName: "workflow_dispatch",
      githubRef: "refs/tags/v0.1.0",
      githubRefName: "v0.1.0",
      requestedReleaseTag: "v0.1.1",
    }),
    /does not match release tag/,
  );
});

test("release workflow context rejects package version mismatches", async () => {
  await assert.rejects(
    assertReleaseWorkflowContext({
      repoRoot: process.cwd(),
      eventName: "push",
      githubRef: "refs/tags/v0.2.0",
      githubRefName: "v0.2.0",
      requestedReleaseTag: "v0.2.0",
    }),
    /does not match package\.json version/,
  );
});
