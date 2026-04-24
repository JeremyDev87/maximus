#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { appendFile, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { assertReleaseUpgrade, resolveReleasePlan } from "./lib/release.mjs";

const fullShaPattern = /^[0-9A-Fa-f]{40}$/;

function git(repoRoot, args, options = {}) {
  return execFileSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: options.quiet ? ["ignore", "pipe", "ignore"] : ["ignore", "pipe", "pipe"],
  }).trim();
}

function gitSucceeds(repoRoot, args) {
  try {
    git(repoRoot, args, { quiet: true });
    return true;
  } catch {
    return false;
  }
}

async function readCurrentPackageVersion(repoRoot) {
  const packageJson = JSON.parse(await readFile(path.join(repoRoot, "package.json"), "utf8"));
  return packageJson.version;
}

function readPackageVersionAt(repoRoot, ref) {
  const packageJson = git(repoRoot, ["show", `${ref}:package.json`]);
  return JSON.parse(packageJson).version;
}

function resolvePushPreviousVersion(repoRoot, beforeSha) {
  if (!fullShaPattern.test(beforeSha ?? "")) {
    throw new Error("github.event.before must be a full 40-character commit SHA.");
  }

  if (!gitSucceeds(repoRoot, ["cat-file", "-e", `${beforeSha}:package.json`])) {
    throw new Error("github.event.before must contain package.json for release upgrade validation.");
  }

  return readPackageVersionAt(repoRoot, beforeSha);
}

function resolveManualPreviousVersion(repoRoot, targetSha, verifiedSha, fetchMaster) {
  if (!fullShaPattern.test(targetSha ?? "")) {
    throw new Error("target_sha must be a full 40-character commit SHA.");
  }

  if (verifiedSha !== targetSha) {
    throw new Error(`Expected checkout to resolve to ${targetSha}, got ${verifiedSha}`);
  }

  if (fetchMaster) {
    git(repoRoot, ["fetch", "origin", "refs/heads/master:refs/remotes/origin/master"]);
  }

  if (!gitSucceeds(repoRoot, ["merge-base", "--is-ancestor", verifiedSha, "refs/remotes/origin/master"])) {
    throw new Error("target_sha must already be reachable from origin/master before tagging.");
  }

  const parentSha = git(repoRoot, ["rev-parse", `${verifiedSha}^`]);
  if (!gitSucceeds(repoRoot, ["cat-file", "-e", `${parentSha}:package.json`])) {
    throw new Error("target_sha parent must contain package.json.");
  }

  return readPackageVersionAt(repoRoot, parentSha);
}

export async function resolveReleaseCandidateMetadata({
  repoRoot = process.cwd(),
  eventName,
  beforeSha = "",
  targetSha = "",
  fetchMaster = true,
} = {}) {
  const version = await readCurrentPackageVersion(repoRoot);
  const plan = resolveReleasePlan(`v${version}`);
  const verifiedSha = git(repoRoot, ["rev-parse", "HEAD"]);
  let previousVersion = "";

  if (eventName === "push") {
    previousVersion = resolvePushPreviousVersion(repoRoot, beforeSha);
  } else if (eventName === "workflow_dispatch") {
    previousVersion = resolveManualPreviousVersion(repoRoot, targetSha, verifiedSha, fetchMaster);
  } else {
    throw new Error(`Unsupported release candidate event: ${eventName}`);
  }

  if (previousVersion) {
    assertReleaseUpgrade(previousVersion, plan.version);
  }

  return {
    version: plan.version,
    tag: plan.tag,
    previousVersion,
    verifiedSha,
  };
}

async function writeGithubOutputs(metadata) {
  if (!process.env.GITHUB_OUTPUT) {
    return;
  }

  await appendFile(
    process.env.GITHUB_OUTPUT,
    [
      `version=${metadata.version}`,
      `tag=${metadata.tag}`,
      `previousVersion=${metadata.previousVersion}`,
      `verifiedSha=${metadata.verifiedSha}`,
    ].join("\n") + "\n",
    "utf8",
  );
}

const isDirectExecution =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isDirectExecution) {
  try {
    const metadata = await resolveReleaseCandidateMetadata({
      eventName: process.env.RELEASE_EVENT_NAME ?? process.env.GITHUB_EVENT_NAME,
      beforeSha: process.env.RELEASE_EVENT_BEFORE ?? process.env.GITHUB_EVENT_BEFORE,
      targetSha: process.env.TARGET_SHA,
    });
    await writeGithubOutputs(metadata);
    console.log(
      `Resolved release candidate ${metadata.tag} at ${metadata.verifiedSha} (previous: ${metadata.previousVersion || "unknown"}).`,
    );
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
