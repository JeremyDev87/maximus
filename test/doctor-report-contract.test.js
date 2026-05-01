import assert from "node:assert/strict";
import test from "node:test";

import { formatDoctorReport } from "../src/core/format-report.js";

test("JS doctor formatter includes the Korean top-priority section", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "blocking issues",
      fixesAvailable: 1,
    },
    structure: {
      isMonorepo: false,
      packageCount: 1,
      configFiles: 2,
      envDirectories: 1,
      recommendations: [],
    },
    findings: [
      {
        severity: "error",
        title: "Missing example env file",
        file: "/tmp/project/.env",
        detail: "A committed .env.example file is missing.",
        hint: "Create .env.example with safe defaults.",
        fixable: false,
      },
      {
        severity: "warn",
        title: "Path alias target does not exist",
        file: "/tmp/project/tsconfig.json",
        detail: "@app/* points to src/missing/*.",
        hint: "Update the alias to an existing directory.",
        fixable: false,
      },
      {
        severity: "info",
        title: "Package scripts are tidy",
        file: null,
        detail: "No extra work is needed.",
        hint: "",
        fixable: false,
      },
    ],
  });

  assert.match(report, /상위 3개 우선순위/);
  assert.match(report, /1\. \[오류\] 예시 env 파일 누락/);
  assert.match(report, /   파일: \.env/);
  assert.match(report, /   다음: 안전한 기본값으로 \.env\.example을 생성하세요\./);
  assert.match(report, /2\. \[경고\] 경로 alias 대상이 존재하지 않음/);
  assert.match(report, /3\. \[정보\] package script가 정리되어 있음/);
});

test("JS formatter translates output-path finding titles like Rust", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "blocking issues",
      fixesAvailable: 0,
    },
    structure: {
      isMonorepo: false,
      packageCount: 1,
      configFiles: 1,
      envDirectories: 0,
      recommendations: [],
    },
    findings: [
      {
        severity: "error",
        title: "Output directory overlaps the TypeScript source root",
        file: "/tmp/project/tsconfig.json",
        detail: 'outDir "src" overlaps source root "src".',
        hint: "Move emit output outside the source root so build artifacts do not overwrite source files.",
        fixable: false,
      },
    ],
  });

  assert.match(report, /출력 디렉터리가 TypeScript source root와 겹침/);
  assert.match(report, /outDir "src"이 source root "src"와 겹칩니다\./);
  assert.match(report, /build artifact가 source file을 덮어쓰지 않도록/);
});

test("JS formatter translates dynamic missing concrete env details", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "attention needed",
      fixesAvailable: 0,
    },
    structure: {
      isMonorepo: false,
      packageCount: 1,
      configFiles: 1,
      envDirectories: 1,
      recommendations: [],
    },
    findings: [
      {
        severity: "warn",
        title: "Declared env contract is not satisfied locally",
        file: "/tmp/project/.env.example",
        detail: "No concrete value was found for: CI_ONLY.",
        hint: "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files.",
        fixable: false,
      },
    ],
  });

  assert.match(report, /선언된 env 계약이 로컬에서 충족되지 않음/);
  assert.match(report, /다음 env key에 대한 구체 값을 찾을 수 없습니다: CI_ONLY\./);
  assert.doesNotMatch(report, /No concrete value was found/);
});

test("JS formatter translates dynamic env sync details", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "attention needed",
      fixesAvailable: 1,
    },
    structure: {
      isMonorepo: false,
      packageCount: 1,
      configFiles: 1,
      envDirectories: 1,
      recommendations: [],
    },
    findings: [
      {
        severity: "warn",
        title: ".env.example is missing keys",
        file: "/tmp/project/.env.example",
        detail: "Missing keys: OTHER.",
        hint: 'Run "maximus fix" to append the missing keys to .env.example.',
        fixable: true,
      },
    ],
  });

  assert.match(report, /\.env\.example에 누락된 key가 있음/);
  assert.match(report, /누락된 key: OTHER\./);
  assert.match(report, /"maximus fix"를 실행해 \.env\.example에 누락된 key를 추가하세요\./);
  assert.doesNotMatch(report, /Missing keys/);
  assert.doesNotMatch(report, /append the missing keys/);
});

test("JS formatter translates workspace runner and EditorConfig human messages", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "attention needed",
      fixesAvailable: 0,
    },
    structure: {
      isMonorepo: true,
      packageCount: 2,
      configFiles: 4,
      envDirectories: 0,
      recommendations: [],
    },
    findings: [
      {
        severity: "warn",
        title: "pnpm-workspace.yaml does not declare any package patterns",
        file: "/tmp/project/pnpm-workspace.yaml",
        detail: "No package globs were found under packages:, so workspace packages are not declared yet.",
        hint: "Add a packages: block with one or more workspace globs, or remove the file until the repo actually needs a workspace definition.",
        fixable: false,
      },
      {
        severity: "warn",
        title: "Jest and Vitest configs coexist",
        file: "/tmp/project/package.json",
        detail:
          "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command.",
        hint: "Pick one runner for this package, or document the split with separate config ownership and scripts.",
        fixable: false,
      },
      {
        severity: "warn",
        title: "EditorConfig and Prettier disagree",
        file: "/tmp/project/.editorconfig",
        detail:
          "EditorConfig sets indent_style=tab, indent_size=4, end_of_line=crlf, but Prettier sets useTabs=false, tabWidth=2, endOfLine=lf.",
        hint: "Align EditorConfig and Prettier so editor saves do not fight formatter output.",
        fixable: false,
      },
    ],
  });

  assert.match(report, /pnpm-workspace\.yaml이 package pattern을 선언하지 않음/);
  assert.match(report, /workspace package가 아직 선언되지 않았습니다/);
  assert.match(report, /Jest와 Vitest config가 함께 존재함/);
  assert.match(report, /명령에 따라 서로 다른 환경에서 test가 실행될 수 있습니다/);
  assert.match(report, /EditorConfig와 Prettier 설정이 일치하지 않음/);
  assert.match(report, /EditorConfig는 indent_style=tab, indent_size=4, end_of_line=crlf를 설정하지만/);
  assert.match(report, /편집기 저장과 포매터 출력이 충돌하지 않도록/);
  assert.doesNotMatch(report, /does not declare any package patterns/);
  assert.doesNotMatch(report, /This directory declares both Jest and Vitest/);
  assert.doesNotMatch(report, /EditorConfig sets/);
  assert.doesNotMatch(report, /formatter output/);
});

test("JS formatter translates duplicate config and structure guidance", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "blocking issues",
      fixesAvailable: 0,
    },
    structure: {
      isMonorepo: true,
      packageCount: 2,
      configFiles: 4,
      envDirectories: 0,
      recommendations: [
        "Introduce a shared tsconfig.base.json so packages inherit one source of truth.",
        "Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets.",
      ],
    },
    findings: [
      {
        severity: "error",
        title: "ESLint config is declared in multiple places",
        file: "/tmp/project/package.json",
        detail: "Found 2 ESLint config sources in ..",
        hint: "Keep a single ESLint entry point per directory to avoid drift.",
        fixable: false,
      },
      {
        severity: "error",
        title: "Legacy and flat ESLint configs coexist",
        file: "/tmp/project/.eslintrc.json",
        detail:
          "This directory contains both legacy .eslintrc.* files and flat eslint.config.* files, so ESLint can resolve different rule sets depending on the entry point.",
        hint:
          "Migrate to eslint.config.* as the single source of truth, then remove the legacy .eslintrc.* files after the new config fully replaces them.",
        fixable: false,
      },
    ],
  });

  assert.match(report, /ESLint 설정이 여러 위치에 선언됨/);
  assert.match(report, /\.에서 ESLint 설정 출처 2개를 찾았습니다/);
  assert.match(report, /차이를 피하려면 디렉터리마다 ESLint 진입점을 하나만 유지하세요/);
  assert.match(report, /legacy ESLint 설정과 flat 설정이 함께 존재함/);
  assert.match(report, /eslint\.config\.\*를 단일 기준으로 마이그레이션/);
  assert.match(report, /shared tsconfig\.base\.json을 도입하세요/);
  assert.match(report, /repo 전체 ESLint 진입점을 줄이세요/);
  assert.doesNotMatch(report, /Found 2 ESLint config sources/);
  assert.doesNotMatch(report, /Keep a single ESLint/);
  assert.doesNotMatch(report, /Migrate to eslint\.config/);
  assert.doesNotMatch(report, /Reduce repo-wide ESLint/);
});
