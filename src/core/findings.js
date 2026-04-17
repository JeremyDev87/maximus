const SEVERITY_ORDER = {
  error: 0,
  warn: 1,
  info: 2,
};

export function makeFinding(finding) {
  return {
    category: "general",
    detail: "",
    file: null,
    fixIds: [],
    fixable: false,
    hint: "",
    severity: "warn",
    ...finding,
  };
}

export function sortFindings(findings) {
  return findings.slice().sort((left, right) => {
    const severityDelta = SEVERITY_ORDER[left.severity] - SEVERITY_ORDER[right.severity];
    if (severityDelta !== 0) {
      return severityDelta;
    }

    const fileDelta = (left.file ?? "").localeCompare(right.file ?? "");
    if (fileDelta !== 0) {
      return fileDelta;
    }

    return left.title.localeCompare(right.title);
  });
}

export function uniqueFixes(fixes) {
  const seen = new Set();
  const unique = [];

  for (const fix of fixes) {
    if (seen.has(fix.id)) {
      continue;
    }

    seen.add(fix.id);
    unique.push(fix);
  }

  return unique;
}

export function summarizeFindings(findings, fixes, structure) {
  const blockingFindings = findings.filter((finding) => finding.severity === "error").length;
  const warningFindings = findings.filter((finding) => finding.severity === "warn").length;
  const infoFindings = findings.filter((finding) => finding.severity === "info").length;
  const fixableFindings = findings.filter((finding) => finding.fixable).length;

  let status = "clean";
  if (blockingFindings > 0) {
    status = "blocking issues";
  } else if (warningFindings > 0) {
    status = "attention needed";
  }

  return {
    status,
    totalFindings: findings.length,
    blockingFindings,
    warningFindings,
    infoFindings,
    fixableFindings,
    fixesAvailable: fixes.length,
    configFiles: structure.configFiles,
    packageCount: structure.packageCount,
    envDirectories: structure.envDirectories,
  };
}

export function serializeAuditResult(result) {
  return {
    rootDir: result.rootDir,
    summary: result.summary,
    structure: result.structure,
    findings: result.findings.map(serializeFinding),
    fixes: result.fixes.map((fix) => ({
      id: fix.id,
      title: fix.title,
      files: fix.files,
    })),
  };
}

function serializeFinding(finding) {
  return {
    id: finding.id,
    severity: finding.severity,
    category: finding.category,
    title: finding.title,
    detail: finding.detail,
    file: finding.file,
    hint: finding.hint,
    fixable: finding.fixable,
    fixIds: finding.fixIds,
  };
}
