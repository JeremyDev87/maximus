import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, writeFile } from "node:fs/promises";
import { buildReleasePlan } from "../scripts/release-plan.mjs";

async function writePackageJson(dir, version) {
  await writeFile(
    path.join(dir, "package.json"),
    JSON.stringify({ name: "@jeremyfellaz/maximus", version }, null, 2),
    "utf8",
  );
}

test("release plan resolves stable tags to latest", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-plan-stable-"));
  await writePackageJson(repoRoot, "1.2.3");

  const plan = await buildReleasePlan({
    repoRoot,
    eventName: "push",
    githubRef: "refs/tags/v1.2.3",
    githubRefName: "v1.2.3",
    requestedReleaseTag: "v1.2.3",
  });

  assert.equal(plan.releaseTag, "v1.2.3");
  assert.equal(plan.packageVersion, "1.2.3");
  assert.equal(plan.distTag, "latest");
  assert.equal(plan.isPrerelease, false);
});

test("release plan resolves prerelease tags to next", async () => {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-plan-prerelease-"));
  await writePackageJson(repoRoot, "1.2.3-alpha.1");

  const plan = await buildReleasePlan({
    repoRoot,
    eventName: "workflow_dispatch",
    githubRef: "refs/tags/v1.2.3-alpha.1",
    githubRefName: "v1.2.3-alpha.1",
    requestedReleaseTag: "v1.2.3-alpha.1",
  });

  assert.equal(plan.releaseTag, "v1.2.3-alpha.1");
  assert.equal(plan.packageVersion, "1.2.3-alpha.1");
  assert.equal(plan.distTag, "next");
  assert.equal(plan.isPrerelease, true);
});
