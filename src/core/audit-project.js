import { runConfigDuplicateCheck } from "../checks/config-duplicates.js";
import { runEnvCheck } from "../checks/env.js";
import { runEslintPrettierCheck } from "../checks/eslint-prettier.js";
import { buildStructureReport } from "../checks/structure.js";
import { runTsconfigCheck } from "../checks/tsconfig.js";
import { sortFindings, summarizeFindings, uniqueFixes } from "./findings.js";
import { discoverProject } from "./discover.js";

export async function auditProject(rootDir) {
  const project = await discoverProject(rootDir);
  const checkResults = await Promise.all([
    runConfigDuplicateCheck(project),
    runEnvCheck(project),
    runEslintPrettierCheck(project),
    runTsconfigCheck(project),
  ]);

  const findings = sortFindings(checkResults.flatMap((result) => result.findings));
  const fixes = uniqueFixes(checkResults.flatMap((result) => result.fixes));
  const structure = buildStructureReport(project, findings);
  const summary = summarizeFindings(findings, fixes, structure);

  return {
    rootDir,
    project,
    findings,
    fixes,
    structure,
    summary,
  };
}
