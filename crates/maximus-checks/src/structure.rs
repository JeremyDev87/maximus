use std::collections::BTreeSet;

use maximus_core::{get_files, FileKind, Finding, ProjectSnapshot, StructureReport};

pub fn build_structure_report(project: &ProjectSnapshot, findings: &[Finding]) -> StructureReport {
    let package_count = get_files(project, FileKind::Package).len();
    let env_directories = get_files(project, FileKind::Env)
        .iter()
        .map(|file| file.dir.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let config_files = project.files.len();
    let is_monorepo = package_count > 1;
    let mut recommendations = Vec::new();

    let has_shared_tsconfig = get_files(project, FileKind::Tsconfig)
        .iter()
        .any(|file| file.name == "tsconfig.base.json");
    let has_root_env_example = get_files(project, FileKind::Env)
        .iter()
        .any(|file| file.dir == project.root_dir && file.name == ".env.example");
    let eslint_config_count = get_files(project, FileKind::Eslint).len();

    if is_monorepo && !has_shared_tsconfig {
        recommendations.push(
            "Introduce a shared tsconfig.base.json so packages inherit one source of truth."
                .to_string(),
        );
    }

    if eslint_config_count > 1 {
        recommendations.push(
            "Reduce repo-wide ESLint entry points unless packages genuinely need different rule sets."
                .to_string(),
        );
    }

    if env_directories > 1 && !has_root_env_example {
        recommendations.push(
            "Use .env.example files consistently so onboarding does not depend on tribal knowledge."
                .to_string(),
        );
    }

    if findings.is_empty() {
        recommendations.push(
            "Current config surface looks healthy. Keep shared rules centralized as the repo grows."
                .to_string(),
        );
    }

    StructureReport {
        is_monorepo,
        package_count,
        env_directories,
        config_files,
        recommendations,
    }
}
