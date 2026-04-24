import path from "node:path";

export function parseEnv(text, { label = ".env" } = {}) {
  const entries = [];
  const duplicates = [];
  const invalidLines = [];
  const values = new Map();
  const order = [];

  const lines = text.split(/\r?\n/u);
  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const trimmed = rawLine.trim();

    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }

    const line = trimmed.startsWith("export ") ? trimmed.slice(7).trim() : trimmed;
    const match = line.match(/^([A-Za-z_][A-Za-z0-9_.-]*)\s*=\s*(.*)$/u);

    if (!match) {
      invalidLines.push({
        label,
        line: index + 1,
        content: rawLine,
      });
      continue;
    }

    const key = match[1];
    const rawValue = match[2];
    const value = normalizeEnvValue(rawValue);
    const entry = {
      key,
      rawValue,
      value,
      line: index + 1,
    };

    if (values.has(key)) {
      duplicates.push({
        key,
        firstLine: values.get(key).line,
        secondLine: index + 1,
      });
    } else {
      order.push(key);
    }

    values.set(key, entry);
    entries.push(entry);
  }

  return {
    entries,
    duplicates,
    invalidLines,
    order,
    values,
  };
}

export function renderEnvTemplate(keys) {
  const uniqueKeys = Array.from(new Set(keys)).sort((left, right) => left.localeCompare(right));
  if (uniqueKeys.length === 0) {
    return "";
  }

  return `${uniqueKeys.map((key) => `${key}=`).join("\n")}\n`;
}

const TEMPLATE_ENV_SEGMENTS = new Set(["dist", "example", "sample", "template"]);

export function isTemplateEnvFileName(name) {
  if (!/^\.env(?:\..+)?$/u.test(name)) {
    return false;
  }

  const segments = name.split(".").filter(Boolean).slice(1);
  return segments.some((segment) => TEMPLATE_ENV_SEGMENTS.has(segment.toLowerCase()));
}

export function isConcreteEnvFileName(name) {
  return /^\.env(?:\..+)?$/u.test(name) && !isTemplateEnvFileName(name);
}

export function looksLikeSecret(value) {
  if (!value) {
    return false;
  }

  if (/^(?:change-me|example|placeholder|your-[a-z-]+|localhost|127\.0\.0\.1|true|false|0|1)$/iu.test(value)) {
    return false;
  }

  if (/^[A-Za-z0-9/_+=-]{16,}$/u.test(value)) {
    return true;
  }

  return false;
}

export function parseExactGitignorePatterns(text) {
  return text
    .split(/\r?\n/u)
    .map((line) => line.trimEnd())
    .filter((line) => line.length > 0 && !line.startsWith("#"))
    .map((line) => {
      const negated = line.startsWith("!");
      const pattern = normalizeGitignorePattern(negated ? line.slice(1) : line);
      return pattern ? { ...pattern, negated } : null;
    })
    .filter(Boolean);
}

export function isPathProtectedByExactGitignore(targetPath, gitignoreSources) {
  const fileName = path.basename(targetPath);
  let protectedState = null;

  for (const [ignoreRoot, gitignoreText] of gitignoreSources) {
    const relativePath = normalizePath(path.relative(ignoreRoot, targetPath));
    for (const pattern of parseExactGitignorePatterns(gitignoreText)) {
      if (patternMatchesFile(pattern, relativePath, fileName)) {
        protectedState = !pattern.negated;
      }
    }
  }

  return protectedState ?? false;
}

export function formatGitignoreProtectionHint(rootDir, directoryDir, filePath) {
  const currentGitignorePath = path.join(directoryDir, ".gitignore");
  const currentGitignoreDisplay = normalizePath(path.relative(rootDir, currentGitignorePath)) || ".gitignore";
  const currentPattern = path.basename(filePath);
  const rootPattern = normalizePath(path.relative(rootDir, filePath));

  if (directoryDir === rootDir || rootPattern === currentPattern) {
    return `Add "${currentPattern}" to ${currentGitignoreDisplay}.`;
  }

  return `Add "${currentPattern}" to ${currentGitignoreDisplay} or "${rootPattern}" to .gitignore.`;
}

function normalizeEnvValue(rawValue) {
  const trimmed = rawValue.trim();

  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1);
  }

  return trimmed;
}

function normalizeGitignorePattern(pattern) {
  const normalized = normalizePath(pattern).replace(/^\.\/+/u, "");
  const anchored = normalized.startsWith("/");
  const directoryOnly = normalized.endsWith("/");
  const value = normalized.replace(/^\/+/u, "").replace(/\/+$/u, "");

  return value.length > 0 ? { pattern: value, anchored, directoryOnly } : null;
}

function patternMatchesFile(pattern, relativePath, fileName) {
  if (pattern.directoryOnly) {
    return directoryPatternMatchesFile(pattern, relativePath);
  }

  return (
    directoryPatternMatchesFile(pattern, relativePath) ||
    gitignorePatternMatches(pattern.pattern, relativePath) ||
    (!pattern.anchored && !pattern.pattern.includes("/") && gitignorePatternMatches(pattern.pattern, fileName))
  );
}

function directoryPatternMatchesFile(pattern, relativePath) {
  const directoryPath = relativePath.includes("/") ? relativePath.slice(0, relativePath.lastIndexOf("/")) : "";
  if (!directoryPath) {
    return false;
  }

  const directories = directoryAncestors(directoryPath);
  if (!pattern.anchored && !pattern.pattern.includes("/")) {
    return directories.some((directory) => gitignorePatternMatches(pattern.pattern, path.basename(directory)));
  }

  return directories.some((directory) => gitignorePatternMatches(pattern.pattern, directory));
}

function directoryAncestors(directoryPath) {
  const parts = directoryPath.split("/");
  const directories = [];
  for (let index = 0; index < parts.length; index += 1) {
    directories.push(parts.slice(0, index + 1).join("/"));
  }
  return directories;
}

function gitignorePatternMatches(pattern, value) {
  const memo = new Map();

  function matches(patternIndex, valueIndex) {
    const memoKey = `${patternIndex}:${valueIndex}`;
    if (memo.has(memoKey)) {
      return memo.get(memoKey);
    }

    let result;
    if (patternIndex >= pattern.length) {
      result = valueIndex >= value.length;
    } else if (pattern.startsWith("**", patternIndex)) {
      const afterGlobstar = patternIndex + 2;
      if (pattern[afterGlobstar] === "/") {
        result = matches(afterGlobstar + 1, valueIndex);
        for (let index = valueIndex; !result && index < value.length; index += 1) {
          if (value[index] === "/") {
            result = matches(afterGlobstar + 1, index + 1);
          }
        }
      } else {
        result = false;
        for (let index = valueIndex; !result && index <= value.length; index += 1) {
          result = matches(afterGlobstar, index);
        }
      }
    } else if (pattern[patternIndex] === "*") {
      result = matches(patternIndex + 1, valueIndex);
      for (let index = valueIndex; !result && index < value.length && value[index] !== "/"; index += 1) {
        result = matches(patternIndex + 1, index + 1);
      }
    } else if (pattern[patternIndex] === "?") {
      result = valueIndex < value.length && value[valueIndex] !== "/" && matches(patternIndex + 1, valueIndex + 1);
    } else {
      result = valueIndex < value.length && pattern[patternIndex] === value[valueIndex] && matches(patternIndex + 1, valueIndex + 1);
    }

    memo.set(memoKey, result);
    return result;
  }

  return matches(0, 0);
}

function normalizePath(value) {
  return value.replaceAll("\\", "/");
}
