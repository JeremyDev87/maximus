#![cfg_attr(not(test), allow(dead_code))]

use maximus_core::AuditSummary;

pub const SUCCESS: i32 = 0;
pub const FINDINGS_PRESENT: i32 = 1;
pub const FAILURE: i32 = 2;

pub fn audit_exit_code(summary: &AuditSummary) -> i32 {
    if summary.blocking_findings > 0 || summary.warning_findings > 0 {
        FINDINGS_PRESENT
    } else {
        SUCCESS
    }
}

pub fn fix_exit_code(summary: &AuditSummary) -> i32 {
    audit_exit_code(summary)
}

#[cfg(test)]
mod tests {
    use maximus_core::AuditSummary;

    use super::{audit_exit_code, fix_exit_code, FINDINGS_PRESENT, SUCCESS};

    #[test]
    fn exit_codes_follow_js_cli_contract() {
        let clean = AuditSummary {
            status: "clean".to_string(),
            total_findings: 0,
            blocking_findings: 0,
            warning_findings: 0,
            info_findings: 0,
            fixable_findings: 0,
            fixes_available: 0,
            suppressed_by_config: 0,
            config_files: 1,
            package_count: 1,
            env_directories: 0,
        };

        let noisy = AuditSummary {
            warning_findings: 1,
            ..clean.clone()
        };

        assert_eq!(audit_exit_code(&clean), SUCCESS);
        assert_eq!(fix_exit_code(&clean), SUCCESS);
        assert_eq!(audit_exit_code(&noisy), FINDINGS_PRESENT);
        assert_eq!(fix_exit_code(&noisy), FINDINGS_PRESENT);
    }
}
