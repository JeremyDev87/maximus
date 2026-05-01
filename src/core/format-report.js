import path from "node:path";

export function formatHelp() {
  return [
    "Maximus",
    "",
    "혼란스러운 설정을 정리합니다.",
    "",
    "사용법",
    "  maximus audit [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json] [--output <path>]",
    "  maximus doctor [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--format <format>] [--json] [--output <path>]",
    "  maximus fix [path] [--only <checks>] [--skip <checks>] [--fail-on <level>] [--dry-run] [--diff] [--env-source-comments] [--fix-id <id>] [--fix-prefix <prefix>] [--format <format>] [--json] [--output <path>]",
    "  maximus help",
  ].join("\n");
}

export function formatAuditReport(result) {
  const lines = [];

  lines.push("Maximus audit");
  lines.push(`대상: ${result.rootDir}`);
  lines.push("");
  lines.push(`상태: ${translateStatus(result.summary.status)}`);
  lines.push(
    `발견 항목: 오류 ${result.summary.blockingFindings}개, 경고 ${result.summary.warningFindings}개, 정보 ${result.summary.infoFindings}개`,
  );
  lines.push(`적용 가능한 수정: ${result.summary.fixesAvailable}개`);
  lines.push("");
  lines.push(`구조: ${describeStructure(result.structure)}`);

  if (result.findings.length === 0) {
    lines.push("");
    lines.push("설정 차이가 감지되지 않았습니다.");
  } else {
    lines.push("");
    lines.push("발견 항목");
    lines.push(...formatFindings(result));
  }

  if (result.structure.recommendations.length > 0) {
    lines.push("");
    lines.push("권장 사항");
    for (const recommendation of result.structure.recommendations) {
      lines.push(`- ${translateMessage(recommendation)}`);
    }
  }

  return lines.join("\n");
}

export function formatDoctorReport(result) {
  const lines = [];
  const manualFindings = result.findings.filter((finding) => !finding.fixable);
  const fixableFindings = result.findings.filter((finding) => finding.fixable);

  lines.push("Maximus doctor");
  lines.push(`대상: ${result.rootDir}`);
  lines.push("");
  lines.push(`진단: ${translateStatus(result.summary.status)}`);
  lines.push(`프로젝트 구조: ${describeStructure(result.structure)}`);
  lines.push("");
  lines.push("처방");

  if (fixableFindings.length > 0) {
    lines.push(`- 안전한 수정 ${result.summary.fixesAvailable}개를 적용하려면 "maximus fix"를 실행하세요.`);
  } else {
    lines.push("- 현재 적용 가능한 자동 수정이 없습니다.");
  }

  if (manualFindings.length > 0) {
    lines.push(`- 아래 우선순위에 따라 수동 확인 항목 ${manualFindings.length}개를 검토하세요.`);
  } else {
    lines.push("- 지금은 수동 후속 조치가 필요하지 않습니다.");
  }

  if (result.findings.length === 0) {
    lines.push("");
    lines.push("설정 차이가 감지되지 않았습니다.");
  } else {
    lines.push("");
    lines.push("상위 3개 우선순위");
    lines.push(...formatTopPriorities(result));
    lines.push("");
    lines.push("발견 항목");
    lines.push(...formatFindings(result));
  }

  if (result.structure.recommendations.length > 0) {
    lines.push("");
    lines.push("권장 구조");
    for (const recommendation of result.structure.recommendations) {
      lines.push(`- ${translateMessage(recommendation)}`);
    }
  }

  return lines.join("\n");
}

export function formatFixResult({ dryRun, targetDir, initial, applied, final }) {
  const lines = [];
  const result = dryRun ? initial : final;

  lines.push("Maximus fix");
  lines.push(`대상: ${targetDir}`);
  lines.push("");

  if (dryRun) {
    lines.push(`Dry run: 적용 가능한 안전한 수정 ${initial.summary.fixesAvailable}개가 있습니다.`);
  } else {
    lines.push(`적용됨: 수정 ${applied.length}개.`);
  }

  if (applied.length > 0) {
    lines.push("");
    lines.push("변경 사항");
    for (const fix of applied) {
      lines.push(`- ${translateFixTitle(fix.title)}`);
      for (const file of fix.files) {
        lines.push(`  파일: ${file}`);
      }
    }
  }

  lines.push("");
  lines.push(
    `사후 점검: 오류 ${result.summary.blockingFindings}개, 경고 ${result.summary.warningFindings}개, 정보 ${result.summary.infoFindings}개`,
  );

  if (result.findings.length > 0) {
    lines.push("");
    lines.push("남은 발견 항목");
    lines.push(...formatFindings(result));
  } else {
    lines.push("");
    lines.push("현재 프로젝트는 정상입니다.");
  }

  return lines.join("\n");
}

function formatFindings(result) {
  return result.findings.flatMap((finding) => {
    const lines = [];
    lines.push(`- [${translateSeverity(finding.severity)}] ${translateMessage(finding.title)}`);

    if (finding.file) {
      lines.push(`  파일: ${formatFile(result.rootDir, finding.file)}`);
    }

    if (finding.detail) {
      lines.push(`  상세: ${translateMessage(finding.detail)}`);
    }

    if (finding.hint) {
      lines.push(`  힌트: ${translateMessage(finding.hint)}`);
    }

    return lines;
  });
}

function formatTopPriorities(result) {
  return result.findings.slice(0, 3).flatMap((finding, index) => {
    const lines = [
      `${index + 1}. [${translateSeverity(finding.severity)}] ${translateMessage(finding.title)}`,
    ];

    if (finding.file) {
      lines.push(`   파일: ${formatFile(result.rootDir, finding.file)}`);
    }

    if (finding.hint) {
      lines.push(`   다음: ${translateMessage(finding.hint)}`);
    } else if (finding.detail) {
      lines.push(`   다음: ${translateMessage(finding.detail)}`);
    }

    return lines;
  });
}

function formatFile(rootDir, filePath) {
  return path.relative(rootDir, filePath) || ".";
}

function describeStructure(structure) {
  const repoType = structure.isMonorepo ? "모노레포" : "단일 패키지";
  return `${repoType}, 패키지 ${structure.packageCount}개, 설정 파일 ${structure.configFiles}개, env 폴더 ${structure.envDirectories}개`;
}

function translateStatus(status) {
  return (
    {
      clean: "정상",
      "attention needed": "조치 필요",
      "blocking issues": "차단 이슈 있음",
    }[status] ?? status
  );
}

function translateSeverity(severity) {
  return (
    {
      error: "오류",
      warn: "경고",
      info: "정보",
    }[severity] ?? severity
  );
}

function translateFixTitle(value) {
  if (value.startsWith("Create ")) {
    return `${value.slice("Create ".length)} 생성`;
  }
  if (value.startsWith("Append missing keys to ")) {
    return `${value.slice("Append missing keys to ".length)}에 누락된 키 추가`;
  }
  return translateMessage(value);
}

function translateMessage(value) {
  const dynamic = translateDynamicMessage(value);
  if (dynamic) {
    return dynamic;
  }

  return MESSAGE_TRANSLATIONS.get(value) ?? value;
}

function translateDynamicMessage(value) {
  const concreteEnv = value.match(/^Concrete env file "(.+)" is not protected by \.gitignore$/);
  if (concreteEnv) {
    return `구체 env 파일 "${concreteEnv[1]}"이 .gitignore로 보호되지 않음`;
  }

  const duplicateEnv = value.match(/^Duplicate env key "(.+)"$/);
  if (duplicateEnv) {
    return `중복 env key "${duplicateEnv[1]}"`;
  }

  const missingConcreteEnv = value.match(/^No concrete value was found for: (.+)\.$/);
  if (missingConcreteEnv) {
    return `다음 env key에 대한 구체 값을 찾을 수 없습니다: ${missingConcreteEnv[1]}.`;
  }

  const missingEnvKeys = value.match(/^Missing keys: (.+)\.$/);
  if (missingEnvKeys) {
    return `누락된 key: ${missingEnvKeys[1]}.`;
  }

  const appendMissingKeys = value.match(/^Run "maximus fix" to append the missing keys to (.+)\.$/);
  if (appendMissingKeys) {
    return `"maximus fix"를 실행해 ${appendMissingKeys[1]}에 누락된 key를 추가하세요.`;
  }

  const editorconfigPrettier = value.match(/^EditorConfig sets (.+), but Prettier sets (.+)\.$/);
  if (editorconfigPrettier) {
    return `EditorConfig는 ${editorconfigPrettier[1]}를 설정하지만 Prettier는 ${editorconfigPrettier[2]}를 설정합니다.`;
  }

  const duplicateConfigDetail = value.match(/^Found (\d+) (.+) config sources in (.*)\.$/);
  if (duplicateConfigDetail) {
    const directory = duplicateConfigDetail[3] || ".";
    return `${directory}에서 ${duplicateConfigDetail[2]} 설정 출처 ${duplicateConfigDetail[1]}개를 찾았습니다.`;
  }

  const singleConfigHint = value.match(/^Keep a single (.+) entry point per directory to avoid drift\.$/);
  if (singleConfigHint) {
    return `차이를 피하려면 디렉터리마다 ${singleConfigHint[1]} 진입점을 하나만 유지하세요.`;
  }

  const lockfilesDetail = value.match(/^Found (\d+) known lockfiles in (.+): (.+)\.$/);
  if (lockfilesDetail) {
    return `${lockfilesDetail[2]}에서 알려진 lockfile ${lockfilesDetail[1]}개를 찾았습니다: ${lockfilesDetail[3]}.`;
  }

  const deprecatedOption = value.match(/^Deprecated compiler option "(.+)"$/);
  if (deprecatedOption) {
    return `deprecated compiler option "${deprecatedOption[1]}" 사용 중`;
  }

  const aliasNoTargets = value.match(/^Alias "(.+)" does not declare any targets$/);
  if (aliasNoTargets) {
    return `alias "${aliasNoTargets[1]}"가 대상을 선언하지 않음`;
  }

  const aliasNonString = value.match(/^Alias "(.+)" contains a non-string target$/);
  if (aliasNonString) {
    return `alias "${aliasNonString[1]}"에 string이 아닌 대상이 포함됨`;
  }

  const wildcard = value.match(/^Wildcard shape does not match for alias "(.+)"$/);
  if (wildcard) {
    return `alias "${wildcard[1]}"의 wildcard 형태가 일치하지 않음`;
  }

  const aliasDiffers = value.match(/^Alias "(.+)" differs between tsconfig and package imports$/);
  if (aliasDiffers) {
    return `alias "${aliasDiffers[1]}"가 tsconfig와 package imports 사이에서 다름`;
  }

  const packageShadow = value.match(/^Path alias "(.+)" shadows a package import$/);
  if (packageShadow) {
    return `path alias "${packageShadow[1]}"가 package import를 shadow함`;
  }

  const aliasShadow = value.match(/^Path alias "(.+)" shadows "(.+)"$/);
  if (aliasShadow) {
    return `path alias "${aliasShadow[1]}"가 "${aliasShadow[2]}"를 shadow함`;
  }

  const viteDiffers = value.match(/^Vite alias "(.+)" differs from tsconfig paths$/);
  if (viteDiffers) {
    return `Vite alias "${viteDiffers[1]}"가 tsconfig paths와 다름`;
  }

  const viteMissing = value.match(/^Vite alias "(.+)" is missing from tsconfig paths$/);
  if (viteMissing) {
    return `Vite alias "${viteMissing[1]}"가 tsconfig paths에 없음`;
  }

  const stringArray = value.match(/^"(.+)" must be an array of strings$/);
  if (stringArray) {
    return `"${stringArray[1]}"는 string array여야 함`;
  }

  const nonStringPattern = value.match(/^"(.+)" contains a non-string pattern$/);
  if (nonStringPattern) {
    return `"${nonStringPattern[1]}"에 string이 아닌 pattern이 포함됨`;
  }

  const multiConfig = value.match(/^(.+) config is declared in multiple places$/);
  if (multiConfig) {
    return `${multiConfig[1]} 설정이 여러 위치에 선언됨`;
  }

  const missingKeys = value.match(/^(.+) is missing keys$/);
  if (missingKeys) {
    return `${missingKeys[1]}에 누락된 key가 있음`;
  }

  const addToGitignore = value.match(/^Add "(.+)" to (.+)\.$/);
  if (addToGitignore) {
    return `${addToGitignore[2]}에 "${addToGitignore[1]}"를 추가하세요.`;
  }

  const missingPath = value.match(/^(.+) points to (.+), but the resolved path was not found\.$/);
  if (missingPath) {
    return `${missingPath[1]}는 ${missingPath[2]}를 가리키지만 해석된 경로를 찾을 수 없습니다.`;
  }

  const pointsToPath = value.match(/^(.+) points to (.+)\.$/);
  if (pointsToPath) {
    return `${pointsToPath[1]}는 ${pointsToPath[2]}를 가리킵니다.`;
  }

  const outDirOverlaps = value.match(/^outDir "(.+)" overlaps source root "(.+)"\.$/);
  if (outDirOverlaps) {
    return `outDir "${outDirOverlaps[1]}"이 source root "${outDirOverlaps[2]}"와 겹칩니다.`;
  }

  const outDirNested = value.match(/^outDir "(.+)" is nested inside source root "(.+)"\.$/);
  if (outDirNested) {
    return `outDir "${outDirNested[1]}"이 source root "${outDirNested[2]}" 안에 있습니다.`;
  }

  const outDirContainsInput = value.match(/^outDir "(.+)" contains TypeScript input "(.+)"\.$/);
  if (outDirContainsInput) {
    return `outDir "${outDirContainsInput[1]}"에 TypeScript 입력 "${outDirContainsInput[2]}"이 포함됩니다.`;
  }

  const outDirContainsRoot = value.match(/^outDir "(.+)" contains source root "(.+)"\.$/);
  if (outDirContainsRoot) {
    return `outDir "${outDirContainsRoot[1]}"이 source root "${outDirContainsRoot[2]}"를 포함합니다.`;
  }

  const expectsStringPatterns = value.match(
    /^(.+) declares (.+), but TypeScript expects string patterns\.$/,
  );
  if (expectsStringPatterns) {
    return `${expectsStringPatterns[1]}는 ${expectsStringPatterns[2]}를 선언하지만 TypeScript는 string pattern을 기대합니다.`;
  }

  const expectsPatternArray = value.match(
    /^(.+) declares (.+), but TypeScript expects an array of string patterns\.$/,
  );
  if (expectsPatternArray) {
    return `${expectsPatternArray[1]}는 ${expectsPatternArray[2]}를 선언하지만 TypeScript는 string pattern array를 기대합니다.`;
  }

  const filesGlob = value.match(
    /^(.+) declares (.+), but TypeScript files entries cannot use glob wildcards\.$/,
  );
  if (filesGlob) {
    return `${filesGlob[1]}는 ${filesGlob[2]}를 선언하지만 TypeScript files 항목에는 glob wildcard를 사용할 수 없습니다.`;
  }

  const filesDirectory = value.match(/^(.+) declares (.+), but that path resolves to a directory\.$/);
  if (filesDirectory) {
    return `${filesDirectory[1]}는 ${filesDirectory[2]}를 선언하지만 해당 path는 directory로 해석됩니다.`;
  }

  const filesMissing = value.match(
    /^(.+) declares (.+), but that path does not resolve to an existing file\.$/,
  );
  if (filesMissing) {
    return `${filesMissing[1]}는 ${filesMissing[2]}를 선언하지만 해당 path는 존재하는 파일로 해석되지 않습니다.`;
  }

  const includePattern = value.match(
    /^include pattern "(.+)" matched (\d+) files under base dir (.+)\.$/,
  );
  if (includePattern) {
    return `include pattern "${includePattern[1]}"은 base dir ${includePattern[3]} 아래에서 파일 ${includePattern[2]}개와 일치했습니다.`;
  }

  const excludePattern = value.match(
    /^exclude pattern "(.+)" removed (\d+) files from (\d+) included file\(s\) under base dir (.+)\.$/,
  );
  if (excludePattern) {
    return `exclude pattern "${excludePattern[1]}"은 base dir ${excludePattern[4]} 아래 포함 파일 ${excludePattern[3]}개 중 ${excludePattern[2]}개를 제외했습니다.`;
  }

  return null;
}

const MESSAGE_TRANSLATIONS = new Map([
  ["Config file could not be parsed", "설정 파일을 파싱할 수 없음"],
  ["compilerOptions.paths must be an object", "compilerOptions.paths는 object여야 함"],
  ["Path alias target does not exist", "경로 alias 대상이 존재하지 않음"],
  ["Include pattern does not match any files", "include pattern이 어떤 파일과도 일치하지 않음"],
  ["Exclude pattern does not filter any included files", "exclude pattern이 포함 파일을 제외하지 않음"],
  ["Output directory overlaps the TypeScript source root", "출력 디렉터리가 TypeScript source root와 겹침"],
  ["Output directory is nested inside the TypeScript source root", "출력 디렉터리가 TypeScript source root 안에 있음"],
  ["Output directory contains TypeScript input files", "출력 디렉터리에 TypeScript 입력 파일이 포함됨"],
  ["Output directory contains the TypeScript source root", "출력 디렉터리가 TypeScript source root를 포함함"],
  ["Missing .env.example contract", ".env.example 계약 파일 누락"],
  ["Missing example env file", "예시 env 파일 누락"],
  ["Invalid env syntax", "env 문법이 올바르지 않음"],
  ["Local env overrides detected", "local env override 감지됨"],
  ["Declared env contract is not satisfied locally", "선언된 env 계약이 로컬에서 충족되지 않음"],
  ["pnpm-workspace.yaml could not be parsed", "pnpm-workspace.yaml을 파싱할 수 없음"],
  [
    "pnpm-workspace.yaml does not declare any package patterns",
    "pnpm-workspace.yaml이 package pattern을 선언하지 않음",
  ],
  ["turbo.json could not be parsed", "turbo.json을 파싱할 수 없음"],
  ["turbo.json does not declare any workspace tasks", "turbo.json이 workspace task를 선언하지 않음"],
  ["ESLint formatting rules may conflict with Prettier", "ESLint 서식 규칙이 Prettier와 충돌할 수 있음"],
  [
    "Formatting-oriented ESLint rules were found, but no explicit Prettier bridge was detected.",
    "서식 중심 ESLint 규칙이 발견됐지만 명시적인 Prettier 연결은 감지되지 않았습니다.",
  ],
  [
    "Consider eslint-config-prettier or plugin:prettier/recommended to reduce formatter churn.",
    "포매터 변경 소음을 줄이려면 eslint-config-prettier 또는 plugin:prettier/recommended를 검토하세요.",
  ],
  ["ESLint and Prettier are configured separately", "ESLint와 Prettier가 별도로 설정됨"],
  [
    "That can be fine, but teams often prefer an explicit integration strategy.",
    "문제 없을 수도 있지만, 팀에서는 명시적인 통합 전략을 선호하는 경우가 많습니다.",
  ],
  [
    "Document which tool owns formatting and which tool owns code-quality rules.",
    "서식은 어느 도구가 맡고 코드 품질 규칙은 어느 도구가 맡는지 문서화하세요.",
  ],
  ["Legacy and flat ESLint configs coexist", "legacy ESLint 설정과 flat 설정이 함께 존재함"],
  [
    "This directory contains both legacy .eslintrc.* files and flat eslint.config.* files, so ESLint can resolve different rule sets depending on the entry point.",
    "이 디렉터리에는 legacy .eslintrc.* 파일과 flat eslint.config.* 파일이 함께 있어 진입점에 따라 ESLint가 서로 다른 규칙 집합을 해석할 수 있습니다.",
  ],
  [
    "Migrate to eslint.config.* as the single source of truth, then remove the legacy .eslintrc.* files after the new config fully replaces them.",
    "eslint.config.*를 단일 기준으로 마이그레이션한 뒤, 새 config가 완전히 대체하면 legacy .eslintrc.* 파일을 제거하세요.",
  ],
  ["EditorConfig and Prettier disagree", "EditorConfig와 Prettier 설정이 일치하지 않음"],
  ["Ignore files disagree on generated artifact coverage", "ignore 파일들의 generated artifact 범위가 일치하지 않음"],
  ["Multiple lockfiles are present in one directory", "한 디렉터리에 여러 lockfile이 존재함"],
  ["Package tsconfig drifts from the shared base", "package tsconfig가 shared base와 달라짐"],
  ["Jest and Vitest configs coexist", "Jest와 Vitest config가 함께 존재함"],
  [
    "Node engine support and GitHub Actions matrix are out of sync",
    "Node engine 지원 범위와 GitHub Actions matrix가 동기화되지 않음",
  ],
  [
    "Package ESM type conflicts with tsconfig module output",
    "package ESM type이 tsconfig module output과 충돌함",
  ],
  [
    "Package CommonJS type conflicts with tsconfig module output",
    "package CommonJS type이 tsconfig module output과 충돌함",
  ],
  ["Package entrypoint target is invalid", "package entrypoint 대상이 올바르지 않음"],
  ["Package entrypoint target does not exist", "package entrypoint 대상이 존재하지 않음"],
  ["Package entrypoint key is invalid", "package entrypoint key가 올바르지 않음"],
  ["Package exports object is invalid", "package exports object가 올바르지 않음"],
  [
    "Package entrypoint target uses an incompatible file type",
    "package entrypoint 대상이 호환되지 않는 파일 타입을 사용함",
  ],
  [
    "package.json files entry is excluded by nested .npmignore",
    "package.json files 항목이 nested .npmignore에 의해 제외됨",
  ],
  ["references must be an array", "references는 array여야 함"],
  [
    "Each project reference entry must be an object with a path",
    "각 project reference 항목은 path가 있는 object여야 함",
  ],
  [
    "Each project reference entry must declare a string path",
    "각 project reference 항목은 string path를 선언해야 함",
  ],
  ["Project reference target does not exist", "project reference 대상이 존재하지 않음"],
  ["Project reference target could not be read", "project reference 대상을 읽을 수 없음"],
  ["Project reference target could not be parsed", "project reference 대상을 파싱할 수 없음"],
  [
    "Project reference target must point to a tsconfig file",
    "project reference 대상은 tsconfig 파일을 가리켜야 함",
  ],
  ["Referenced project must enable composite", "참조된 project는 composite을 활성화해야 함"],
  ["Configured typeRoots entry does not exist", "설정된 typeRoots 항목이 존재하지 않음"],
  [
    "compilerOptions.types and typeRoots both narrow ambient type resolution",
    "compilerOptions.types와 typeRoots가 모두 ambient type 해석 범위를 좁힘",
  ],
  [
    "compilerOptions.typeRoots disables default @types discovery",
    "compilerOptions.typeRoots가 기본 @types discovery를 비활성화함",
  ],
  ["\"compilerOptions.types\" must be an array of package names", "\"compilerOptions.types\"는 package name array여야 함"],
  [
    "\"compilerOptions.types\" contains a non-string package name",
    "\"compilerOptions.types\"에 string이 아닌 package name이 포함됨",
  ],
  [
    "\"compilerOptions.typeRoots\" must be an array of directory paths",
    "\"compilerOptions.typeRoots\"는 directory path array여야 함",
  ],
  [
    "\"compilerOptions.typeRoots\" contains a non-string path",
    "\"compilerOptions.typeRoots\"에 string이 아닌 path가 포함됨",
  ],
  ["\"files\" entries must point to explicit files", "\"files\" 항목은 명시적인 파일을 가리켜야 함"],
  ["\"files\" entries must point to readable files", "\"files\" 항목은 읽을 수 있는 파일을 가리켜야 함"],
  ["\"files\" entries must point to files", "\"files\" 항목은 파일을 가리켜야 함"],
  [
    "\"files\" entries must point to supported TypeScript input files",
    "\"files\" 항목은 지원되는 TypeScript 입력 파일을 가리켜야 함",
  ],
  ["\"files\" entries must point to existing files", "\"files\" 항목은 존재하는 파일을 가리켜야 함"],
  ["Inherited tsconfig could not be found", "상속된 tsconfig를 찾을 수 없음"],
  ["Inherited tsconfig extends cycle detected", "상속된 tsconfig extends cycle이 감지됨"],
  ["Inherited tsconfig could not be read", "상속된 tsconfig를 읽을 수 없음"],
  ["Inherited tsconfig could not be parsed", "상속된 tsconfig를 파싱할 수 없음"],
  ["Inherited config must point to a tsconfig file", "상속된 config는 tsconfig 파일을 가리켜야 함"],
  [
    "Referenced project must set compilerOptions.composite to a boolean",
    "참조된 project는 compilerOptions.composite을 boolean으로 설정해야 함",
  ],
  [
    "Inherited tsconfig must set compilerOptions.composite to a boolean",
    "상속된 tsconfig는 compilerOptions.composite을 boolean으로 설정해야 함",
  ],
  ["Runtime env files exist, but .env.example is missing.", "실행 env 파일은 있지만 .env.example이 없습니다."],
  [
    "The file uses a pnpm-workspace.yaml shape that this check does not understand.",
    "이 파일은 이 check가 이해하지 못하는 pnpm-workspace.yaml 형태를 사용합니다.",
  ],
  [
    'Use a simple packages: block list such as packages: ["apps/*", "packages/*"].',
    'packages: ["apps/*", "packages/*"]처럼 단순한 packages: block list를 사용하세요.',
  ],
  [
    "No package globs were found under packages:, and this repo has at most one package file, so the workspace file looks like a placeholder.",
    "packages: 아래에 package glob이 없고 이 repo에는 package file이 최대 1개라 workspace 파일이 placeholder처럼 보입니다.",
  ],
  [
    "No package globs were found under packages:, so workspace packages are not declared yet.",
    "packages: 아래에 package glob이 없어 workspace package가 아직 선언되지 않았습니다.",
  ],
  [
    "Add a packages: block with one or more workspace globs, or remove the file until the repo actually needs a workspace definition.",
    "하나 이상의 workspace glob이 있는 packages: block을 추가하거나, 실제 workspace 정의가 필요할 때까지 파일을 제거하세요.",
  ],
  ["The turbo.json file is not valid JSONC.", "turbo.json 파일이 올바른 JSONC가 아닙니다."],
  [
    "Fix the syntax error or replace the placeholder file with a real Turbo config.",
    "문법 오류를 고치거나 placeholder 파일을 실제 Turbo config로 바꾸세요.",
  ],
  [
    "The file does not contain any task definitions that would make the workspace config meaningful.",
    "이 파일에는 workspace config를 의미 있게 만드는 task 정의가 없습니다.",
  ],
  [
    "Add a non-empty tasks or pipeline map, or remove the placeholder turbo.json until the workspace needs it.",
    "비어 있지 않은 tasks 또는 pipeline map을 추가하거나, workspace가 필요할 때까지 placeholder turbo.json을 제거하세요.",
  ],
  ["A committed .env.example file is missing.", "커밋된 .env.example 파일이 없습니다."],
  [
    "Current config surface looks healthy. Keep shared rules centralized as the repo grows.",
    "현재 설정 표면은 정상입니다. repo가 커져도 shared rule을 중앙에 유지하세요.",
  ],
  [
    "Introduce a shared tsconfig.base.json so packages inherit one source of truth.",
    "package들이 단일 기준을 상속하도록 shared tsconfig.base.json을 도입하세요.",
  ],
  [
    "Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets.",
    "package별로 다른 규칙 집합이 꼭 필요하지 않다면 repo 전체 ESLint 진입점을 줄이세요.",
  ],
  [
    "Use .env.example files consistently so onboarding does not depend on tribal knowledge.",
    ".env.example 파일을 일관되게 사용해 온보딩이 구두 지식에 의존하지 않게 하세요.",
  ],
  [
    "Use shell-style env syntax or move comments to their own line.",
    "shell-style env 문법을 사용하거나 주석을 별도 줄로 옮기세요.",
  ],
  [
    "Protect concrete env files with an exact .gitignore entry before committing secrets.",
    "secret을 커밋하기 전에 구체 env 파일을 정확한 .gitignore 항목으로 보호하세요.",
  ],
  ["Run \"maximus fix\" to create a blank contract file.", "\"maximus fix\"를 실행해 빈 계약 파일을 생성하세요."],
  [
    "Replace the value with a blank or placeholder string before sharing the repo.",
    "repo를 공유하기 전에 값을 빈 문자열이나 placeholder로 바꾸세요.",
  ],
  [
    "Make sure local-only overrides are intentional and documented in .env.example.",
    "local-only override가 의도된 것이며 .env.example에 문서화되어 있는지 확인하세요.",
  ],
  [
    "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files.",
    "CI에서 주입되는 값이면 계약을 문서화하세요. 아니면 로컬 env 파일에 추가하세요.",
  ],
  ["Fix invalid JSONC syntax before relying on this config.", "이 config를 신뢰하기 전에 올바르지 않은 JSONC 문법을 고치세요."],
  ["Remove legacy flags before they become upgrade blockers.", "upgrade blocker가 되기 전에 legacy flag를 제거하세요."],
  ["Rewrite paths to the standard { alias: [targets] } shape.", "paths를 표준 { alias: [targets] } 형태로 다시 작성하세요."],
  ["Add a valid target or remove the alias entry.", "유효한 대상을 추가하거나 alias 항목을 제거하세요."],
  ["Replace non-string entries with valid path strings.", "string이 아닌 항목을 유효한 path string으로 교체하세요."],
  ["Keep wildcard placement aligned so imports resolve predictably.", "import가 예측 가능하게 해석되도록 wildcard 위치를 맞추세요."],
  [
    "Update or remove stale aliases before they break editor and build resolution.",
    "editor와 build 해석을 깨기 전에 오래된 alias를 수정하거나 제거하세요.",
  ],
  [
    "Align both alias surfaces so runtime and editor resolution stay consistent.",
    "runtime과 editor 해석이 일치하도록 두 alias 표면을 맞추세요.",
  ],
  [
    "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command.",
    "이 디렉터리는 Jest와 Vitest config를 모두 선언하므로 명령에 따라 서로 다른 환경에서 test가 실행될 수 있습니다.",
  ],
  [
    "Pick one runner for this package, or document the split with separate config ownership and scripts.",
    "이 package의 runner 하나를 선택하거나, 별도 config ownership과 script로 분리를 문서화하세요.",
  ],
  [
    "Align EditorConfig and Prettier so editor saves do not fight formatter output.",
    "편집기 저장과 포매터 출력이 충돌하지 않도록 EditorConfig와 Prettier를 맞추세요.",
  ],
  [
    "Keep one lockfile per directory so dependency resolution stays predictable. Separate package directories can each have their own lockfile.",
    "dependency 해석이 예측 가능하도록 디렉터리마다 lockfile을 하나만 유지하세요. 별도 package 디렉터리는 각자 lockfile을 가질 수 있습니다.",
  ],
  [
    "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
    "TypeScript가 예상 입력을 조용히 건너뛰기 전에 빈 include pattern을 수정하거나 제거하세요.",
  ],
  [
    "Remove or tighten exclude entries that do not change the effective TypeScript input set.",
    "실제 TypeScript 입력 집합을 바꾸지 않는 exclude 항목을 제거하거나 좁히세요.",
  ],
  [
    "Next.js generates .next/types during development or build, so this include can be empty before .next exists.",
    "Next.js는 개발 또는 build 중 .next/types를 생성하므로 .next가 생기기 전에는 이 include가 비어 있을 수 있습니다.",
  ],
  [
    "Move emit output outside the source root so build artifacts do not overwrite source files.",
    "build artifact가 source file을 덮어쓰지 않도록 emit output을 source root 밖으로 옮기세요.",
  ],
  [
    "Move emit output outside any directory that currently contains TypeScript input files.",
    "현재 TypeScript 입력 파일이 들어 있는 디렉터리 밖으로 emit output을 옮기세요.",
  ],
  [
    "Prefer an output directory that is completely separate from the TypeScript source root.",
    "TypeScript source root와 완전히 분리된 output directory를 사용하세요.",
  ],
  ["Create .env.example with safe defaults.", "안전한 기본값으로 .env.example을 생성하세요."],
  ["Update the alias to an existing directory.", "alias를 존재하는 디렉터리로 수정하세요."],
  ["No extra work is needed.", "추가 작업은 필요하지 않습니다."],
  ["Package scripts are tidy", "package script가 정리되어 있음"],
]);
