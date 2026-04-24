import path from "node:path";
import { execFile } from "node:child_process";
import { stat } from "node:fs/promises";
import { promisify } from "node:util";

import { getDirectories } from "../core/discover.js";
import { makeFinding } from "../core/findings.js";
import {
  formatGitignoreProtectionHint,
  isConcreteEnvFileName,
  isPathProtectedByExactGitignore,
  isTemplateEnvFileName,
  looksLikeSecret,
  parseEnv,
  renderEnvTemplate,
} from "../lib/env.js";
import { readTextIfExists, writeText } from "../lib/fs.js";

const execFileAsync = promisify(execFile);

export async function runEnvCheck(project) {
  const findings = [];
  const fixes = [];
  const gitignoreTraversalRoot = await findGitignoreTraversalRoot(project.rootDir);

  for (const directory of getDirectories(project)) {
    const envFiles = directory.filesByKind.get("env") ?? [];
    if (envFiles.length === 0) {
      continue;
    }

    const parsedByName = new Map();

    for (const file of envFiles) {
      const text = await readTextIfExists(file.path);
      if (text == null) {
        continue;
      }

      const parsed = parseEnv(text, { label: file.name });
      parsedByName.set(file.name, { file, parsed, text });

      for (const duplicate of parsed.duplicates) {
        findings.push(
          makeFinding({
            id: `env-duplicate:${file.path}:${duplicate.key}:${duplicate.secondLine}`,
            category: "env",
            severity: "error",
            title: `Duplicate env key "${duplicate.key}"`,
            file: file.path,
            detail: `${duplicate.key} is declared on lines ${duplicate.firstLine} and ${duplicate.secondLine}.`,
            hint: "Keep one declaration per env file so overrides stay explicit.",
          }),
        );
      }

      for (const invalidLine of parsed.invalidLines) {
        findings.push(
          makeFinding({
            id: `env-invalid:${file.path}:${invalidLine.line}`,
            category: "env",
            severity: "warn",
            title: "Invalid env syntax",
            file: file.path,
            detail: `Line ${invalidLine.line} could not be parsed as KEY=value.`,
            hint: "Use shell-style env syntax or move comments to their own line.",
          }),
        );
      }
    }

    const contractRecords = Array.from(parsedByName.entries())
      .filter(([name]) => isTemplateEnvFileName(name))
      .map(([, record]) => record)
      .sort(compareContractRecords);
    const exampleRecord = contractRecords[0] ?? null;
    const concreteRecords = Array.from(parsedByName.entries())
      .filter(([name]) => isConcreteEnvFileName(name))
      .map(([, record]) => record);

    const contractKeys = new Set();
    for (const record of concreteRecords) {
      for (const key of record.parsed.order) {
        contractKeys.add(key);
      }
    }

    const gitignoreSources = await readAncestorGitignoreSources(gitignoreTraversalRoot, directory.dir);

    for (const record of concreteRecords) {
      const isTracked = await isPathTrackedByGit(gitignoreTraversalRoot, record.file.path);
      const isProtected = !isTracked && isPathProtectedByExactGitignore(record.file.path, gitignoreSources);

      if (isProtected) {
        continue;
      }

      findings.push(
        makeFinding({
          id: `env-gitignore:${record.file.path}`,
          category: "env",
          severity: "warn",
          title: `Concrete env file "${record.file.name}" is not protected by .gitignore`,
          file: record.file.path,
          detail: formatGitignoreProtectionHint(project.rootDir, directory.dir, record.file.path),
          hint: "Protect concrete env files with an exact .gitignore entry before committing secrets.",
        }),
      );
    }

    if (contractKeys.size > 0 && !exampleRecord) {
      const outputPath = path.join(directory.dir, ".env.example");
      const sortedKeys = Array.from(contractKeys).sort((left, right) => left.localeCompare(right));

      findings.push(
        makeFinding({
          id: `env-example-missing:${directory.dir}`,
          category: "env",
          severity: "warn",
          title: "Missing .env.example contract",
          file: concreteRecords[0]?.file.path ?? null,
          detail: "Runtime env files exist, but .env.example is missing.",
          hint: 'Run "maximus fix" to create a blank contract file.',
          fixable: true,
          fixIds: [`env-example:create:${directory.dir}`],
        }),
      );

      fixes.push({
        id: `env-example:create:${directory.dir}`,
        title: `Create ${path.relative(project.rootDir, outputPath) || ".env.example"}`,
        files: [outputPath],
        apply: async () => {
          await writeText(outputPath, renderEnvTemplate(sortedKeys));
          return {
            outcome: "created",
          };
        },
      });
    }

    if (exampleRecord) {
      const exampleKeys = new Set(exampleRecord.parsed.order);
      const missingKeys = Array.from(contractKeys).filter((key) => !exampleKeys.has(key));

      if (missingKeys.length > 0) {
        findings.push(
          makeFinding({
            id: `env-example-sync:${directory.dir}`,
            category: "env",
            severity: "warn",
          title: `${exampleRecord.file.name} is missing keys`,
          file: exampleRecord.file.path,
          detail: `Missing keys: ${missingKeys.join(", ")}.`,
          hint: `Run "maximus fix" to append the missing keys to ${exampleRecord.file.name}.`,
          fixable: true,
          fixIds: [`env-example:sync:${directory.dir}`],
        }),
        );

        fixes.push({
          id: `env-example:sync:${directory.dir}`,
          title: `Append missing keys to ${path.relative(project.rootDir, exampleRecord.file.path) || exampleRecord.file.name}`,
          files: [exampleRecord.file.path],
          apply: async () => {
            const prefix = exampleRecord.text.endsWith("\n") || exampleRecord.text.length === 0 ? "" : "\n";
            const addition = missingKeys
              .slice()
              .sort((left, right) => left.localeCompare(right))
              .map((key) => `${key}=`)
              .join("\n");
            const suffix = addition.length > 0 ? "\n" : "";

            await writeText(exampleRecord.file.path, `${exampleRecord.text}${prefix}${addition}${suffix}`);
            return {
              outcome: "updated",
            };
          },
        });
      }

      for (const contractRecord of contractRecords) {
        for (const entry of contractRecord.parsed.entries) {
          if (!looksLikeSecret(entry.value)) {
            continue;
          }

          findings.push(
            makeFinding({
              id: `env-example-secret:${contractRecord.file.path}:${entry.key}`,
              category: "env",
              severity: "warn",
              title: `${contractRecord.file.name} appears to contain a real value for "${entry.key}"`,
              file: contractRecord.file.path,
              detail: "Contract files should describe the interface, not ship concrete secrets.",
              hint: "Replace the value with a blank or placeholder string before sharing the repo.",
            }),
          );
        }
      }
    }

    const baseEnv = parsedByName.get(".env");
    const localEnv = parsedByName.get(".env.local");
    if (baseEnv && localEnv) {
      const mismatchedKeys = [];
      for (const [key, baseEntry] of baseEnv.parsed.values.entries()) {
        const localEntry = localEnv.parsed.values.get(key);
        if (!localEntry) {
          continue;
        }

        if (baseEntry.value !== localEntry.value) {
          mismatchedKeys.push(key);
        }
      }

      if (mismatchedKeys.length > 0) {
        findings.push(
          makeFinding({
            id: `env-mismatch:${directory.dir}`,
            category: "env",
            severity: "info",
            title: "Local env overrides detected",
            file: localEnv.file.path,
            detail: `.env.local overrides ${mismatchedKeys.length} key(s): ${mismatchedKeys.join(", ")}.`,
            hint: "Make sure local-only overrides are intentional and documented in .env.example.",
          }),
        );
      }
    }

    if (exampleRecord) {
      const providedKeys = new Set();
      for (const record of concreteRecords) {
        for (const key of record.parsed.order) {
          providedKeys.add(key);
        }
      }

      const missingConcreteKeys = exampleRecord.parsed.order.filter((key) => !providedKeys.has(key));
      if (missingConcreteKeys.length > 0 && concreteRecords.length > 0) {
        findings.push(
          makeFinding({
            id: `env-missing-concrete:${directory.dir}`,
            category: "env",
            severity: "warn",
            title: "Declared env contract is not satisfied locally",
            file: exampleRecord.file.path,
            detail: `No concrete value was found for: ${missingConcreteKeys.join(", ")}.`,
            hint: "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files.",
          }),
        );
      }
    }
  }

  return { findings, fixes };
}

async function isPathTrackedByGit(repoRoot, filePath) {
  const relativePath = path.relative(repoRoot, filePath);
  if (!relativePath || relativePath.startsWith("..") || path.isAbsolute(relativePath)) {
    return false;
  }

  try {
    await execFileAsync("git", ["-C", repoRoot, "ls-files", "--error-unmatch", "--", relativePath]);
    return true;
  } catch {
    return false;
  }
}

async function findGitignoreTraversalRoot(rootDir) {
  let currentDir = rootDir;

  while (true) {
    try {
      await stat(path.join(currentDir, ".git"));
      return currentDir;
    } catch {
      const parentDir = path.dirname(currentDir);
      if (parentDir === currentDir) {
        return rootDir;
      }
      currentDir = parentDir;
    }
  }
}

async function readAncestorGitignoreSources(rootDir, directoryDir) {
  const sources = [];
  let currentDir = rootDir;

  while (true) {
    const gitignoreText = await readTextIfExists(path.join(currentDir, ".gitignore"));
    if (gitignoreText != null) {
      sources.push([currentDir, gitignoreText]);
    }

    if (currentDir === directoryDir) {
      break;
    }

    const relative = path.relative(currentDir, directoryDir);
    if (relative === "" || relative.startsWith("..") || path.isAbsolute(relative)) {
      break;
    }

    const [nextSegment] = relative.split(path.sep);
    if (!nextSegment) {
      break;
    }
    currentDir = path.join(currentDir, nextSegment);
  }

  return sources;
}

function compareContractRecords(left, right) {
  return scoreContractRecord(left.file.name) - scoreContractRecord(right.file.name) || left.file.name.localeCompare(right.file.name);
}

function scoreContractRecord(fileName) {
  const priority = [".env.example", ".env.sample", ".env.template", ".env.dist"];
  const index = priority.indexOf(fileName);
  return index === -1 ? priority.length : index;
}
