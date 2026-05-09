import assert from "node:assert/strict";
import path from "node:path";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const rootPackageName = "@jeremyfellaz/maximus";
const platformPackages = [
  "@jeremyfellaz/maximus-darwin-arm64",
  "@jeremyfellaz/maximus-darwin-x64",
  "@jeremyfellaz/maximus-linux-arm64-gnu",
  "@jeremyfellaz/maximus-linux-x64-gnu",
];

const requiredFiles = {
  action: "action.yml",
  marketplaceWrapperAction: ".github/actions/marketplace-wrapper/action.yml",
  packageManifest: "package.json",
  devWorkflow: ".github/workflows/dev.yml",
  actionSmokeWorkflow: ".github/workflows/action-smoke.yml",
  releaseWorkflow: ".github/workflows/release.yml",
  releaseCandidateWorkflow: ".github/workflows/release-candidate.yml",
  manualReleaseBumpWorkflow: ".github/workflows/manual-release-bump.yml",
  rustReleaseWorkflow: ".github/workflows/rust-release-binaries.yml",
  releaseDrafterWorkflow: ".github/workflows/release-drafter.yml",
  releaseDrafterConfig: ".github/release-drafter.yml",
  readmeKo: "README.md",
  readmeEn: "README.en.md",
  marketplaceGuide: "docs/github-action-marketplace.md",
  npmWrapperRuntime: "docs/npm-wrapper-runtime.md",
  contributing: "CONTRIBUTING.md",
  releaseRunbook: "docs/release-operator-runbook.md",
  releaseContextAssertion: "scripts/assert-release-workflow-context.mjs",
  releasePlan: "scripts/release-plan.mjs",
  releaseCandidateMetadata: "scripts/release-candidate-metadata.mjs",
  releaseHelpers: "scripts/lib/release.mjs",
  releaseBumpScript: "scripts/bump-release-version.mjs",
  npmLookupClassifier: "scripts/classify-npm-lookup-error.mjs",
  npmPublishClassifier: "scripts/classify-npm-publish-error.mjs",
  nativeRuntimeAssertion: "scripts/assert-installed-native-runtime.mjs",
};

export async function validateRustReleaseWiring(repoRoot = process.cwd()) {
  const fileContents = await readRequiredFiles(repoRoot);
  const packageManifest = JSON.parse(fileContents.packageManifest);
  validateAction(fileContents.action);
  validateMarketplaceWrapperAction(fileContents.marketplaceWrapperAction);
  validateDevWorkflow(fileContents.devWorkflow);
  validateActionSmokeWorkflow(fileContents.actionSmokeWorkflow);
  validateReleaseCandidateWorkflow(fileContents.releaseCandidateWorkflow);
  validateManualReleaseBumpWorkflow(fileContents.manualReleaseBumpWorkflow);
  validateRustReleaseWorkflow(fileContents.rustReleaseWorkflow);
  validateReleaseWorkflow(fileContents.releaseWorkflow);
  validateReleaseDrafterWorkflow(fileContents.releaseDrafterWorkflow);
  validateReleaseDrafterConfig(fileContents.releaseDrafterConfig);
  validateReadmes(fileContents.readmeKo, fileContents.readmeEn);
  validateMarketplaceGuide(fileContents.marketplaceGuide);
  validateNpmWrapperRuntime(fileContents.npmWrapperRuntime);
  validateContributing(fileContents.contributing);
  validateReleaseRunbook(fileContents.releaseRunbook);
  validateReleaseContextAssertion(fileContents.releaseContextAssertion);
  validateReleasePlanScript(fileContents.releasePlan);
  validateReleaseCandidateMetadata(fileContents.releaseCandidateMetadata);
  validateReleaseHelpers(fileContents.releaseHelpers);
  validateReleaseBumpScript(fileContents.releaseBumpScript);
  validateNpmLookupClassifier(fileContents.npmLookupClassifier);
  validateNpmPublishClassifier(fileContents.npmPublishClassifier);
  validateNativeRuntimeAssertion(fileContents.nativeRuntimeAssertion);

  return {
    checkedFiles: Object.values(requiredFiles),
    platformPackages,
  };
}

async function readRequiredFiles(repoRoot) {
  const entries = await Promise.all(
    Object.entries(requiredFiles).map(async ([key, relativePath]) => {
      const absolutePath = path.join(repoRoot, relativePath);
      return [key, await readFile(absolutePath, "utf8")];
    }),
  );
  return Object.fromEntries(entries);
}

function validateAction(actionText) {
  assertContains(actionText, "name: Maximus", "action metadata name");
  assertContains(actionText, 'uses: actions/setup-node@6044e13b5dc448c55e2357c09f80417699197238', "action node setup");
  assertContains(actionText, "MAXIMUS_REGISTRY_URL: ${{ inputs.registry-url }}", "action registry env wiring");
  assertContains(actionText, 'if [[ -n "$MAXIMUS_REGISTRY_URL" ]]; then', "action registry env usage");
  assertContains(actionText, "Install published Maximus wrapper for this action ref", "action published package install step");
  assertContains(actionText, "process.env.GITHUB_ACTION_PATH + '/package.json'", "action package version lookup");
  assertContains(actionText, 'npm install --no-package-lock --prefix "$install_root" "@jeremyfellaz/maximus@$package_version"', "action published package install");
  assertContains(actionText, 'node "$GITHUB_ACTION_PATH/scripts/assert-installed-native-runtime.mjs" "$install_root"', "action native runtime assertion");
  assertContains(actionText, "MAXIMUS_COMMAND: ${{ inputs.command }}", "action command env wiring");
  assertContains(actionText, "MAXIMUS_TARGET_PATH: ${{ inputs.path }}", "action path env wiring");
  assertContains(actionText, '"$MAXIMUS_COMMAND" "$MAXIMUS_TARGET_PATH"', "action wrapper env argv usage");
  assertContains(actionText, 'node "$install_root/node_modules/@jeremyfellaz/maximus/bin/maximus.js"', "action wrapper invocation");
  assertContains(actionText, "registry-url", "action registry override input");
  assert.ok(
    !actionText.includes('if [[ -n "${{ inputs.registry-url }}" ]]; then'),
    "action should not interpolate registry input directly inside bash",
  );
  assert.ok(
    !actionText.includes('"${{ inputs.command }}" "${{ inputs.path }}"'),
    "action should not interpolate command or path inputs directly inside bash",
  );
  assert.ok(
    !actionText.includes('npm install --no-package-lock --prefix "$install_root" "$GITHUB_ACTION_PATH"'),
    "action should install the published package version instead of the local action path",
  );
}

function validateDevWorkflow(devText) {
  const requiredPaths = [
    "action.yml",
    ".github/actions/marketplace-wrapper/action.yml",
    ".github/workflows/action-smoke.yml",
    ".github/workflows/manual-release-bump.yml",
    ".github/workflows/release-candidate.yml",
    ".github/workflows/release.yml",
    ".github/workflows/rust-release-binaries.yml",
    "docs/github-action-marketplace.md",
  ];

  for (const requiredPath of requiredPaths) {
    assertContains(devText, `"${requiredPath}"`, `dev path filter for ${requiredPath}`);
  }

  assertContains(devText, '"scripts/**"', "dev path filter for scripts");
  assertContains(devText, '"test/**"', "dev path filter for tests");

  assertContains(devText, "release-wiring:", "release wiring job");
  assertContains(devText, "node ./scripts/validate-rust-release-wiring.mjs", "release wiring validation command");
  assertContains(devText, "node --test test/release-workflow-context.test.js test/github-action-wiring.test.js test/release-docs.test.js test/release-plan.test.js test/release-candidate-metadata.test.js test/release-helpers.test.js test/npm-error-classifiers.test.js test/bump-release-version.test.js", "release wiring node test command");
}

function validateMarketplaceWrapperAction(actionText) {
  assertContains(actionText, "name: Maximus Marketplace Wrapper", "marketplace wrapper metadata name");
  assertContains(actionText, "registry-url", "marketplace wrapper registry input");
  assertContains(actionText, "Resolve repository root", "marketplace wrapper repo root step");
  assertContains(actionText, 'repo_root="$(cd "$GITHUB_ACTION_PATH/../../.." && pwd)"', "marketplace wrapper repo root resolution");
  assertContains(actionText, "Install published Maximus wrapper for this action ref", "marketplace wrapper published package install step");
  assertContains(actionText, "process.env.REPO_ROOT + '/package.json'", "marketplace wrapper package version lookup");
  assertContains(actionText, 'npm install --no-package-lock --prefix "$install_root" "@jeremyfellaz/maximus@$package_version"', "marketplace wrapper published package install");
  assertContains(actionText, 'node "$REPO_ROOT/scripts/assert-installed-native-runtime.mjs" "$install_root"', "marketplace wrapper runtime assertion");
  assertContains(actionText, 'node "$install_root/node_modules/@jeremyfellaz/maximus/bin/maximus.js"', "marketplace wrapper runtime invocation");
  assert.ok(
    !actionText.includes('npm install --no-package-lock --prefix "$install_root" "$REPO_ROOT"'),
    "marketplace wrapper should install the published package version instead of the local repository path",
  );
}

function validateActionSmokeWorkflow(actionSmokeText) {
  assertContains(actionSmokeText, "workflow_call:", "action smoke reusable trigger");
  assertContains(actionSmokeText, "workflow_dispatch:", "action smoke manual trigger");
  assertContains(actionSmokeText, "release_tag:", "action smoke release tag input");
  assertContains(actionSmokeText, "release_sha:", "action smoke release sha input");
  assertContains(actionSmokeText, "ref: ${{ inputs.release_sha || inputs.release_tag }}", "action smoke checkout ref");
  assertContains(actionSmokeText, "RELEASE_SHA: ${{ inputs.release_sha }}", "action smoke release sha env handoff");
  assertContains(actionSmokeText, "RELEASE_TAG: ${{ inputs.release_tag }}", "action smoke release tag env handoff");
  assertContains(actionSmokeText, 'test "$(git rev-parse HEAD)" = "$RELEASE_SHA"', "action smoke sha comparison");
  assertContains(actionSmokeText, 'git fetch --depth=1 origin "refs/tags/${RELEASE_TAG}:refs/tags/${RELEASE_TAG}"', "action smoke tag fetch");
  assertContains(actionSmokeText, 'test "$(git rev-list -n 1 "$RELEASE_TAG")" = "$RELEASE_SHA"', "action smoke tag to sha comparison");
  assertContains(actionSmokeText, 'git describe --tags --exact-match HEAD', "action smoke exact tag assertion");
  assertContains(actionSmokeText, "uses: ./", "action smoke local tag checkout usage");
  assertContains(actionSmokeText, "uses: ./.github/actions/marketplace-wrapper", "action smoke marketplace wrapper usage");
  assertContains(actionSmokeText, "dynamic expressions in step-level `uses:`", "action smoke dynamic uses rationale");
  assertContains(actionSmokeText, "registry-url: ${{ inputs.registry_url }}", "action smoke registry passthrough");
  assert.ok(
    !actionSmokeText.includes('"${{ inputs.release_sha }}"'),
    "action smoke workflow should not interpolate release_sha directly inside bash",
  );
  assert.ok(
    !actionSmokeText.includes('"${{ inputs.release_tag }}"'),
    "action smoke workflow should not interpolate release_tag directly inside bash",
  );
  assert.ok(
    !actionSmokeText.includes("uses: JeremyDev87/maximus@v0.1.0"),
    "action smoke should not pin a static published action tag",
  );
}

function validateRustReleaseWorkflow(rustReleaseText) {
  assertContains(rustReleaseText, "workflow_call:", "reusable rust release trigger");
  assertContains(rustReleaseText, "workflow_dispatch:", "manual rust release trigger");
  assertContains(rustReleaseText, "release_ref:", "rust release ref input");
  assertContains(rustReleaseText, "ref: ${{ inputs.release_ref }}", "rust release checkout ref");
  assertContains(rustReleaseText, "cargo build --release -p maximus-cli", "rust release build step");
  assertContains(rustReleaseText, "actions/upload-artifact@v4", "rust release artifact upload");

  for (const packageName of platformPackages) {
    assertContains(rustReleaseText, packageName, `rust release matrix entry for ${packageName}`);
  }
}

function validateReleaseCandidateWorkflow(releaseCandidateText) {
  assertContains(releaseCandidateText, "workflow_dispatch:", "release candidate manual trigger");
  assertContains(releaseCandidateText, "crates/maximus-cli/Cargo.toml", "release candidate CLI Cargo path filter");
  assertContains(releaseCandidateText, "crates/maximus-core/Cargo.toml", "release candidate core Cargo path filter");
  assertContains(releaseCandidateText, "crates/maximus-checks/Cargo.toml", "release candidate checks Cargo path filter");
  assertContains(releaseCandidateText, "Cargo.lock", "release candidate Cargo lock path filter");
  assertContains(releaseCandidateText, "target_sha:", "release candidate target sha input");
  assertContains(releaseCandidateText, "TARGET_SHA: ${{ inputs.target_sha }}", "release candidate target sha env handoff");
  assertContains(releaseCandidateText, "Ensure release tag does not already exist", "release candidate tag guard");
  assertContains(releaseCandidateText, "Verify package manifest alignment", "release candidate manifest alignment gate");
  assertContains(releaseCandidateText, "readCargoTomlPackageVersion", "release candidate Rust crate version gate");
  assertContains(releaseCandidateText, "readCargoLockWorkspacePackageVersions", "release candidate Cargo.lock version gate");
  assertContains(releaseCandidateText, "rustCrateManifestPaths", "release candidate Rust crate manifest list");
  assertContains(releaseCandidateText, "node ./scripts/release-candidate-metadata.mjs", "release candidate metadata resolver");
  assertContains(releaseCandidateText, "node ./scripts/validate-rust-release-wiring.mjs", "release candidate release wiring validation");
  assertContains(releaseCandidateText, "test/release-candidate-metadata.test.js", "release candidate metadata test coverage");
  assertContains(releaseCandidateText, "test/release-helpers.test.js", "release helper test coverage");
  assertContains(releaseCandidateText, "test/bump-release-version.test.js", "release candidate bump script test coverage");
  assertContains(releaseCandidateText, "cargo test --workspace", "release candidate rust tests");
  assertContains(releaseCandidateText, "node ./scripts/run-packed-wrapper-smoke.mjs", "release candidate packed wrapper smoke");
  assert.ok(
    !releaseCandidateText.includes('"${{ inputs.target_sha }}"'),
    "release candidate workflow should not interpolate target_sha directly inside bash",
  );
  assert.ok(
    !releaseCandidateText.includes("shouldVerify"),
    "release candidate workflow should not skip verification when only platform manifests changed",
  );
}

function validateManualReleaseBumpWorkflow(manualReleaseBumpText) {
  assertContains(manualReleaseBumpText, "workflow_dispatch:", "manual bump workflow trigger");
  assertContains(manualReleaseBumpText, "dry_run:", "manual bump dry-run input");
  assertContains(manualReleaseBumpText, "INPUT_TAG: ${{ inputs.tag }}", "manual bump tag env handoff");
  assertContains(manualReleaseBumpText, 'tag="$INPUT_TAG"', "manual bump tag shell variable usage");
  assertContains(manualReleaseBumpText, "tag must look like v1.2.3", "manual bump tag validation");
  assertContains(manualReleaseBumpText, "node ./scripts/bump-release-version.mjs", "manual bump script usage");
  assertContains(manualReleaseBumpText, "versionFilePaths", "manual bump full version file allowlist");
  assertContains(manualReleaseBumpText, "readCargoTomlPackageVersion", "manual bump existing PR Rust manifest validation");
  assertContains(manualReleaseBumpText, "readCargoLockWorkspacePackageVersions", "manual bump existing PR Cargo.lock validation");
  assertContains(
    manualReleaseBumpText,
    "package.json optionalDependencies version mismatch",
    "manual bump existing PR optional dependency validation",
  );
  assertContains(
    manualReleaseBumpText,
    "git add package.json npm/*/package.json crates/maximus-cli/Cargo.toml crates/maximus-core/Cargo.toml crates/maximus-checks/Cargo.toml Cargo.lock",
    "manual bump full version file staging",
  );
  assertContains(manualReleaseBumpText, "version-only bump", "manual bump existing PR version-only wording");
  assertContains(manualReleaseBumpText, "gh workflow run dev.yml --ref", "manual bump CI dispatch");
  assertContains(manualReleaseBumpText, "skip-changelog", "manual bump skip-changelog label");
  assertContains(manualReleaseBumpText, "Unable to create a draft PR", "manual bump hard-fail PR creation");
  assert.ok(
    !manualReleaseBumpText.includes('tag="${{ inputs.tag }}"'),
    "manual bump workflow should not interpolate input tag directly inside bash",
  );
}

function validateReleaseWorkflow(releaseText) {
  assertContains(releaseText, 'Moving major tags', "release major tag non-publish comment");
  assertContains(releaseText, 'push:\n    tags:\n      - "v[0-9]+.[0-9]+.[0-9]+"\n      - "v[0-9]+.[0-9]+.[0-9]+-*"', "release package tag push trigger");
  assertContains(releaseText, "workflow_dispatch:", "release manual trigger");
  assertContains(releaseText, "release_tag:", "release workflow dispatch tag input");
  assertContains(releaseText, "validate-release-context:", "release context validation job");
  assertContains(releaseText, "needs: validate-release-context", "release context ordering");
  assertContains(releaseText, "id-token: write", "release trusted publishing permission");
  assertContains(releaseText, "Build release plan", "release plan step");
  assertContains(releaseText, "node ./scripts/release-plan.mjs", "release plan command");
  assertContains(releaseText, "dist_tag: ${{ steps.release_plan.outputs.dist_tag }}", "release dist-tag output");
  assertContains(releaseText, "is_prerelease: ${{ steps.release_plan.outputs.is_prerelease }}", "release prerelease output");
  assertContains(releaseText, "release_commit_sha: ${{ steps.capture_release_commit.outputs.release_commit_sha }}", "release commit sha output");
  assertContains(releaseText, "git rev-parse HEAD", "release commit capture command");
  assertContains(releaseText, "RELEASE_EVENT_NAME: ${{ github.event_name }}", "release event name env wiring");
  assertContains(releaseText, "RELEASE_GITHUB_REF: ${{ github.ref }}", "release ref env wiring");
  assertContains(releaseText, "RELEASE_GITHUB_REF_NAME: ${{ github.ref_name }}", "release ref_name env wiring");
  assertContains(releaseText, "RELEASE_SELECTED_TAG: ${{ github.event_name == 'push' && github.ref_name || inputs.release_tag }}", "release selected tag env wiring");
  assertContains(releaseText, "uses: ./.github/workflows/rust-release-binaries.yml", "release reusable binary workflow call");
  assertContains(releaseText, "release_ref: ${{ needs.validate-release-context.outputs.release_commit_sha }}", "release reusable binary ref input");
  assertContains(releaseText, "node ./scripts/classify-npm-lookup-error.mjs", "npm lookup classifier usage");
  assertContains(releaseText, "node ./scripts/classify-npm-publish-error.mjs", "npm publish classifier usage");
  assertContains(releaseText, "npm publish . --provenance --access public --tag \"$RELEASE_DIST_TAG\"", "root wrapper trusted publish");
  assertContains(releaseText, "NODE_AUTH_TOKEN=\"$NPM_TOKEN\" npm publish . --access public --tag \"$RELEASE_DIST_TAG\"", "root wrapper token fallback publish");
  assertContains(releaseText, "--provenance --access public --tag \"$RELEASE_DIST_TAG\"", "platform trusted publish");
  assertContains(releaseText, `npm install --no-package-lock --prefix "$install_root" "${rootPackageName}@\${{ needs.validate-release-context.outputs.package_version }}"`, "published wrapper smoke install");
  assertContains(releaseText, 'node ./scripts/assert-installed-native-runtime.mjs "$install_root"', "published wrapper native runtime assertion");
  assertContains(releaseText, 'node "$install_root/node_modules/@jeremyfellaz/maximus/bin/maximus.js" audit ./test/fixtures/clean-project', "published wrapper smoke audit");
  assertContains(releaseText, "uses: ./.github/workflows/action-smoke.yml", "release action smoke call");
  assertContains(releaseText, "release_tag: ${{ needs.validate-release-context.outputs.release_tag }}", "release action smoke tag input");
  assertContains(releaseText, "release_sha: ${{ needs.validate-release-context.outputs.release_commit_sha }}", "release action smoke sha input");
  assertContains(releaseText, "ref: ${{ github.ref }}", "release validation checkout ref");
  assertContains(releaseText, "ref: ${{ needs.validate-release-context.outputs.release_commit_sha }}", "release downstream sha checkout");
  assertContains(releaseText, "gh workflow run release.yml --ref <tag> -f release_tag=<tag>", "release workflow dispatch guidance");
  assertContains(releaseText, "publish-platform-packages", "wrapper publish ordering");
  assertContains(releaseText, "- publish-wrapper", "published wrapper smoke ordering");
  assertContains(releaseText, "strategy:\n      fail-fast: false\n      matrix:", "published wrapper smoke matrix");
  assert.ok(
    !releaseText.includes('- "v*"'),
    "release workflow should not run for moving major tags like v1",
  );
  assert.ok(
    !releaseText.includes("types: [published]"),
    "release workflow should not use release.published as the source of truth",
  );
  assert.ok(
    !releaseText.includes('"${{ github.event_name }}"'),
    "release workflow should not interpolate github event values directly inside bash argv",
  );

  for (const packageName of platformPackages) {
    assertContains(releaseText, packageName, `release platform publish matrix for ${packageName}`);
  }
}

function validateReadmes(readmeKoText, readmeEnText) {
  assertContains(readmeKoText, "npx @jeremyfellaz/maximus audit", "Korean README scoped npx example");
  assertContains(readmeKoText, "## GitHub Action", "Korean README action section");
  assertContains(readmeKoText, "uses: JeremyDev87/maximus@<release-tag>", "Korean README action example");
  assertContains(readmeKoText, "예: `v1.0.0`", "Korean README release tag guidance");
  assertContains(readmeKoText, "`v1`도 사용할 수 있습니다", "Korean README major tag guidance");
  assertContains(readmeKoText, "release operator runbook", "Korean README runbook link");
  assertContains(readmeKoText, "draft notes", "Korean README draft notes wording");
  assertContains(readmeEnText, "npx @jeremyfellaz/maximus audit", "English README scoped npx example");
  assertContains(readmeEnText, "## GitHub Action", "English README action section");
  assertContains(readmeEnText, "uses: JeremyDev87/maximus@<release-tag>", "English README action example");
  assertContains(readmeEnText, "for example `v1.0.0`", "English README release tag guidance");
  assertContains(readmeEnText, "`v1` is also valid", "English README major tag guidance");
  assertContains(readmeEnText, "release operator runbook", "English README runbook link");
  assertContains(readmeEnText, "draft notes", "English README draft notes wording");
}

function validateMarketplaceGuide(guideText) {
  assertContains(guideText, "JeremyDev87/maximus@v1", "marketplace guide root major tag usage");
  assertContains(guideText, "JeremyDev87/maximus/.github/actions/marketplace-wrapper@v1", "marketplace guide subpath usage");
  assertContains(guideText, "`v1.0.0`", "marketplace guide immutable tag guidance");
  assertContains(guideText, "`release.yml`은 `v1.0.0` 같은 package release tag만 받습니다", "marketplace guide v1 non-publish contract");
  assertContains(guideText, "`action-smoke.yml`을 `--ref v1`", "marketplace guide v1 smoke guidance");
  assertContains(guideText, "`registry-url`", "marketplace guide registry input");
  assertContains(guideText, "root `action.yml`", "marketplace guide root action source-of-truth note");
}

function validateNpmWrapperRuntime(runtimeText) {
  assertContains(runtimeText, "v1.0.0 native runtime 지원 플랫폼", "npm runtime v1 support heading");
  assertContains(runtimeText, "Windows", "npm runtime Windows unsupported policy");
  assertContains(runtimeText, "Linux musl", "npm runtime musl unsupported policy");
  assertContains(runtimeText, "limited compatibility fallback", "npm runtime fallback wording");
  assertContains(runtimeText, "fix --dry-run", "npm runtime fix dry-run compatibility");
  assertContains(runtimeText, "별도 hard cutover 작업", "npm runtime hard cutover split");

  for (const packageName of ["darwin-arm64", "darwin-x64", "linux-arm64-gnu", "linux-x64-gnu"]) {
    assertContains(runtimeText, packageName, `npm runtime support policy for ${packageName}`);
  }
}

function validateContributing(contributingText) {
  assertContains(contributingText, "docs/release-operator-runbook.md", "CONTRIBUTING runbook link");
  assertContains(contributingText, "Release Drafter as draft-notes automation", "CONTRIBUTING release-drafter contract");
  assertContains(contributingText, "node ./scripts/validate-rust-release-wiring.mjs", "CONTRIBUTING release validation command");
}

function validateReleaseDrafterWorkflow(releaseDrafterWorkflowText) {
  assertContains(releaseDrafterWorkflowText, "push:", "release drafter push trigger");
  assertContains(releaseDrafterWorkflowText, "workflow_dispatch:", "release drafter manual trigger");
  assertContains(releaseDrafterWorkflowText, "if: github.ref == 'refs/heads/master'", "release drafter master-only guard");
  assertContains(releaseDrafterWorkflowText, "config-name: release-drafter.yml", "release drafter config wiring");
  assertContains(releaseDrafterWorkflowText, "release-drafter/release-drafter@", "release drafter action usage");
  assertContains(releaseDrafterWorkflowText, "only maintains draft notes on master", "release drafter notes-only comment");
}

function validateReleaseDrafterConfig(releaseDrafterConfigText) {
  assertContains(releaseDrafterConfigText, "Actual npm publication and smoke verification are tag-driven", "release drafter config notes-only comment");
  assertContains(releaseDrafterConfigText, 'name-template: "v$NEXT_PATCH_VERSION"', "release drafter name template");
  assertContains(releaseDrafterConfigText, 'tag-template: "v$NEXT_PATCH_VERSION"', "release drafter tag template");
  assertContains(releaseDrafterConfigText, "## Changes", "release drafter notes template");
}

function validateReleaseRunbook(releaseRunbookText) {
  assertContains(releaseRunbookText, "## Preflight Before Creating A New Tag", "runbook new-tag preflight section");
  assertContains(releaseRunbookText, "## Preflight Before A Same-Tag Rerun", "runbook rerun preflight section");
  assertContains(releaseRunbookText, 'git switch --detach "$RELEASE_TAG"', "runbook detached tag rerun command");
  assertContains(releaseRunbookText, 'gh workflow run release.yml --ref v1.0.0 -f release_tag=v1.0.0', "runbook rerun workflow command");
  assertContains(releaseRunbookText, 'env npm_config_cache="$NPM_CONFIG_CACHE" npm view "@jeremyfellaz/maximus@$RELEASE_VERSION" version', "runbook exact root wrapper version check");
  assertContains(releaseRunbookText, 'env npm_config_cache="$NPM_CONFIG_CACHE" npm view "${package}@${RELEASE_VERSION}" version', "runbook exact platform package version check");
  assertContains(releaseRunbookText, 'env npm_config_cache="$PACK_ROOT/.npm-cache" npm pack --json --pack-destination "$PACK_ROOT"', "runbook temp-cache npm pack command");
  assertContains(releaseRunbookText, "git tag -f v1 \"$RELEASE_TAG\"", "runbook v1 moving tag command");
  assertContains(releaseRunbookText, "gh workflow run action-smoke.yml --ref v1 -f release_tag=v1", "runbook v1 action smoke command");
  assertContains(releaseRunbookText, "Moving `v1` does not trigger the tag-driven release workflow.", "runbook v1 non-publish contract");
  assertContains(releaseRunbookText, "Windows and Linux musl do not have prebuilt native runtime packages in v1.0.0", "runbook unsupported platform contract");
  assertContains(releaseRunbookText, "Do not validate a same-tag rerun from a newer `master` checkout.", "runbook rerun master warning");

  for (const packageName of platformPackages) {
    assertContains(releaseRunbookText, packageName, `runbook platform package coverage for ${packageName}`);
  }
}

function validateNativeRuntimeAssertion(nativeRuntimeAssertionText) {
  assertContains(nativeRuntimeAssertionText, "MAXIMUS_RUST_BINARY_PLACEHOLDER", "native runtime placeholder marker check");
  assertContains(nativeRuntimeAssertionText, "EXECUTABLE_PROBE_TIMEOUT_MS = 5000", "native runtime probe timeout");
  assertContains(nativeRuntimeAssertionText, "node_modules", "native runtime node_modules lookup");
  assertContains(nativeRuntimeAssertionText, "Verified native runtime", "native runtime success output");
}

function validateReleaseContextAssertion(releaseContextAssertionText) {
  assertContains(releaseContextAssertionText, "release workflow must run from a tag ref", "release context tag gate");
  assertContains(releaseContextAssertionText, "package.json version", "release context package version gate");
  assertContains(releaseContextAssertionText, "GITHUB_OUTPUT", "release context GitHub output export");
  assertContains(releaseContextAssertionText, "eventName === \"push\" || eventName === \"workflow_dispatch\"", "release context supported events");
}

function validateReleasePlanScript(releasePlanText) {
  assertContains(releasePlanText, "distTag", "release plan dist-tag logic");
  assertContains(releasePlanText, "isPrerelease", "release plan prerelease flag");
  assertContains(releasePlanText, 'resolveReleasePlan', "release plan helper wiring");
}

function validateReleaseCandidateMetadata(releaseCandidateMetadataText) {
  assertContains(releaseCandidateMetadataText, "assertReleaseUpgrade", "release candidate upgrade assertion");
  assertContains(releaseCandidateMetadataText, "github.event.before must contain package.json for release upgrade validation", "release candidate push fail-closed guard");
  assertContains(releaseCandidateMetadataText, "merge-base", "release candidate master ancestry gate");
  assertContains(releaseCandidateMetadataText, "refs/remotes/origin/master", "release candidate master ref gate");
  assertContains(releaseCandidateMetadataText, "`${verifiedSha}^`", "release candidate parent version check");
  assertContains(releaseCandidateMetadataText, "target_sha must already be reachable from origin/master before tagging", "release candidate unmerged target guard");
}

function validateReleaseHelpers(releaseHelpersText) {
  assertContains(releaseHelpersText, 'npmDistTag: isPrerelease ? "next" : "latest"', "release helper dist-tag mapping");
  assertContains(releaseHelpersText, "compareReleaseVersions", "release helper version comparison");
  assertContains(releaseHelpersText, "assertReleaseUpgrade", "release helper upgrade assertion");
  assertContains(releaseHelpersText, "PRERELEASE_IDENTIFIER_PATTERN", "release helper strict prerelease identifiers");
}

function validateReleaseBumpScript(releaseBumpScriptText) {
  assertContains(releaseBumpScriptText, "packageManifestPaths", "release bump manifest path list");
  assertContains(releaseBumpScriptText, "versionFilePaths", "release bump full version file path list");
  assertContains(releaseBumpScriptText, "rustCrateManifestPaths", "release bump Rust crate manifest path list");
  assertContains(releaseBumpScriptText, "cargoLockPath", "release bump Cargo.lock path constant");
  assertContains(releaseBumpScriptText, "updateCargoTomlPackageVersion", "release bump Cargo.toml updater");
  assertContains(releaseBumpScriptText, "updateCargoLockWorkspacePackageVersions", "release bump Cargo.lock updater");
  assertContains(releaseBumpScriptText, "createManualBumpBranchName", "release bump branch naming");
  assertContains(releaseBumpScriptText, "assertReleaseUpgrade", "release bump upgrade guard");
}

function validateNpmLookupClassifier(lookupClassifierText) {
  assertContains(lookupClassifierText, "not-found", "npm lookup not-found classification");
  assertContains(lookupClassifierText, "registry-or-auth-failure", "npm lookup registry failure classification");
}

function validateNpmPublishClassifier(publishClassifierText) {
  assertContains(publishClassifierText, "already-published", "npm publish already-published classification");
  assertContains(publishClassifierText, "publish-failure", "npm publish failure classification");
}

function assertContains(text, expected, label) {
  assert.match(text, new RegExp(escapeRegExp(expected), "m"), `${label} is missing`);
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

const isDirectExecution =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isDirectExecution) {
  try {
    const summary = await validateRustReleaseWiring();
    console.log(`Validated Rust release wiring in ${summary.checkedFiles.length} files.`);
    console.log(`Platform packages: ${summary.platformPackages.join(", ")}`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Rust release wiring validation failed: ${message}`);
    process.exitCode = 1;
  }
}
