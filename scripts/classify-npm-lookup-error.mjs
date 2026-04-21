import path from "node:path";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const NOT_FOUND_PATTERNS = [
  /\bE404\b/,
  /\b404 Not Found\b/i,
  /\bnpm ERR! 404\b/i,
];

export function classifyNpmLookupError(stderrText) {
  for (const pattern of NOT_FOUND_PATTERNS) {
    if (pattern.test(stderrText)) {
      return "not-found";
    }
  }

  return "registry-or-auth-failure";
}

const isDirectExecution =
  process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isDirectExecution) {
  const [logPath] = process.argv.slice(2);

  if (!logPath) {
    console.error("Usage: node ./scripts/classify-npm-lookup-error.mjs <log-path>");
    process.exitCode = 1;
  } else {
    const stderrText = await readFile(logPath, "utf8");
    console.log(classifyNpmLookupError(stderrText));
  }
}
