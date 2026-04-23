import { readFile, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

import { auditProject } from "../src/core/audit-project.js";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const fixtureRoot = path.join(repoRoot, "test/fixtures/benchmark-large-repo");
const defaultPackageCount = 120;

const packageCount = parsePositiveInteger(
  process.env.BENCHMARK_LARGE_REPO_PACKAGES,
  defaultPackageCount,
);

const tempRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-large-repo-"));

try {
  await stageSeedFixture(tempRoot);
  await synthesizeLargeRepo(tempRoot, packageCount);

  const startedAt = performance.now();
  const result = await auditProject(tempRoot);
  const durationMs = performance.now() - startedAt;

  console.log(`Total duration: ${durationMs.toFixed(2)} ms`);
  console.log(`Discovered files: ${result.project.files.length}`);
  console.log(`Findings: ${result.findings.length}`);
} finally {
  await rm(tempRoot, { recursive: true, force: true });
}

async function stageSeedFixture(targetRoot) {
  const sourcePackagePath = path.join(fixtureRoot, "package.json");
  const sourcePackage = await readFile(sourcePackagePath, "utf8");

  await writeFile(path.join(targetRoot, "package.json"), sourcePackage, "utf8");
}

async function synthesizeLargeRepo(targetRoot, count) {
  const packagesRoot = path.join(targetRoot, "packages");
  await mkdir(packagesRoot, { recursive: true });

  for (let index = 0; index < count; index += 1) {
    const packageName = `pkg-${String(index + 1).padStart(4, "0")}`;
    const packageDir = path.join(packagesRoot, packageName);

    await mkdir(packageDir, { recursive: true });
    await Promise.all([
      writeFile(path.join(packageDir, "package.json"), buildPackageJson(packageName), "utf8"),
      writeFile(path.join(packageDir, ".eslintrc.json"), buildEslintConfig(index), "utf8"),
      writeFile(path.join(packageDir, ".prettierrc.toml"), 'semi = false\n', "utf8"),
      writeFile(path.join(packageDir, "jest.config.js"), buildJestConfig(packageName), "utf8"),
      writeFile(path.join(packageDir, "tsconfig.json"), buildTsconfig(packageName), "utf8"),
    ]);
  }
}

function buildPackageJson(packageName) {
  return JSON.stringify(
    {
      name: packageName,
      private: true,
      eslintConfig: {
        rules: {
          semi: ["error", "always"],
          quotes: ["error", "single"],
        },
      },
      prettier: {
        semi: false,
        singleQuote: true,
      },
      jest: {
        testEnvironment: "node",
      },
    },
    null,
    2,
  ) + "\n";
}

function buildEslintConfig(index) {
  return JSON.stringify(
    {
      root: true,
      extends: ["eslint:recommended"],
      rules: {
        semi: index % 2 === 0 ? "error" : ["error", "always"],
        quotes: ["error", "single"],
      },
    },
    null,
    2,
  ) + "\n";
}

function buildJestConfig(packageName) {
  return [
    "module.exports = {",
    `  displayName: ${JSON.stringify(packageName)},`,
    "  testEnvironment: 'node',",
    "};",
    "",
  ].join("\n");
}

function buildTsconfig(packageName) {
  return JSON.stringify(
    {
      extends: "../../tsconfig.base.json",
      compilerOptions: {
        baseUrl: ".",
        paths: {
          [`@${packageName}/*`]: ["src/*"],
        },
      },
    },
    null,
    2,
  ) + "\n";
}

function parsePositiveInteger(value, fallback) {
  if (value == null || value === "") {
    return fallback;
  }

  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`BENCHMARK_LARGE_REPO_PACKAGES must be a positive integer, got ${value}`);
  }

  return parsed;
}
