import { readTextIfExists } from "../lib/fs.js";
import { parseJsonc } from "../lib/jsonc.js";
import { makeFinding } from "../core/findings.js";
import { getDirectories } from "../core/discover.js";

const CONFIG_FAMILIES = [
  { label: "ESLint", fileKind: "eslint", packageField: "eslintConfig" },
  { label: "Prettier", fileKind: "prettier", packageField: "prettier" },
  { label: "Jest", fileKind: "jest", packageField: "jest" },
];

export async function runConfigDuplicateCheck(project) {
  const findings = [];

  for (const directory of getDirectories(project)) {
    const packageFile = directory.files.find((file) => file.kind === "package");
    const packageJson = packageFile ? await readPackageJson(packageFile.path) : null;

    for (const family of CONFIG_FAMILIES) {
      const familyFiles = directory.filesByKind.get(family.fileKind) ?? [];
      const hasPackageField = packageJson && Object.hasOwn(packageJson, family.packageField);
      const totalSources = familyFiles.length + (hasPackageField ? 1 : 0);

      if (totalSources <= 1) {
        continue;
      }

      findings.push(
        makeFinding({
          id: `duplicate-config:${family.label}:${directory.dir}`,
          category: "duplicates",
          severity: family.label === "ESLint" ? "error" : "warn",
          title: `${family.label} config is declared in multiple places`,
          file: packageFile?.path ?? familyFiles[0]?.path ?? null,
          detail: `Found ${totalSources} ${family.label} config sources in ${directory.relativeDir}.`,
          hint: `Keep a single ${family.label} entry point per directory to avoid drift.`,
        }),
      );
    }

    const eslintFiles = directory.filesByKind.get("eslint") ?? [];
    const hasLegacyEslint = eslintFiles.some((file) => file.name.startsWith(".eslintrc"));
    const hasFlatEslint = eslintFiles.some((file) => file.name.startsWith("eslint.config."));

    if (hasLegacyEslint && hasFlatEslint) {
      findings.push(
        makeFinding({
          id: `eslint-mixed-modes:${directory.dir}`,
          category: "duplicates",
          severity: "error",
          title: "Legacy and flat ESLint configs coexist",
          file: eslintFiles[0]?.path ?? null,
          detail:
            "This directory contains both legacy .eslintrc.* files and flat eslint.config.* files, so ESLint can resolve different rule sets depending on the entry point.",
          hint:
            "Migrate to eslint.config.* as the single source of truth, then remove the legacy .eslintrc.* files after the new config fully replaces them.",
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
