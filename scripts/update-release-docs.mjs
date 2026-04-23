import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageJsonPath = path.join(repoRoot, "package.json");
const readmePaths = [
  path.join(repoRoot, "README.md"),
  path.join(repoRoot, "README.en.md"),
];
const markerStart = "<!-- release-docs:start -->";
const markerEnd = "<!-- release-docs:end -->";

const isCheckMode = process.argv.includes("--check");

async function main() {
  const packageManifest = JSON.parse(await readFile(packageJsonPath, "utf8"));
  const releaseTag = `v${packageManifest.version}`;

  const readmes = new Map(
    await Promise.all(
      readmePaths.map(async (readmePath) => [readmePath, await readFile(readmePath, "utf8")]),
    ),
  );

  const nextReadmes = new Map();
  for (const [readmePath, currentText] of readmes) {
    nextReadmes.set(readmePath, updateReleaseDocs(readmePath, currentText, releaseTag));
  }

  let changed = false;
  for (const [readmePath, nextText] of nextReadmes) {
    const currentText = readmes.get(readmePath);
    if (currentText !== nextText) {
      changed = true;
      if (!isCheckMode) {
        await writeFile(readmePath, nextText, "utf8");
      }
    }
  }

  if (isCheckMode) {
    if (changed) {
      console.error("Release docs are out of date. Run `node ./scripts/update-release-docs.mjs`.");
      process.exitCode = 1;
      return;
    }

    console.log(`Release docs are up to date for ${releaseTag}.`);
    return;
  }

  if (changed) {
    console.log(`Updated release docs for ${releaseTag}.`);
  } else {
    console.log(`Release docs are already up to date for ${releaseTag}.`);
  }
}

function updateReleaseDocs(readmePath, text, releaseTag) {
  const sections = {
    "README.md": [
      `${markerStart}`,
      "릴리즈 태그 이후에는 같은 npm wrapper 진입점을 GitHub Action에서도 그대로 사용합니다.",
      "",
      "```yaml",
      "- uses: JeremyDev87/maximus@<release-tag>",
      "  with:",
      "    command: audit",
      "    path: .",
      "```",
      "",
      "기본 입력:",
      "",
      "- `command`: `audit`, `doctor`, `fix`",
      "- `path`: 검사할 프로젝트 경로, 기본값 `.`",
      "- `registry-url`: pre-release smoke나 사설 registry 검증이 필요할 때만 쓰는 optional npm registry override",
      `- \`release-tag\`: publish된 릴리즈 태그를 넣으세요. 예: \`${releaseTag}\``,
      "",
      "유지보수자가 실제 alpha/stable 릴리즈를 준비하거나 같은 태그를 안전하게 재실행할 때는 [release operator runbook](https://github.com/JeremyDev87/maximus/blob/master/docs/release-operator-runbook.md)을 기준으로 진행합니다. Release Drafter는 `master`에서 draft notes만 갱신하며, 실제 publish는 tag-driven release workflow만 담당합니다.",
      `${markerEnd}`,
    ].join("\n"),
    "README.en.md": [
      `${markerStart}`,
      "After release tags are published, GitHub Actions use the same npm-wrapper entrypoint as well.",
      "",
      "```yaml",
      "- uses: JeremyDev87/maximus@<release-tag>",
      "  with:",
      "    command: audit",
      "    path: .",
      "```",
      "",
      "Default inputs:",
      "",
      "- `command`: `audit`, `doctor`, `fix`",
      "- `path`: project path to inspect, default `.`",
      "- `registry-url`: optional npm registry override for pre-release smoke or private registry validation",
      `- \`release-tag\`: replace this with a published release tag, for example \`${releaseTag}\``,
      "",
      "Maintainers should use the [release operator runbook](https://github.com/JeremyDev87/maximus/blob/master/docs/release-operator-runbook.md) for alpha or stable releases and same-tag reruns. Release Drafter only refreshes draft notes on `master`; actual publication stays in the tag-driven release workflow.",
      `${markerEnd}`,
    ].join("\n"),
  };

  const nextSection = sections[path.basename(readmePath)];
  if (!nextSection) {
    throw new Error(`Unsupported README path: ${readmePath}`);
  }

  const startIndex = text.indexOf(markerStart);
  const endIndex = text.indexOf(markerEnd);

  if (startIndex === -1 || endIndex === -1 || endIndex < startIndex) {
    throw new Error(`Missing release docs markers in ${path.basename(readmePath)}`);
  }

  const before = text.slice(0, startIndex).trimEnd();
  const after = text.slice(endIndex + markerEnd.length).trimStart();

  return `${before}\n\n${nextSection}${after ? `\n\n${after}` : ""}`.replace(/\n{3,}/g, "\n\n");
}

await main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Release docs update failed: ${message}`);
  process.exitCode = 1;
});
