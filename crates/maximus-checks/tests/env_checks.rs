use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../src/check_outcome.rs"]
mod check_outcome;
#[path = "../src/env.rs"]
mod env;

use env::{
    render_created_env_example, render_created_env_example_with_sources, render_synced_env_example,
    render_synced_env_example_with_sources, run_env_check, run_env_check_with_options,
    EnvCheckOptions,
};
use maximus_core::{
    apply_fix, discover_project, EnvTemplateRenderOptions, EnvTemplateSourceGroup, FixOperation,
    FixPlan, Severity,
};
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
fn env_check_reports_unprotected_concrete_env_files_and_respects_root_and_nested_gitignore_entries()
{
    let root_fixture = TempDir::new().expect("temp dir should exist");

    write(
        root_fixture.path().join(".env"),
        "API_URL=https://example.test\n",
    );

    let root_project = discover_project(root_fixture.path()).expect("project should discover");
    let root_outcome = run_env_check(&root_project).expect("check should run");

    assert_has_finding(
        &root_outcome.findings,
        &format!(
            "env-gitignore:{}",
            root_fixture.path().join(".env").to_string_lossy()
        ),
        Severity::Warn,
        "Concrete env file \".env\" is not protected by .gitignore",
        "Add \".env\" to .gitignore.",
        "Protect concrete env files with an exact .gitignore entry before committing secrets.",
        Some(root_fixture.path().join(".env")),
        false,
        &[],
    );

    write(root_fixture.path().join(".gitignore"), ".env\n");
    let protected_project = discover_project(root_fixture.path()).expect("project should discover");
    let protected_outcome = run_env_check(&protected_project).expect("check should run");

    assert!(
        !protected_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "root .gitignore should protect .env"
    );

    write(root_fixture.path().join(".gitignore"), ".env\n!.env\n");
    let negated_project = discover_project(root_fixture.path()).expect("project should discover");
    let negated_outcome = run_env_check(&negated_project).expect("check should run");

    assert!(
        negated_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "later .gitignore negation should make .env unprotected"
    );

    let leading_space_fixture = TempDir::new().expect("temp dir should exist");
    write(
        leading_space_fixture.path().join(".env"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(leading_space_fixture.path().join(".gitignore"), " .env\n");

    let leading_space_project =
        discover_project(leading_space_fixture.path()).expect("project should discover");
    let leading_space_outcome = run_env_check(&leading_space_project).expect("check should run");

    assert!(
        leading_space_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "leading-space .gitignore pattern should not protect .env"
    );

    let glob_fixture = TempDir::new().expect("temp dir should exist");
    write(
        glob_fixture.path().join(".env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(glob_fixture.path().join(".env.example"), "API_TOKEN=\n");
    write(
        glob_fixture.path().join(".gitignore"),
        ".env*\n!.env.example\n",
    );

    let glob_project = discover_project(glob_fixture.path()).expect("project should discover");
    let glob_outcome = run_env_check(&glob_project).expect("check should run");

    assert!(
        !glob_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "glob .gitignore pattern should protect .env.local"
    );

    let globstar_fixture = TempDir::new().expect("temp dir should exist");
    write(
        globstar_fixture.path().join("apps/web/.env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        globstar_fixture.path().join(".gitignore"),
        "**/.env.local\n",
    );

    let globstar_project =
        discover_project(globstar_fixture.path()).expect("project should discover");
    let globstar_outcome = run_env_check(&globstar_project).expect("check should run");

    assert!(
        !globstar_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "globstar .gitignore pattern should protect nested .env.local"
    );

    let directory_fixture = TempDir::new().expect("temp dir should exist");
    write(
        directory_fixture.path().join("apps/web/.env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(directory_fixture.path().join(".gitignore"), "apps/\n");

    let directory_project =
        discover_project(directory_fixture.path()).expect("project should discover");
    let directory_outcome = run_env_check(&directory_project).expect("check should run");

    assert!(
        !directory_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "directory-only .gitignore pattern should protect files under that directory"
    );

    let bare_directory_fixture = TempDir::new().expect("temp dir should exist");
    write(
        bare_directory_fixture.path().join("secrets/.env"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        bare_directory_fixture.path().join(".gitignore"),
        "secrets\n",
    );

    let bare_directory_project =
        discover_project(bare_directory_fixture.path()).expect("project should discover");
    let bare_directory_outcome = run_env_check(&bare_directory_project).expect("check should run");

    assert!(
        !bare_directory_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "bare directory .gitignore pattern should protect files under that directory"
    );

    let negated_directory_fixture = TempDir::new().expect("temp dir should exist");
    write(
        negated_directory_fixture.path().join("secrets/.env"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        negated_directory_fixture.path().join(".gitignore"),
        "*.env\n!secrets/\n",
    );

    let negated_directory_project =
        discover_project(negated_directory_fixture.path()).expect("project should discover");
    let negated_directory_outcome =
        run_env_check(&negated_directory_project).expect("check should run");

    assert!(
        !negated_directory_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "negated directory pattern should not unprotect ignored env files inside that directory"
    );

    let tracked_fixture = TempDir::new().expect("temp dir should exist");
    write(
        tracked_fixture.path().join(".env"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(tracked_fixture.path().join(".gitignore"), ".env\n");
    run_git(tracked_fixture.path(), &["init"]);
    run_git(tracked_fixture.path(), &["add", "-f", ".env"]);

    let tracked_project =
        discover_project(tracked_fixture.path()).expect("project should discover");
    let tracked_outcome = run_env_check(&tracked_project).expect("check should run");

    assert!(
        tracked_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "tracked concrete env files should not be treated as protected by .gitignore"
    );

    let nested_fixture = TempDir::new().expect("temp dir should exist");
    write(
        nested_fixture.path().join("apps/web/.env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        nested_fixture.path().join("apps/web/.gitignore"),
        ".env.local\n",
    );

    let nested_project = discover_project(nested_fixture.path()).expect("project should discover");
    let nested_outcome = run_env_check(&nested_project).expect("check should run");

    assert!(
        !nested_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "nested .gitignore should protect .env.local"
    );

    let ancestor_fixture = TempDir::new().expect("temp dir should exist");
    write(
        ancestor_fixture.path().join("apps/web/.env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        ancestor_fixture.path().join("apps/.gitignore"),
        ".env.local\n",
    );

    let ancestor_project =
        discover_project(ancestor_fixture.path()).expect("project should discover");
    let ancestor_outcome = run_env_check(&ancestor_project).expect("check should run");

    assert!(
        !ancestor_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "ancestor .gitignore should protect nested .env.local"
    );

    let subdir_audit_fixture = TempDir::new().expect("temp dir should exist");
    write(
        subdir_audit_fixture.path().join(".git/HEAD"),
        "ref: refs/heads/main\n",
    );
    write(
        subdir_audit_fixture.path().join(".gitignore"),
        "packages/app/.env.local\n",
    );
    write(
        subdir_audit_fixture.path().join("packages/app/.env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );

    let subdir_project = discover_project(subdir_audit_fixture.path().join("packages/app"))
        .expect("project should discover");
    let subdir_outcome = run_env_check(&subdir_project).expect("check should run");

    assert!(
        !subdir_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "repo root .gitignore should protect subdir audit targets"
    );

    let anchored_nested_fixture = TempDir::new().expect("temp dir should exist");
    write(
        anchored_nested_fixture.path().join("apps/web/.env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        anchored_nested_fixture.path().join(".gitignore"),
        "/.env.local\n",
    );

    let anchored_nested_project =
        discover_project(anchored_nested_fixture.path()).expect("project should discover");
    let anchored_nested_outcome =
        run_env_check(&anchored_nested_project).expect("check should run");

    assert!(
        anchored_nested_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "anchored root .gitignore pattern should not protect nested .env.local"
    );

    let anchored_root_fixture = TempDir::new().expect("temp dir should exist");
    write(
        anchored_root_fixture.path().join(".env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        anchored_root_fixture.path().join(".gitignore"),
        "/.env.local\n",
    );

    let anchored_root_project =
        discover_project(anchored_root_fixture.path()).expect("project should discover");
    let anchored_root_outcome = run_env_check(&anchored_root_project).expect("check should run");

    assert!(
        !anchored_root_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "anchored root .gitignore pattern should protect root .env.local"
    );

    let directory_only_fixture = TempDir::new().expect("temp dir should exist");
    write(
        directory_only_fixture.path().join(".env.local"),
        "API_TOKEN=abcdef1234567890\n",
    );
    write(
        directory_only_fixture.path().join(".gitignore"),
        ".env.local/\n",
    );

    let directory_only_project =
        discover_project(directory_only_fixture.path()).expect("project should discover");
    let directory_only_outcome = run_env_check(&directory_only_project).expect("check should run");

    assert!(
        directory_only_outcome
            .findings
            .iter()
            .any(|finding| finding.id.starts_with("env-gitignore:")),
        "directory-only .gitignore pattern should not protect env files"
    );
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
            groups,
            ..
        } => {
            assert_eq!(existing_text, "PRIMARY=\n");
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].keys, vec!["SECONDARY".to_string()]);
        }
        _ => panic!("expected sync env example operation"),
    }

    fs::write(&example_path, "MUTATED=\n").expect("mutated example file should write");
    apply_fix(&planned).expect("planned fix should apply");

    let output = fs::read_to_string(&example_path).expect("example file should exist");
    assert_eq!(output, "PRIMARY=\nSECONDARY=\n");
}

#[test]
fn env_sync_planned_fix_excludes_ambient_platform_keys_and_keeps_app_keys() {
    let fixture = TempDir::new().expect("temp dir should exist");
    let example_path = fixture.path().join(".env.local.example");

    write(
        fixture.path().join(".env.local"),
        "VERCEL=1\nVERCEL_URL=example.vercel.app\nVERCEL_ENV=preview\nTURBO_TOKEN=turbo-token\nTURBO_TEAM=team\nNX_DAEMON=true\nNEXT_PUBLIC_OKTA_CLIENT_ID=okta-client\nGITHUB_TOKEN=github-token\nSUPABASE_URL=https://example.supabase.co\nSUPABASE_ANON_KEY=supabase-anon\nVALIDATION_MODE=strict\n",
    );
    write(fixture.path().join(".gitignore"), ".env.local\n");
    write(&example_path, "EXISTING=\n");

    let project = discover_project(fixture.path()).expect("project should discover");
    let outcome = run_env_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!("env-example-sync:{}", fixture.path().to_string_lossy()),
        Severity::Warn,
        ".env.local.example is missing keys",
        "Missing keys: NEXT_PUBLIC_OKTA_CLIENT_ID, GITHUB_TOKEN, SUPABASE_URL, SUPABASE_ANON_KEY, VALIDATION_MODE.",
        "Run \"maximus fix\" to append the missing keys to .env.local.example.",
        Some(example_path.clone()),
        true,
        &[format!(
            "env-example:sync:{}",
            fixture.path().to_string_lossy()
        )],
    );

    let planned = outcome
        .planned_fixes
        .iter()
        .find(|fix| {
            fix.public.id == format!("env-example:sync:{}", fixture.path().to_string_lossy())
        })
        .expect("planned sync fix should exist")
        .clone();

    match &planned.operation {
        FixOperation::SyncEnvExample { groups, .. } => {
            assert_eq!(groups.len(), 1);
            assert_eq!(
                groups[0].keys,
                vec![
                    "NEXT_PUBLIC_OKTA_CLIENT_ID".to_string(),
                    "GITHUB_TOKEN".to_string(),
                    "SUPABASE_URL".to_string(),
                    "SUPABASE_ANON_KEY".to_string(),
                    "VALIDATION_MODE".to_string(),
                ]
            );
            for ambient_key in [
                "VERCEL",
                "VERCEL_URL",
                "VERCEL_ENV",
                "TURBO_TOKEN",
                "TURBO_TEAM",
                "NX_DAEMON",
            ] {
                assert!(
                    !groups[0].keys.iter().any(|key| key == ambient_key),
                    "{ambient_key} should not be planned for env example sync"
                );
            }
        }
        _ => panic!("expected sync env example operation"),
    }

    apply_fix(&planned).expect("planned fix should apply");

    let output = fs::read_to_string(&example_path).expect("example file should exist");
    for app_key in [
        "NEXT_PUBLIC_OKTA_CLIENT_ID",
        "GITHUB_TOKEN",
        "SUPABASE_URL",
        "SUPABASE_ANON_KEY",
        "VALIDATION_MODE",
    ] {
        assert!(
            output.contains(&format!("{app_key}=\n")),
            "{app_key} should be appended to env example"
        );
    }
    for ambient_key in [
        "VERCEL",
        "VERCEL_URL",
        "VERCEL_ENV",
        "TURBO_TOKEN",
        "TURBO_TEAM",
        "NX_DAEMON",
    ] {
        assert!(
            !output.contains(&format!("{ambient_key}=\n")),
            "{ambient_key} should not be appended to env example"
        );
    }
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
fn env_example_source_comment_helpers_group_and_sort_opt_in_output() {
    let groups = vec![
        EnvTemplateSourceGroup {
            source: Some(".env.local".to_string()),
            keys: vec!["LOCAL_Z".to_string(), "LOCAL_A".to_string()],
        },
        EnvTemplateSourceGroup {
            source: Some(".env".to_string()),
            keys: vec![
                "BASE_Z".to_string(),
                "BASE_A".to_string(),
                "BASE_A".to_string(),
            ],
        },
    ];

    assert_eq!(
        render_created_env_example_with_sources(groups.clone()),
        "# Source: .env\nBASE_A=\nBASE_Z=\n\n# Source: .env.local\nLOCAL_A=\nLOCAL_Z=\n"
    );
    assert_eq!(
        render_synced_env_example_with_sources("EXISTING=", groups),
        "EXISTING=\n# Source: .env\nBASE_A=\nBASE_Z=\n\n# Source: .env.local\nLOCAL_A=\nLOCAL_Z=\n"
    );
}

#[test]
fn env_source_comment_option_changes_planned_fix_output_without_default_regression() {
    let fixture = TempDir::new().expect("temp dir should exist");
    write(
        fixture.path().join(".env.local"),
        "LOCAL_Z=1\nLOCAL_A=2\nSHARED=local\n",
    );
    write(
        fixture.path().join(".env"),
        "BASE_Z=1\nBASE_A=2\nSHARED=base\n",
    );

    let project = discover_project(fixture.path()).expect("project should discover");
    let default_outcome = run_env_check(&project).expect("default check should run");
    let default_fix = default_outcome
        .planned_fixes
        .first()
        .expect("default planned fix should exist")
        .clone();
    apply_fix(&default_fix).expect("default fix should apply");
    let default_output = fs::read_to_string(fixture.path().join(".env.example"))
        .expect("default example should exist");
    assert_eq!(
        default_output,
        "BASE_A=\nBASE_Z=\nLOCAL_A=\nLOCAL_Z=\nSHARED=\n"
    );

    fs::remove_file(fixture.path().join(".env.example")).expect("example should remove");
    let opt_in_outcome = run_env_check_with_options(
        &project,
        &EnvCheckOptions {
            template_render: EnvTemplateRenderOptions {
                source_comments: true,
            },
        },
    )
    .expect("opt-in check should run");
    let opt_in_fix = opt_in_outcome
        .planned_fixes
        .first()
        .expect("opt-in planned fix should exist")
        .clone();
    apply_fix(&opt_in_fix).expect("opt-in fix should apply");
    let opt_in_output = fs::read_to_string(fixture.path().join(".env.example"))
        .expect("opt-in example should exist");
    assert_eq!(
        opt_in_output,
        "# Source: .env\nBASE_A=\nBASE_Z=\nSHARED=\n\n# Source: .env.local\nLOCAL_A=\nLOCAL_Z=\n"
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

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dir should exist");
    }

    fs::write(path, content).expect("fixture file should write");
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git command should run");
    assert!(output.status.success(), "{output:?}");
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
