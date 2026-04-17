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
