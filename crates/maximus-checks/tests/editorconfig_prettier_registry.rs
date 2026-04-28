use std::path::Path;

use maximus_checks::{run_editorconfig_prettier_check, run_registered_checks};
use maximus_core::{discover_project, Severity};

#[test]
fn editorconfig_prettier_check_reports_conflicts_and_registry_wiring() {
    let fixture = fixture("conflict");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_editorconfig_prettier_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("editorconfig-prettier-conflict:{}", fixture.to_string_lossy()),
        Severity::Warn,
        "EditorConfig and Prettier disagree",
        "EditorConfig sets indent_style=tab, indent_size=4, end_of_line=crlf, but Prettier sets useTabs=false, tabWidth=2, endOfLine=lf.",
        "Align EditorConfig and Prettier so editor saves do not fight formatter output.",
        Some(fixture.join(".editorconfig")),
    );

    let registered = run_registered_checks(&project).expect("registry should run");
    assert!(
        registered.findings.iter().any(|finding| {
            finding.id
                == format!(
                    "editorconfig-prettier-conflict:{}",
                    fixture.to_string_lossy()
                )
        }),
        "registry should include the EditorConfig/Prettier check"
    );
}

#[test]
fn editorconfig_prettier_check_reads_universal_section_values() {
    let fixture = fixture("universal-section-conflict");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_editorconfig_prettier_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("editorconfig-prettier-conflict:{}", fixture.to_string_lossy()),
        Severity::Warn,
        "EditorConfig and Prettier disagree",
        "EditorConfig sets indent_style=tab, indent_size=4, end_of_line=crlf, but Prettier sets useTabs=false, tabWidth=2, endOfLine=lf.",
        "Align EditorConfig and Prettier so editor saves do not fight formatter output.",
        Some(fixture.join(".editorconfig")),
    );
}

#[test]
fn editorconfig_prettier_check_accepts_aligned_configs() {
    let fixture = fixture("clean");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_editorconfig_prettier_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "aligned EditorConfig and Prettier values should not produce findings"
    );
}

#[test]
fn editorconfig_prettier_check_ignores_section_specific_values() {
    let fixture = fixture("section-override");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_editorconfig_prettier_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "section-specific EditorConfig overrides should not count as root defaults"
    );
}

#[test]
fn editorconfig_prettier_check_ignores_nested_editorconfig_files() {
    let fixture = fixture("nested-override");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_editorconfig_prettier_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "nested EditorConfig files should not override the repository root default block"
    );
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/fixtures/editorconfig-prettier")
        .join(name)
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    severity: Severity,
    title: &str,
    detail: &str,
    hint: &str,
    file: Option<std::path::PathBuf>,
) {
    let finding = findings
        .iter()
        .find(|finding| finding.id == id)
        .unwrap_or_else(|| panic!("missing finding {id}"));

    assert_eq!(finding.severity, severity);
    assert_eq!(finding.title, title);
    assert_eq!(finding.detail, detail);
    assert_eq!(finding.hint, hint);
    assert_eq!(finding.file, file);
    assert!(!finding.fixable);
    assert!(finding.fix_ids.is_empty());
}
