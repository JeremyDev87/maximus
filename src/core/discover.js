import path from "node:path";
import { readdir } from "node:fs/promises";

const IGNORED_DIRECTORIES = new Set([
  ".git",
  ".hg",
  ".idea",
  ".next",
  ".nuxt",
  ".output",
  ".pnpm-store",
  ".svelte-kit",
  ".turbo",
  ".vercel",
  "build",
  "coverage",
  "dist",
  "node_modules",
  "out",
  "tmp",
]);

const MATCHERS = [
  { kind: "package", test: (name) => name === "package.json" },
  { kind: "tsconfig", test: (name) => name === "jsconfig.json" || /^tsconfig(?:\..+)?\.json$/.test(name) },
  {
    kind: "eslint",
    test: (name) =>
      /^\.eslintrc(?:\.(?:json|ya?ml|js|cjs|mjs))?$/.test(name) ||
      /^eslint\.config\.(?:js|cjs|mjs|ts|mts|cts)$/.test(name),
  },
  {
    kind: "prettier",
    test: (name) =>
      /^\.prettierrc(?:\.(?:json|ya?ml|js|cjs|mjs))?$/.test(name) ||
      name === ".prettierrc.toml" ||
      /^prettier\.config\.(?:js|cjs|mjs|ts|mts|cts)$/.test(name),
  },
  { kind: "vite", test: (name) => /^vite\.config\.(?:js|cjs|mjs|ts|mts|cts)$/.test(name) },
  { kind: "jest", test: (name) => /^jest\.config\.(?:js|cjs|mjs|ts|mts|cts)$/.test(name) },
  { kind: "next", test: (name) => /^next\.config\.(?:js|cjs|mjs|ts|mts|cts)$/.test(name) },
  { kind: "env", test: (name) => /^\.env(?:\..+)?$/.test(name) },
  { kind: "workspace", test: (name) => name === "pnpm-workspace.yaml" || name === "turbo.json" },
];

export async function discoverProject(rootDir) {
  const files = [];
  await walk(rootDir, rootDir, files);
  files.sort((left, right) => left.relativePath.localeCompare(right.relativePath));

  const directories = new Map();
  const filesByKind = new Map();

  for (const file of files) {
    if (!directories.has(file.dir)) {
      directories.set(file.dir, {
        dir: file.dir,
        relativeDir: path.relative(rootDir, file.dir) || ".",
        files: [],
        filesByKind: new Map(),
      });
    }

    const directory = directories.get(file.dir);
    directory.files.push(file);

    if (!directory.filesByKind.has(file.kind)) {
      directory.filesByKind.set(file.kind, []);
    }

    directory.filesByKind.get(file.kind).push(file);

    if (!filesByKind.has(file.kind)) {
      filesByKind.set(file.kind, []);
    }

    filesByKind.get(file.kind).push(file);
  }

  const packageFiles = (filesByKind.get("package") ?? []).slice().sort((left, right) => {
    return path.dirname(left.path).length - path.dirname(right.path).length;
  });

  return {
    rootDir,
    files,
    directories,
    filesByKind,
    packageFiles,
  };
}

export function getFiles(project, kind) {
  return project.filesByKind.get(kind) ?? [];
}

export function getDirectories(project) {
  return Array.from(project.directories.values()).sort((left, right) => {
    return left.relativeDir.localeCompare(right.relativeDir);
  });
}

export function findNearestPackageFile(project, directory) {
  const packageFiles = project.packageFiles.slice().reverse();

  for (const file of packageFiles) {
    const packageDir = path.dirname(file.path);
    if (directory === packageDir || directory.startsWith(`${packageDir}${path.sep}`)) {
      return file;
    }
  }

  return null;
}

async function walk(rootDir, currentDir, files) {
  const entries = await readdir(currentDir, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name));

  for (const entry of entries) {
    if (entry.isDirectory()) {
      if (IGNORED_DIRECTORIES.has(entry.name)) {
        continue;
      }

      await walk(rootDir, path.join(currentDir, entry.name), files);
      continue;
    }

    if (!entry.isFile()) {
      continue;
    }

    const kind = matchFileKind(entry.name);
    if (!kind) {
      continue;
    }

    const filePath = path.join(currentDir, entry.name);
    files.push({
      kind,
      name: entry.name,
      path: filePath,
      dir: currentDir,
      relativePath: path.relative(rootDir, filePath) || entry.name,
    });
  }
}

function matchFileKind(name) {
  for (const matcher of MATCHERS) {
    if (matcher.test(name)) {
      return matcher.kind;
    }
  }

  return null;
}
