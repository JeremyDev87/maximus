use std::path::Path;

use maximus_checks::{run_registered_checks, run_vite_tsconfig_alias_check};
use maximus_core::{discover_project, Severity};

#[test]
fn vite_tsconfig_alias_check_reports_mismatch_and_registry_wiring() {
    let fixture = fixture("mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );

    let registered = run_registered_checks(&project).expect("registry should run");
    assert!(
        registered.findings.iter().any(|finding| {
            finding.id
                == format!(
                    "vite-alias-sync:{}:@app",
                    fixture.join("vite.config.ts").to_string_lossy()
                )
        }),
        "registry should include the Vite alias sync check"
    );
}

#[test]
fn vite_tsconfig_alias_check_reports_one_side_only_infos() {
    let vite_only = fixture("vite-only");

    let vite_project = discover_project(&vite_only).expect("project should discover");
    let vite_outcome = run_vite_tsconfig_alias_check(&vite_project).expect("check should run");
    assert_has_finding(
        &vite_outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app:vite-only",
            vite_only.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Info,
        "Vite alias \"@app\" is missing from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Add the alias to tsconfig paths or remove it from Vite so imports resolve consistently.",
        Some(vite_only.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_skips_non_vite_tsconfig_aliases() {
    let tsconfig_only = fixture("tsconfig-only");

    let ts_project = discover_project(&tsconfig_only).expect("project should discover");
    let ts_outcome = run_vite_tsconfig_alias_check(&ts_project).expect("check should run");
    assert!(
        ts_outcome.findings.is_empty(),
        "non-Vite projects should not report Vite alias sync noise"
    );
}

#[test]
fn vite_tsconfig_alias_check_skips_dynamic_alias_fixture_and_accepts_normalized_paths() {
    let dynamic = fixture("dynamic-skip");
    let clean = fixture("clean");

    let dynamic_project = discover_project(&dynamic).expect("project should discover");
    let dynamic_outcome =
        run_vite_tsconfig_alias_check(&dynamic_project).expect("check should run");
    assert!(
        dynamic_outcome.findings.is_empty(),
        "dynamic alias form should be skipped"
    );

    let clean_project = discover_project(&clean).expect("project should discover");
    let clean_outcome = run_vite_tsconfig_alias_check(&clean_project).expect("check should run");
    assert!(
        clean_outcome.findings.is_empty(),
        "normalized but equivalent paths should not produce false positives"
    );
}

#[test]
fn vite_tsconfig_alias_check_reads_extended_tsconfig_paths() {
    let fixture = fixture("extends-clean");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "aliases inherited through tsconfig extends should participate in comparison"
    );
}

#[test]
fn vite_tsconfig_alias_check_resolves_inherited_paths_from_parent_config_directory() {
    let fixture = fixture("extends-parent-base");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "paths inherited from a parent tsconfig should resolve relative to that parent file"
    );
}

#[test]
fn vite_tsconfig_alias_check_honors_tsconfig_base_url_for_paths() {
    let fixture = fixture("base-url-clean");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "paths targets should resolve relative to compilerOptions.baseUrl when present"
    );
}

#[test]
fn vite_tsconfig_alias_check_reports_array_alias_mismatches() {
    let fixture = fixture("array-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_keeps_static_array_aliases_after_dynamic_entries() {
    let fixture = fixture("array-dynamic-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_reports_path_resolve_object_mismatches() {
    let fixture = fixture("path-resolve-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_reports_function_form_config_mismatches() {
    let fixture = fixture("function-form-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_reports_function_form_block_return_mismatches() {
    let fixture = fixture("function-form-block-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_ignores_alias_tokens_inside_comments() {
    let fixture = fixture("comment-prefix-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_ignores_alias_tokens_inside_template_literals() {
    let fixture = fixture("template-prefix-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_parses_quoted_alias_property_keys() {
    let fixture = fixture("quoted-alias-mismatch");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert_has_finding(
        &outcome.findings,
        &format!(
            "vite-alias-sync:{}:@app",
            fixture.join("vite.config.ts").to_string_lossy()
        ),
        Severity::Warn,
        "Vite alias \"@app\" differs from tsconfig paths",
        "Vite resolves \"@app\" to",
        "Align both alias surfaces so editor and bundler resolution stay in sync.",
        Some(fixture.join("vite.config.ts")),
    );
}

#[test]
fn vite_tsconfig_alias_check_reads_resolve_alias_only() {
    let fixture = fixture("resolve-scope-clean");

    let project = discover_project(&fixture).expect("project should discover");
    let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");

    assert!(
        outcome.findings.is_empty(),
        "non-resolve alias properties should not shadow the real Vite resolve.alias map"
    );
}

#[test]
fn native_tsconfig_paths_handles_literal_and_uncertain_configs() {
    let cases = [
        ("literal", "export default { resolve: { tsconfigPaths: true } }", None),
        ("defineConfig", "export default defineConfig({ resolve: { tsconfigPaths: true } });", None),
        ("quoted comments", "/* 한국어 */ export default { 'resolve': { alias: {}, 'tsconfigPaths': true /* 참 */ } }", None),
        ("false", "export default { resolve: { tsconfigPaths: false } }", Some(Severity::Info)),
        ("missing", "export default {}", Some(Severity::Info)),
        ("variable", "export default { resolve: { tsconfigPaths: enabled } }", Some(Severity::Info)),
        ("call", "export default { resolve: { tsconfigPaths: enabled() } }", Some(Severity::Info)),
        ("conditional", "export default { resolve: { tsconfigPaths: enabled ? true : false } }", Some(Severity::Info)),
        ("string", "export default { resolve: { tsconfigPaths: 'true' } }", Some(Severity::Info)),
        ("prefix", "export default { resolve: { tsconfigPaths: trueValue } }", Some(Severity::Info)),
        ("expression", "export default { resolve: { tsconfigPaths: true && false } }", Some(Severity::Info)),
        ("comment expression", "export default { resolve: { alias: {}, tsconfigPaths: true /* comment */ || false } }", Some(Severity::Info)),
        ("missing comma", "export default { resolve: { alias: {}, tsconfigPaths: true other: false } }", Some(Severity::Info)),
        ("fake scopes", "const note = 'resolve: { tsconfigPaths: true }'; export default { other: { tsconfigPaths: true }, resolve: { /* tsconfigPaths: true */ alias: {} } }", Some(Severity::Info)),
        ("resolve duplicate", "export default { resolve: { tsconfigPaths: true }, resolve: {} }", Some(Severity::Info)),
        ("native duplicate", "export default { resolve: { alias: {}, tsconfigPaths: true, tsconfigPaths: false } }", Some(Severity::Info)),
        ("regex fake export", "const pattern = /export default { resolve: { tsconfigPaths: true } };/; export default { resolve: { tsconfigPaths: false } };", Some(Severity::Info)),
        ("regex closing comment", "const pattern = /export default { resolve: { tsconfigPaths: true } };/// comment", Some(Severity::Info)),
        ("division prefix uncertain", "const ratio = 4 / 2; export default { resolve: { tsconfigPaths: true } };", Some(Severity::Info)),
        ("comment prefix", "// /export default { resolve: { tsconfigPaths: false } }/\n/* / */ export default { resolve: { tsconfigPaths: true } };", None),
        ("escaped native duplicate", r"export default { resolve: { tsconfigPaths: true, 'tsconfig\u0050aths': false } }", Some(Severity::Info)),
        ("escaped resolve duplicate", r"export default { resolve: { tsconfigPaths: true }, 'res\u006flve': { tsconfigPaths: false } }", Some(Severity::Info)),
        ("root spread", "export default { resolve: { tsconfigPaths: true }, ...other }", Some(Severity::Info)),
        ("leading root spread", "export default { ...other, resolve: { tsconfigPaths: true } }", None),
        ("resolve spread", "export default { resolve: { alias: {}, tsconfigPaths: true, ...other } }", Some(Severity::Info)),
        ("root computed", "export default { resolve: { tsconfigPaths: true }, [key]: other }", Some(Severity::Info)),
        ("resolve computed", "export default { resolve: { alias: {}, tsconfigPaths: true, [key]: false } }", Some(Severity::Info)),
        ("root shorthand", "export default { resolve: { tsconfigPaths: true }, other }", Some(Severity::Info)),
        ("resolve shorthand", "export default { resolve: { alias: {}, tsconfigPaths: true, other } }", Some(Severity::Info)),
        ("root method", "export default { resolve: { tsconfigPaths: true }, other() {} }", Some(Severity::Info)),
        ("resolve method", "export default { resolve: { alias: {}, tsconfigPaths: true, other() {} } }", Some(Severity::Info)),
        ("callback", "export default defineConfig(() => ({ resolve: { tsconfigPaths: true } }))", Some(Severity::Info)),
        ("callback block", "export default defineConfig(() => { return { resolve: { tsconfigPaths: true } }; })", Some(Severity::Info)),
        ("conditional config", "export default { resolve: { tsconfigPaths: true } } || other", Some(Severity::Info)),
        ("conditional resolve", "export default { resolve: { tsconfigPaths: true } || other }", Some(Severity::Info)),
        ("extra argument", "export default defineConfig({ resolve: { tsconfigPaths: true } }, other)", Some(Severity::Info)),
        ("wrapper", "export default wrapper({ resolve: { tsconfigPaths: true } })", None),
        ("variable config", "const config = { resolve: { tsconfigPaths: true } }; export default config", None),
        ("malformed", "export default { resolve: { tsconfigPaths: true", None),
        ("truncated comment", "export default { resolve: { tsconfigPaths: true /*", None),
        ("utf8 text", "const note = '한글 문자열'; export default { resolve: { tsconfigPaths: true } }", None),
        ("mismatch", "export default { resolve: { alias: { '@app': './wrong' }, tsconfigPaths: true } }", Some(Severity::Warn)),
        ("regex prefix legacy mismatch", "const pattern = /foo/; export default { resolve: { alias: { '@app': './wrong' }, tsconfigPaths: true } };", Some(Severity::Warn)),
        ("escaped key legacy mismatch", r"export default { 'res\olve': { alias: { '@app': './wrong' }, tsconfigPaths: true } }", Some(Severity::Warn)),
        ("mismatch root spread", "export default { resolve: { alias: { '@app': './wrong' }, tsconfigPaths: true }, ...other }", Some(Severity::Warn)),
        ("mismatch resolve spread", "export default { resolve: { alias: { '@app': './wrong' }, tsconfigPaths: true, ...other } }", Some(Severity::Warn)),
        ("callback mismatch", "export default defineConfig(() => ({ resolve: { alias: { '@app': './wrong' }, tsconfigPaths: true } }))", Some(Severity::Warn)),
        ("match", "export default { resolve: { alias: { '@app': './src' }, tsconfigPaths: true } }", None),
        ("vite only", "export default { resolve: { alias: { '@other': './other' }, tsconfigPaths: true } }", Some(Severity::Info)),
    ];
    for (name, config, expected) in cases {
        let fixture = tempfile::TempDir::new().expect("temp dir should exist");
        let root = fixture.path();
        std::fs::write(root.join("package.json"), "{}").expect("package should write");
        std::fs::write(
            root.join("base.json"),
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}"#,
        )
        .expect("base should write");
        std::fs::write(root.join("tsconfig.json"), r#"{"extends":"./base.json"}"#)
            .expect("tsconfig should write");
        std::fs::write(root.join("vite.config.mjs"), config).expect("Vite config should write");
        let project = discover_project(root).expect("project should discover");
        let outcome = run_vite_tsconfig_alias_check(&project).expect("check should run");
        assert_eq!(
            outcome.findings.len(),
            usize::from(expected.is_some()),
            "{name}: {:?}",
            outcome.findings
        );
        if let Some(severity) = expected {
            let finding = &outcome.findings[0];
            assert_eq!(finding.severity, severity, "{name}");
            assert!(!finding.fixable, "{name}");
            assert_eq!(finding.category, "vite-tsconfig-alias", "{name}");
            assert!(finding.id.starts_with("vite-alias-sync:"), "{name}");
            if severity == Severity::Info {
                assert!(
                    finding.id.ends_with(if name == "vite only" {
                        ":vite-only"
                    } else {
                        ":tsconfig-only"
                    }),
                    "{name}"
                );
            }
        }
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/fixtures/vite-tsconfig-alias-sync")
        .join(name)
}

fn assert_has_finding(
    findings: &[maximus_core::Finding],
    id: &str,
    severity: Severity,
    title: &str,
    detail_prefix: &str,
    hint: &str,
    file: Option<std::path::PathBuf>,
) {
    let finding = findings
        .iter()
        .find(|finding| finding.id == id)
        .unwrap_or_else(|| panic!("missing finding {id}"));

    assert_eq!(finding.severity, severity);
    assert_eq!(finding.title, title);
    assert!(
        finding.detail.starts_with(detail_prefix),
        "unexpected detail: {}",
        finding.detail
    );
    assert_eq!(finding.hint, hint);
    assert_eq!(finding.file, file);
}
