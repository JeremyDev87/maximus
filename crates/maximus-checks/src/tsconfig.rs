use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use maximus_core::{
    find_nearest_package_file, get_files, make_finding, parse_jsonc, path_exists,
    read_text_if_exists, FileKind, Finding, FindingInput, ProjectSnapshot, Severity,
};
use serde_json::{Map, Value};

use crate::check_outcome::CheckOutcome;

const DEPRECATED_COMPILER_OPTIONS: &[(&str, &str)] = &[
    (
        "charset",
        "Remove it. TypeScript ignores this option in modern versions.",
    ),
    (
        "importsNotUsedAsValues",
        "Prefer verbatimModuleSyntax in modern TypeScript.",
    ),
    (
        "keyofStringsOnly",
        "Remove it. Modern TypeScript no longer needs this compatibility flag.",
    ),
    (
        "noStrictGenericChecks",
        "Remove it and rely on strict mode checks instead.",
    ),
    ("out", "Use outFile if you truly need single-file emit."),
    (
        "preserveValueImports",
        "Prefer verbatimModuleSyntax in modern TypeScript.",
    ),
    (
        "suppressExcessPropertyErrors",
        "Remove it. This hides useful structural typing errors.",
    ),
    (
        "suppressImplicitAnyIndexErrors",
        "Remove it. This suppresses important type safety signals.",
    ),
];

const CHECKABLE_EXTENSIONS: &[&str] = &[
    ".cjs", ".cts", ".js", ".json", ".jsx", ".mjs", ".mts", ".ts", ".tsx",
];
const TS_PATTERN_EXTENSIONS: &[&str] =
    &[".cts", ".d.cts", ".d.mts", ".d.ts", ".mts", ".ts", ".tsx"];
const TS_PATTERN_EXTENSIONS_WITH_JS: &[&str] = &[
    ".cjs", ".cts", ".d.cts", ".d.mts", ".d.ts", ".js", ".jsx", ".mjs", ".mts", ".ts", ".tsx",
];

enum CompositeResolution {
    Enabled,
    Disabled,
    Issue {
        suffix: &'static str,
        title: &'static str,
        detail: String,
        hint: &'static str,
    },
}

#[derive(Default)]
struct EffectivePatternOptions {
    allow_js: bool,
    out_dir: Option<PathBuf>,
    root_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct EffectivePatternField {
    values: Vec<String>,
    base_dir: PathBuf,
    config_path: PathBuf,
}

#[derive(Default)]
struct EffectivePatternConfig {
    options: EffectivePatternOptions,
    include: Option<EffectivePatternField>,
    exclude: Option<EffectivePatternField>,
    files: Option<EffectivePatternField>,
    issues: Vec<PatternFieldIssue>,
}

struct PatternFieldIssue {
    field_name: &'static str,
    config_path: PathBuf,
    suffix: String,
    title: String,
    detail: String,
    hint: &'static str,
}

enum ExtendedPatternConfigDocument {
    None,
    Loaded(PathBuf, Value),
    Issue(PatternFieldIssue),
}

pub fn run_tsconfig_check(project: &ProjectSnapshot) -> io::Result<CheckOutcome> {
    let mut findings = Vec::new();

    for file in get_files(project, FileKind::Tsconfig) {
        let Some(text) = read_text_if_exists(&file.path)? else {
            continue;
        };

        let config = match parse_jsonc::<Value>(&text, &file.path.to_string_lossy()) {
            Ok(config) => config,
            Err(error) => {
                findings.push(make_finding(FindingInput {
                    id: format!("tsconfig-parse:{}", file.path.to_string_lossy()),
                    title: "Config file could not be parsed".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(error.to_string()),
                    file: Some(file.path.clone()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Fix invalid JSONC syntax before relying on this config.".to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
                continue;
            }
        };

        let compiler_options = config.get("compilerOptions").and_then(Value::as_object);
        let references = config.get("references");

        collect_deprecated_option_findings(&mut findings, &file.path, compiler_options);
        collect_project_reference_findings(&mut findings, &file.path, &file.dir, references)?;
        collect_include_exclude_pattern_findings(
            &mut findings,
            project,
            &file.path,
            &file.dir,
            &config,
        )?;
        collect_output_path_overlap_findings(
            &mut findings,
            &project.root_dir,
            &file.path,
            &file.dir,
            &config,
        )?;
        collect_types_and_type_roots_findings(
            &mut findings,
            &file.path,
            &file.dir,
            compiler_options,
        )?;

        let Some(paths_config) = compiler_options.and_then(|options| options.get("paths")) else {
            continue;
        };

        if paths_config.is_null() {
            continue;
        }

        let Some(paths_config) = paths_config.as_object() else {
            findings.push(make_finding(FindingInput {
                id: format!("tsconfig-paths-shape:{}", file.path.to_string_lossy()),
                title: "compilerOptions.paths must be an object".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(
                    "TypeScript expects alias keys mapped to arrays of target strings.".to_string(),
                ),
                file: Some(file.path.clone()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some("Rewrite paths to the standard { alias: [targets] } shape.".to_string()),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        let base_dir = compiler_options
            .and_then(|options| options.get("baseUrl"))
            .and_then(Value::as_str)
            .map(|base_url| resolve_path(&file.dir, base_url))
            .unwrap_or_else(|| file.dir.clone());

        collect_paths_findings(&mut findings, &file.path, &base_dir, paths_config)?;
        collect_path_alias_shadowing_findings(&mut findings, &file.path, paths_config);

        if let Some(package_file) = find_nearest_package_file(project, &file.dir) {
            if let Some(package_text) = read_text_if_exists(&package_file.path)? {
                if let Ok(package_json) =
                    parse_jsonc::<Value>(&package_text, &package_file.path.to_string_lossy())
                {
                    let imports = package_json
                        .get("imports")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default();
                    compare_imports_and_paths(
                        &mut findings,
                        &file.path,
                        package_file
                            .path
                            .parent()
                            .unwrap_or(project.root_dir.as_path()),
                        &base_dir,
                        &imports,
                        paths_config,
                    )?;
                }
            }
        }
    }

    Ok(CheckOutcome {
        findings,
        fixes: Vec::new(),
        planned_fixes: Vec::new(),
    })
}

fn collect_deprecated_option_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    compiler_options: Option<&Map<String, Value>>,
) {
    let Some(compiler_options) = compiler_options else {
        return;
    };

    for (option, guidance) in DEPRECATED_COMPILER_OPTIONS {
        if !compiler_options.contains_key(*option) {
            continue;
        }

        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-deprecated:{}:{option}",
                file_path.to_string_lossy()
            ),
            title: format!("Deprecated compiler option \"{option}\""),
            category: Some("tsconfig".to_string()),
            detail: Some((*guidance).to_string()),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some("Remove legacy flags before they become upgrade blockers.".to_string()),
            severity: Some(Severity::Warn),
        }));
    }
}

fn collect_project_reference_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    config_dir: &Path,
    references: Option<&Value>,
) -> io::Result<()> {
    let Some(references) = references else {
        return Ok(());
    };
    let Some(references) = references.as_array() else {
        findings.push(make_finding(FindingInput {
            id: format!("tsconfig-references-shape:{}", file_path.to_string_lossy()),
            title: "references must be an array".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(
                "TypeScript project references must use an array of { path } entries.".to_string(),
            ),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Rewrite references to the standard [{ \"path\": \"../pkg\" }] shape.".to_string(),
            ),
            severity: Some(Severity::Error),
        }));
        return Ok(());
    };

    for (index, reference) in references.iter().enumerate() {
        let Some(reference_object) = reference.as_object() else {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-references-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "Each project reference entry must be an object with a path".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "references[{index}] must be an object like {{ \"path\": \"../pkg\" }}."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Replace malformed reference entries with explicit { path } objects."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        };
        let Some(reference_path) = reference_object.get("path").and_then(Value::as_str) else {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-references-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "Each project reference entry must declare a string path".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "references[{index}] must declare a non-empty string path."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Use { \"path\": \"../pkg\" } entries so TypeScript can resolve referenced projects."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        };
        if reference_path.trim().is_empty() {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-references-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "Each project reference entry must declare a string path".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "references[{index}] must declare a non-empty string path."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Use { \"path\": \"../pkg\" } entries so TypeScript can resolve referenced projects."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        }

        let Some(target_config_path) = resolve_reference_config_path(config_dir, reference_path)?
        else {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-references:{}:{reference_path}:missing",
                    file_path.to_string_lossy()
                ),
                title: "Project reference target does not exist".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{reference_path} does not resolve to an existing tsconfig file."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Update stale project references before they break TypeScript build mode."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        let target_text = match read_text_if_exists(&target_config_path) {
            Ok(Some(target_text)) => target_text,
            Ok(None) => {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-references:{}:{reference_path}:unreadable",
                        file_path.to_string_lossy()
                    ),
                    title: "Project reference target could not be read".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "{reference_path} resolves to {}, but the file could not be read.",
                        target_config_path.to_string_lossy()
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Make sure referenced tsconfig files are readable before relying on project references."
                            .to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-references:{}:{reference_path}:unreadable",
                        file_path.to_string_lossy()
                    ),
                    title: "Project reference target could not be read".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "{reference_path} resolves to {}, but reading it failed: {error}.",
                        target_config_path.to_string_lossy()
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Make sure referenced tsconfig files are readable before relying on project references."
                            .to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
                continue;
            }
            Err(error) => return Err(error),
        };

        let target_config = match parse_jsonc::<Value>(
            &target_text,
            &target_config_path.to_string_lossy(),
        ) {
            Ok(target_config) => target_config,
            Err(error) => {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-references:{}:{reference_path}:parse",
                        file_path.to_string_lossy()
                    ),
                    title: "Project reference target could not be parsed".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(error.to_string()),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Fix invalid JSONC syntax in referenced tsconfig files before relying on project references."
                            .to_string(),
                    ),
                    severity: Some(Severity::Error),
                }));
                continue;
            }
        };

        if !looks_like_tsconfig_document(&target_config_path, &target_config) {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-references:{}:{reference_path}:invalid-target",
                    file_path.to_string_lossy()
                ),
                title: "Project reference target must point to a tsconfig file".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{reference_path} resolves to {}, but that file does not look like a tsconfig document.",
                    target_config_path.to_string_lossy()
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Point project references at a directory with tsconfig.json or an explicit tsconfig-style JSON file."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        }

        match resolve_effective_composite(&target_config_path, &target_config)? {
            CompositeResolution::Enabled => continue,
            CompositeResolution::Disabled => {}
            CompositeResolution::Issue {
                suffix,
                title,
                detail,
                hint,
            } => {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-references:{}:{reference_path}:extends-{suffix}",
                        file_path.to_string_lossy()
                    ),
                    title: title.to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(detail),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(hint.to_string()),
                    severity: Some(Severity::Error),
                }));
                continue;
            }
        }

        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-references:{}:{reference_path}:composite",
                file_path.to_string_lossy()
            ),
            title: "Referenced project must enable composite".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "{reference_path} resolves to {}, but compilerOptions.composite is not true.",
                target_config_path.to_string_lossy()
            )),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Enable composite on referenced projects so TypeScript build mode can consume them reliably."
                    .to_string(),
            ),
            severity: Some(Severity::Error),
        }));
    }

    Ok(())
}

fn collect_paths_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    base_dir: &Path,
    paths_config: &Map<String, Value>,
) -> io::Result<()> {
    for (alias, targets) in paths_config {
        let Some(targets) = targets.as_array() else {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-paths-empty:{}:{alias}",
                    file_path.to_string_lossy()
                ),
                title: format!("Alias \"{alias}\" does not declare any targets"),
                category: Some("tsconfig".to_string()),
                detail: Some(
                    "Each path alias should map to at least one target string.".to_string(),
                ),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some("Add a valid target or remove the alias entry.".to_string()),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        if targets.is_empty() {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-paths-empty:{}:{alias}",
                    file_path.to_string_lossy()
                ),
                title: format!("Alias \"{alias}\" does not declare any targets"),
                category: Some("tsconfig".to_string()),
                detail: Some(
                    "Each path alias should map to at least one target string.".to_string(),
                ),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some("Add a valid target or remove the alias entry.".to_string()),
                severity: Some(Severity::Error),
            }));
            continue;
        }

        let alias_has_wildcard = alias.contains('*');
        let mut missing_targets = Vec::new();
        let mut has_existing_string_target = false;
        for target in targets {
            let Some(target) = target.as_str() else {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-paths-type:{}:{alias}",
                        file_path.to_string_lossy()
                    ),
                    title: format!("Alias \"{alias}\" contains a non-string target"),
                    category: Some("tsconfig".to_string()),
                    detail: Some("TypeScript path targets must be strings.".to_string()),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some("Replace non-string entries with valid path strings.".to_string()),
                    severity: Some(Severity::Error),
                }));
                continue;
            };

            let target_has_wildcard = target.contains('*');
            if alias_has_wildcard != target_has_wildcard {
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-paths-wildcard:{}:{alias}:{target}",
                        file_path.to_string_lossy()
                    ),
                    title: format!("Wildcard shape does not match for alias \"{alias}\""),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "{alias} maps to {target}, but only one side uses \"*\"."
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Keep wildcard placement aligned so imports resolve predictably."
                            .to_string(),
                    ),
                    severity: Some(Severity::Warn),
                }));
            }

            if alias_target_exists(base_dir, target)? {
                has_existing_string_target = true;
            } else {
                missing_targets.push(target.to_string());
            }
        }

        if has_existing_string_target {
            continue;
        }

        for target in missing_targets {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-paths-missing:{}:{alias}:{target}",
                    file_path.to_string_lossy()
                ),
                title: "Path alias target does not exist".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{alias} points to {target}, but the resolved path was not found."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Update or remove stale aliases before they break editor and build resolution."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
        }
    }

    Ok(())
}

fn collect_path_alias_shadowing_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    paths_config: &Map<String, Value>,
) {
    let aliases = paths_config.keys().cloned().collect::<Vec<_>>();

    for alias in &aliases {
        if !is_package_style_alias(alias) {
            continue;
        }

        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-paths-shadow-package:{}:{alias}",
                file_path.to_string_lossy()
            ),
            title: format!("Path alias \"{alias}\" shadows a package import"),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "\"{alias}\" is a bare package-style specifier, so this alias can override an installed package or workspace package with the same import path."
            )),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Prefer #internal/* or another dedicated namespace for app-local aliases so package imports stay unambiguous."
                    .to_string(),
            ),
            severity: Some(Severity::Warn),
        }));
    }

    for left_index in 0..aliases.len() {
        for right_index in left_index + 1..aliases.len() {
            let left = &aliases[left_index];
            let right = &aliases[right_index];
            let Some((shadowing_alias, shadowed_alias)) =
                determine_shadowing_alias_pair(left, right)
            else {
                continue;
            };
            if is_exact_specialization_of_wildcard_alias(shadowing_alias, shadowed_alias) {
                continue;
            }

            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-paths-shadow-alias:{}:{shadowing_alias}:{shadowed_alias}",
                    file_path.to_string_lossy()
                ),
                title: format!(
                    "Path alias \"{shadowing_alias}\" shadows \"{shadowed_alias}\""
                ),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "Both aliases can match the same import specifier, so TypeScript will prefer \"{shadowing_alias}\" and hide \"{shadowed_alias}\" for those imports."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Make overlapping aliases disjoint so imports keep a single obvious target."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        }
    }
}

fn collect_types_and_type_roots_findings(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    config_dir: &Path,
    compiler_options: Option<&Map<String, Value>>,
) -> io::Result<()> {
    let Some(compiler_options) = compiler_options else {
        return Ok(());
    };

    let types = parse_types_option(findings, file_path, compiler_options);
    let type_roots = parse_type_roots_option(findings, file_path, compiler_options);

    if let Some(type_roots) = type_roots.as_ref() {
        for type_root in type_roots {
            let resolved_type_root = resolve_path_with_backslash_support(config_dir, type_root);
            if path_exists(&resolved_type_root) {
                continue;
            }

            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-typeroots-missing:{}:{type_root}",
                    file_path.to_string_lossy()
                ),
                title: "Configured typeRoots entry does not exist".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "compilerOptions.typeRoots includes \"{type_root}\", but the resolved path was not found."
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Create the missing types directory or remove stale typeRoots entries before TypeScript silently skips expected declarations."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        }
    }

    match (types.as_ref(), type_roots.as_ref()) {
        (Some(types), Some(type_roots)) => {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-types-typeroots-guidance:{}",
                    file_path.to_string_lossy()
                ),
                title: "compilerOptions.types and typeRoots both narrow ambient type resolution"
                    .to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "compilerOptions.types only includes {}, and compilerOptions.typeRoots only searches {}, so unlisted ambient packages outside those roots will be hidden from TypeScript.",
                    format_string_list(types),
                    format_string_list(type_roots)
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Keep both lists aligned with every global types package your runtime and tests rely on."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        }
        (Some(types), None) => {
            let (title, detail, severity) = if types.is_empty() {
                (
                    "compilerOptions.types disables automatic @types inclusion".to_string(),
                    "compilerOptions.types is set to [], so TypeScript will not auto-include any ambient @types packages.".to_string(),
                    Severity::Warn,
                )
            } else {
                (
                    "compilerOptions.types limits ambient type packages".to_string(),
                    format!(
                        "compilerOptions.types only includes {}, so unlisted ambient @types packages will not be injected automatically.",
                        format_string_list(types)
                    ),
                    Severity::Info,
                )
            };

            findings.push(make_finding(FindingInput {
                id: format!("tsconfig-types-guidance:{}", file_path.to_string_lossy()),
                title,
                category: Some("tsconfig".to_string()),
                detail: Some(detail),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Keep this list in sync with every test and runtime package that should contribute global types."
                        .to_string(),
                ),
                severity: Some(severity),
            }));
        }
        (None, Some(type_roots)) => {
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-typeroots-guidance:{}",
                    file_path.to_string_lossy()
                ),
                title: "compilerOptions.typeRoots disables default @types discovery".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "compilerOptions.typeRoots only searches {}, so TypeScript will stop using the default node_modules/@types lookup for this config.",
                    format_string_list(type_roots)
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Include every required ambient types directory or remove typeRoots to restore default discovery."
                        .to_string(),
                ),
                severity: Some(Severity::Warn),
            }));
        }
        (None, None) => {}
    }

    Ok(())
}

fn parse_types_option(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    compiler_options: &Map<String, Value>,
) -> Option<Vec<String>> {
    let Some(types_value) = compiler_options.get("types") else {
        return None;
    };
    let Some(types) = types_value.as_array() else {
        findings.push(make_finding(FindingInput {
            id: format!("tsconfig-types-shape:{}", file_path.to_string_lossy()),
            title: "\"compilerOptions.types\" must be an array of package names".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(
                "TypeScript expects compilerOptions.types to be an array of string package names."
                    .to_string(),
            ),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Rewrite compilerOptions.types as [\"node\", \"jest\"]-style package names or remove it."
                    .to_string(),
            ),
            severity: Some(Severity::Error),
        }));
        return None;
    };

    let mut collected_types = Vec::new();
    let mut has_invalid_entry = false;
    for (index, value) in types.iter().enumerate() {
        let Some(type_name) = value.as_str() else {
            has_invalid_entry = true;
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-types-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "\"compilerOptions.types\" contains a non-string package name"
                    .to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{} declares compilerOptions.types[{index}], but TypeScript expects a non-empty string package name.",
                    file_path.to_string_lossy()
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Rewrite compilerOptions.types as [\"node\", \"jest\"]-style package names or remove it."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        if type_name.trim().is_empty() {
            has_invalid_entry = true;
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-types-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "\"compilerOptions.types\" contains a non-string package name"
                    .to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{} declares compilerOptions.types[{index}], but TypeScript expects a non-empty string package name.",
                    file_path.to_string_lossy()
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Rewrite compilerOptions.types as [\"node\", \"jest\"]-style package names or remove it."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        }

        collected_types.push(type_name.trim().to_string());
    }

    if has_invalid_entry {
        return None;
    }

    Some(collected_types)
}

fn parse_type_roots_option(
    findings: &mut Vec<Finding>,
    file_path: &Path,
    compiler_options: &Map<String, Value>,
) -> Option<Vec<String>> {
    let Some(type_roots_value) = compiler_options.get("typeRoots") else {
        return None;
    };
    let Some(type_roots) = type_roots_value.as_array() else {
        findings.push(make_finding(FindingInput {
            id: format!("tsconfig-typeroots-shape:{}", file_path.to_string_lossy()),
            title: "\"compilerOptions.typeRoots\" must be an array of directory paths"
                .to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(
                "TypeScript expects compilerOptions.typeRoots to be an array of string directory paths."
                    .to_string(),
            ),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Rewrite compilerOptions.typeRoots as [\"./types\", \"./node_modules/@types\"]-style paths or remove it."
                    .to_string(),
            ),
            severity: Some(Severity::Error),
        }));
        return None;
    };

    let mut collected_type_roots = Vec::new();
    let mut has_invalid_entry = false;
    for (index, value) in type_roots.iter().enumerate() {
        let Some(type_root) = value.as_str() else {
            has_invalid_entry = true;
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-typeroots-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "\"compilerOptions.typeRoots\" contains a non-string path".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{} declares compilerOptions.typeRoots[{index}], but TypeScript expects a non-empty string directory path.",
                    file_path.to_string_lossy()
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Rewrite compilerOptions.typeRoots as [\"./types\", \"./node_modules/@types\"]-style paths or remove it."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        };

        if type_root.trim().is_empty() {
            has_invalid_entry = true;
            findings.push(make_finding(FindingInput {
                id: format!(
                    "tsconfig-typeroots-entry:{}:{index}",
                    file_path.to_string_lossy()
                ),
                title: "\"compilerOptions.typeRoots\" contains a non-string path".to_string(),
                category: Some("tsconfig".to_string()),
                detail: Some(format!(
                    "{} declares compilerOptions.typeRoots[{index}], but TypeScript expects a non-empty string directory path.",
                    file_path.to_string_lossy()
                )),
                file: Some(file_path.to_path_buf()),
                fix_ids: Vec::new(),
                fixable: false,
                hint: Some(
                    "Rewrite compilerOptions.typeRoots as [\"./types\", \"./node_modules/@types\"]-style paths or remove it."
                        .to_string(),
                ),
                severity: Some(Severity::Error),
            }));
            continue;
        }

        collected_type_roots.push(type_root.trim().to_string());
    }

    if has_invalid_entry {
        return None;
    }

    Some(collected_type_roots)
}

fn collect_include_exclude_pattern_findings(
    findings: &mut Vec<Finding>,
    project: &ProjectSnapshot,
    file_path: &Path,
    config_dir: &Path,
    config: &Value,
) -> io::Result<()> {
    let base_dir = normalize_path(config_dir);
    let effective_pattern_config = resolve_effective_pattern_config(file_path, config)?;
    let has_next_dependency = nearest_package_has_next_dependency(project, config_dir)?;

    for issue in &effective_pattern_config.issues {
        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-patterns-shape:{}:{}:{}:{}",
                file_path.to_string_lossy(),
                issue.field_name,
                issue.suffix,
                issue.config_path.to_string_lossy()
            ),
            title: issue.title.clone(),
            category: Some("tsconfig".to_string()),
            detail: Some(issue.detail.clone()),
            file: Some(issue.config_path.clone()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(issue.hint.to_string()),
            severity: Some(Severity::Error),
        }));
    }

    let candidate_extensions = ts_pattern_extensions(effective_pattern_config.options.allow_js);
    let default_excluded_dirs = collect_default_excluded_dirs(&effective_pattern_config.options);
    let (explicit_files, explicit_file_issues) = collect_explicit_tsconfig_files(
        effective_pattern_config.files.as_ref(),
        candidate_extensions,
    )?;
    for issue in explicit_file_issues {
        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-patterns-shape:{}:{}:{}:{}",
                file_path.to_string_lossy(),
                issue.field_name,
                issue.suffix,
                issue.config_path.to_string_lossy()
            ),
            title: issue.title,
            category: Some("tsconfig".to_string()),
            detail: Some(issue.detail),
            file: Some(issue.config_path),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(issue.hint.to_string()),
            severity: Some(Severity::Error),
        }));
    }
    let mut exclude_eligible_files = if effective_pattern_config.include.is_some()
        || effective_pattern_config.files.is_some()
    {
        BTreeSet::new()
    } else {
        collect_default_pattern_matches(&base_dir, candidate_extensions, &default_excluded_dirs)?
    };

    if let Some(include_field) = effective_pattern_config.include.as_ref() {
        for pattern in &include_field.values {
            let matches = collect_pattern_matches(
                &include_field.base_dir,
                pattern,
                candidate_extensions,
                &default_excluded_dirs,
            )?;
            if matches.is_empty() {
                let is_next_generated_types_pattern =
                    has_next_dependency && is_next_generated_types_pattern(pattern);
                let (severity, hint) = if is_next_generated_types_pattern {
                    (
                        Severity::Info,
                        "Next.js generates .next/types during development or build, so this include can be empty before .next exists.",
                    )
                } else {
                    (
                        Severity::Warn,
                        "Fix or remove empty include patterns before TypeScript silently skips expected inputs.",
                    )
                };
                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-patterns:{}:include:{pattern}",
                        file_path.to_string_lossy()
                    ),
                    title: "Include pattern does not match any files".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "include pattern \"{pattern}\" matched 0 files under base dir {}.",
                        include_field.base_dir.to_string_lossy()
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(hint.to_string()),
                    severity: Some(severity),
                }));
                continue;
            }

            exclude_eligible_files.extend(matches);
        }
    }

    let mut included_files = explicit_files.clone();
    included_files.extend(exclude_eligible_files.iter().cloned());

    if included_files.is_empty() {
        return Ok(());
    }

    if let Some(exclude_field) = effective_pattern_config.exclude.as_ref() {
        for pattern in &exclude_field.values {
            let effective_included_count = explicit_files.len() + exclude_eligible_files.len();
            let matches = exclude_eligible_files
                .iter()
                .filter(|candidate| {
                    pattern_matches_file(
                        &exclude_field.base_dir,
                        pattern,
                        candidate,
                        candidate_extensions,
                        &default_excluded_dirs,
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            let removed_count = matches.len();

            if removed_count == 0 {
                if is_default_excluded_directory_pattern(pattern) {
                    continue;
                }

                findings.push(make_finding(FindingInput {
                    id: format!(
                        "tsconfig-patterns:{}:exclude:{pattern}",
                        file_path.to_string_lossy()
                    ),
                    title: "Exclude pattern does not filter any included files".to_string(),
                    category: Some("tsconfig".to_string()),
                    detail: Some(format!(
                        "exclude pattern \"{pattern}\" removed 0 files from {} included file(s) under base dir {}.",
                        effective_included_count,
                        exclude_field.base_dir.to_string_lossy()
                    )),
                    file: Some(file_path.to_path_buf()),
                    fix_ids: Vec::new(),
                    fixable: false,
                    hint: Some(
                        "Remove or tighten exclude entries that do not change the effective TypeScript input set."
                            .to_string(),
                    ),
                    severity: Some(Severity::Info),
                }));
                continue;
            }

            for matched in matches {
                exclude_eligible_files.remove(&matched);
            }
        }
    }

    Ok(())
}

fn nearest_package_has_next_dependency(
    project: &ProjectSnapshot,
    config_dir: &Path,
) -> io::Result<bool> {
    let Some(package_file) = find_nearest_package_file(project, config_dir) else {
        return Ok(false);
    };
    let Some(package_text) = read_text_if_exists(&package_file.path)? else {
        return Ok(false);
    };
    let Ok(package_json) =
        parse_jsonc::<Value>(&package_text, &package_file.path.to_string_lossy())
    else {
        return Ok(false);
    };

    Ok(
        package_has_dependency(&package_json, "dependencies", "next")
            || package_has_dependency(&package_json, "devDependencies", "next")
            || package_has_dependency(&package_json, "peerDependencies", "next")
            || package_has_dependency(&package_json, "optionalDependencies", "next"),
    )
}

fn package_has_dependency(package_json: &Value, field_name: &str, dependency_name: &str) -> bool {
    package_json
        .get(field_name)
        .and_then(Value::as_object)
        .is_some_and(|dependencies| dependencies.contains_key(dependency_name))
}

fn is_next_generated_types_pattern(pattern: &str) -> bool {
    pattern == ".next/types/**/*.ts" || pattern == "./.next/types/**/*.ts"
}

fn is_default_excluded_directory_pattern(pattern: &str) -> bool {
    let normalized = pattern.trim().replace('\\', "/");
    let normalized = normalized
        .strip_prefix("./")
        .unwrap_or(&normalized)
        .trim_end_matches('/');

    matches!(
        normalized,
        "node_modules" | "bower_components" | "jspm_packages"
    )
}

fn collect_output_path_overlap_findings(
    findings: &mut Vec<Finding>,
    project_root: &Path,
    file_path: &Path,
    config_dir: &Path,
    config: &Value,
) -> io::Result<()> {
    let base_dir = normalize_path(config_dir);
    let effective_pattern_config = resolve_effective_pattern_config(file_path, config)?;
    let Some(output_dir) = effective_pattern_config.options.out_dir.clone() else {
        return Ok(());
    };
    let output_dir = normalize_path(&output_dir);
    let candidate_extensions = ts_pattern_extensions(effective_pattern_config.options.allow_js);
    let default_excluded_dirs = collect_default_excluded_dirs(&effective_pattern_config.options);
    let effective_source_files = collect_effective_tsconfig_input_files(
        &base_dir,
        &effective_pattern_config,
        candidate_extensions,
        &default_excluded_dirs,
    )?;
    let raw_source_files = if output_dir == base_dir && effective_source_files.is_empty() {
        collect_effective_tsconfig_input_files(
            &base_dir,
            &effective_pattern_config,
            candidate_extensions,
            &[],
        )?
    } else {
        BTreeSet::new()
    };
    let source_roots = collect_output_source_roots(
        &base_dir,
        &effective_pattern_config,
        candidate_extensions,
        &effective_source_files,
        &default_excluded_dirs,
    )?;

    if let Some(source_root) = source_roots
        .iter()
        .find(|source_root| output_dir == **source_root)
    {
        let output_display = display_project_relative_path(project_root, &output_dir);
        let source_display = display_project_relative_path(project_root, source_root);
        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-output-paths:{}:outdir-equals-source:{output_display}",
                file_path.to_string_lossy()
            ),
            title: "Output directory overlaps the TypeScript source root".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "outDir \"{output_display}\" overlaps source root \"{source_display}\"."
            )),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Move emit output outside the source root so build artifacts do not overwrite source files."
                    .to_string(),
            ),
            severity: Some(Severity::Error),
        }));
        return Ok(());
    }

    if let Some(source_root) = source_roots
        .iter()
        .find(|source_root| output_dir.starts_with(*source_root))
    {
        let output_display = display_project_relative_path(project_root, &output_dir);
        let source_display = display_project_relative_path(project_root, source_root);
        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-output-paths:{}:outdir-nested-in-source:{output_display}",
                file_path.to_string_lossy()
            ),
            title: "Output directory is nested inside the TypeScript source root".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "outDir \"{output_display}\" is nested inside source root \"{source_display}\"."
            )),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Move emit output outside the source root so build artifacts do not overwrite source files."
                    .to_string(),
            ),
            severity: Some(Severity::Error),
        }));
        return Ok(());
    }

    if let Some(overlapping_input) = effective_source_files
        .iter()
        .find(|candidate| candidate.starts_with(&output_dir))
        .or_else(|| {
            raw_source_files
                .iter()
                .find(|candidate| candidate.starts_with(&output_dir))
        })
    {
        let output_display = display_project_relative_path(project_root, &output_dir);
        let input_display = display_project_relative_path(project_root, overlapping_input);
        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-output-paths:{}:outdir-contains-input:{output_display}",
                file_path.to_string_lossy()
            ),
            title: "Output directory contains TypeScript input files".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "outDir \"{output_display}\" contains TypeScript input \"{input_display}\"."
            )),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Move emit output outside any directory that currently contains TypeScript input files."
                    .to_string(),
            ),
            severity: Some(Severity::Error),
        }));
        return Ok(());
    }

    if let Some(source_root) = source_roots
        .iter()
        .find(|source_root| source_root.starts_with(&output_dir))
    {
        let output_display = display_project_relative_path(project_root, &output_dir);
        let source_display = display_project_relative_path(project_root, source_root);
        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-output-paths:{}:outdir-contains-source:{output_display}",
                file_path.to_string_lossy()
            ),
            title: "Output directory contains the TypeScript source root".to_string(),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "outDir \"{output_display}\" contains source root \"{source_display}\"."
            )),
            file: Some(file_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Prefer an output directory that is completely separate from the TypeScript source root."
                    .to_string(),
            ),
            severity: Some(Severity::Warn),
        }));
    }

    Ok(())
}

fn collect_effective_tsconfig_input_files(
    base_dir: &Path,
    effective_pattern_config: &EffectivePatternConfig,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
) -> io::Result<BTreeSet<PathBuf>> {
    let (explicit_files, _) = collect_explicit_tsconfig_files(
        effective_pattern_config.files.as_ref(),
        candidate_extensions,
    )?;
    let mut exclude_eligible_files =
        if effective_pattern_config.include.is_some() || effective_pattern_config.files.is_some() {
            BTreeSet::new()
        } else {
            collect_default_pattern_matches(base_dir, candidate_extensions, default_excluded_dirs)?
        };

    if let Some(include_field) = effective_pattern_config.include.as_ref() {
        for pattern in &include_field.values {
            let matches = collect_pattern_matches(
                &include_field.base_dir,
                pattern,
                candidate_extensions,
                default_excluded_dirs,
            )?;
            exclude_eligible_files.extend(matches);
        }
    }

    if let Some(exclude_field) = effective_pattern_config.exclude.as_ref() {
        for pattern in &exclude_field.values {
            let matches = exclude_eligible_files
                .iter()
                .filter(|candidate| {
                    pattern_matches_file(
                        &exclude_field.base_dir,
                        pattern,
                        candidate,
                        candidate_extensions,
                        default_excluded_dirs,
                    )
                })
                .cloned()
                .collect::<Vec<_>>();

            for matched in matches {
                exclude_eligible_files.remove(&matched);
            }
        }
    }

    let mut included_files = explicit_files;
    included_files.extend(exclude_eligible_files);
    Ok(included_files)
}

fn collect_output_source_roots(
    base_dir: &Path,
    effective_pattern_config: &EffectivePatternConfig,
    candidate_extensions: &[&str],
    included_files: &BTreeSet<PathBuf>,
    default_excluded_dirs: &[PathBuf],
) -> io::Result<BTreeSet<PathBuf>> {
    let mut source_roots = BTreeSet::new();

    if let Some(root_dir) = effective_pattern_config.options.root_dir.as_ref() {
        source_roots.insert(normalize_path(root_dir));
    }

    if let Some(include_field) = effective_pattern_config.include.as_ref() {
        for pattern in &include_field.values {
            let matches = collect_pattern_matches(
                &include_field.base_dir,
                pattern,
                candidate_extensions,
                default_excluded_dirs,
            )?;
            if matches.is_empty() {
                continue;
            }

            if let Some(source_root) = resolve_pattern_source_root(&include_field.base_dir, pattern)
            {
                source_roots.insert(source_root);
            }
        }
    }

    for file in included_files {
        if let Some(parent) = file.parent() {
            let parent = normalize_path(parent);
            if parent != *base_dir {
                source_roots.insert(parent);
            }
        }
    }

    Ok(prune_nested_paths(source_roots))
}

fn resolve_pattern_source_root(base_dir: &Path, pattern: &str) -> Option<PathBuf> {
    if pattern.trim().is_empty() {
        return None;
    }

    if pattern_contains_wildcard(pattern) {
        let prefix = pattern
            .find(['*', '?'])
            .map(|index| &pattern[..index])
            .unwrap_or(pattern);
        let search_prefix = wildcard_search_prefix(prefix);
        if matches!(search_prefix, "." | "") {
            return None;
        }
        return Some(resolve_path_with_backslash_support(base_dir, search_prefix));
    }

    let resolved = resolve_path_with_backslash_support(base_dir, pattern);
    if path_exists(&resolved) {
        match fs::metadata(&resolved) {
            Ok(metadata) if metadata.is_dir() => return Some(normalize_path(&resolved)),
            Ok(_) => return resolved.parent().map(normalize_path),
            Err(_) => return None,
        }
    }

    if has_explicit_extension(pattern) {
        return resolved.parent().map(normalize_path);
    }

    Some(normalize_path(&resolved))
}

fn prune_nested_paths(paths: BTreeSet<PathBuf>) -> BTreeSet<PathBuf> {
    let mut pruned = BTreeSet::new();

    for path in paths {
        if pruned
            .iter()
            .any(|existing: &PathBuf| path.starts_with(existing))
        {
            continue;
        }
        pruned.retain(|existing: &PathBuf| !existing.starts_with(&path));
        pruned.insert(path);
    }

    pruned
}

fn display_project_relative_path(project_root: &Path, path: &Path) -> String {
    let project_root = normalize_path(project_root);
    let path = normalize_path(path);

    match path.strip_prefix(&project_root) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => normalize_path_for_match(relative),
        Err(_) => normalize_path_for_match(&path),
    }
}

fn collect_explicit_tsconfig_files(
    files_field: Option<&EffectivePatternField>,
    candidate_extensions: &[&str],
) -> io::Result<(BTreeSet<PathBuf>, Vec<PatternFieldIssue>)> {
    let Some(files_field) = files_field else {
        return Ok((BTreeSet::new(), Vec::new()));
    };

    let mut explicit_files = BTreeSet::new();
    let mut issues = Vec::new();

    for (index, file) in files_field.values.iter().enumerate() {
        if file.trim().is_empty() {
            continue;
        }
        if pattern_contains_wildcard(file) {
            issues.push(PatternFieldIssue {
                field_name: "files",
                config_path: files_field.config_path.clone(),
                suffix: format!("entry-{index}-wildcard"),
                title: "\"files\" entries must point to explicit files".to_string(),
                detail: format!(
                    "{} declares files[{index}] as {file}, but TypeScript files entries cannot use glob wildcards.",
                    files_field.config_path.to_string_lossy()
                ),
                hint: pattern_field_hint("files"),
            });
            continue;
        }

        let resolved = resolve_path(&files_field.base_dir, file);
        if path_exists(&resolved) {
            let metadata = match fs::metadata(&resolved) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    issues.push(PatternFieldIssue {
                        field_name: "files",
                        config_path: files_field.config_path.clone(),
                        suffix: format!("entry-{index}-unreadable"),
                        title: "\"files\" entries must point to readable files".to_string(),
                        detail: format!(
                            "{} declares files[{index}] as {file}, but reading that path failed: {error}.",
                            files_field.config_path.to_string_lossy()
                        ),
                        hint: pattern_field_hint("files"),
                    });
                    continue;
                }
                Err(error) => return Err(error),
            };
            if metadata.is_dir() {
                issues.push(PatternFieldIssue {
                    field_name: "files",
                    config_path: files_field.config_path.clone(),
                    suffix: format!("entry-{index}-directory"),
                    title: "\"files\" entries must point to files".to_string(),
                    detail: format!(
                        "{} declares files[{index}] as {file}, but that path resolves to a directory.",
                        files_field.config_path.to_string_lossy()
                    ),
                    hint: pattern_field_hint("files"),
                });
                continue;
            }
            if metadata.is_file() && is_ts_pattern_candidate_file(&resolved, candidate_extensions) {
                explicit_files.insert(normalize_path(&resolved));
                continue;
            }

            issues.push(PatternFieldIssue {
                field_name: "files",
                config_path: files_field.config_path.clone(),
                suffix: format!("entry-{index}-unsupported"),
                title: "\"files\" entries must point to supported TypeScript input files".to_string(),
                detail: format!(
                    "{} declares files[{index}] as {file}, but that file is not a supported TypeScript input.",
                    files_field.config_path.to_string_lossy()
                ),
                hint: pattern_field_hint("files"),
            });
            continue;
        }

        if has_explicit_extension(file) {
            issues.push(PatternFieldIssue {
                field_name: "files",
                config_path: files_field.config_path.clone(),
                suffix: format!("entry-{index}-missing"),
                title: "\"files\" entries must point to existing files".to_string(),
                detail: format!(
                    "{} declares files[{index}] as {file}, but that path does not resolve to an existing file.",
                    files_field.config_path.to_string_lossy()
                ),
                hint: pattern_field_hint("files"),
            });
            continue;
        }

        let resolved_string = resolved.to_string_lossy();
        let mut matched_extension = false;
        for extension in candidate_extensions {
            let candidate = PathBuf::from(format!("{resolved_string}{extension}"));
            if path_exists(&candidate) && fs::metadata(&candidate)?.is_file() {
                explicit_files.insert(normalize_path(&candidate));
                matched_extension = true;
            }
        }
        if !matched_extension {
            issues.push(PatternFieldIssue {
                field_name: "files",
                config_path: files_field.config_path.clone(),
                suffix: format!("entry-{index}-missing"),
                title: "\"files\" entries must point to existing files".to_string(),
                detail: format!(
                    "{} declares files[{index}] as {file}, but that path does not resolve to an existing file.",
                    files_field.config_path.to_string_lossy()
                ),
                hint: pattern_field_hint("files"),
            });
        }
    }

    Ok((explicit_files, issues))
}

fn collect_default_excluded_dirs(
    effective_pattern_options: &EffectivePatternOptions,
) -> Vec<PathBuf> {
    effective_pattern_options.out_dir.iter().cloned().collect()
}

fn ts_pattern_extensions(allow_js: bool) -> &'static [&'static str] {
    if allow_js {
        TS_PATTERN_EXTENSIONS_WITH_JS
    } else {
        TS_PATTERN_EXTENSIONS
    }
}

fn should_skip_default_pattern_dir(path: &Path, default_excluded_dirs: &[PathBuf]) -> bool {
    let path = normalize_path(path);
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value, "node_modules" | "bower_components" | "jspm_packages"))
    {
        return true;
    }

    is_within_default_excluded_dir(&path, default_excluded_dirs)
}

fn is_within_default_excluded_dir(path: &Path, default_excluded_dirs: &[PathBuf]) -> bool {
    let path = normalize_path(path);
    default_excluded_dirs
        .iter()
        .any(|excluded_dir| path.starts_with(excluded_dir))
}

fn compare_imports_and_paths(
    findings: &mut Vec<Finding>,
    tsconfig_path: &Path,
    package_dir: &Path,
    tsconfig_base_dir: &Path,
    imports: &Map<String, Value>,
    paths_config: &Map<String, Value>,
) -> io::Result<()> {
    for (import_key, import_target) in imports {
        let Some(ts_targets) = paths_config.get(import_key).and_then(Value::as_array) else {
            continue;
        };
        let Some(effective_ts_target) =
            select_effective_tsconfig_target(tsconfig_base_dir, ts_targets)?
        else {
            continue;
        };
        let Some(normalized_effective_ts_target) =
            normalize_comparable_target(tsconfig_base_dir, &effective_ts_target)
        else {
            continue;
        };

        let normalized_import_targets = normalize_import_targets(package_dir, import_target);
        if normalized_import_targets.is_empty() {
            continue;
        }
        if normalized_import_targets.contains(&normalized_effective_ts_target) {
            continue;
        }

        findings.push(make_finding(FindingInput {
            id: format!(
                "tsconfig-import-conflict:{}:{import_key}",
                tsconfig_path.to_string_lossy()
            ),
            title: format!(
                "Alias \"{import_key}\" differs between tsconfig and package imports"
            ),
            category: Some("tsconfig".to_string()),
            detail: Some(format!(
                "tsconfig resolves to {effective_ts_target}, while package.json imports resolves to {}.",
                stringify_import_target(import_target)
            )),
            file: Some(tsconfig_path.to_path_buf()),
            fix_ids: Vec::new(),
            fixable: false,
            hint: Some(
                "Align both alias surfaces so runtime and editor resolution stay consistent."
                    .to_string(),
            ),
            severity: Some(Severity::Warn),
        }));
    }

    Ok(())
}

fn normalize_import_targets(package_dir: &Path, import_target: &Value) -> Vec<String> {
    let mut normalized = Vec::new();

    for target in collect_import_targets(import_target) {
        if let Some(comparable) = normalize_comparable_target(package_dir, &target) {
            if !normalized.contains(&comparable) {
                normalized.push(comparable);
            }
        }
    }

    normalized
}

fn collect_import_targets(import_target: &Value) -> Vec<String> {
    match import_target {
        Value::String(value) => vec![value.clone()],
        Value::Object(object) => object.values().flat_map(collect_import_targets).collect(),
        _ => Vec::new(),
    }
}

fn normalize_comparable_target(base_dir: &Path, target: &str) -> Option<String> {
    if target.is_empty() {
        return None;
    }

    Some(normalize_path_for_match(&resolve_path(base_dir, target)))
}

fn stringify_import_target(import_target: &Value) -> String {
    import_target
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| serde_json::to_string(import_target).unwrap_or_default())
}

fn resolve_reference_config_path(
    base_dir: &Path,
    reference_path: &str,
) -> io::Result<Option<PathBuf>> {
    let resolved = resolve_path(base_dir, reference_path);

    if path_exists(&resolved) {
        let metadata = fs::metadata(&resolved)?;
        if metadata.is_dir() {
            let directory_target = resolved.join("tsconfig.json");
            if path_exists(&directory_target) {
                return Ok(Some(directory_target));
            }
            return Ok(None);
        }

        return Ok(Some(resolved));
    }

    let directory_target = resolved.join("tsconfig.json");
    if path_exists(&directory_target) {
        return Ok(Some(directory_target));
    }

    Ok(None)
}

fn resolve_effective_composite(
    config_path: &Path,
    config: &Value,
) -> io::Result<CompositeResolution> {
    let mut visited = Vec::new();
    resolve_effective_composite_inner(config_path, config, &mut visited)
}

fn resolve_effective_composite_inner(
    config_path: &Path,
    config: &Value,
    visited: &mut Vec<PathBuf>,
) -> io::Result<CompositeResolution> {
    if let Some(compiler_options) = config.get("compilerOptions").and_then(Value::as_object) {
        if let Some(composite_value) = compiler_options.get("composite") {
            return match composite_value.as_bool() {
                Some(true) => Ok(CompositeResolution::Enabled),
                Some(false) => Ok(CompositeResolution::Disabled),
                None => Ok(invalid_composite_type_issue(
                    config_path,
                    visited.is_empty(),
                )),
            };
        }
    }

    let Some(extends_path) = config.get("extends").and_then(Value::as_str) else {
        return Ok(CompositeResolution::Disabled);
    };
    let Some(parent_config_path) = resolve_extends_config_path(
        config_path.parent().unwrap_or_else(|| Path::new(".")),
        extends_path,
    ) else {
        return Ok(CompositeResolution::Issue {
            suffix: "missing",
            title: "Inherited tsconfig could not be found",
            detail: format!(
                "Referenced project extends {extends_path}, but that config file could not be resolved."
            ),
            hint: "Make sure extends points at an existing tsconfig-style file before relying on inherited composite settings.",
        });
    };
    if visited.contains(&parent_config_path) {
        return Ok(CompositeResolution::Issue {
            suffix: "cycle",
            title: "Inherited tsconfig extends cycle detected",
            detail: format!(
                "Referenced project extends {}, but that path creates a cycle in the extends chain.",
                parent_config_path.to_string_lossy()
            ),
            hint: "Break extends cycles before relying on inherited composite settings.",
        });
    }
    visited.push(parent_config_path.clone());

    let parent_text = match read_text_if_exists(&parent_config_path) {
        Ok(Some(parent_text)) => parent_text,
        Ok(None) => {
            return Ok(CompositeResolution::Issue {
                suffix: "unreadable",
                title: "Inherited tsconfig could not be read",
                detail: format!(
                    "Referenced project extends {}, but that config file could not be read.",
                    parent_config_path.to_string_lossy()
                ),
                hint: "Make sure extended tsconfig files are readable before relying on inherited composite settings.",
            });
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Ok(CompositeResolution::Issue {
                suffix: "unreadable",
                title: "Inherited tsconfig could not be read",
                detail: format!(
                    "Referenced project extends {}, but reading it failed: {error}.",
                    parent_config_path.to_string_lossy()
                ),
                hint: "Make sure extended tsconfig files are readable before relying on inherited composite settings.",
            });
        }
        Err(error) => return Err(error),
    };
    let parent_config = match parse_jsonc::<Value>(
        &parent_text,
        &parent_config_path.to_string_lossy(),
    ) {
        Ok(parent_config) => parent_config,
        Err(error) => {
            return Ok(CompositeResolution::Issue {
                suffix: "parse",
                title: "Inherited tsconfig could not be parsed",
                detail: error.to_string(),
                hint: "Fix invalid JSONC syntax in extended tsconfig files before relying on inherited composite settings.",
            });
        }
    };
    if !looks_like_tsconfig_document(&parent_config_path, &parent_config) {
        return Ok(CompositeResolution::Issue {
            suffix: "invalid-target",
            title: "Inherited config must point to a tsconfig file",
            detail: format!(
                "Referenced project extends {}, but that file does not look like a tsconfig document.",
                parent_config_path.to_string_lossy()
            ),
            hint: "Point extends at a real tsconfig-style file before relying on inherited composite settings.",
        });
    }

    resolve_effective_composite_inner(&parent_config_path, &parent_config, visited)
}

fn invalid_composite_type_issue(config_path: &Path, is_direct_target: bool) -> CompositeResolution {
    if is_direct_target {
        return CompositeResolution::Issue {
            suffix: "composite-type",
            title: "Referenced project must set compilerOptions.composite to a boolean",
            detail: format!(
                "{} declares compilerOptions.composite, but the value is not a boolean.",
                config_path.to_string_lossy()
            ),
            hint: "Set compilerOptions.composite to true or false before relying on project references.",
        };
    }

    CompositeResolution::Issue {
        suffix: "extends-composite-type",
        title: "Inherited tsconfig must set compilerOptions.composite to a boolean",
        detail: format!(
            "{} declares compilerOptions.composite, but the value is not a boolean.",
            config_path.to_string_lossy()
        ),
        hint: "Set compilerOptions.composite to true or false in extended tsconfig files before relying on inherited settings.",
    }
}

fn resolve_effective_pattern_config(
    config_path: &Path,
    config: &Value,
) -> io::Result<EffectivePatternConfig> {
    let mut visited = Vec::new();
    resolve_effective_pattern_config_inner(config_path, config, &mut visited)
}

fn resolve_effective_pattern_config_inner(
    config_path: &Path,
    config: &Value,
    visited: &mut Vec<PathBuf>,
) -> io::Result<EffectivePatternConfig> {
    let normalized_config_path = normalize_path(config_path);
    if visited.contains(&normalized_config_path) {
        return Ok(EffectivePatternConfig::default());
    }
    visited.push(normalized_config_path);

    let mut effective_config = match load_extended_tsconfig_document(config_path, config, visited)?
    {
        ExtendedPatternConfigDocument::Loaded(parent_config_path, parent_config) => {
            resolve_effective_pattern_config_inner(&parent_config_path, &parent_config, visited)?
        }
        ExtendedPatternConfigDocument::Issue(issue) => EffectivePatternConfig {
            issues: vec![issue],
            ..EffectivePatternConfig::default()
        },
        ExtendedPatternConfigDocument::None => EffectivePatternConfig::default(),
    };

    if let Some(compiler_options) = config.get("compilerOptions").and_then(Value::as_object) {
        if let Some(allow_js) = compiler_options.get("allowJs").and_then(Value::as_bool) {
            effective_config.options.allow_js = allow_js;
        }

        if let Some(root_dir) = compiler_options.get("rootDir").and_then(Value::as_str) {
            effective_config.options.root_dir = Some(resolve_path_with_backslash_support(
                config_path.parent().unwrap_or_else(|| Path::new(".")),
                root_dir,
            ));
        }

        if let Some(out_dir) = compiler_options.get("outDir").and_then(Value::as_str) {
            effective_config.options.out_dir = Some(resolve_path_with_backslash_support(
                config_path.parent().unwrap_or_else(|| Path::new(".")),
                out_dir,
            ));
        }
    }

    if let Some(config_object) = config.as_object() {
        apply_pattern_field_override(&mut effective_config, config_object, "include", config_path);
        apply_pattern_field_override(&mut effective_config, config_object, "exclude", config_path);
        apply_pattern_field_override(&mut effective_config, config_object, "files", config_path);
    }

    visited.pop();
    Ok(effective_config)
}

fn apply_pattern_field_override(
    effective_config: &mut EffectivePatternConfig,
    config_object: &Map<String, Value>,
    field_name: &'static str,
    config_path: &Path,
) {
    let Some(value) = config_object.get(field_name) else {
        return;
    };

    effective_config
        .issues
        .retain(|issue| issue.field_name != field_name);
    let (field, issues) = parse_effective_pattern_field(field_name, value, config_path);
    match field_name {
        "include" => effective_config.include = Some(field),
        "exclude" => effective_config.exclude = Some(field),
        "files" => effective_config.files = Some(field),
        _ => {}
    }
    effective_config.issues.extend(issues);
}

fn parse_effective_pattern_field(
    field_name: &'static str,
    value: &Value,
    config_path: &Path,
) -> (EffectivePatternField, Vec<PatternFieldIssue>) {
    let base_dir = normalize_path(config_path.parent().unwrap_or_else(|| Path::new(".")));
    let mut issues = Vec::new();

    let Some(values) = value.as_array() else {
        issues.push(PatternFieldIssue {
            field_name,
            config_path: config_path.to_path_buf(),
            suffix: "shape".to_string(),
            title: format!("\"{field_name}\" must be an array of strings"),
            detail: format!(
                "{} declares {field_name}, but TypeScript expects an array of string patterns.",
                config_path.to_string_lossy()
            ),
            hint: pattern_field_hint(field_name),
        });
        return (
            EffectivePatternField {
                values: Vec::new(),
                base_dir,
                config_path: config_path.to_path_buf(),
            },
            issues,
        );
    };

    let mut collected_values = Vec::new();
    for (index, entry) in values.iter().enumerate() {
        let Some(pattern) = entry.as_str() else {
            issues.push(PatternFieldIssue {
                field_name,
                config_path: config_path.to_path_buf(),
                suffix: format!("entry-{index}"),
                title: format!("\"{field_name}\" contains a non-string pattern"),
                detail: format!(
                    "{} declares {field_name}[{index}], but TypeScript expects string patterns.",
                    config_path.to_string_lossy()
                ),
                hint: pattern_field_hint(field_name),
            });
            continue;
        };
        collected_values.push(pattern.to_string());
    }

    (
        EffectivePatternField {
            values: collected_values,
            base_dir,
            config_path: config_path.to_path_buf(),
        },
        issues,
    )
}

fn pattern_field_hint(field_name: &str) -> &'static str {
    match field_name {
        "include" => "Rewrite include as an array of string globs before relying on TypeScript input discovery.",
        "exclude" => "Rewrite exclude as an array of string globs before relying on TypeScript input filtering.",
        "files" => "Rewrite files as an array of string paths before relying on explicit TypeScript inputs.",
        _ => "Rewrite tsconfig pattern fields as arrays of strings.",
    }
}

fn resolve_extends_config_path(base_dir: &Path, extends_path: &str) -> Option<PathBuf> {
    let extends_candidate = Path::new(extends_path);
    let is_local_extends = extends_candidate.is_absolute()
        || extends_path.starts_with("./")
        || extends_path.starts_with("../")
        || extends_path.starts_with(".\\")
        || extends_path.starts_with("..\\");
    if is_local_extends {
        return resolve_tsconfig_candidate(&resolve_path(base_dir, extends_path))
            .ok()
            .flatten();
    }

    for ancestor in base_dir.ancestors() {
        let candidate = ancestor.join("node_modules").join(extends_path);
        if let Ok(Some(resolved)) = resolve_tsconfig_candidate(&candidate) {
            return Some(resolved);
        }
    }

    None
}

fn load_extended_tsconfig_document(
    config_path: &Path,
    config: &Value,
    visited: &[PathBuf],
) -> io::Result<ExtendedPatternConfigDocument> {
    let Some(extends_path) = config.get("extends").and_then(Value::as_str) else {
        return Ok(ExtendedPatternConfigDocument::None);
    };
    let Some(parent_config_path) = resolve_extends_config_path(
        config_path.parent().unwrap_or_else(|| Path::new(".")),
        extends_path,
    ) else {
        return Ok(ExtendedPatternConfigDocument::Issue(pattern_extends_issue(
            "missing",
            config_path.to_path_buf(),
            "Inherited tsconfig could not be found".to_string(),
            format!(
                "{} extends {extends_path}, but that config file could not be resolved.",
                config_path.to_string_lossy()
            ),
            "Fix missing extends targets before relying on inherited tsconfig pattern settings.",
        )));
    };
    if visited_contains_path(visited, &parent_config_path) {
        return Ok(ExtendedPatternConfigDocument::Issue(pattern_extends_issue(
            "cycle",
            parent_config_path.clone(),
            "Inherited tsconfig extends cycle detected".to_string(),
            format!(
                "{} creates a cycle in the extends chain for pattern resolution.",
                parent_config_path.to_string_lossy()
            ),
            "Break extends cycles before relying on inherited tsconfig pattern settings.",
        )));
    };

    let parent_text = match read_text_if_exists(&parent_config_path) {
        Ok(Some(parent_text)) => parent_text,
        Ok(None) => {
            return Ok(ExtendedPatternConfigDocument::Issue(pattern_extends_issue(
                "unreadable",
                parent_config_path.clone(),
                "Inherited tsconfig could not be read".to_string(),
                format!(
                    "{} extends {}, but that config file could not be read.",
                    config_path.to_string_lossy(),
                    parent_config_path.to_string_lossy()
                ),
                "Make sure extended tsconfig files are readable before relying on inherited pattern settings.",
            )))
        }
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Ok(ExtendedPatternConfigDocument::Issue(pattern_extends_issue(
                "unreadable",
                parent_config_path.clone(),
                "Inherited tsconfig could not be read".to_string(),
                format!(
                    "{} extends {}, but reading it failed: {error}.",
                    config_path.to_string_lossy(),
                    parent_config_path.to_string_lossy()
                ),
                "Make sure extended tsconfig files are readable before relying on inherited pattern settings.",
            )))
        }
        Err(error) => return Err(error),
    };
    let parent_config =
        match parse_jsonc::<Value>(&parent_text, &parent_config_path.to_string_lossy()) {
            Ok(parent_config) => parent_config,
            Err(error) => {
                return Ok(ExtendedPatternConfigDocument::Issue(pattern_extends_issue(
                    "parse",
                    parent_config_path.clone(),
                    "Inherited tsconfig could not be parsed".to_string(),
                    error.to_string(),
                    "Fix invalid JSONC syntax in extended tsconfig files before relying on inherited pattern settings.",
                )))
            }
        };
    if !looks_like_tsconfig_document(&parent_config_path, &parent_config) {
        return Ok(ExtendedPatternConfigDocument::Issue(pattern_extends_issue(
            "invalid-target",
            parent_config_path.clone(),
            "Inherited config must point to a tsconfig file".to_string(),
            format!(
                "{} extends {}, but that file does not look like a tsconfig document.",
                config_path.to_string_lossy(),
                parent_config_path.to_string_lossy()
            ),
            "Point extends at a real tsconfig-style file before relying on inherited pattern settings.",
        )));
    }

    Ok(ExtendedPatternConfigDocument::Loaded(
        parent_config_path,
        parent_config,
    ))
}

fn visited_contains_path(visited: &[PathBuf], candidate: &Path) -> bool {
    let candidate = normalize_path(candidate);
    visited.contains(&candidate)
}

fn pattern_extends_issue(
    suffix: &str,
    config_path: PathBuf,
    title: String,
    detail: String,
    hint: &'static str,
) -> PatternFieldIssue {
    PatternFieldIssue {
        field_name: "extends",
        config_path,
        suffix: suffix.to_string(),
        title,
        detail,
        hint,
    }
}

fn looks_like_tsconfig_document(file_path: &Path, config: &Value) -> bool {
    if is_known_non_tsconfig_file(file_path) {
        return false;
    }

    let Some(object) = config.as_object() else {
        return false;
    };

    if object.is_empty() {
        return looks_like_tsconfig_file_name(file_path)
            || looks_like_explicit_tsconfig_target_file(file_path);
    }

    object.contains_key("$schema")
        || object.contains_key("compileOnSave")
        || object.contains_key("compilerOptions")
        || object.contains_key("extends")
        || object.contains_key("references")
        || object.contains_key("files")
        || object.contains_key("include")
        || object.contains_key("exclude")
        || object.contains_key("watchOptions")
        || object.contains_key("typeAcquisition")
}

fn looks_like_explicit_tsconfig_target_file(file_path: &Path) -> bool {
    if is_known_non_tsconfig_file(file_path) {
        return false;
    }

    let Some(stem) = file_path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };

    stem == "build" || stem.starts_with("tsconfig")
}

fn looks_like_tsconfig_file_name(file_path: &Path) -> bool {
    let Some(file_name) = file_path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    file_name == "tsconfig.json"
        || (file_name.starts_with("tsconfig.") && file_name.ends_with(".json"))
}

fn is_known_non_tsconfig_file(file_path: &Path) -> bool {
    matches!(
        file_path.file_name().and_then(|value| value.to_str()),
        Some("package.json" | "package-lock.json" | "npm-shrinkwrap.json")
    )
}

fn resolve_tsconfig_candidate(candidate: &Path) -> io::Result<Option<PathBuf>> {
    if path_exists(candidate) {
        let metadata = fs::metadata(candidate)?;
        if metadata.is_dir() {
            let directory_target = candidate.join("tsconfig.json");
            if path_exists(&directory_target) {
                return Ok(Some(directory_target));
            }
            return Ok(None);
        }

        return Ok(Some(candidate.to_path_buf()));
    }

    if candidate.extension().is_none() {
        let file_target = candidate.with_extension("json");
        if path_exists(&file_target) {
            return Ok(Some(file_target));
        }
    }

    let directory_target = candidate.join("tsconfig.json");
    if path_exists(&directory_target) {
        return Ok(Some(directory_target));
    }

    Ok(None)
}

fn select_effective_tsconfig_target(
    base_dir: &Path,
    ts_targets: &[Value],
) -> io::Result<Option<String>> {
    let mut first_string_target = None;

    for target in ts_targets.iter().filter_map(Value::as_str) {
        if first_string_target.is_none() {
            first_string_target = Some(target.to_string());
        }

        if alias_target_exists(base_dir, target)? {
            return Ok(Some(target.to_string()));
        }
    }

    Ok(first_string_target)
}

fn determine_shadowing_alias_pair<'a>(left: &'a str, right: &'a str) -> Option<(&'a str, &'a str)> {
    if !aliases_overlap(left, right) {
        return None;
    }

    let left_specificity = alias_specificity(left);
    let right_specificity = alias_specificity(right);
    if left_specificity > right_specificity {
        return Some((left, right));
    }
    if right_specificity > left_specificity {
        return Some((right, left));
    }

    None
}

fn is_exact_specialization_of_wildcard_alias(shadowing_alias: &str, shadowed_alias: &str) -> bool {
    !shadowing_alias.contains('*')
        && shadowed_alias.contains('*')
        && wildcard_pattern_matches(shadowing_alias, shadowed_alias)
}

fn aliases_overlap(left: &str, right: &str) -> bool {
    match (left.contains('*'), right.contains('*')) {
        (false, false) => left == right,
        (false, true) => wildcard_pattern_matches(left, right),
        (true, false) => wildcard_pattern_matches(right, left),
        (true, true) => wildcard_aliases_overlap(left, right),
    }
}

fn wildcard_aliases_overlap(left: &str, right: &str) -> bool {
    if left.matches('*').count() != 1 || right.matches('*').count() != 1 {
        return wildcard_overlap_samples(left)
            .into_iter()
            .chain(wildcard_overlap_samples(right))
            .any(|candidate| {
                wildcard_pattern_matches(&candidate, left)
                    && wildcard_pattern_matches(&candidate, right)
            });
    }

    let (left_prefix, left_suffix) = split_wildcard_alias(left);
    let (right_prefix, right_suffix) = split_wildcard_alias(right);
    let Some(merged_prefix) = merge_prefixes(left_prefix, right_prefix) else {
        return false;
    };
    let Some(merged_suffix) = merge_suffixes(left_suffix, right_suffix) else {
        return false;
    };

    ["", "shadow", "shadow/nested"]
        .into_iter()
        .map(|middle| format!("{merged_prefix}{middle}{merged_suffix}"))
        .any(|candidate| {
            wildcard_pattern_matches(&candidate, left)
                && wildcard_pattern_matches(&candidate, right)
        })
}

fn wildcard_overlap_samples(pattern: &str) -> Vec<String> {
    if !pattern.contains('*') {
        return vec![pattern.to_string()];
    }

    ["shadow", "shadow/nested"]
        .into_iter()
        .map(|replacement| pattern.replace('*', replacement))
        .collect()
}

fn split_wildcard_alias(pattern: &str) -> (&str, &str) {
    let wildcard_index = pattern.find('*').unwrap_or(pattern.len());
    (&pattern[..wildcard_index], &pattern[wildcard_index + 1..])
}

fn merge_prefixes<'a>(left: &'a str, right: &'a str) -> Option<&'a str> {
    if left.starts_with(right) {
        return Some(left);
    }
    if right.starts_with(left) {
        return Some(right);
    }

    None
}

fn merge_suffixes<'a>(left: &'a str, right: &'a str) -> Option<&'a str> {
    if left.ends_with(right) {
        return Some(left);
    }
    if right.ends_with(left) {
        return Some(right);
    }

    None
}

fn alias_specificity(alias: &str) -> (usize, usize, usize) {
    let wildcard_count = alias.matches('*').count();
    let fixed_length = alias.chars().filter(|character| *character != '*').count();
    let separator_count = alias
        .chars()
        .filter(|character| matches!(character, '/' | '\\'))
        .count();

    (
        usize::from(wildcard_count == 0),
        fixed_length + separator_count,
        alias.len(),
    )
}

fn is_package_style_alias(alias: &str) -> bool {
    let alias = alias.trim_end_matches('*').trim_end_matches(['/', '\\']);
    if alias.is_empty()
        || alias.starts_with('#')
        || alias.starts_with('.')
        || alias.starts_with('/')
        || alias.starts_with('\\')
        || has_url_like_prefix(alias)
    {
        return false;
    }

    if let Some(rest) = alias.strip_prefix('@') {
        let segments = rest
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        return segments.len() >= 2
            && is_package_name_segment(segments[0])
            && is_package_name_segment(segments[1]);
    }

    let Some(first_segment) = alias.split('/').next() else {
        return false;
    };
    is_package_name_segment(first_segment)
}

fn is_package_name_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
}

fn format_string_list(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn alias_target_exists(base_dir: &Path, target: &str) -> io::Result<bool> {
    if has_url_like_prefix(target) || target.starts_with('@') {
        return Ok(true);
    }

    if target.contains('*') {
        return wildcard_target_exists(base_dir, target);
    }

    static_target_exists(base_dir, target)
}

fn static_target_exists(base_dir: &Path, target: &str) -> io::Result<bool> {
    let stem = target.split('*').next().unwrap_or(target);
    let resolved = resolve_path(base_dir, stem);

    if path_exists(&resolved) {
        return Ok(true);
    }

    let resolved_string = resolved.to_string_lossy();
    for extension in CHECKABLE_EXTENSIONS {
        if path_exists(PathBuf::from(format!("{resolved_string}{extension}"))) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn wildcard_target_exists(base_dir: &Path, target: &str) -> io::Result<bool> {
    let mut segments = target.split('*');
    let prefix = segments.next().unwrap_or_default();
    let rest = segments.collect::<Vec<_>>();
    let suffix = rest.join("*");
    let search_prefix = wildcard_search_prefix(prefix);
    let search_root = resolve_path(base_dir, search_prefix);

    if !path_exists(&search_root) {
        return Ok(false);
    }

    if rest.len() == 1 && suffix.is_empty() {
        return Ok(true);
    }

    let patterns = build_wildcard_patterns(base_dir, target);
    has_matching_path(&search_root, &patterns)
}

fn wildcard_search_prefix(prefix: &str) -> &str {
    if prefix.is_empty() {
        return ".";
    }

    if prefix.ends_with('/') || prefix.ends_with('\\') {
        return prefix;
    }

    match prefix.rfind(['/', '\\']) {
        Some(0) => &prefix[..1],
        Some(index) => &prefix[..index],
        None => ".",
    }
}

fn has_matching_path(candidate_path: &Path, patterns: &[String]) -> io::Result<bool> {
    if matches_path(candidate_path, patterns) {
        return Ok(true);
    }

    let entries = match fs::read_dir(candidate_path) {
        Ok(entries) => entries,
        Err(_) => return Ok(false),
    };

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();

        if matches_path(&entry_path, patterns) {
            return Ok(true);
        }

        if entry.file_type()?.is_dir() && has_matching_path(&entry_path, patterns)? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn matches_path(candidate_path: &Path, patterns: &[String]) -> bool {
    let normalized_path = normalize_path_for_match(candidate_path);
    patterns
        .iter()
        .any(|pattern| pattern_matches_candidate(&normalized_path, pattern))
}

fn build_wildcard_patterns(base_dir: &Path, target: &str) -> Vec<String> {
    let resolved = resolve_path(base_dir, target);
    let mut patterns = vec![normalize_path_for_match(&resolved)];

    if !has_explicit_extension(target) {
        let resolved_string = resolved.to_string_lossy();
        for extension in CHECKABLE_EXTENSIONS {
            patterns.push(normalize_path_for_match(&PathBuf::from(format!(
                "{resolved_string}{extension}"
            ))));
        }
    }

    patterns
}

fn has_explicit_extension(target: &str) -> bool {
    let tail = target.rsplit('*').next().unwrap_or(target);
    Path::new(tail).extension().is_some()
}

fn wildcard_pattern_matches(candidate: &str, pattern: &str) -> bool {
    fn segment_pattern_matches(candidate: &str, pattern: &str) -> bool {
        let candidate_chars = candidate.chars().collect::<Vec<_>>();
        let pattern_chars = pattern.chars().collect::<Vec<_>>();
        let mut memo = vec![vec![None; pattern_chars.len() + 1]; candidate_chars.len() + 1];

        fn matches(
            candidate: &[char],
            pattern: &[char],
            candidate_index: usize,
            pattern_index: usize,
            memo: &mut [Vec<Option<bool>>],
        ) -> bool {
            if let Some(result) = memo[candidate_index][pattern_index] {
                return result;
            }

            let result = if pattern_index == pattern.len() {
                candidate_index == candidate.len()
            } else if pattern[pattern_index] == '*' {
                let mut offset = candidate_index;
                let mut matched = false;
                while offset <= candidate.len() {
                    if matches(candidate, pattern, offset, pattern_index + 1, memo) {
                        matched = true;
                        break;
                    }
                    if offset == candidate.len() {
                        break;
                    }
                    offset += 1;
                }
                matched
            } else if candidate_index < candidate.len()
                && pattern[pattern_index] == '?'
                && candidate[candidate_index] != '/'
            {
                matches(
                    candidate,
                    pattern,
                    candidate_index + 1,
                    pattern_index + 1,
                    memo,
                )
            } else if candidate_index < candidate.len()
                && candidate[candidate_index] == pattern[pattern_index]
            {
                matches(
                    candidate,
                    pattern,
                    candidate_index + 1,
                    pattern_index + 1,
                    memo,
                )
            } else {
                false
            };

            memo[candidate_index][pattern_index] = Some(result);
            result
        }

        matches(&candidate_chars, &pattern_chars, 0, 0, &mut memo)
    }

    let candidate_segments = candidate.split('/').collect::<Vec<_>>();
    let pattern_segments = pattern.split('/').collect::<Vec<_>>();
    let mut memo = vec![vec![None; pattern_segments.len() + 1]; candidate_segments.len() + 1];

    fn matches_segments(
        candidate: &[&str],
        pattern: &[&str],
        candidate_index: usize,
        pattern_index: usize,
        memo: &mut [Vec<Option<bool>>],
    ) -> bool {
        if let Some(result) = memo[candidate_index][pattern_index] {
            return result;
        }

        let result = if pattern_index == pattern.len() {
            candidate_index == candidate.len()
        } else if pattern[pattern_index] == "**" {
            (candidate_index..=candidate.len())
                .any(|offset| matches_segments(candidate, pattern, offset, pattern_index + 1, memo))
        } else if candidate_index < candidate.len()
            && segment_pattern_matches(candidate[candidate_index], pattern[pattern_index])
        {
            matches_segments(
                candidate,
                pattern,
                candidate_index + 1,
                pattern_index + 1,
                memo,
            )
        } else {
            false
        };

        memo[candidate_index][pattern_index] = Some(result);
        result
    }

    matches_segments(&candidate_segments, &pattern_segments, 0, 0, &mut memo)
}

fn pattern_matches_candidate(candidate: &str, pattern: &str) -> bool {
    wildcard_pattern_matches(candidate, pattern)
}

fn normalize_path_for_match(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn has_url_like_prefix(target: &str) -> bool {
    let Some((prefix, _)) = target.split_once(':') else {
        return false;
    };

    !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn collect_pattern_matches(
    base_dir: &Path,
    pattern: &str,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
) -> io::Result<Vec<PathBuf>> {
    if pattern.trim().is_empty() {
        return Ok(Vec::new());
    }

    if !pattern_contains_wildcard(pattern) {
        return collect_static_pattern_matches(
            base_dir,
            pattern,
            candidate_extensions,
            default_excluded_dirs,
        );
    }

    let search_root = resolve_pattern_search_root(base_dir, pattern);
    if !path_exists(&search_root) {
        return Ok(Vec::new());
    }
    if is_within_default_excluded_dir(&search_root, default_excluded_dirs) {
        return Ok(Vec::new());
    }

    let mut matches = BTreeSet::new();
    collect_pattern_matches_recursive(
        &search_root,
        &resolve_path(base_dir, pattern),
        candidate_extensions,
        default_excluded_dirs,
        &mut matches,
    )?;
    Ok(matches.into_iter().collect())
}

fn collect_default_pattern_matches(
    base_dir: &Path,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
) -> io::Result<BTreeSet<PathBuf>> {
    let mut matches = BTreeSet::new();
    collect_supported_files_recursively(
        base_dir,
        &mut matches,
        candidate_extensions,
        default_excluded_dirs,
    )?;
    Ok(matches)
}

fn collect_static_pattern_matches(
    base_dir: &Path,
    pattern: &str,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
) -> io::Result<Vec<PathBuf>> {
    let resolved = resolve_path(base_dir, pattern);
    if path_exists(&resolved) {
        let metadata = fs::metadata(&resolved)?;
        if metadata.is_dir() {
            if is_within_default_excluded_dir(&resolved, default_excluded_dirs) {
                return Ok(Vec::new());
            }
            let mut matches = BTreeSet::new();
            collect_supported_files_recursively(
                &resolved,
                &mut matches,
                candidate_extensions,
                default_excluded_dirs,
            )?;
            return Ok(matches.into_iter().collect());
        }

        if metadata.is_file()
            && !is_within_default_excluded_dir(&resolved, default_excluded_dirs)
            && is_ts_pattern_candidate_file(&resolved, candidate_extensions)
        {
            return Ok(vec![resolved]);
        }
        return Ok(Vec::new());
    }

    if has_explicit_extension(pattern) {
        return Ok(Vec::new());
    }

    let resolved_string = resolved.to_string_lossy();
    let mut matches = Vec::new();
    for extension in candidate_extensions {
        let candidate = PathBuf::from(format!("{resolved_string}{extension}"));
        if path_exists(&candidate)
            && !is_within_default_excluded_dir(&candidate, default_excluded_dirs)
            && fs::metadata(&candidate)?.is_file()
        {
            matches.push(candidate);
        }
    }

    Ok(matches)
}

fn collect_supported_files_recursively(
    root: &Path,
    matches: &mut BTreeSet<PathBuf>,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
) -> io::Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if should_skip_default_pattern_dir(&entry_path, default_excluded_dirs) {
                continue;
            }
            collect_supported_files_recursively(
                &entry_path,
                matches,
                candidate_extensions,
                default_excluded_dirs,
            )?;
            continue;
        }

        if file_type.is_file() && is_ts_pattern_candidate_file(&entry_path, candidate_extensions) {
            matches.insert(normalize_path(&entry_path));
        }
    }

    Ok(())
}

fn collect_pattern_matches_recursive(
    root: &Path,
    absolute_pattern: &Path,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
    matches: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if should_skip_default_pattern_dir(&entry_path, default_excluded_dirs) {
                continue;
            }
            collect_pattern_matches_recursive(
                &entry_path,
                absolute_pattern,
                candidate_extensions,
                default_excluded_dirs,
                matches,
            )?;
            continue;
        }

        if file_type.is_file()
            && is_ts_pattern_candidate_file(&entry_path, candidate_extensions)
            && pattern_matches_candidate(
                &normalize_path_for_match(&entry_path),
                &normalize_path_for_match(absolute_pattern),
            )
        {
            matches.insert(normalize_path(&entry_path));
        }
    }

    Ok(())
}

fn pattern_matches_file(
    base_dir: &Path,
    pattern: &str,
    candidate: &Path,
    candidate_extensions: &[&str],
    default_excluded_dirs: &[PathBuf],
) -> bool {
    if pattern.trim().is_empty() {
        return false;
    }

    let candidate = normalize_path(candidate);
    if !pattern_contains_wildcard(pattern) {
        let resolved = resolve_path(base_dir, pattern);
        if path_exists(&resolved) {
            return fs::metadata(&resolved)
                .map(|metadata| {
                    if metadata.is_dir() {
                        !should_skip_default_pattern_dir(&resolved, default_excluded_dirs)
                            && candidate.starts_with(&resolved)
                    } else {
                        candidate == normalize_path(&resolved)
                    }
                })
                .unwrap_or(false);
        }

        if has_explicit_extension(pattern) {
            return false;
        }

        let resolved_string = resolved.to_string_lossy();
        return candidate_extensions
            .iter()
            .any(|extension| candidate == PathBuf::from(format!("{resolved_string}{extension}")));
    }

    pattern_matches_candidate(
        &normalize_path_for_match(&candidate),
        &normalize_path_for_match(&resolve_path(base_dir, pattern)),
    )
}

fn resolve_pattern_search_root(base_dir: &Path, pattern: &str) -> PathBuf {
    let prefix = pattern
        .find(['*', '?'])
        .map(|index| &pattern[..index])
        .unwrap_or(pattern);
    let search_prefix = wildcard_search_prefix(prefix);
    resolve_path(base_dir, search_prefix)
}

fn pattern_contains_wildcard(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

fn is_ts_pattern_candidate_file(path: &Path, candidate_extensions: &[&str]) -> bool {
    let normalized = normalize_path_for_match(path);
    candidate_extensions
        .iter()
        .any(|extension| normalized.ends_with(extension))
}

fn resolve_path(base_dir: &Path, target: &str) -> PathBuf {
    normalize_path(&base_dir.join(target))
}

fn resolve_path_with_backslash_support(base_dir: &Path, target: &str) -> PathBuf {
    resolve_path(base_dir, &target.replace('\\', "/"))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|value| matches!(value, Component::Normal(_)))
                {
                    normalized.pop();
                } else if normalized.as_os_str().is_empty() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}
