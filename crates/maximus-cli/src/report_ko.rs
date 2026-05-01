use std::borrow::Cow;

use maximus_core::{Severity, StructureReport};

pub fn status_label(status: &str) -> Cow<'_, str> {
    match status {
        "clean" => Cow::Borrowed("정상"),
        "attention needed" => Cow::Borrowed("조치 필요"),
        "blocking issues" => Cow::Borrowed("차단 이슈 있음"),
        _ => Cow::Borrowed(status),
    }
}

pub fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "오류",
        Severity::Warn => "경고",
        Severity::Info => "정보",
    }
}

pub fn describe_structure(structure: &StructureReport) -> String {
    let repo_type = if structure.is_monorepo {
        "모노레포"
    } else {
        "단일 패키지"
    };

    format!(
        "{repo_type}, 패키지 {}개, 설정 파일 {}개, env 폴더 {}개",
        structure.package_count, structure.config_files, structure.env_directories
    )
}

pub fn message(value: &str) -> Cow<'_, str> {
    if let Some(translated) = dynamic_message(value) {
        return Cow::Owned(translated);
    }

    match value {
        "Config file could not be parsed" => Cow::Borrowed("설정 파일을 파싱할 수 없음"),
        "compilerOptions.paths must be an object" => {
            Cow::Borrowed("compilerOptions.paths는 object여야 함")
        }
        "Path alias target does not exist" => Cow::Borrowed("경로 alias 대상이 존재하지 않음"),
        "Include pattern does not match any files" => {
            Cow::Borrowed("include pattern이 어떤 파일과도 일치하지 않음")
        }
        "Exclude pattern does not filter any included files" => {
            Cow::Borrowed("exclude pattern이 포함 파일을 제외하지 않음")
        }
        "Output directory overlaps the TypeScript source root" => {
            Cow::Borrowed("출력 디렉터리가 TypeScript source root와 겹침")
        }
        "Output directory is nested inside the TypeScript source root" => {
            Cow::Borrowed("출력 디렉터리가 TypeScript source root 안에 있음")
        }
        "Output directory contains TypeScript input files" => {
            Cow::Borrowed("출력 디렉터리에 TypeScript 입력 파일이 포함됨")
        }
        "Output directory contains the TypeScript source root" => {
            Cow::Borrowed("출력 디렉터리가 TypeScript source root를 포함함")
        }
        "Missing .env.example contract" => Cow::Borrowed(".env.example 계약 파일 누락"),
        "Missing example env file" => Cow::Borrowed("예시 env 파일 누락"),
        "Invalid env syntax" => Cow::Borrowed("env 문법이 올바르지 않음"),
        "Local env overrides detected" => Cow::Borrowed("local env override 감지됨"),
        "Declared env contract is not satisfied locally" => {
            Cow::Borrowed("선언된 env 계약이 로컬에서 충족되지 않음")
        }
        "pnpm-workspace.yaml could not be parsed" => {
            Cow::Borrowed("pnpm-workspace.yaml을 파싱할 수 없음")
        }
        "pnpm-workspace.yaml does not declare any package patterns" => {
            Cow::Borrowed("pnpm-workspace.yaml이 package pattern을 선언하지 않음")
        }
        "turbo.json could not be parsed" => Cow::Borrowed("turbo.json을 파싱할 수 없음"),
        "turbo.json does not declare any workspace tasks" => {
            Cow::Borrowed("turbo.json이 workspace task를 선언하지 않음")
        }
        "ESLint formatting rules may conflict with Prettier" => {
            Cow::Borrowed("ESLint 서식 규칙이 Prettier와 충돌할 수 있음")
        }
        "Formatting-oriented ESLint rules were found, but no explicit Prettier bridge was detected." => {
            Cow::Borrowed("서식 중심 ESLint 규칙이 발견됐지만 명시적인 Prettier 연결은 감지되지 않았습니다.")
        }
        "Consider eslint-config-prettier or plugin:prettier/recommended to reduce formatter churn." => {
            Cow::Borrowed("포매터 변경 소음을 줄이려면 eslint-config-prettier 또는 plugin:prettier/recommended를 검토하세요.")
        }
        "ESLint and Prettier are configured separately" => {
            Cow::Borrowed("ESLint와 Prettier가 별도로 설정됨")
        }
        "That can be fine, but teams often prefer an explicit integration strategy." => {
            Cow::Borrowed("문제 없을 수도 있지만, 팀에서는 명시적인 통합 전략을 선호하는 경우가 많습니다.")
        }
        "Document which tool owns formatting and which tool owns code-quality rules." => {
            Cow::Borrowed("서식은 어느 도구가 맡고 코드 품질 규칙은 어느 도구가 맡는지 문서화하세요.")
        }
        "Legacy and flat ESLint configs coexist" => {
            Cow::Borrowed("legacy ESLint 설정과 flat 설정이 함께 존재함")
        }
        "This directory contains both legacy .eslintrc.* files and flat eslint.config.* files, so ESLint can resolve different rule sets depending on the entry point." => {
            Cow::Borrowed("이 디렉터리에는 legacy .eslintrc.* 파일과 flat eslint.config.* 파일이 함께 있어 진입점에 따라 ESLint가 서로 다른 규칙 집합을 해석할 수 있습니다.")
        }
        "Migrate to eslint.config.* as the single source of truth, then remove the legacy .eslintrc.* files after the new config fully replaces them." => {
            Cow::Borrowed("eslint.config.*를 단일 기준으로 마이그레이션한 뒤, 새 config가 완전히 대체하면 legacy .eslintrc.* 파일을 제거하세요.")
        }
        "EditorConfig and Prettier disagree" => {
            Cow::Borrowed("EditorConfig와 Prettier 설정이 일치하지 않음")
        }
        "Ignore files disagree on generated artifact coverage" => {
            Cow::Borrowed("ignore 파일들의 generated artifact 범위가 일치하지 않음")
        }
        "Multiple lockfiles are present in one directory" => {
            Cow::Borrowed("한 디렉터리에 여러 lockfile이 존재함")
        }
        "Package tsconfig drifts from the shared base" => {
            Cow::Borrowed("package tsconfig가 shared base와 달라짐")
        }
        "Jest and Vitest configs coexist" => Cow::Borrowed("Jest와 Vitest config가 함께 존재함"),
        "Node engine support and GitHub Actions matrix are out of sync" => {
            Cow::Borrowed("Node engine 지원 범위와 GitHub Actions matrix가 동기화되지 않음")
        }
        "Package ESM type conflicts with tsconfig module output" => {
            Cow::Borrowed("package ESM type이 tsconfig module output과 충돌함")
        }
        "Package CommonJS type conflicts with tsconfig module output" => {
            Cow::Borrowed("package CommonJS type이 tsconfig module output과 충돌함")
        }
        "Package entrypoint target is invalid" => Cow::Borrowed("package entrypoint 대상이 올바르지 않음"),
        "Package entrypoint target does not exist" => {
            Cow::Borrowed("package entrypoint 대상이 존재하지 않음")
        }
        "Package entrypoint key is invalid" => Cow::Borrowed("package entrypoint key가 올바르지 않음"),
        "Package exports object is invalid" => Cow::Borrowed("package exports object가 올바르지 않음"),
        "Package entrypoint target uses an incompatible file type" => {
            Cow::Borrowed("package entrypoint 대상이 호환되지 않는 파일 타입을 사용함")
        }
        "package.json files entry is excluded by nested .npmignore" => {
            Cow::Borrowed("package.json files 항목이 nested .npmignore에 의해 제외됨")
        }
        "references must be an array" => Cow::Borrowed("references는 array여야 함"),
        "Each project reference entry must be an object with a path" => {
            Cow::Borrowed("각 project reference 항목은 path가 있는 object여야 함")
        }
        "Each project reference entry must declare a string path" => {
            Cow::Borrowed("각 project reference 항목은 string path를 선언해야 함")
        }
        "Project reference target does not exist" => {
            Cow::Borrowed("project reference 대상이 존재하지 않음")
        }
        "Project reference target could not be read" => {
            Cow::Borrowed("project reference 대상을 읽을 수 없음")
        }
        "Project reference target could not be parsed" => {
            Cow::Borrowed("project reference 대상을 파싱할 수 없음")
        }
        "Project reference target must point to a tsconfig file" => {
            Cow::Borrowed("project reference 대상은 tsconfig 파일을 가리켜야 함")
        }
        "Referenced project must enable composite" => {
            Cow::Borrowed("참조된 project는 composite을 활성화해야 함")
        }
        "Configured typeRoots entry does not exist" => {
            Cow::Borrowed("설정된 typeRoots 항목이 존재하지 않음")
        }
        "compilerOptions.types and typeRoots both narrow ambient type resolution" => {
            Cow::Borrowed("compilerOptions.types와 typeRoots가 모두 ambient type 해석 범위를 좁힘")
        }
        "compilerOptions.typeRoots disables default @types discovery" => {
            Cow::Borrowed("compilerOptions.typeRoots가 기본 @types discovery를 비활성화함")
        }
        "\"compilerOptions.types\" must be an array of package names" => {
            Cow::Borrowed("\"compilerOptions.types\"는 package name array여야 함")
        }
        "\"compilerOptions.types\" contains a non-string package name" => {
            Cow::Borrowed("\"compilerOptions.types\"에 string이 아닌 package name이 포함됨")
        }
        "\"compilerOptions.typeRoots\" must be an array of directory paths" => {
            Cow::Borrowed("\"compilerOptions.typeRoots\"는 directory path array여야 함")
        }
        "\"compilerOptions.typeRoots\" contains a non-string path" => {
            Cow::Borrowed("\"compilerOptions.typeRoots\"에 string이 아닌 path가 포함됨")
        }
        "\"files\" entries must point to explicit files" => {
            Cow::Borrowed("\"files\" 항목은 명시적인 파일을 가리켜야 함")
        }
        "\"files\" entries must point to readable files" => {
            Cow::Borrowed("\"files\" 항목은 읽을 수 있는 파일을 가리켜야 함")
        }
        "\"files\" entries must point to files" => Cow::Borrowed("\"files\" 항목은 파일을 가리켜야 함"),
        "\"files\" entries must point to supported TypeScript input files" => {
            Cow::Borrowed("\"files\" 항목은 지원되는 TypeScript 입력 파일을 가리켜야 함")
        }
        "\"files\" entries must point to existing files" => {
            Cow::Borrowed("\"files\" 항목은 존재하는 파일을 가리켜야 함")
        }
        "Inherited tsconfig could not be found" => Cow::Borrowed("상속된 tsconfig를 찾을 수 없음"),
        "Inherited tsconfig extends cycle detected" => {
            Cow::Borrowed("상속된 tsconfig extends cycle이 감지됨")
        }
        "Inherited tsconfig could not be read" => Cow::Borrowed("상속된 tsconfig를 읽을 수 없음"),
        "Inherited tsconfig could not be parsed" => Cow::Borrowed("상속된 tsconfig를 파싱할 수 없음"),
        "Inherited config must point to a tsconfig file" => {
            Cow::Borrowed("상속된 config는 tsconfig 파일을 가리켜야 함")
        }
        "Referenced project must set compilerOptions.composite to a boolean" => {
            Cow::Borrowed("참조된 project는 compilerOptions.composite을 boolean으로 설정해야 함")
        }
        "Inherited tsconfig must set compilerOptions.composite to a boolean" => {
            Cow::Borrowed("상속된 tsconfig는 compilerOptions.composite을 boolean으로 설정해야 함")
        }
        "Runtime env files exist, but .env.example is missing." => {
            Cow::Borrowed("실행 env 파일은 있지만 .env.example이 없습니다.")
        }
        "The file uses a pnpm-workspace.yaml shape that this check does not understand." => {
            Cow::Borrowed("이 파일은 이 check가 이해하지 못하는 pnpm-workspace.yaml 형태를 사용합니다.")
        }
        "Use a simple packages: block list such as packages: [\"apps/*\", \"packages/*\"]." => {
            Cow::Borrowed("packages: [\"apps/*\", \"packages/*\"]처럼 단순한 packages: block list를 사용하세요.")
        }
        "No package globs were found under packages:, and this repo has at most one package file, so the workspace file looks like a placeholder." => {
            Cow::Borrowed("packages: 아래에 package glob이 없고 이 repo에는 package file이 최대 1개라 workspace 파일이 placeholder처럼 보입니다.")
        }
        "No package globs were found under packages:, so workspace packages are not declared yet." => {
            Cow::Borrowed("packages: 아래에 package glob이 없어 workspace package가 아직 선언되지 않았습니다.")
        }
        "Add a packages: block with one or more workspace globs, or remove the file until the repo actually needs a workspace definition." => {
            Cow::Borrowed("하나 이상의 workspace glob이 있는 packages: block을 추가하거나, 실제 workspace 정의가 필요할 때까지 파일을 제거하세요.")
        }
        "The turbo.json file is not valid JSONC." => {
            Cow::Borrowed("turbo.json 파일이 올바른 JSONC가 아닙니다.")
        }
        "Fix the syntax error or replace the placeholder file with a real Turbo config." => {
            Cow::Borrowed("문법 오류를 고치거나 placeholder 파일을 실제 Turbo config로 바꾸세요.")
        }
        "The file does not contain any task definitions that would make the workspace config meaningful." => {
            Cow::Borrowed("이 파일에는 workspace config를 의미 있게 만드는 task 정의가 없습니다.")
        }
        "Add a non-empty tasks or pipeline map, or remove the placeholder turbo.json until the workspace needs it." => {
            Cow::Borrowed("비어 있지 않은 tasks 또는 pipeline map을 추가하거나, workspace가 필요할 때까지 placeholder turbo.json을 제거하세요.")
        }
        "Contract files should describe the interface, not ship concrete secrets." => {
            Cow::Borrowed("계약 파일은 interface를 설명해야 하며 실제 secret을 담으면 안 됩니다.")
        }
        "A committed .env.example file is missing." => {
            Cow::Borrowed("커밋된 .env.example 파일이 없습니다.")
        }
        "TypeScript expects alias keys mapped to arrays of target strings." => {
            Cow::Borrowed("TypeScript는 alias key가 대상 string array에 매핑되기를 기대합니다.")
        }
        "Each path alias should map to at least one target string." => {
            Cow::Borrowed("각 path alias는 최소 하나의 대상 string에 매핑되어야 합니다.")
        }
        "TypeScript path targets must be strings." => {
            Cow::Borrowed("TypeScript path target은 string이어야 합니다.")
        }
        "Current config surface looks healthy. Keep shared rules centralized as the repo grows." => {
            Cow::Borrowed("현재 설정 표면은 정상입니다. repo가 커져도 shared rule을 중앙에 유지하세요.")
        }
        "Introduce a shared tsconfig.base.json so packages inherit one source of truth." => {
            Cow::Borrowed("package들이 단일 기준을 상속하도록 shared tsconfig.base.json을 도입하세요.")
        }
        "Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets." => {
            Cow::Borrowed("package별로 다른 규칙 집합이 꼭 필요하지 않다면 repo 전체 ESLint 진입점을 줄이세요.")
        }
        "Use .env.example files consistently so onboarding does not depend on tribal knowledge." => {
            Cow::Borrowed(".env.example 파일을 일관되게 사용해 온보딩이 구두 지식에 의존하지 않게 하세요.")
        }
        "Use shell-style env syntax or move comments to their own line." => {
            Cow::Borrowed("shell-style env 문법을 사용하거나 주석을 별도 줄로 옮기세요.")
        }
        "Protect concrete env files with an exact .gitignore entry before committing secrets." => {
            Cow::Borrowed("secret을 커밋하기 전에 구체 env 파일을 정확한 .gitignore 항목으로 보호하세요.")
        }
        "Run \"maximus fix\" to create a blank contract file." => {
            Cow::Borrowed("\"maximus fix\"를 실행해 빈 계약 파일을 생성하세요.")
        }
        "Replace the value with a blank or placeholder string before sharing the repo." => {
            Cow::Borrowed("repo를 공유하기 전에 값을 빈 문자열이나 placeholder로 바꾸세요.")
        }
        "Make sure local-only overrides are intentional and documented in .env.example." => {
            Cow::Borrowed("local-only override가 의도된 것이며 .env.example에 문서화되어 있는지 확인하세요.")
        }
        "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files." => {
            Cow::Borrowed("CI에서 주입되는 값이면 계약을 문서화하세요. 아니면 로컬 env 파일에 추가하세요.")
        }
        "Fix invalid JSONC syntax before relying on this config." => {
            Cow::Borrowed("이 config를 신뢰하기 전에 올바르지 않은 JSONC 문법을 고치세요.")
        }
        "Remove legacy flags before they become upgrade blockers." => {
            Cow::Borrowed("upgrade blocker가 되기 전에 legacy flag를 제거하세요.")
        }
        "Rewrite paths to the standard { alias: [targets] } shape." => {
            Cow::Borrowed("paths를 표준 { alias: [targets] } 형태로 다시 작성하세요.")
        }
        "Add a valid target or remove the alias entry." => {
            Cow::Borrowed("유효한 대상을 추가하거나 alias 항목을 제거하세요.")
        }
        "Replace non-string entries with valid path strings." => {
            Cow::Borrowed("string이 아닌 항목을 유효한 path string으로 교체하세요.")
        }
        "Keep wildcard placement aligned so imports resolve predictably." => {
            Cow::Borrowed("import가 예측 가능하게 해석되도록 wildcard 위치를 맞추세요.")
        }
        "Update or remove stale aliases before they break editor and build resolution." => {
            Cow::Borrowed("editor와 build 해석을 깨기 전에 오래된 alias를 수정하거나 제거하세요.")
        }
        "Align both alias surfaces so runtime and editor resolution stay consistent." => {
            Cow::Borrowed("runtime과 editor 해석이 일치하도록 두 alias 표면을 맞추세요.")
        }
        "This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command." => {
            Cow::Borrowed("이 디렉터리는 Jest와 Vitest config를 모두 선언하므로 명령에 따라 서로 다른 환경에서 test가 실행될 수 있습니다.")
        }
        "Pick one runner for this package, or document the split with separate config ownership and scripts." => {
            Cow::Borrowed("이 package의 runner 하나를 선택하거나, 별도 config ownership과 script로 분리를 문서화하세요.")
        }
        "Align EditorConfig and Prettier so editor saves do not fight formatter output." => {
            Cow::Borrowed(
                "편집기 저장과 포매터 출력이 충돌하지 않도록 EditorConfig와 Prettier를 맞추세요.",
            )
        }
        "Keep one lockfile per directory so dependency resolution stays predictable. Separate package directories can each have their own lockfile." => {
            Cow::Borrowed("dependency 해석이 예측 가능하도록 디렉터리마다 lockfile을 하나만 유지하세요. 별도 package 디렉터리는 각자 lockfile을 가질 수 있습니다.")
        }
        "Fix or remove empty include patterns before TypeScript silently skips expected inputs." => {
            Cow::Borrowed("TypeScript가 예상 입력을 조용히 건너뛰기 전에 빈 include pattern을 수정하거나 제거하세요.")
        }
        "Remove or tighten exclude entries that do not change the effective TypeScript input set." => {
            Cow::Borrowed("실제 TypeScript 입력 집합을 바꾸지 않는 exclude 항목을 제거하거나 좁히세요.")
        }
        "Next.js generates .next/types during development or build, so this include can be empty before .next exists." => {
            Cow::Borrowed("Next.js는 개발 또는 build 중 .next/types를 생성하므로 .next가 생기기 전에는 이 include가 비어 있을 수 있습니다.")
        }
        "Move emit output outside the source root so build artifacts do not overwrite source files." => {
            Cow::Borrowed("build artifact가 source file을 덮어쓰지 않도록 emit output을 source root 밖으로 옮기세요.")
        }
        "Move emit output outside any directory that currently contains TypeScript input files." => {
            Cow::Borrowed("현재 TypeScript 입력 파일이 들어 있는 디렉터리 밖으로 emit output을 옮기세요.")
        }
        "Prefer an output directory that is completely separate from the TypeScript source root." => {
            Cow::Borrowed("TypeScript source root와 완전히 분리된 output directory를 사용하세요.")
        }
        "Create .env.example with safe defaults." => {
            Cow::Borrowed("안전한 기본값으로 .env.example을 생성하세요.")
        }
        "Update the alias to an existing directory." => {
            Cow::Borrowed("alias를 존재하는 디렉터리로 수정하세요.")
        }
        "No extra work is needed." => Cow::Borrowed("추가 작업은 필요하지 않습니다."),
        "Package scripts are tidy" => Cow::Borrowed("package script가 정리되어 있음"),
        _ => Cow::Borrowed(value),
    }
}

pub fn fix_title(value: &str) -> Cow<'_, str> {
    if let Some(rest) = value.strip_prefix("Create ") {
        return Cow::Owned(format!("{rest} 생성"));
    }
    if let Some(rest) = value.strip_prefix("Append missing keys to ") {
        return Cow::Owned(format!("{rest}에 누락된 키 추가"));
    }

    message(value)
}

fn dynamic_message(value: &str) -> Option<String> {
    quoted_pattern(
        value,
        "Concrete env file \"",
        "\" is not protected by .gitignore",
    )
    .map(|name| format!("구체 env 파일 \"{name}\"이 .gitignore로 보호되지 않음"))
    .or_else(|| {
        quoted_pattern(value, "Duplicate env key \"", "\"")
            .map(|key| format!("중복 env key \"{key}\""))
    })
    .or_else(|| {
        value
            .strip_prefix("No concrete value was found for: ")
            .and_then(|keys| keys.strip_suffix('.'))
            .map(|keys| format!("다음 env key에 대한 구체 값을 찾을 수 없습니다: {keys}."))
    })
    .or_else(|| {
        value
            .strip_prefix("Missing keys: ")
            .and_then(|keys| keys.strip_suffix('.'))
            .map(|keys| format!("누락된 key: {keys}."))
    })
    .or_else(|| {
        value
            .strip_prefix("Run \"maximus fix\" to append the missing keys to ")
            .and_then(|file| file.strip_suffix('.'))
            .map(|file| format!("\"maximus fix\"를 실행해 {file}에 누락된 key를 추가하세요."))
    })
    .or_else(|| {
        value
            .strip_prefix("EditorConfig sets ")
            .and_then(|rest| rest.split_once(", but Prettier sets "))
            .and_then(|(editor, prettier)| {
                prettier
                    .strip_suffix('.')
                    .map(|prettier| (editor, prettier))
            })
            .map(|(editor, prettier)| {
                format!("EditorConfig는 {editor}를 설정하지만 Prettier는 {prettier}를 설정합니다.")
            })
    })
    .or_else(|| {
        quoted_pattern(value, "Deprecated compiler option \"", "\"")
            .map(|option| format!("deprecated compiler option \"{option}\" 사용 중"))
    })
    .or_else(|| {
        quoted_pattern(value, "Alias \"", "\" does not declare any targets")
            .map(|alias| format!("alias \"{alias}\"가 대상을 선언하지 않음"))
    })
    .or_else(|| {
        quoted_pattern(value, "Alias \"", "\" contains a non-string target")
            .map(|alias| format!("alias \"{alias}\"에 string이 아닌 대상이 포함됨"))
    })
    .or_else(|| {
        quoted_pattern(value, "Wildcard shape does not match for alias \"", "\"")
            .map(|alias| format!("alias \"{alias}\"의 wildcard 형태가 일치하지 않음"))
    })
    .or_else(|| {
        quoted_pattern(value, "Path alias \"", "\" shadows a package import")
            .map(|alias| format!("path alias \"{alias}\"가 package import를 shadow함"))
    })
    .or_else(|| {
        quoted_pattern(value, "Vite alias \"", "\" differs from tsconfig paths")
            .map(|alias| format!("Vite alias \"{alias}\"가 tsconfig paths와 다름"))
    })
    .or_else(|| {
        quoted_pattern(value, "Vite alias \"", "\" is missing from tsconfig paths")
            .map(|alias| format!("Vite alias \"{alias}\"가 tsconfig paths에 없음"))
    })
    .or_else(|| {
        quoted_pattern(value, "\"", "\" must be an array of strings")
            .map(|field| format!("\"{field}\"는 string array여야 함"))
    })
    .or_else(|| {
        quoted_pattern(value, "\"", "\" contains a non-string pattern")
            .map(|field| format!("\"{field}\"에 string이 아닌 pattern이 포함됨"))
    })
    .or_else(|| {
        value
            .strip_suffix(" config is declared in multiple places")
            .map(|label| format!("{label} 설정이 여러 위치에 선언됨"))
    })
    .or_else(|| translate_duplicate_config_detail(value))
    .or_else(|| translate_single_config_hint(value))
    .or_else(|| translate_lockfiles_detail(value))
    .or_else(|| {
        value
            .strip_suffix(" is missing keys")
            .map(|file| format!("{file}에 누락된 key가 있음"))
    })
    .or_else(|| {
        if let Some((left, _)) = value
            .strip_prefix("Alias \"")
            .and_then(|tail| tail.split_once("\" differs between tsconfig and package imports"))
        {
            return Some(format!(
                "alias \"{}\"가 tsconfig와 package imports 사이에서 다름",
                left
            ));
        }
        if let Some((left, right)) = value
            .strip_prefix("Path alias \"")
            .and_then(|tail| tail.split_once("\" shadows \""))
        {
            let shadowed = right.strip_suffix('"').unwrap_or(right);
            return Some(format!(
                "path alias \"{}\"가 \"{}\"를 shadow함",
                left, shadowed
            ));
        }
        None
    })
    .or_else(|| translate_add_to_gitignore_detail(value))
    .or_else(|| translate_points_to_missing_path(value))
    .or_else(|| translate_points_to_path(value))
    .or_else(|| translate_output_dir_detail(value))
    .or_else(|| translate_tsconfig_declaration_detail(value))
    .or_else(|| translate_include_detail(value))
    .or_else(|| translate_exclude_detail(value))
}

#[cfg(test)]
mod tests {
    use super::message;

    #[test]
    fn translates_dynamic_missing_concrete_env_detail() {
        assert_eq!(
            message("No concrete value was found for: CI_ONLY.").as_ref(),
            "다음 env key에 대한 구체 값을 찾을 수 없습니다: CI_ONLY."
        );
        assert_eq!(
            message("Missing keys: OTHER.").as_ref(),
            "누락된 key: OTHER."
        );
        assert_eq!(
            message("Run \"maximus fix\" to append the missing keys to .env.example.").as_ref(),
            "\"maximus fix\"를 실행해 .env.example에 누락된 key를 추가하세요."
        );
    }

    #[test]
    fn translates_workspace_runner_and_editorconfig_messages() {
        assert_eq!(
            message("pnpm-workspace.yaml does not declare any package patterns").as_ref(),
            "pnpm-workspace.yaml이 package pattern을 선언하지 않음"
        );
        assert_eq!(
            message("No package globs were found under packages:, so workspace packages are not declared yet.").as_ref(),
            "packages: 아래에 package glob이 없어 workspace package가 아직 선언되지 않았습니다."
        );
        assert_eq!(
            message("This directory declares both Jest and Vitest configuration, so tests can run under different environments depending on the command.").as_ref(),
            "이 디렉터리는 Jest와 Vitest config를 모두 선언하므로 명령에 따라 서로 다른 환경에서 test가 실행될 수 있습니다."
        );
        assert_eq!(
            message("EditorConfig sets indent_style=tab, indent_size=4, end_of_line=crlf, but Prettier sets useTabs=false, tabWidth=2, endOfLine=lf.").as_ref(),
            "EditorConfig는 indent_style=tab, indent_size=4, end_of_line=crlf를 설정하지만 Prettier는 useTabs=false, tabWidth=2, endOfLine=lf를 설정합니다."
        );
    }

    #[test]
    fn translates_duplicate_config_and_structure_messages() {
        assert_eq!(
            message("Found 2 ESLint config sources in .").as_ref(),
            ".에서 ESLint 설정 출처 2개를 찾았습니다."
        );
        assert_eq!(
            message("Keep a single ESLint entry point per directory to avoid drift.").as_ref(),
            "차이를 피하려면 디렉터리마다 ESLint 진입점을 하나만 유지하세요."
        );
        assert_eq!(
            message("Migrate to eslint.config.* as the single source of truth, then remove the legacy .eslintrc.* files after the new config fully replaces them.").as_ref(),
            "eslint.config.*를 단일 기준으로 마이그레이션한 뒤, 새 config가 완전히 대체하면 legacy .eslintrc.* 파일을 제거하세요."
        );
        assert_eq!(
            message("Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets.").as_ref(),
            "package별로 다른 규칙 집합이 꼭 필요하지 않다면 repo 전체 ESLint 진입점을 줄이세요."
        );
        assert_eq!(
            message("Found 2 known lockfiles in .: package-lock.json, yarn.lock.").as_ref(),
            ".에서 알려진 lockfile 2개를 찾았습니다: package-lock.json, yarn.lock."
        );
    }
}

fn quoted_pattern<'a>(value: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    value.strip_prefix(prefix)?.strip_suffix(suffix)
}

fn translate_add_to_gitignore_detail(value: &str) -> Option<String> {
    let rest = value.strip_prefix("Add \"")?;
    let (pattern, target) = rest.split_once("\" to ")?;
    let target = target.strip_suffix('.')?;
    Some(format!("{target}에 \"{pattern}\"를 추가하세요."))
}

fn translate_points_to_missing_path(value: &str) -> Option<String> {
    let (alias, rest) = value.split_once(" points to ")?;
    let (target, _) = rest.split_once(", but the resolved path was not found.")?;
    Some(format!(
        "{alias}는 {target}를 가리키지만 해석된 경로를 찾을 수 없습니다."
    ))
}

fn translate_points_to_path(value: &str) -> Option<String> {
    let (alias, rest) = value.split_once(" points to ")?;
    let target = rest.strip_suffix('.')?;
    Some(format!("{alias}는 {target}를 가리킵니다."))
}

fn translate_duplicate_config_detail(value: &str) -> Option<String> {
    let rest = value.strip_prefix("Found ")?;
    let (count, rest) = rest.split_once(' ')?;
    let (label, rest) = rest.split_once(" config sources in ")?;
    let directory = rest.strip_suffix('.')?;
    let directory = if directory.is_empty() { "." } else { directory };
    Some(format!(
        "{directory}에서 {label} 설정 출처 {count}개를 찾았습니다."
    ))
}

fn translate_single_config_hint(value: &str) -> Option<String> {
    let rest = value.strip_prefix("Keep a single ")?;
    let label = rest.strip_suffix(" entry point per directory to avoid drift.")?;
    Some(format!(
        "차이를 피하려면 디렉터리마다 {label} 진입점을 하나만 유지하세요."
    ))
}

fn translate_lockfiles_detail(value: &str) -> Option<String> {
    let rest = value.strip_prefix("Found ")?;
    let (count, rest) = rest.split_once(" known lockfiles in ")?;
    let (directory, files) = rest.split_once(": ")?;
    let directory = if directory.is_empty() { "." } else { directory };
    let files = files.strip_suffix('.')?;
    Some(format!(
        "{directory}에서 알려진 lockfile {count}개를 찾았습니다: {files}."
    ))
}

fn translate_output_dir_detail(value: &str) -> Option<String> {
    if let Some(rest) = value.strip_prefix("outDir \"") {
        if let Some((out_dir, source_root)) = rest.split_once("\" overlaps source root \"") {
            let source_root = source_root.strip_suffix("\".")?;
            return Some(format!(
                "outDir \"{out_dir}\"이 source root \"{source_root}\"와 겹칩니다."
            ));
        }
        if let Some((out_dir, source_root)) = rest.split_once("\" is nested inside source root \"")
        {
            let source_root = source_root.strip_suffix("\".")?;
            return Some(format!(
                "outDir \"{out_dir}\"이 source root \"{source_root}\" 안에 있습니다."
            ));
        }
        if let Some((out_dir, input)) = rest.split_once("\" contains TypeScript input \"") {
            let input = input.strip_suffix("\".")?;
            return Some(format!(
                "outDir \"{out_dir}\"에 TypeScript 입력 \"{input}\"이 포함됩니다."
            ));
        }
        if let Some((out_dir, source_root)) = rest.split_once("\" contains source root \"") {
            let source_root = source_root.strip_suffix("\".")?;
            return Some(format!(
                "outDir \"{out_dir}\"이 source root \"{source_root}\"를 포함합니다."
            ));
        }
    }

    None
}

fn translate_tsconfig_declaration_detail(value: &str) -> Option<String> {
    if let Some((path, rest)) = value.split_once(" declares ") {
        if let Some((field, _)) = rest.split_once(", but TypeScript expects string patterns.") {
            return Some(format!(
                "{path}는 {field}를 선언하지만 TypeScript는 string pattern을 기대합니다."
            ));
        }
        if let Some((field, _)) =
            rest.split_once(", but TypeScript expects an array of string patterns.")
        {
            return Some(format!(
                "{path}는 {field}를 선언하지만 TypeScript는 string pattern array를 기대합니다."
            ));
        }
        if let Some((field, _)) =
            rest.split_once(", but TypeScript files entries cannot use glob wildcards.")
        {
            return Some(format!(
                "{path}는 {field}를 선언하지만 TypeScript files 항목에는 glob wildcard를 사용할 수 없습니다."
            ));
        }
        if let Some((field, _)) = rest.split_once(", but that path resolves to a directory.") {
            return Some(format!(
                "{path}는 {field}를 선언하지만 해당 path는 directory로 해석됩니다."
            ));
        }
        if let Some((field, _)) =
            rest.split_once(", but that path does not resolve to an existing file.")
        {
            return Some(format!(
                "{path}는 {field}를 선언하지만 해당 path는 존재하는 파일로 해석되지 않습니다."
            ));
        }
    }

    None
}

fn translate_include_detail(value: &str) -> Option<String> {
    let rest = value.strip_prefix("include pattern \"")?;
    let (pattern, rest) = rest.split_once("\" matched ")?;
    let (count, base_dir) = rest.split_once(" files under base dir ")?;
    let base_dir = base_dir.strip_suffix('.')?;
    Some(format!(
        "include pattern \"{pattern}\"은 base dir {base_dir} 아래에서 파일 {count}개와 일치했습니다."
    ))
}

fn translate_exclude_detail(value: &str) -> Option<String> {
    let rest = value.strip_prefix("exclude pattern \"")?;
    let (pattern, rest) = rest.split_once("\" removed ")?;
    let (count, rest) = rest.split_once(" files from ")?;
    let (included, base_dir) = rest.split_once(" included file(s) under base dir ")?;
    let base_dir = base_dir.strip_suffix('.')?;
    Some(format!(
        "exclude pattern \"{pattern}\"은 base dir {base_dir} 아래 포함 파일 {included}개 중 {count}개를 제외했습니다."
    ))
}
