use maximus_core::{
    discover_project, find_nearest_package_file, get_directories, get_files,
    is_concrete_env_file_name, is_template_env_file_name, looks_like_secret, make_finding,
    parse_env, parse_jsonc, render_env_template, serialize_audit_result, sort_findings,
    summarize_findings, unique_fixes, FileKind, FindingInput, FixPlan, ProjectSnapshot, Severity,
    StructureReport,
};
use serde::Deserialize;
use tempfile::TempDir;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct TsConfigFixture {
    extends: String,
    #[serde(rename = "compilerOptions")]
    compiler_options: CompilerOptions,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CompilerOptions {
    #[serde(rename = "baseUrl")]
    base_url: String,
}

#[test]
fn parse_jsonc_supports_comments_and_trailing_commas() {
    let parsed: TsConfigFixture = parse_jsonc(
        r#"
        {
          // comment
          "compilerOptions": {
            "baseUrl": ".",
          },
          "extends": "./tsconfig.base.json",
        }
        "#,
        "fixture.jsonc",
    )
    .unwrap();

    assert_eq!(
        parsed,
        TsConfigFixture {
            extends: "./tsconfig.base.json".to_string(),
            compiler_options: CompilerOptions {
                base_url: ".".to_string(),
            },
        }
    );
}

#[test]
fn parse_jsonc_prefixes_error_messages_with_the_label() {
    let error = parse_jsonc::<serde_json::Value>("{ invalid", "tsconfig.json").unwrap_err();

    assert_eq!(error.label(), "tsconfig.json");
    assert!(
        error.to_string().starts_with("tsconfig.json: "),
        "unexpected message: {}",
        error
    );
}

#[test]
fn parse_env_tracks_duplicates_invalid_lines_and_order() {
    let parsed = parse_env(
        "export API_URL=http://localhost:3000\nAUTH_TOKEN=\"secretvalue123456\"\nINVALID LINE\nAPI_URL=https://example.com\n",
        Some(".env"),
    );

    assert_eq!(
        parsed.order,
        vec!["API_URL".to_string(), "AUTH_TOKEN".to_string()]
    );
    assert_eq!(parsed.entries.len(), 3);
    assert_eq!(
        parsed.duplicates,
        vec![maximus_core::EnvDuplicate {
            key: "API_URL".to_string(),
            first_line: 1,
            second_line: 4,
        }]
    );
    assert_eq!(parsed.invalid_lines.len(), 1);
    assert_eq!(parsed.invalid_lines[0].line, 3);
    assert_eq!(
        parsed.values.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["API_URL", "AUTH_TOKEN"]
    );
    assert_eq!(parsed.values["AUTH_TOKEN"].raw_value, "\"secretvalue123456\"");
    assert_eq!(parsed.values["AUTH_TOKEN"].value, "secretvalue123456");
    assert_eq!(parsed.values["API_URL"].value, "https://example.com");
}

#[test]
fn parse_env_accepts_utf8_bom_on_the_first_line() {
    let parsed = parse_env("\u{feff}API_URL=http://localhost:3000\n", Some(".env"));

    assert_eq!(parsed.invalid_lines, Vec::new());
    assert_eq!(parsed.order, vec!["API_URL".to_string()]);
    assert_eq!(parsed.values["API_URL"].value, "http://localhost:3000");
}

#[test]
fn env_helpers_match_current_js_behavior() {
    assert_eq!(
        render_env_template(["AUTH_TOKEN", "API_URL", "AUTH_TOKEN"]),
        "API_URL=\nAUTH_TOKEN=\n"
    );
    assert_eq!(
        render_env_template(["API-URL", "API_URL", "API.URL"]),
        "API_URL=\nAPI-URL=\nAPI.URL=\n"
    );
    assert!(is_template_env_file_name(".env.sample"));
    assert!(is_template_env_file_name(".env.example.local"));
    assert!(!is_template_env_file_name(".env."));
    assert!(!is_template_env_file_name(".env.local"));
    assert!(is_concrete_env_file_name(".env.local"));
    assert!(!is_concrete_env_file_name(".env."));
    assert!(!is_concrete_env_file_name(".env.template"));
    assert!(looks_like_secret("supersecretvalue12345"));
    assert!(!looks_like_secret("localhost"));
    assert!(!looks_like_secret("your-api-key"));
}

#[test]
fn discovery_includes_prettier_toml_and_finds_nearest_package() {
    let fixture = temp_fixture();
    let snapshot = discover_project(fixture.path()).unwrap();

    assert_eq!(
        snapshot
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>(),
        vec![
            ".prettierrc.toml",
            "package.json",
            "packages/app/.env",
            "packages/app/eslint.config.js",
            "packages/app/package.json",
            "packages/app/tsconfig.json",
        ]
    );
    assert_eq!(get_files(&snapshot, FileKind::Prettier).len(), 1);
    assert!(
        snapshot
            .files
            .iter()
            .all(|file| file.name != ".env."),
        "malformed .env. files should not be classified as env files"
    );
    assert!(
        snapshot
            .files
            .iter()
            .all(|file| file.name != "tsconfig..json"),
        "malformed tsconfig..json files should not be classified as tsconfig files"
    );
    assert_eq!(get_directories(&snapshot).len(), 2);

    let nearest =
        find_nearest_package_file(&snapshot, fixture.path().join("packages/app/src")).unwrap();
    assert_eq!(
        relative_file_name(&snapshot, nearest),
        "packages/app/package.json"
    );
}

#[test]
fn discovery_ignores_common_build_directories() {
    let fixture = temp_fixture();
    let snapshot = discover_project(fixture.path()).unwrap();

    assert!(snapshot
        .files
        .iter()
        .all(|file| !file.relative_path.contains("node_modules")
            && !file.relative_path.contains("dist")
            && !file.relative_path.contains("target")));
}

#[test]
fn findings_are_defaulted_sorted_deduplicated_and_summarized() {
    let error = make_finding(FindingInput {
        id: "env-missing".to_string(),
        title: "Missing env example".to_string(),
        category: None,
        detail: Some("Create .env.example".to_string()),
        file: Some("b.env".into()),
        fix_ids: vec!["env-example".to_string()],
        fixable: true,
        hint: None,
        severity: Some(Severity::Error),
    });
    let info = make_finding(FindingInput {
        id: "structure-ok".to_string(),
        title: "Structure is healthy".to_string(),
        category: Some("structure".to_string()),
        detail: None,
        file: None,
        fix_ids: Vec::new(),
        fixable: false,
        hint: Some("Keep configs centralized.".to_string()),
        severity: Some(Severity::Info),
    });
    let warn = make_finding(FindingInput {
        id: "duplicate-prettier".to_string(),
        title: "Prettier config is declared in multiple places".to_string(),
        category: None,
        detail: None,
        file: Some("a.env".into()),
        fix_ids: Vec::new(),
        fixable: false,
        hint: None,
        severity: None,
    });

    let sorted = sort_findings(&[info.clone(), warn.clone(), error.clone()]);
    assert_eq!(
        sorted
            .iter()
            .map(|finding| finding.id.as_str())
            .collect::<Vec<_>>(),
        vec!["env-missing", "duplicate-prettier", "structure-ok"]
    );

    let unique = unique_fixes(&[
        FixPlan {
            id: "env-example".to_string(),
            title: "Create .env.example".to_string(),
            files: vec!["/tmp/.env.example".into()],
        },
        FixPlan {
            id: "env-example".to_string(),
            title: "Create .env.example".to_string(),
            files: vec!["/tmp/.env.example".into()],
        },
    ]);
    assert_eq!(unique.len(), 1);

    let summary = summarize_findings(
        &sorted,
        &unique,
        &StructureReport {
            is_monorepo: true,
            package_count: 2,
            env_directories: 1,
            config_files: 6,
            recommendations: vec!["Introduce a shared tsconfig.base.json".to_string()],
        },
    );
    assert_eq!(summary.status, "blocking issues");
    assert_eq!(summary.total_findings, 3);
    assert_eq!(summary.blocking_findings, 1);
    assert_eq!(summary.warning_findings, 1);
    assert_eq!(summary.info_findings, 1);
    assert_eq!(summary.fixable_findings, 1);
    assert_eq!(summary.fixes_available, 1);
}

#[test]
fn discovery_uses_js_like_order_for_mixed_case_and_accents() {
    let fixture = TempDir::new().unwrap();

    for name in ["Beta", "a", "ä", "z"] {
        let dir = fixture.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), format!(r#"{{"name":"{name}"}}"#)).unwrap();
    }

    let snapshot = discover_project(fixture.path()).unwrap();
    assert_eq!(
        snapshot
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "a/package.json",
            "ä/package.json",
            "Beta/package.json",
            "z/package.json",
        ]
    );
}

#[test]
fn findings_sort_uses_js_like_order_for_mixed_case_and_accents() {
    let sorted = sort_findings(&[
        make_finding(FindingInput {
            id: "z".to_string(),
            title: "z".to_string(),
            category: None,
            detail: None,
            file: Some("z.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "B".to_string(),
            title: "B".to_string(),
            category: None,
            detail: None,
            file: Some("B.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "b".to_string(),
            title: "b".to_string(),
            category: None,
            detail: None,
            file: Some("b.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "a".to_string(),
            title: "a".to_string(),
            category: None,
            detail: None,
            file: Some("a.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "ä".to_string(),
            title: "ä".to_string(),
            category: None,
            detail: None,
            file: Some("ä.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
    ]);

    assert_eq!(
        sorted
            .iter()
            .map(|finding| finding.file.as_ref().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        vec!["a.env", "ä.env", "b.env", "B.env", "z.env"]
    );
}

#[test]
fn discovery_uses_js_like_order_for_punctuation() {
    let fixture = TempDir::new().unwrap();

    for name in ["A_B", "A-B", "A.B", "AB"] {
        let dir = fixture.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("package.json"), format!(r#"{{"name":"{name}"}}"#)).unwrap();
    }

    let snapshot = discover_project(fixture.path()).unwrap();
    assert_eq!(
        snapshot
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "A_B/package.json",
            "A-B/package.json",
            "A.B/package.json",
            "AB/package.json",
        ]
    );
}

#[test]
fn findings_sort_uses_js_like_order_for_punctuation() {
    let sorted = sort_findings(&[
        make_finding(FindingInput {
            id: "ab".to_string(),
            title: "ab".to_string(),
            category: None,
            detail: None,
            file: Some("AB.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "underscore".to_string(),
            title: "underscore".to_string(),
            category: None,
            detail: None,
            file: Some("A_B.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "dash".to_string(),
            title: "dash".to_string(),
            category: None,
            detail: None,
            file: Some("A-B.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
        make_finding(FindingInput {
            id: "dot".to_string(),
            title: "dot".to_string(),
            category: None,
            detail: None,
            file: Some("A.B.env".into()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: None,
            severity: Some(Severity::Warn),
        }),
    ]);

    assert_eq!(
        sorted
            .iter()
            .map(|finding| finding.file.as_ref().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        vec!["A_B.env", "A-B.env", "A.B.env", "AB.env"]
    );
}

#[test]
fn audit_result_serialization_keeps_js_json_shape() {
    let finding = make_finding(FindingInput {
        id: "env-missing".to_string(),
        title: "Missing env example".to_string(),
        category: None,
        detail: Some("Create .env.example".to_string()),
        file: Some("/tmp/project/.env".into()),
        fix_ids: vec!["env-example".to_string()],
        fixable: true,
        hint: Some("Generate a template file.".to_string()),
        severity: Some(Severity::Warn),
    });
    let fix = FixPlan {
        id: "env-example".to_string(),
        title: "Create .env.example".to_string(),
        files: vec!["/tmp/project/.env.example".into()],
    };
    let structure = StructureReport {
        is_monorepo: false,
        package_count: 1,
        env_directories: 1,
        config_files: 3,
        recommendations: vec!["Current config surface looks healthy.".to_string()],
    };
    let summary = summarize_findings(&[finding.clone()], &[fix.clone()], &structure);
    let serialized = serialize_audit_result(&maximus_core::AuditResult {
        root_dir: "/tmp/project".into(),
        summary,
        structure,
        findings: vec![finding],
        fixes: vec![fix],
    });
    let json = serde_json::to_value(&serialized).unwrap();

    assert_eq!(serialized.findings.len(), 1);
    assert_eq!(serialized.fixes.len(), 1);
    assert_eq!(serialized.findings[0].category, "general");
    assert!(serialized.findings[0].fixable);
    assert_eq!(json["rootDir"], "/tmp/project");
    assert_eq!(json["summary"]["blockingFindings"], 0);
    assert_eq!(json["summary"]["warningFindings"], 1);
    assert_eq!(json["summary"]["fixableFindings"], 1);
    assert_eq!(json["summary"]["fixesAvailable"], 1);
    assert_eq!(json["summary"]["configFiles"], 3);
    assert_eq!(json["summary"]["packageCount"], 1);
    assert_eq!(json["summary"]["envDirectories"], 1);
    assert_eq!(json["structure"]["isMonorepo"], false);
    assert_eq!(json["structure"]["packageCount"], 1);
    assert_eq!(json["structure"]["envDirectories"], 1);
    assert_eq!(json["structure"]["configFiles"], 3);
    assert_eq!(json["findings"][0]["fixIds"][0], "env-example");
    assert!(json["findings"][0].get("fix_ids").is_none());
    assert!(json.get("root_dir").is_none());
}

fn temp_fixture() -> TempDir {
    let fixture = TempDir::new().unwrap();

    std::fs::write(
        fixture.path().join("package.json"),
        r#"{"name":"root-fixture"}"#,
    )
    .unwrap();
    std::fs::write(fixture.path().join(".prettierrc.toml"), "semi = false\n").unwrap();

    std::fs::create_dir_all(fixture.path().join("packages/app/src")).unwrap();
    std::fs::write(
        fixture.path().join("packages/app/package.json"),
        r#"{"name":"app-fixture"}"#,
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("packages/app/tsconfig.json"),
        r#"{"compilerOptions":{"baseUrl":"."}}"#,
    )
    .unwrap();
    std::fs::write(fixture.path().join("packages/app/tsconfig..json"), "{}").unwrap();
    std::fs::write(
        fixture.path().join("packages/app/eslint.config.js"),
        "export default [];\n",
    )
    .unwrap();
    std::fs::write(
        fixture.path().join("packages/app/.env"),
        "API_URL=http://localhost:3000\n",
    )
    .unwrap();
    std::fs::write(fixture.path().join("packages/app/.env."), "SHOULD_IGNORE=1\n").unwrap();

    std::fs::create_dir_all(fixture.path().join("node_modules/pkg")).unwrap();
    std::fs::write(
        fixture.path().join("node_modules/pkg/package.json"),
        r#"{"name":"ignored"}"#,
    )
    .unwrap();
    std::fs::create_dir_all(fixture.path().join("dist")).unwrap();
    std::fs::write(fixture.path().join("dist/tsconfig.json"), "{}").unwrap();
    std::fs::create_dir_all(fixture.path().join("target/debug")).unwrap();
    std::fs::write(fixture.path().join("target/debug/package.json"), r#"{"name":"ignored-target"}"#)
        .unwrap();

    fixture
}

fn relative_file_name(snapshot: &ProjectSnapshot, file: &maximus_core::ProjectFile) -> String {
    let relative = file
        .path
        .strip_prefix(&snapshot.root_dir)
        .ok()
        .unwrap_or_else(|| file.path.as_path());

    relative.to_string_lossy().replace('\\', "/")
}
