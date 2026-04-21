import assert from "node:assert/strict";
import path from "node:path";
import { appendFile, readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

async function readPackageVersion(repoRoot) {
  const packageJsonPath = path.join(repoRoot, "package.json");
  const packageJson = JSON.parse(await readFile(packageJsonPath, "utf8"));
  return packageJson.version;
}

export async function assertReleaseWorkflowContext({
  repoRoot = process.cwd(),
  eventName,
  githubRef,
  githubRefName,
  requestedReleaseTag,
}) {
  assert.ok(
    eventName === "push" || eventName === "workflow_dispatch",
    `unsupported release workflow event: ${eventName}`,
  );
  assert.ok(requestedReleaseTag, "release tag is required");
  assert.ok(
    githubRef.startsWith("refs/tags/"),
    `release workflow must run from a tag ref, received ${githubRef}`,
  );

  const resolvedRefName = githubRefName || githubRef.slice("refs/tags/".length);
  assert.equal(
    resolvedRefName,
    requestedReleaseTag,
    `selected ref ${resolvedRefName} does not match release tag ${requestedReleaseTag}`,
  );

  const packageVersion = await readPackageVersion(repoRoot);
  const expectedReleaseTag = `v${packageVersion}`;
  assert.equal(
    requestedReleaseTag,
    expectedReleaseTag,
    `release tag ${requestedReleaseTag} does not match package.json version ${expectedReleaseTag}`,
  );

  return {
    eventName,
    packageVersion,
    releaseTag: requestedReleaseTag,
  };
}

async function writeGithubOutputs(summary) {
  if (!process.env.GITHUB_OUTPUT) {
    return;
  }

  await appendFile(
    process.env.GITHUB_OUTPUT,
    `package_version=${summary.packageVersion}\nrelease_tag=${summary.releaseTag}\n`,
    "utf8",
  );
}

const isDirectExecution =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isDirectExecution) {
  const [eventName, githubRef, githubRefName, requestedReleaseTag, repoRootArg] =
    process.argv.slice(2);

  try {
    const summary = await assertReleaseWorkflowContext({
      repoRoot: repoRootArg || process.cwd(),
      eventName,
      githubRef,
      githubRefName,
      requestedReleaseTag,
    });
    await writeGithubOutputs(summary);
    console.log(`Validated release workflow context for ${summary.releaseTag}.`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Release workflow context validation failed: ${message}`);
    process.exitCode = 1;
  }
}
