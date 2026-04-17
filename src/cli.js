import path from "node:path";
import process from "node:process";

import { auditProject } from "./core/audit-project.js";
import { applyFixes } from "./core/fixers.js";
import {
  formatAuditReport,
  formatDoctorReport,
  formatFixResult,
  formatHelp,
} from "./core/format-report.js";
import { serializeAuditResult } from "./core/findings.js";

export async function runCli(argv = []) {
  const { command, args, flags } = parseArgs(argv);

  if (!command || command === "help" || flags.help) {
    console.log(formatHelp());
    return;
  }

  const targetDir = path.resolve(args[0] ?? process.cwd());

  if (command === "audit" || command === "doctor") {
    const result = await auditProject(targetDir);
    process.exitCode =
      result.summary.blockingFindings > 0 || result.summary.warningFindings > 0 ? 1 : 0;

    if (flags.json) {
      console.log(JSON.stringify(serializeAuditResult(result), null, 2));
      return;
    }

    console.log(command === "audit" ? formatAuditReport(result) : formatDoctorReport(result));
    return;
  }

  if (command === "fix") {
    const initial = await auditProject(targetDir);
    const applied = flags.dryRun ? [] : await applyFixes(initial.fixes);
    const final = flags.dryRun ? initial : await auditProject(targetDir);
    const finalSummary = flags.dryRun ? initial.summary : final.summary;

    process.exitCode =
      finalSummary.blockingFindings > 0 || finalSummary.warningFindings > 0 ? 1 : 0;

    if (flags.json) {
      console.log(
        JSON.stringify(
          {
            dryRun: flags.dryRun,
            targetDir,
            initial: serializeAuditResult(initial),
            applied,
            final: serializeAuditResult(final),
          },
          null,
          2,
        ),
      );
      return;
    }

    console.log(formatFixResult({ dryRun: flags.dryRun, targetDir, initial, applied, final }));
    return;
  }

  throw new Error(`Unknown command "${command}". Run "maximus help" for usage.`);
}

function parseArgs(argv) {
  const args = [];
  const flags = {
    dryRun: false,
    help: false,
    json: false,
  };

  for (const token of argv) {
    if (token === "--dry-run") {
      flags.dryRun = true;
      continue;
    }

    if (token === "--json") {
      flags.json = true;
      continue;
    }

    if (token === "--help" || token === "-h") {
      flags.help = true;
      continue;
    }

    args.push(token);
  }

  return {
    command: args.shift(),
    args,
    flags,
  };
}
