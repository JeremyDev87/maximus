import path from "node:path";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const ALREADY_PUBLISHED_PATTERNS = [
  /\bEPUBLISHCONFLICT\b/,
  /cannot publish over the previously published versions/i,
  /cannot publish over existing version/i,
  /previously published version/i,
];

export function classifyNpmPublishError(stderrText) {
  for (const pattern of ALREADY_PUBLISHED_PATTERNS) {
    if (pattern.test(stderrText)) {
      return "already-published";
    }
  }

  return "publish-failure";
}

const isDirectExecution =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isDirectExecution) {
  const [logPath] = process.argv.slice(2);

  if (!logPath) {
    console.error("Usage: node ./scripts/classify-npm-publish-error.mjs <log-path>");
    process.exitCode = 1;
  } else {
    const stderrText = await readFile(logPath, "utf8");
    console.log(classifyNpmPublishError(stderrText));
  }
}
