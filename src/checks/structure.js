import { getFiles } from "../core/discover.js";

export function buildStructureReport(project, findings) {
  const packageCount = getFiles(project, "package").length;
  const envDirectories = new Set(getFiles(project, "env").map((file) => file.dir)).size;
  const configFiles = project.files.length;
  const isMonorepo = packageCount > 1;
  const recommendations = [];
  const hasSharedTsconfig = getFiles(project, "tsconfig").some((file) => file.name === "tsconfig.base.json");
  const hasRootEnvExample = getFiles(project, "env").some(
    (file) => file.dir === project.rootDir && file.name === ".env.example",
  );
  const eslintConfigCount = getFiles(project, "eslint").length;

  if (isMonorepo && !hasSharedTsconfig) {
    recommendations.push("Introduce a shared tsconfig.base.json so packages inherit one source of truth.");
  }

  if (eslintConfigCount > 1) {
    recommendations.push("Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets.");
  }

  if (envDirectories > 1 && !hasRootEnvExample) {
    recommendations.push("Use .env.example files consistently so onboarding does not depend on tribal knowledge.");
  }

  if (findings.length === 0) {
    recommendations.push("Current config surface looks healthy. Keep shared rules centralized as the repo grows.");
  }

  return {
    isMonorepo,
    packageCount,
    envDirectories,
    configFiles,
    recommendations,
  };
}
