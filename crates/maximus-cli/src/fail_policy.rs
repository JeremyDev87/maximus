use maximus_core::{AuditSummary, FailOnLevel};

use crate::exit_codes::{FINDINGS_PRESENT, SUCCESS};

pub fn exit_code(summary: &AuditSummary, fail_on: &FailOnLevel) -> i32 {
    let has_matching_findings = match fail_on {
        FailOnLevel::Error => summary.blocking_findings > 0,
        FailOnLevel::Warn => summary.blocking_findings > 0 || summary.warning_findings > 0,
        FailOnLevel::Info => {
            summary.blocking_findings > 0
                || summary.warning_findings > 0
                || summary.info_findings > 0
        }
        FailOnLevel::None => false,
    };

    if has_matching_findings {
        FINDINGS_PRESENT
    } else {
        SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use maximus_core::{AuditSummary, FailOnLevel};

    use super::exit_code;
    use crate::exit_codes::{FINDINGS_PRESENT, SUCCESS};

    fn summary() -> AuditSummary {
        AuditSummary {
            status: "attention needed".to_string(),
            total_findings: 3,
            blocking_findings: 1,
            warning_findings: 1,
            info_findings: 1,
            fixable_findings: 0,
            fixes_available: 0,
            suppressed_by_config: 0,
            config_files: 1,
            package_count: 1,
            env_directories: 0,
        }
    }

    #[test]
    fn fail_policy_respects_selected_threshold() {
        let noisy = summary();
        let warnings_only = AuditSummary {
            blocking_findings: 0,
            ..noisy.clone()
        };
        let info_only = AuditSummary {
            blocking_findings: 0,
            warning_findings: 0,
            ..noisy.clone()
        };

        assert_eq!(exit_code(&noisy, &FailOnLevel::Error), FINDINGS_PRESENT);
        assert_eq!(exit_code(&warnings_only, &FailOnLevel::Error), SUCCESS);
        assert_eq!(
            exit_code(&warnings_only, &FailOnLevel::Warn),
            FINDINGS_PRESENT
        );
        assert_eq!(exit_code(&info_only, &FailOnLevel::Warn), SUCCESS);
        assert_eq!(exit_code(&info_only, &FailOnLevel::Info), FINDINGS_PRESENT);
        assert_eq!(exit_code(&noisy, &FailOnLevel::None), SUCCESS);
    }
}
