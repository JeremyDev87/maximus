import assert from "node:assert/strict";
import path from "node:path";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const platformPackages = [
  "maximus-darwin-arm64",
  "maximus-darwin-x64",
  "maximus-linux-arm64-gnu",
  "maximus-linux-x64-gnu",
];

const requiredFiles = {
  action: "action.yml",
  packageManifest: "package.json",
  devWorkflow: ".github/workflows/dev.yml",
  actionSmokeWorkflow: ".github/workflows/action-smoke.yml",
  releaseWorkflow: ".github/workflows/release.yml",
  rustReleaseWorkflow: ".github/workflows/rust-release-binaries.yml",
  readmeKo: "README.md",
  readmeEn: "README.en.md",
  releaseContextAssertion: "scripts/assert-release-workflow-context.mjs",
  nativeRuntimeAssertion: "scripts/assert-installed-native-runtime.mjs",
};

export async function validateRustReleaseWiring(repoRoot = process.cwd()) {
  const fileContents = await readRequiredFiles(repoRoot);
  const packageManifest = JSON.parse(fileContents.packageManifest);
  validateAction(fileContents.action);
  validateDevWorkflow(fileContents.devWorkflow);
  validateActionSmokeWorkflow(fileContents.actionSmokeWorkflow, packageManifest.version);
  validateRustReleaseWorkflow(fileContents.rustReleaseWorkflow);
  validateReleaseWorkflow(fileContents.releaseWorkflow);
  validateReadmes(fileContents.readmeKo, fileContents.readmeEn);
  validateReleaseContextAssertion(fileContents.releaseContextAssertion);
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
  assertContains(actionText, 'npm install --no-package-lock --prefix "$install_root" "$GITHUB_ACTION_PATH"', "action local package install");
  assertContains(actionText, 'node "$GITHUB_ACTION_PATH/scripts/assert-installed-native-runtime.mjs" "$install_root"', "action native runtime assertion");
  assertContains(actionText, "MAXIMUS_COMMAND: ${{ inputs.command }}", "action command env wiring");
  assertContains(actionText, "MAXIMUS_TARGET_PATH: ${{ inputs.path }}", "action path env wiring");
  assertContains(actionText, '"$MAXIMUS_COMMAND" "$MAXIMUS_TARGET_PATH"', "action wrapper env argv usage");
  assertContains(actionText, 'node "$install_root/node_modules/maximus/bin/maximus.js"', "action wrapper invocation");
  assertContains(actionText, "registry-url", "action registry override input");
  assert.ok(
    !actionText.includes('if [[ -n "${{ inputs.registry-url }}" ]]; then'),
    "action should not interpolate registry input directly inside bash",
  );
  assert.ok(
    !actionText.includes('"${{ inputs.command }}" "${{ inputs.path }}"'),
    "action should not interpolate command or path inputs directly inside bash",
  );
}

function validateDevWorkflow(devText) {
  const requiredPaths = [
    "action.yml",
    ".github/workflows/action-smoke.yml",
    ".github/workflows/release.yml",
    ".github/workflows/rust-release-binaries.yml",
  ];

  for (const requiredPath of requiredPaths) {
    assertContains(devText, `"${requiredPath}"`, `dev path filter for ${requiredPath}`);
  }

  assertContains(devText, '"scripts/**"', "dev path filter for scripts");
  assertContains(devText, '"test/**"', "dev path filter for tests");

  assertContains(devText, "release-wiring:", "release wiring job");
  assertContains(devText, "node ./scripts/validate-rust-release-wiring.mjs", "release wiring validation command");
  assertContains(devText, "node --test test/github-action-wiring.test.js", "release wiring node test command");
}

function validateActionSmokeWorkflow(actionSmokeText, packageVersion) {
  assertContains(actionSmokeText, "workflow_call:", "action smoke reusable trigger");
  assertContains(actionSmokeText, "workflow_dispatch:", "action smoke manual trigger");
  assertContains(
    actionSmokeText,
    `uses: JeremyDev87/maximus@v${packageVersion}`,
    "action smoke published action usage",
  );
  assertContains(actionSmokeText, "registry-url: ${{ inputs.registry_url }}", "action smoke registry passthrough");
  assert.ok(!actionSmokeText.includes("uses: ./"), "action smoke should not use the local checkout action");
}

function validateRustReleaseWorkflow(rustReleaseText) {
  assertContains(rustReleaseText, "workflow_call:", "reusable rust release trigger");
  assertContains(rustReleaseText, "workflow_dispatch:", "manual rust release trigger");
  assertContains(rustReleaseText, "cargo build --release -p maximus-cli", "rust release build step");
  assertContains(rustReleaseText, "actions/upload-artifact@v4", "rust release artifact upload");

  for (const packageName of platformPackages) {
    assertContains(rustReleaseText, packageName, `rust release matrix entry for ${packageName}`);
  }
}

function validateReleaseWorkflow(releaseText) {
  assertContains(releaseText, "release:\n    types: [published]", "release published trigger");
  assertContains(releaseText, "workflow_dispatch:", "release manual trigger");
  assertContains(releaseText, "release_tag:", "release workflow dispatch tag input");
  assertContains(releaseText, "validate-release-context:", "release context validation job");
  assertContains(releaseText, "needs: validate-release-context", "release context ordering");
  assertContains(releaseText, "Resolve release tag from release event", "release event tag resolution step");
  assertContains(releaseText, "Resolve release tag from workflow input", "release dispatch tag resolution step");
  assertContains(releaseText, "RELEASE_EVENT_TAG: ${{ github.event.release.tag_name }}", "release event tag env wiring");
  assertContains(releaseText, "DISPATCH_RELEASE_TAG: ${{ inputs.release_tag }}", "release dispatch tag env wiring");
  assertContains(releaseText, "RELEASE_EVENT_NAME: ${{ github.event_name }}", "release event name env wiring");
  assertContains(releaseText, "RELEASE_GITHUB_REF: ${{ github.ref }}", "release ref env wiring");
  assertContains(releaseText, "RELEASE_GITHUB_REF_NAME: ${{ github.ref_name }}", "release ref_name env wiring");
  assertContains(releaseText, 'printf \'value=%s\\n\' "$RELEASE_EVENT_TAG" >> "$GITHUB_OUTPUT"', "release event tag output");
  assertContains(releaseText, 'printf \'value=%s\\n\' "$DISPATCH_RELEASE_TAG" >> "$GITHUB_OUTPUT"', "release dispatch tag output");
  assertContains(releaseText, "uses: ./.github/workflows/rust-release-binaries.yml", "release reusable binary workflow call");
  assertContains(releaseText, "node ./scripts/assert-release-workflow-context.mjs", "release context assertion command");
  assertContains(releaseText, "npm publish . --access public", "root wrapper publish");
  assertContains(releaseText, 'npm install --no-package-lock --prefix "$install_root" "maximus@${{ needs.validate-release-context.outputs.package_version }}"', "published wrapper smoke install");
  assertContains(releaseText, 'node ./scripts/assert-installed-native-runtime.mjs "$install_root"', "published wrapper native runtime assertion");
  assertContains(releaseText, "uses: ./.github/workflows/action-smoke.yml", "release action smoke call");
  assertContains(releaseText, "needs: publish-platform-packages", "wrapper publish ordering");
  assertContains(releaseText, "- publish-wrapper", "published wrapper smoke ordering");
  assertContains(releaseText, "strategy:\n      fail-fast: false\n      matrix:", "published wrapper smoke matrix");
  assert.ok(
    !releaseText.includes('echo "value=${{ github.event.release.tag_name }}" >> "$GITHUB_OUTPUT"'),
    "release workflow should not interpolate release tag directly inside bash",
  );
  assert.ok(
    !releaseText.includes('echo "value=${{ inputs.release_tag }}" >> "$GITHUB_OUTPUT"'),
    "release workflow should not interpolate dispatch input directly inside bash",
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
  assertContains(readmeKoText, "## GitHub Action", "Korean README action section");
  assertContains(readmeKoText, "uses: JeremyDev87/maximus@<release-tag>", "Korean README action example");
  assertContains(readmeKoText, "예: `v0.1.0`", "Korean README release tag guidance");
  assertContains(readmeEnText, "## GitHub Action", "English README action section");
  assertContains(readmeEnText, "uses: JeremyDev87/maximus@<release-tag>", "English README action example");
  assertContains(readmeEnText, "for example `v0.1.0`", "English README release tag guidance");
}

function validateNativeRuntimeAssertion(nativeRuntimeAssertionText) {
  assertContains(nativeRuntimeAssertionText, "MAXIMUS_RUST_BINARY_PLACEHOLDER", "native runtime placeholder marker check");
  assertContains(nativeRuntimeAssertionText, "node_modules", "native runtime node_modules lookup");
  assertContains(nativeRuntimeAssertionText, "Verified native runtime", "native runtime success output");
}

function validateReleaseContextAssertion(releaseContextAssertionText) {
  assertContains(releaseContextAssertionText, "release workflow must run from a tag ref", "release context tag gate");
  assertContains(releaseContextAssertionText, "package.json version", "release context package version gate");
  assertContains(releaseContextAssertionText, "GITHUB_OUTPUT", "release context GitHub output export");
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
