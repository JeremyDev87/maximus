import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { mkdtemp, writeFile } from "node:fs/promises";

import { resolveReleaseCandidateMetadata } from "../scripts/release-candidate-metadata.mjs";

function git(repoRoot, args) {
  return execFileSync("git", args, { cwd: repoRoot, encoding: "utf8" }).trim();
}

async function writePackageJson(repoRoot, version) {
  await writeFile(
    path.join(repoRoot, "package.json"),
    JSON.stringify({ name: "@jeremyfellaz/maximus", version }, null, 2) + "\n",
    "utf8",
  );
}

async function createReleaseRepo() {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-candidate-"));
  git(repoRoot, ["init", "-b", "master"]);
  git(repoRoot, ["config", "user.email", "test@example.com"]);
  git(repoRoot, ["config", "user.name", "Test User"]);

  await writePackageJson(repoRoot, "0.1.0");
  git(repoRoot, ["add", "package.json"]);
  git(repoRoot, ["commit", "-m", "initial release"]);
  const previousSha = git(repoRoot, ["rev-parse", "HEAD"]);

  await writePackageJson(repoRoot, "0.1.1");
  git(repoRoot, ["add", "package.json"]);
  git(repoRoot, ["commit", "-m", "release 0.1.1"]);
  const releaseSha = git(repoRoot, ["rev-parse", "HEAD"]);

  const remoteRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-release-candidate-remote-"));
  git(remoteRoot, ["init", "--bare"]);
  git(repoRoot, ["remote", "add", "origin", remoteRoot]);
  git(repoRoot, ["push", "-u", "origin", "master"]);
  git(repoRoot, ["fetch", "origin", "refs/heads/master:refs/remotes/origin/master"]);

  return { repoRoot, previousSha, releaseSha };
}

test("workflow_dispatch metadata compares the target commit against its parent version", async () => {
  const { repoRoot, releaseSha } = await createReleaseRepo();

  const metadata = await resolveReleaseCandidateMetadata({
    repoRoot,
    eventName: "workflow_dispatch",
    targetSha: releaseSha,
    fetchMaster: false,
  });

  assert.equal(metadata.version, "0.1.1");
  assert.equal(metadata.tag, "v0.1.1");
  assert.equal(metadata.previousVersion, "0.1.0");
  assert.equal(metadata.verifiedSha, releaseSha);
});

test("workflow_dispatch metadata rejects targets that are not on origin/master", async () => {
  const { repoRoot, previousSha } = await createReleaseRepo();
  git(repoRoot, ["switch", "-c", "unmerged-release", previousSha]);
  await writePackageJson(repoRoot, "0.1.2");
  git(repoRoot, ["add", "package.json"]);
  git(repoRoot, ["commit", "-m", "unmerged release"]);
  const unmergedSha = git(repoRoot, ["rev-parse", "HEAD"]);

  await assert.rejects(
    resolveReleaseCandidateMetadata({
      repoRoot,
      eventName: "workflow_dispatch",
      targetSha: unmergedSha,
      fetchMaster: false,
    }),
    /target_sha must already be reachable from origin\/master/,
  );
});

test("push metadata compares the current version against github.event.before", async () => {
  const { repoRoot, previousSha, releaseSha } = await createReleaseRepo();

  const metadata = await resolveReleaseCandidateMetadata({
    repoRoot,
    eventName: "push",
    beforeSha: previousSha,
  });

  assert.equal(metadata.version, "0.1.1");
  assert.equal(metadata.tag, "v0.1.1");
  assert.equal(metadata.previousVersion, "0.1.0");
  assert.equal(metadata.verifiedSha, releaseSha);
});

test("push metadata fails closed when github.event.before cannot be inspected", async () => {
  const { repoRoot } = await createReleaseRepo();

  await assert.rejects(
    resolveReleaseCandidateMetadata({
      repoRoot,
      eventName: "push",
      beforeSha: "0000000000000000000000000000000000000000",
    }),
    /github\.event\.before must contain package\.json/,
  );
});
