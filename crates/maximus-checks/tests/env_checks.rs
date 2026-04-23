use std::fs;
use std::path::{Path, PathBuf};

#[path = "../src/check_outcome.rs"]
mod check_outcome;
#[path = "../src/env.rs"]
mod env;

use env::{render_created_env_example, render_synced_env_example, run_env_check};
use maximus_core::env_parser::{
    reset_env_template_render_state, set_env_template_render_options, EnvTemplateRenderOptions,
    EnvTemplateSortMode,
};
use maximus_core::{apply_fix, discover_project, FixOperation, FixPlan, Severity};
use tempfile::TempDir;

#[test]
fn env_check_matches_js_findings_for_duplicates_invalid_sync_secret_override_and_missing_concrete()
{
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join(".env"),
        "PRIMARY=one\nDUP=first\nexport DUP=second\nNOT VALID\nSHARED=base\nONLY_BASE=1\n",
    );
    write(
        fixture.path().join(".env.local"),
        "SHARED=local\nLOCAL_ONLY=enabled\n",
    );
    write(
        fixture.path().join(".env.example"),
        "PRIMARY=\nDUP=\nSHARED=sk_live_1234567890abcdef\nCI_ONLY=\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "env-duplicate:{}:DUP:3",
            fixture.path().join(".env").to_string_lossy()
        ),
        Severity::Error,
        "Duplicate env key \"DUP\"",
        "DUP is declared on lines 2 and 3.",
        "Keep one declaration per env file so overrides stay explicit.",
        Some(fixture.path().join(".env")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "env-invalid:{}:4",
            fixture.path().join(".env").to_string_lossy()
        ),
        Severity::Warn,
        "Invalid env syntax",
        "Line 4 could not be parsed as KEY=value.",
        "Use shell-style env syntax or move comments to their own line.",
        Some(fixture.path().join(".env")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-example-sync:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        ".env.example is missing keys",
        "Missing keys: ONLY_BASE, LOCAL_ONLY.",
        "Run \"maximus fix\" to append the missing keys to .env.example.",
        Some(fixture.path().join(".env.example")),
        true,
        &[format!(
            "env-example:sync:{}",
            fixture.path().to_string_lossy()
        )],
    );
    assert_has_finding(
        &outcome.findings,
        &format!(
            "env-example-secret:{}:SHARED",
            fixture.path().join(".env.example").to_string_lossy()
        ),
        Severity::Warn,
        ".env.example appears to contain a real value for \"SHARED\"",
        "Contract files should describe the interface, not ship concrete secrets.",
        "Replace the value with a blank or placeholder string before sharing the repo.",
        Some(fixture.path().join(".env.example")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-mismatch:{}", fixture.path().to_string_lossy()),
        Severity::Info,
        "Local env overrides detected",
        ".env.local overrides 1 key(s): SHARED.",
        "Make sure local-only overrides are intentional and documented in .env.example.",
        Some(fixture.path().join(".env.local")),
        false,
        &[],
    );
    assert_has_finding(
        &outcome.findings,
        &format!("env-missing-concrete:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        "Declared env contract is not satisfied locally",
        "No concrete value was found for: CI_ONLY.",
        "If these are injected by CI, keep the contract documented. Otherwise add them to your local env files.",
        Some(fixture.path().join(".env.example")),
        false,
        &[],
    );

    assert_has_fix(
        &outcome.fixes,
        &format!("env-example:sync:{}", fixture.path().to_string_lossy()),
        "Append missing keys to .env.example",
        &[fixture.path().join(".env.example")],
    );
}

#[test]
fn env_check_plans_example_creation_when_runtime_env_files_exist_without_contract() {
    let fixture = TempDir::new().expect("temp dir should exist");

    write(
        fixture.path().join("apps/web/.env.local"),
        "BETA_FLAG=1\nAPI_TOKEN=abcdef1234567890\n",
    );
    write(
        fixture.path().join("apps/web/.env.production"),
        "API_TOKEN=abcdef1234567890\nPUBLIC_URL=https://example.test\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");
    let dir = fixture.path().join("apps/web");

    assert_has_finding(
        &outcome.findings,
        &format!("env-example-missing:{}", dir.to_string_lossy()),
        Severity::Warn,
        "Missing .env.example contract",
        "Runtime env files exist, but .env.example is missing.",
        "Run \"maximus fix\" to create a blank contract file.",
        Some(dir.join(".env.local")),
        true,
        &[format!("env-example:create:{}", dir.to_string_lossy())],
    );
    assert_has_fix(
        &outcome.fixes,
        &format!("env-example:create:{}", dir.to_string_lossy()),
        "Create apps/web/.env.example",
        &[dir.join(".env.example")],
    );
    assert_eq!(outcome.planned_fixes.len(), 1);
}

#[test]
fn env_sync_planned_fix_uses_audited_snapshot_text() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let example_path = fixture.path().join(".env.example");

    write(fixture.path().join(".env"), "PRIMARY=1\nSECONDARY=2\n");
    write(&example_path, "PRIMARY=\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");
    let planned = outcome
        .planned_fixes
        .iter()
        .find(|fix| {
            fix.public.id == format!("env-example:sync:{}", fixture.path().to_string_lossy())
        })
        .expect("planned sync fix should exist")
        .clone();

    match &planned.operation {
        FixOperation::SyncEnvExample {
            existing_text,
            missing_keys,
            ..
        } => {
            assert_eq!(existing_text, "PRIMARY=\n");
            assert_eq!(missing_keys, &vec!["SECONDARY".to_string()]);
        }
        _ => panic!("expected sync env example operation"),
    }

    fs::write(&example_path, "MUTATED=\n").expect("mutated example file should write");
    apply_fix(&planned).expect("planned fix should apply");

    let output = fs::read_to_string(&example_path).expect("example file should exist");
    assert_eq!(output, "PRIMARY=\nSECONDARY=\n");
}

#[test]
fn env_example_render_helpers_match_js_create_and_sync_semantics() {
    assert_eq!(
        render_created_env_example(["ZETA", "ALPHA", "ALPHA"]),
        "ALPHA=\nZETA=\n"
    );

    let synced =
        render_synced_env_example("PRIMARY=\n", &["ZETA".to_string(), "ALPHA".to_string()]);
    assert_eq!(synced, "PRIMARY=\nALPHA=\nZETA=\n");

    let synced_without_trailing_newline =
        render_synced_env_example("PRIMARY=", &["ZETA".to_string(), "ALPHA".to_string()]);
    assert_eq!(synced_without_trailing_newline, "PRIMARY=\nALPHA=\nZETA=\n");

    let synced_with_js_like_locale_order = render_synced_env_example(
        "PRIMARY=\n",
        &[
            "API_URL".to_string(),
            "API-URL".to_string(),
            "API.URL".to_string(),
        ],
    );
    assert_eq!(
        synced_with_js_like_locale_order,
        "PRIMARY=\nAPI_URL=\nAPI-URL=\nAPI.URL=\n"
    );
}

#[test]
fn env_contract_matrix_fixtures_cover_template_variants_and_duplicate_chains() {
    let matrix_root = fixture_root("env-contract-matrix");
    let duplicate_root = matrix_root.join("duplicate-chain");
    let duplicate_project = discover_project(&duplicate_root).expect("project should discover");
    let duplicate_outcome = run_env_check(&duplicate_project).expect("check should run");

    assert_has_finding(
        &duplicate_outcome.findings,
        &format!(
            "env-duplicate:{}:A:2",
            duplicate_root.join(".env").to_string_lossy()
        ),
        Severity::Error,
        "Duplicate env key \"A\"",
        "A is declared on lines 1 and 2.",
        "Keep one declaration per env file so overrides stay explicit.",
        Some(duplicate_root.join(".env")),
        false,
        &[],
    );
    assert_has_finding(
        &duplicate_outcome.findings,
        &format!(
            "env-duplicate:{}:A:3",
            duplicate_root.join(".env").to_string_lossy()
        ),
        Severity::Error,
        "Duplicate env key \"A\"",
        "A is declared on lines 2 and 3.",
        "Keep one declaration per env file so overrides stay explicit.",
        Some(duplicate_root.join(".env")),
        false,
        &[],
    );

    let sample_root = matrix_root.join("template-only-sample");
    let sample_project = discover_project(&sample_root).expect("project should discover");
    let sample_outcome = run_env_check(&sample_project).expect("check should run");
    assert!(sample_outcome.findings.is_empty());
    assert!(sample_outcome.fixes.is_empty());
    assert!(sample_outcome.planned_fixes.is_empty());

    let sync_root = matrix_root.join("sync-template-like-example-local");
    let sync_project = discover_project(&sync_root).expect("project should discover");
    let sync_outcome = run_env_check(&sync_project).expect("check should run");

    assert_has_finding(
        &sync_outcome.findings,
        &format!("env-example-sync:{}", sync_root.to_string_lossy()),
        Severity::Warn,
        ".env.example.local is missing keys",
        "Missing keys: SECONDARY.",
        "Run \"maximus fix\" to append the missing keys to .env.example.local.",
        Some(sync_root.join(".env.example.local")),
        true,
        &[format!("env-example:sync:{}", sync_root.to_string_lossy())],
    );
    assert_has_fix(
        &sync_outcome.fixes,
        &format!("env-example:sync:{}", sync_root.to_string_lossy()),
        "Append missing keys to .env.example.local",
        &[sync_root.join(".env.example.local")],
    );
}

#[test]
fn env_contract_matrix_local_only_fixture_creates_example_contract() {
    let fixture = copy_fixture_to_temp("env-contract-matrix/create-from-env-local-only");
    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");
    let planned = outcome
        .planned_fixes
        .iter()
        .find(|fix| {
            fix.public.id == format!("env-example:create:{}", fixture.path().to_string_lossy())
        })
        .expect("planned create fix should exist")
        .clone();

    apply_fix(&planned).expect("planned fix should apply");

    let output =
        fs::read_to_string(fixture.path().join(".env.example")).expect("example file should exist");
    assert_eq!(output, "API_URL=\nAUTH_TOKEN=\n");
}

#[test]
fn env_template_order_preservation_fixtures_match_js_outputs() {
    let create_fixture =
        copy_fixture_to_temp("env-template-order-preservation/create-from-concrete");
    let create_project = discover_project(create_fixture.path()).expect("project should discover");
    let create_outcome = run_env_check(&create_project).expect("check should run");
    let create_fix = create_outcome
        .planned_fixes
        .first()
        .expect("planned create fix should exist")
        .clone();

    apply_fix(&create_fix).expect("planned create fix should apply");
    let created_output = fs::read_to_string(create_fixture.path().join(".env.example"))
        .expect("created example should exist");
    assert_eq!(
        created_output,
        "API_URL=\nAPI-URL=\nAPI.URL=\nVAR_1=\nVAR_10=\nVAR_2=\n"
    );

    let sync_fixture =
        copy_fixture_to_temp("env-template-order-preservation/sync-existing-template");
    let sync_project = discover_project(sync_fixture.path()).expect("project should discover");
    let sync_outcome = run_env_check(&sync_project).expect("check should run");
    let sync_fix = sync_outcome
        .planned_fixes
        .first()
        .expect("planned sync fix should exist")
        .clone();

    apply_fix(&sync_fix).expect("planned sync fix should apply");
    let synced_output = fs::read_to_string(sync_fixture.path().join(".env.example"))
        .expect("synced example should exist");
    assert_eq!(
        synced_output,
        "VAR_2=\nAPI.URL=\nAPI_URL=\nAPI-URL=\nVAR_1=\nVAR_10=\n"
    );
}

#[test]
fn env_template_render_options_can_group_by_prefix_and_annotate_sources() {
    let _guard = EnvTemplateRenderStateGuard;
    set_env_template_render_options(EnvTemplateRenderOptions {
        include_source_comments: true,
        sort_mode: EnvTemplateSortMode::Prefix,
    });

    let create_fixture = TempDir::new().expect("temp dir should exist");
    write(
        create_fixture.path().join(".env"),
        "AUTH_TOKEN=base-token\nAPI_URL=base-url\nSHARED=base-shared\n",
    );
    write(
        create_fixture.path().join(".env.local"),
        "AUTH_SECRET=local-secret\nSHARED=local-shared\n",
    );

    let create_project = discover_project(create_fixture.path()).expect("project should discover");
    let create_outcome = run_env_check(&create_project).expect("check should run");
    let create_fix = create_outcome
        .planned_fixes
        .first()
        .expect("planned create fix should exist")
        .clone();

    apply_fix(&create_fix).expect("planned fix should apply");
    let created_output = fs::read_to_string(create_fixture.path().join(".env.example"))
        .expect("example file should exist");
    assert_eq!(
        created_output,
        "# source: .env\nAPI_URL=\nAUTH_TOKEN=\n\n# source: .env.local\nAUTH_SECRET=\n\n# source: multiple\nSHARED=\n"
    );

    let sync_fixture = TempDir::new().expect("temp dir should exist");
    write(
        sync_fixture.path().join(".env"),
        "AUTH_TOKEN=base-token\nAPI_URL=base-url\nSHARED=base-shared\n",
    );
    write(
        sync_fixture.path().join(".env.local"),
        "AUTH_SECRET=local-secret\nSHARED=local-shared\n",
    );
    write(sync_fixture.path().join(".env.example"), "API_URL=\n");

    let sync_project = discover_project(sync_fixture.path()).expect("project should discover");
    let sync_outcome = run_env_check(&sync_project).expect("check should run");
    let sync_fix = sync_outcome
        .planned_fixes
        .first()
        .expect("planned sync fix should exist")
        .clone();

    apply_fix(&sync_fix).expect("planned fix should apply");
    let synced_output = fs::read_to_string(sync_fixture.path().join(".env.example"))
        .expect("example file should exist");
    assert_eq!(
        synced_output,
        "API_URL=\n# source: .env\nAUTH_TOKEN=\n\n# source: .env.local\nAUTH_SECRET=\n\n# source: multiple\nSHARED=\n"
    );
}

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn fixture_root(relative_path: &str) -> PathBuf {
    workspace_root().join("test/fixtures").join(relative_path)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root should exist")
        .to_path_buf()
}

fn copy_fixture_to_temp(relative_path: &str) -> TempDir {
    let fixture = TempDir::new().expect("temp dir should exist");
    let source = fixture_root(relative_path);

    copy_dir_recursive(&source, fixture.path());
    fixture
}

fn copy_dir_recursive(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("target dir should exist");

    for entry in fs::read_dir(source).expect("fixture dir should read") {
        let entry = entry.expect("fixture entry should exist");
        let entry_path = entry.path();
        let target_path = target.join(entry.file_name());

        if entry.file_type().expect("entry type should load").is_dir() {
            copy_dir_recursive(&entry_path, &target_path);
        } else {
            fs::copy(&entry_path, &target_path).expect("fixture file should copy");
        }
    }
}

fn assert_has_fix(fixes: &[FixPlan], id: &str, title: &str, files: &[PathBuf]) {
    let fix = fixes
        .iter()
        .find(|fix| fix.id == id)
        .unwrap_or_else(|| panic!("missing fix {id}"));

    assert_eq!(fix.title, title);
    assert_eq!(fix.files, files);
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    severity: Severity,
    title: &str,
    detail: &str,
    hint: &str,
    file: Option<PathBuf>,
    fixable: bool,
    fix_ids: &[String],
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
    assert_eq!(finding.fixable, fixable);
    assert_eq!(finding.fix_ids, fix_ids);
}

struct EnvTemplateRenderStateGuard;

impl Drop for EnvTemplateRenderStateGuard {
    fn drop(&mut self) {
        reset_env_template_render_state();
    }
}
