import { getDirectories } from "../core/discover.js";
import { makeFinding } from "../core/findings.js";
import { readTextIfExists } from "../lib/fs.js";
import { parseJsonc } from "../lib/jsonc.js";

const FORMATTING_RULES = [
  "array-bracket-spacing",
  "comma-dangle",
  "indent",
  "max-len",
  "object-curly-spacing",
  "quotes",
  "semi",
];

export async function runEslintPrettierCheck(project) {
  const findings = [];

  for (const directory of getDirectories(project)) {
    const eslintFiles = directory.filesByKind.get("eslint") ?? [];
    const prettierFiles = directory.filesByKind.get("prettier") ?? [];
    const packageFile = directory.files.find((file) => file.kind === "package");
    const packageJson = packageFile ? await readPackageJson(packageFile.path) : null;

    const eslintSources = [];
    const prettierSources = [];

    for (const file of eslintFiles) {
      const text = await readTextIfExists(file.path);
      if (text != null) {
        eslintSources.push(text);
      }
    }

    for (const file of prettierFiles) {
      const text = await readTextIfExists(file.path);
      if (text != null) {
        prettierSources.push(text);
      }
    }

    if (packageJson?.eslintConfig) {
      eslintSources.push(JSON.stringify(packageJson.eslintConfig, null, 2));
    }

    if (packageJson?.prettier) {
      prettierSources.push(JSON.stringify(packageJson.prettier, null, 2));
    }

    if (eslintSources.length === 0 || prettierSources.length === 0) {
      continue;
    }

    const eslintText = eslintSources.join("\n");
    const hasFormattingRules = FORMATTING_RULES.some((rule) => {
      const escaped = escapeForRegExp(rule);
      return new RegExp(`["']${escaped}["']\\s*:`).test(eslintText);
    });
    const hasPrettierIntegration = /\bprettier\b/u.test(eslintText);

    if (hasFormattingRules && !hasPrettierIntegration) {
      findings.push(
        makeFinding({
          id: `eslint-prettier-conflict:${directory.dir}`,
          category: "conflict",
          severity: "warn",
          title: "ESLint formatting rules may conflict with Prettier",
          file: eslintFiles[0]?.path ?? packageFile?.path ?? null,
          detail: "Formatting-oriented ESLint rules were found, but no explicit Prettier bridge was detected.",
          hint: "Consider eslint-config-prettier or plugin:prettier/recommended to reduce formatter churn.",
        }),
      );
    } else if (!hasPrettierIntegration) {
      findings.push(
        makeFinding({
          id: `eslint-prettier-separate:${directory.dir}`,
          category: "conflict",
          severity: "info",
          title: "ESLint and Prettier are configured separately",
          file: eslintFiles[0]?.path ?? packageFile?.path ?? null,
          detail: "That can be fine, but teams often prefer an explicit integration strategy.",
          hint: "Document which tool owns formatting and which tool owns code-quality rules.",
        }),
      );
    }
  }

  return { findings, fixes: [] };
}

async function readPackageJson(filePath) {
  try {
    const text = await readTextIfExists(filePath);
    if (!text) {
      return null;
    }

    return parseJsonc(text, filePath);
  } catch {
    return null;
  }
}

function escapeForRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
