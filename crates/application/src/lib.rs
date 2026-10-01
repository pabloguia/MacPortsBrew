// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew application layer: discovery, inventory, classification,
//! mapping, assessment, recommendation, planning and doctor
//! (`docs/spec/07-quality-and-testing.md`, "Suggested layering").
//!
//! The shape that matters: **capture** talks to providers through the
//! domain's read contract and produces a [`macportsbrew_domain::Snapshot`];
//! **analysis** is a pure function of a snapshot and the policy data.
//! `analyze --from-snapshot` therefore runs the very same function as a live
//! analysis, which is how INV-15 is proven rather than asserted. No I/O
//! happens in this crate except through the traits the domain declares.

pub mod analysis;
pub mod capture;
pub mod classify;
pub mod doctor;
pub mod execution_gate;
pub mod executor;
pub mod explain;
pub mod facts;
pub mod mapping;
pub mod planner;
pub mod provider_commands;
pub mod report;

pub use analysis::{analyze, Analysis, AnalysisOptions, AnalysisSource};
pub use capture::{capture, CaptureContext};
pub use doctor::{doctor, DoctorFinding, DoctorReport, Severity};
pub use execution_gate::{
    evaluable_in_this_build, execution_refusals, ExecutionRefusal, GateReport,
};
pub use executor::{
    judge_step, next_action, outcome_of, refused, Action, CheckKind, Disposition, HaltReason,
    PlannedRun, RunOutcome, StepRead, StepRecord, Unevaluable, PARTIAL_SUCCESS_EXIT_CODE,
};
pub use explain::{explain_package, Explanation};
pub use planner::{
    build_plan, migration_start, plan_outcome, plan_run, MigrationStart, PlanOutcome, PlanRequest,
};
pub use report::{PackageReport, ProviderReport, ProviderRow, RowPresence, Summary};

/// The tool version, from the workspace manifest.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The analysis output schema version (`docs/schemas/analysis.v1.md`).
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;
