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
pub mod configuration_recheck;
pub mod consent;
pub mod doctor;
pub mod execution_gate;
pub mod executor;
pub mod explain;
pub mod facts;
pub mod history;
pub mod mapping;
pub mod plan_tie;
pub mod planner;
pub mod provider_commands;
pub mod report;
pub mod restore;
pub mod verification_json;

pub use analysis::{analyze, Analysis, AnalysisOptions, AnalysisSource};
pub use capture::{capture, CaptureContext};
pub use configuration_recheck::{
    recheck_provider_configuration, ConfigurationDrift, ConfigurationRechecked,
};
pub use consent::{
    confirm, confirmation_phrase, consent_for, elevation_requests, ConsentCode, ConsentDecision,
    ConsentGiven, ConsentHalt, ConsentRefusal, ConsentTokens, ElevationRequest,
};
pub use doctor::{
    doctor, interrupted_run_finding, DoctorFinding, DoctorReport, RunFacts, Severity,
    StrandedRecord,
};
pub use execution_gate::{
    clear, evaluable_in_this_build, execution_refusals, execution_refusals_for, mutation_allowance,
    ExecutionRefusal, GateClear, GateReport, MutationAllowance,
};
pub use executor::{
    certify_destination, destination_still_verified, judge_mutation, judge_step, next_action,
    next_action_with, outcome_of, plan_still_holds, refused, reverify_for, Action, CheckKind,
    Disposition, HaltReason, MutationResult, PlannedRun, Precondition, Reverify, RunOutcome,
    StateFact, StepRead, StepRecord, Unevaluable, PARTIAL_SUCCESS_EXIT_CODE,
};
pub use explain::{explain_package, Explanation};
pub use history::{
    migration_key, must_record, read_back_matches, recognise_retained_run, restorability,
    run_record, HistoryConsultation, NotRecognised, Recognition, SnapshotLook,
};
pub use plan_tie::{confirm_shown, plan_phrase, plan_question_error, ShownPlan};
pub use planner::{
    build_plan, migration_start, plan_outcome, plan_run, MigrationStart, PlanOutcome, PlanRequest,
};
pub use report::{PackageReport, ProviderReport, ProviderRow, RowPresence, Summary};
pub use restore::{
    check_same_package, notes as restore_notes, restore_request, KeepReason, Removal,
    RestoreIntent, RestoreNote, RestoreRefusal,
};

/// The tool version, from the workspace manifest.
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The analysis output schema version (`docs/schemas/analysis.v1.md`).
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;
