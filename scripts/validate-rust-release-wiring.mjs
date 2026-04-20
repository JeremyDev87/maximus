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
  devWorkflow: ".github/workflows/dev.yml",
  actionSmokeWorkflow: ".github/workflows/action-smoke.yml",
  releaseWorkflow: ".github/workflows/release.yml",
  rustReleaseWorkflow: ".github/workflows/rust-release-binaries.yml",
  readmeKo: "README.md",
  readmeEn: "README.en.md",
};

export async function validateRustReleaseWiring(repoRoot = process.cwd()) {
  const fileContents = await readRequiredFiles(repoRoot);
  validateAction(fileContents.action);
  validateDevWorkflow(fileContents.devWorkflow);
  validateActionSmokeWorkflow(fileContents.actionSmokeWorkflow);
  validateRustReleaseWorkflow(fileContents.rustReleaseWorkflow);
  validateReleaseWorkflow(fileContents.releaseWorkflow);
  validateReadmes(fileContents.readmeKo, fileContents.readmeEn);

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
  assertContains(actionText, 'npm install --no-package-lock --prefix "$install_root" "$GITHUB_ACTION_PATH"', "action local package install");
  assertContains(actionText, 'node "$install_root/node_modules/maximus/bin/maximus.js"', "action wrapper invocation");
  assertContains(actionText, "registry-url", "action registry override input");
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

function validateActionSmokeWorkflow(actionSmokeText) {
  assertContains(actionSmokeText, "workflow_call:", "action smoke reusable trigger");
  assertContains(actionSmokeText, "workflow_dispatch:", "action smoke manual trigger");
  assertContains(actionSmokeText, "uses: ./", "action smoke local action usage");
  assertContains(actionSmokeText, "registry-url: ${{ inputs.registry_url }}", "action smoke registry passthrough");
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
  assertContains(releaseText, "uses: ./.github/workflows/rust-release-binaries.yml", "release reusable binary workflow call");
  assertContains(releaseText, "npm publish . --access public", "root wrapper publish");
  assertContains(releaseText, 'npm install --no-package-lock --prefix "$install_root" "maximus@${{ steps.package_version.outputs.value }}"', "published wrapper smoke install");
  assertContains(releaseText, "uses: ./.github/workflows/action-smoke.yml", "release action smoke call");
  assertContains(releaseText, "needs: publish-platform-packages", "wrapper publish ordering");
  assertContains(releaseText, "needs: publish-wrapper", "published wrapper smoke ordering");

  for (const packageName of platformPackages) {
    assertContains(releaseText, packageName, `release platform publish matrix for ${packageName}`);
  }
}

function validateReadmes(readmeKoText, readmeEnText) {
  assertContains(readmeKoText, "## GitHub Action", "Korean README action section");
  assertContains(readmeKoText, "uses: JeremyDev87/maximus@v0", "Korean README action example");
  assertContains(readmeEnText, "## GitHub Action", "English README action section");
  assertContains(readmeEnText, "uses: JeremyDev87/maximus@v0", "English README action example");
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
