use maximus_core::{Finding, FixPlan, PlannedFix};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOutcome {
    pub findings: Vec<Finding>,
    pub fixes: Vec<FixPlan>,
    pub planned_fixes: Vec<PlannedFix>,
}
