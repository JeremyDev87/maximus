import path from "node:path";
import { readdir } from "node:fs/promises";

import { findNearestPackageFile, getFiles } from "../core/discover.js";
import { makeFinding } from "../core/findings.js";
import { readTextIfExists, pathExists } from "../lib/fs.js";
import { parseJsonc } from "../lib/jsonc.js";

const DEPRECATED_COMPILER_OPTIONS = {
  charset: "Remove it. TypeScript ignores this option in modern versions.",
  importsNotUsedAsValues: "Prefer verbatimModuleSyntax in modern TypeScript.",
  keyofStringsOnly: "Remove it. Modern TypeScript no longer needs this compatibility flag.",
  noStrictGenericChecks: "Remove it and rely on strict mode checks instead.",
  out: "Use outFile if you truly need single-file emit.",
  preserveValueImports: "Prefer verbatimModuleSyntax in modern TypeScript.",
  suppressExcessPropertyErrors: "Remove it. This hides useful structural typing errors.",
  suppressImplicitAnyIndexErrors: "Remove it. This suppresses important type safety signals.",
};

const CHECKABLE_EXTENSIONS = [".cjs", ".cts", ".js", ".json", ".jsx", ".mjs", ".mts", ".ts", ".tsx"];

export async function runTsconfigCheck(project) {
  const findings = [];

  for (const file of getFiles(project, "tsconfig")) {
    const text = await readTextIfExists(file.path);
    if (text == null) {
      continue;
    }

    let config;
    try {
      config = parseJsonc(text, file.path);
    } catch (error) {
      findings.push(
        makeFinding({
          id: `tsconfig-parse:${file.path}`,
          category: "tsconfig",
          severity: "error",
          title: "Config file could not be parsed",
          file: file.path,
          detail: error instanceof Error ? error.message : String(error),
          hint: "Fix invalid JSONC syntax before relying on this config.",
        }),
      );
      continue;
    }

    const compilerOptions = config.compilerOptions ?? {};

    for (const [option, guidance] of Object.entries(DEPRECATED_COMPILER_OPTIONS)) {
      if (!Object.hasOwn(compilerOptions, option)) {
        continue;
      }

      findings.push(
        makeFinding({
          id: `tsconfig-deprecated:${file.path}:${option}`,
          category: "tsconfig",
          severity: "warn",
          title: `Deprecated compiler option "${option}"`,
          file: file.path,
          detail: guidance,
          hint: "Remove legacy flags before they become upgrade blockers.",
        }),
      );
    }

    const pathsConfig = compilerOptions.paths;
    if (pathsConfig == null) {
      continue;
    }

    if (typeof pathsConfig !== "object" || Array.isArray(pathsConfig)) {
      findings.push(
        makeFinding({
          id: `tsconfig-paths-shape:${file.path}`,
          category: "tsconfig",
          severity: "error",
          title: "compilerOptions.paths must be an object",
          file: file.path,
          detail: "TypeScript expects alias keys mapped to arrays of target strings.",
          hint: "Rewrite paths to the standard { alias: [targets] } shape.",
        }),
      );
      continue;
    }

    const baseDir = compilerOptions.baseUrl
      ? path.resolve(path.dirname(file.path), compilerOptions.baseUrl)
      : path.dirname(file.path);

    for (const [alias, targets] of Object.entries(pathsConfig)) {
      if (!Array.isArray(targets) || targets.length === 0) {
        findings.push(
          makeFinding({
            id: `tsconfig-paths-empty:${file.path}:${alias}`,
            category: "tsconfig",
            severity: "error",
            title: `Alias "${alias}" does not declare any targets`,
            file: file.path,
            detail: "Each path alias should map to at least one target string.",
            hint: "Add a valid target or remove the alias entry.",
          }),
        );
        continue;
      }

      const aliasHasWildcard = alias.includes("*");
      for (const target of targets) {
        if (typeof target !== "string") {
          findings.push(
            makeFinding({
              id: `tsconfig-paths-type:${file.path}:${alias}`,
              category: "tsconfig",
              severity: "error",
              title: `Alias "${alias}" contains a non-string target`,
              file: file.path,
              detail: "TypeScript path targets must be strings.",
              hint: "Replace non-string entries with valid path strings.",
            }),
          );
          continue;
        }

        const targetHasWildcard = target.includes("*");
        if (aliasHasWildcard !== targetHasWildcard) {
          findings.push(
            makeFinding({
              id: `tsconfig-paths-wildcard:${file.path}:${alias}:${target}`,
              category: "tsconfig",
              severity: "warn",
              title: `Wildcard shape does not match for alias "${alias}"`,
              file: file.path,
              detail: `${alias} maps to ${target}, but only one side uses "*".`,
              hint: "Keep wildcard placement aligned so imports resolve predictably.",
            }),
          );
        }

        if (!(await aliasTargetExists(baseDir, target))) {
          findings.push(
            makeFinding({
              id: `tsconfig-paths-missing:${file.path}:${alias}:${target}`,
              category: "tsconfig",
              severity: "error",
              title: "Path alias target does not exist",
              file: file.path,
              detail: `${alias} points to ${target}, but the resolved path was not found.`,
              hint: "Update or remove stale aliases before they break editor and build resolution.",
            }),
          );
        }
      }
    }

    const nearestPackageFile = findNearestPackageFile(project, path.dirname(file.path));
    if (nearestPackageFile) {
      const packageText = await readTextIfExists(nearestPackageFile.path);
      if (packageText) {
        try {
          const packageJson = parseJsonc(packageText, nearestPackageFile.path);
          const imports = packageJson.imports ?? {};
          compareImportsAndPaths(
            file.path,
            path.dirname(nearestPackageFile.path),
            baseDir,
            imports,
            pathsConfig,
            findings,
          );
        } catch {
          // Ignore package parse failures here; they are not the focus of this check.
        }
      }
    }
  }

  return { findings, fixes: [] };
}

async function aliasTargetExists(baseDir, target) {
  if (/^[A-Za-z]+:/u.test(target) || target.startsWith("@")) {
    return true;
  }

  if (target.includes("*")) {
    return wildcardTargetExists(baseDir, target);
  }

  return staticTargetExists(baseDir, target);
}

async function staticTargetExists(baseDir, target) {
  const stem = target.split("*")[0];
  const resolved = path.resolve(baseDir, stem);

  if (await pathExists(resolved)) {
    return true;
  }

  for (const extension of CHECKABLE_EXTENSIONS) {
    if (await pathExists(`${resolved}${extension}`)) {
      return true;
    }
  }

  return false;
}

async function wildcardTargetExists(baseDir, target) {
  const [prefix, ...rest] = target.split("*");
  const suffix = rest.join("*");
  const searchPrefix =
    prefix.length === 0 ? "." : prefix.endsWith(path.sep) ? prefix : path.dirname(prefix);
  const searchRoot = path.resolve(baseDir, searchPrefix);

  if (!(await pathExists(searchRoot))) {
    return false;
  }

  if (rest.length === 1 && suffix.length === 0) {
    return true;
  }

  const matchers = buildWildcardMatchers(baseDir, target);
  return hasMatchingPath(searchRoot, matchers);
}

async function hasMatchingPath(candidatePath, matchers) {
  if (matchesPath(candidatePath, matchers)) {
    return true;
  }

  let entries;
  try {
    entries = await readdir(candidatePath, { withFileTypes: true });
  } catch {
    return false;
  }

  for (const entry of entries) {
    const fullPath = path.join(candidatePath, entry.name);
    if (matchesPath(fullPath, matchers)) {
      return true;
    }

    if (entry.isDirectory() && (await hasMatchingPath(fullPath, matchers))) {
      return true;
    }
  }

  return false;
}

function compareImportsAndPaths(tsconfigPath, packageDir, tsconfigBaseDir, imports, pathsConfig, findings) {
  for (const [importKey, importTarget] of Object.entries(imports)) {
    if (!Object.hasOwn(pathsConfig, importKey)) {
      continue;
    }

    const tsTargets = Array.isArray(pathsConfig[importKey]) ? pathsConfig[importKey] : [];
    const firstTsTarget = tsTargets[0];
    const normalizedImportTargets = normalizeImportTargets(packageDir, importTarget);
    const normalizedTsTarget = normalizeComparableTarget(tsconfigBaseDir, firstTsTarget);

    if (
      normalizedImportTargets.length === 0 ||
      !normalizedTsTarget ||
      normalizedImportTargets.includes(normalizedTsTarget)
    ) {
      continue;
    }

    findings.push(
      makeFinding({
        id: `tsconfig-import-conflict:${tsconfigPath}:${importKey}`,
        category: "tsconfig",
        severity: "warn",
        title: `Alias "${importKey}" differs between tsconfig and package imports`,
        file: tsconfigPath,
        detail: `tsconfig resolves to ${firstTsTarget}, while package.json imports resolves to ${stringifyImportTarget(importTarget)}.`,
        hint: "Align both alias surfaces so runtime and editor resolution stay consistent.",
      }),
    );
  }
}

function normalizeImportTargets(packageDir, importTarget) {
  const normalized = [];

  for (const target of collectImportTargets(importTarget)) {
    const comparable = normalizeComparableTarget(packageDir, target);
    if (comparable) {
      normalized.push(comparable);
    }
  }

  return Array.from(new Set(normalized));
}

function normalizeComparableTarget(baseDir, target) {
  if (typeof target !== "string") {
    return null;
  }

  return normalizePathForMatch(path.resolve(baseDir, target));
}

function stringifyImportTarget(importTarget) {
  if (typeof importTarget === "string") {
    return importTarget;
  }

  return JSON.stringify(importTarget);
}

function buildWildcardMatchers(baseDir, target) {
  const patterns = [path.resolve(baseDir, target)];

  if (!hasExplicitExtension(target)) {
    for (const extension of CHECKABLE_EXTENSIONS) {
      patterns.push(`${path.resolve(baseDir, target)}${extension}`);
    }
  }

  return patterns.map((pattern) => {
    const normalizedPattern = normalizePathForMatch(pattern);
    const escaped = escapeForRegExp(normalizedPattern).replaceAll("\\*", ".+");
    return new RegExp(`^${escaped}$`, "u");
  });
}

function normalizePathForMatch(candidatePath) {
  return candidatePath.split(path.sep).join("/");
}

function matchesPath(candidatePath, matchers) {
  const normalizedPath = normalizePathForMatch(candidatePath);
  return matchers.some((matcher) => matcher.test(normalizedPath));
}

function hasExplicitExtension(target) {
  const tail = target.split("*").at(-1) ?? target;
  return path.extname(tail) !== "";
}

function collectImportTargets(importTarget) {
  if (typeof importTarget === "string") {
    return [importTarget];
  }

  if (!importTarget || typeof importTarget !== "object" || Array.isArray(importTarget)) {
    return [];
  }

  const targets = [];

  for (const value of Object.values(importTarget)) {
    targets.push(...collectImportTargets(value));
  }

  return targets;
}

function escapeForRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
