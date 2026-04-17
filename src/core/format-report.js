import path from "node:path";

export function formatHelp() {
  return [
    "Maximus",
    "",
    "Bring order to chaotic configs.",
    "",
    "Usage",
    "  maximus audit [path] [--json]",
    "  maximus doctor [path] [--json]",
    "  maximus fix [path] [--dry-run] [--json]",
    "  maximus help",
  ].join("\n");
}

export function formatAuditReport(result) {
  const lines = [];

  lines.push("Maximus audit");
  lines.push(`Target: ${result.rootDir}`);
  lines.push("");
  lines.push(`Status: ${result.summary.status}`);
  lines.push(
    `Findings: ${result.summary.blockingFindings} error, ${result.summary.warningFindings} warnings, ${result.summary.infoFindings} info`,
  );
  lines.push(`Fixes available: ${result.summary.fixesAvailable}`);
  lines.push("");
  lines.push(`Structure: ${describeStructure(result.structure)}`);

  if (result.findings.length === 0) {
    lines.push("");
    lines.push("No config drift detected.");
  } else {
    lines.push("");
    lines.push("Findings");
    lines.push(...formatFindings(result));
  }

  if (result.structure.recommendations.length > 0) {
    lines.push("");
    lines.push("Recommendations");
    for (const recommendation of result.structure.recommendations) {
      lines.push(`- ${recommendation}`);
    }
  }

  return lines.join("\n");
}

export function formatDoctorReport(result) {
  const lines = [];
  const manualFindings = result.findings.filter((finding) => !finding.fixable);
  const fixableFindings = result.findings.filter((finding) => finding.fixable);

  lines.push("Maximus doctor");
  lines.push(`Target: ${result.rootDir}`);
  lines.push("");
  lines.push(`Diagnosis: ${result.summary.status}`);
  lines.push(`Project shape: ${describeStructure(result.structure)}`);
  lines.push("");
  lines.push("Prescription");

  if (fixableFindings.length > 0) {
    lines.push(`- Run "maximus fix" to apply ${result.summary.fixesAvailable} safe fix(es).`);
  } else {
    lines.push("- No automatic fixes are currently available.");
  }

  if (manualFindings.length > 0) {
    lines.push(`- Review ${manualFindings.length} manual issue(s) in priority order below.`);
  } else {
    lines.push("- No manual follow-up is required right now.");
  }

  if (result.findings.length === 0) {
    lines.push("");
    lines.push("No config drift detected.");
  } else {
    lines.push("");
    lines.push("Findings");
    lines.push(...formatFindings(result));
  }

  if (result.structure.recommendations.length > 0) {
    lines.push("");
    lines.push("Recommended structure");
    for (const recommendation of result.structure.recommendations) {
      lines.push(`- ${recommendation}`);
    }
  }

  return lines.join("\n");
}

export function formatFixResult({ dryRun, targetDir, initial, applied, final }) {
  const lines = [];
  const result = dryRun ? initial : final;

  lines.push("Maximus fix");
  lines.push(`Target: ${targetDir}`);
  lines.push("");

  if (dryRun) {
    lines.push(`Dry run: ${initial.summary.fixesAvailable} safe fix(es) available.`);
  } else {
    lines.push(`Applied: ${applied.length} fix(es).`);
  }

  if (applied.length > 0) {
    lines.push("");
    lines.push("Changes");
    for (const fix of applied) {
      lines.push(`- ${fix.title}`);
      for (const file of fix.files) {
        lines.push(`  file: ${file}`);
      }
    }
  }

  lines.push("");
  lines.push(
    `Post-check: ${result.summary.blockingFindings} error, ${result.summary.warningFindings} warnings, ${result.summary.infoFindings} info`,
  );

  if (result.findings.length > 0) {
    lines.push("");
    lines.push("Remaining findings");
    lines.push(...formatFindings(result));
  } else {
    lines.push("");
    lines.push("Project is currently clean.");
  }

  return lines.join("\n");
}

function formatFindings(result) {
  return result.findings.flatMap((finding) => {
    const lines = [];
    lines.push(`- [${finding.severity}] ${finding.title}`);

    if (finding.file) {
      lines.push(`  file: ${formatFile(result.rootDir, finding.file)}`);
    }

    if (finding.detail) {
      lines.push(`  detail: ${finding.detail}`);
    }

    if (finding.hint) {
      lines.push(`  hint: ${finding.hint}`);
    }

    return lines;
  });
}

function formatFile(rootDir, filePath) {
  return path.relative(rootDir, filePath) || ".";
}

function describeStructure(structure) {
  const repoType = structure.isMonorepo ? "monorepo" : "single package";
  return `${repoType}, ${structure.packageCount} package(s), ${structure.configFiles} config file(s), ${structure.envDirectories} env folder(s)`;
}
