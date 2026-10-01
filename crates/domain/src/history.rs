// SPDX-License-Identifier: Apache-2.0

//! The record of one run (`04-migration-and-safety.md`, History).
//!
//! [`RunRecord`] holds every field that section lists, and nothing a run read
//! from a provider beyond [`ReadEvidence`]: a verification read is kept as its
//! result, its line count and its truncation flag, never its output, so a
//! record **cannot** carry the machine's inventory — there is no field a
//! provider read can occupy (`plan.v1.md` obligation 3). The caller's own
//! strings ([`RunStart`], the snapshot path) are the caller's responsibility.
//!
//! **No clock is read here.** The start time is a field the caller supplies
//! ([`RunStart::started_at`]); the one wall-clock read in the workspace is
//! `persistence::time::now_secs`, held there by
//! `invariant_14_the_wall_clock_is_read_in_exactly_one_place`.
//!
//! **Two rules are decided here, once, and nowhere else.**
//!
//! 1. A partial success is recorded as [`RunStatus::Partial`], never as
//!    [`RunStatus::Success`] (INV-03).
//! 2. A run that dispatched no mutating step is [`RunStatus::NoChange`],
//!    whatever its ending, so it is never recorded as a migration left half
//!    done (SE-092; `plan.v1.md` obligation 12) — and
//!    [`RunRecord::mutation_dispatched`] is the one fact a store keys a lock or
//!    an interrupted-run marker on.
//!
//! **Persistence is deferred.** Nothing in this build dispatches a mutating
//! step, so an append-only store would be unreachable code; the type exists so
//! the shape is fixed and tested before the mutating pass writes one
//! (`04-migration-and-safety.md`, History, status note).

use crate::{ErrorId, Outcome, PackageIdentity, ProviderId, ReadEvidence, StepKind, Version};
use serde::Serialize;

/// What only the caller can supply: how the run is named and when it began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunStart {
    /// The run's identifier. Assigned by whatever stores the record.
    pub id: String,
    /// When the run began, RFC 3339 UTC — from the single clock read, never
    /// read here.
    pub started_at: String,
    /// The version of the tool that ran.
    pub tool_version: String,
}

/// What the run was about: taken from the plan it executed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSubject {
    /// The plan's schema version.
    pub plan_schema_version: u32,
    /// The policy version the plan was built under.
    pub policy_version: String,
    /// The package, under its source provider.
    pub package: PackageIdentity,
    /// The direction.
    pub direction: Direction,
    /// The destination package name, when a mapping was selected.
    pub destination_package: Option<String>,
    /// Installed versions on each side.
    pub versions: RecordedVersions,
    /// The action the plan followed.
    pub action: Outcome,
    /// The recommendation it overrode, when it did.
    pub overrode: Option<Outcome>,
    /// Whether the source package was to be kept (`--retain-source`).
    pub retain_source: bool,
    /// Which provider the plan made primary afterwards.
    pub primary_after: ProviderId,
}

/// Source and destination provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Direction {
    /// Where the package was.
    pub source: ProviderId,
    /// Where it was to go.
    pub destination: ProviderId,
}

/// The versions a run knows about. `None` is "not established", never "none".
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedVersions {
    /// The source's active version in the snapshot taken before the run.
    pub source: Option<Version>,
    /// The destination's version after the run. A verification read keeps no
    /// version (it keeps [`ReadEvidence`] only), so nothing in this build
    /// establishes it.
    pub destination: Option<Version>,
}

/// A change a run made to the user's environment (a shell block, a shim).
///
/// **Uninhabited in this build**: no step here can change the environment, so
/// the list is always empty, by type. The mutating pass adds variants, and
/// none may carry an environment variable's value (`04-migration-and-safety.md`,
/// History: "Never records secrets or environment variable values").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum EnvironmentChange {}

/// What became of one step, as history keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "result")]
pub enum StepResult {
    /// A verification step whose every check was evaluated and passed.
    Verified {
        /// One per command, in order.
        evidence: Vec<ReadEvidence>,
    },
    /// `SNAPSHOT_BEFORE` or `RECORD_HISTORY`: not performed by this build.
    NotPerformedInThisBuild,
    /// A mutating step with no command, passed over and not dispatched.
    NotDispatchedMutating,
    /// The step stopped the run.
    Halted {
        /// The error the halt reports — an identifier, never plan text.
        error: ErrorId,
        /// The reads made before it stopped.
        evidence: Vec<ReadEvidence>,
    },
    /// The run stopped before this step.
    NotReached,
}

/// One step of the plan the run executed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordedStep {
    index: u32,
    kind: StepKind,
    provider: Option<ProviderId>,
    dispatched: bool,
    result: StepResult,
}

impl RecordedStep {
    /// A step's record. `halted_after_dispatch` says, for a halt, whether the
    /// step had been handed to a runner when it stopped; it is ignored for
    /// every other result, whose dispatch is fixed by the result itself. A halt
    /// that kept a read was dispatched whatever the caller says.
    pub fn new(
        index: u32,
        kind: StepKind,
        provider: Option<ProviderId>,
        result: StepResult,
        halted_after_dispatch: bool,
    ) -> Self {
        let dispatched = match &result {
            StepResult::Verified { .. } => true,
            StepResult::Halted { evidence, .. } => halted_after_dispatch || !evidence.is_empty(),
            StepResult::NotPerformedInThisBuild
            | StepResult::NotDispatchedMutating
            | StepResult::NotReached => false,
        };
        Self {
            index,
            kind,
            provider,
            dispatched,
            result,
        }
    }

    /// The step's index, as the plan numbers it.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The step's kind.
    pub fn kind(&self) -> StepKind {
        self.kind
    }

    /// Whether the step was handed to a runner.
    pub fn dispatched(&self) -> bool {
        self.dispatched
    }

    /// What became of it.
    pub fn result(&self) -> &StepResult {
        &self.result
    }
}

/// How the run loop said the run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "kind")]
pub enum RunEnding {
    /// Every step was carried out and every check passed.
    Completed,
    /// Some steps were carried out and some were not.
    PartialSuccess {
        /// The step that stopped the run, or the first not carried out.
        step: u32,
        /// Why.
        error: ErrorId,
    },
    /// The run stopped having carried out nothing.
    Halted {
        /// The step it stopped at (0 for the run as a whole).
        step: u32,
        /// Why.
        error: ErrorId,
    },
    /// The execution gate refused the plan; nothing ran.
    Refused {
        /// How many refusals the gate reported.
        refusals: usize,
    },
}

/// The run's outcome as history states it: 04's success / partial / failure,
/// and a fourth for a run that changed nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunStatus {
    /// A mutating run that completed.
    Success,
    /// A mutating run that carried out some steps and not others (INV-03).
    Partial,
    /// A mutating run that halted.
    Failure,
    /// The run dispatched no mutating step: no migration happened, partially
    /// or otherwise, whatever its verification reads showed (SE-092).
    NoChange,
}

impl RunStatus {
    /// The status of a run that ended as `ending`. **Partial is never
    /// success** (INV-03), and a run that dispatched no mutation is
    /// [`RunStatus::NoChange`] before anything else is asked (SE-092).
    pub fn of(ending: &RunEnding, mutation_dispatched: bool) -> Self {
        if !mutation_dispatched {
            return Self::NoChange;
        }
        match ending {
            RunEnding::Completed => Self::Success,
            RunEnding::PartialSuccess { .. } => Self::Partial,
            // A refused plan runs nothing, so a dispatched mutation beside a
            // refusal is records that contradict the ending: never success.
            RunEnding::Halted { .. } | RunEnding::Refused { .. } => Self::Failure,
        }
    }
}

/// Whether what a run changed can be put back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "restorable")]
pub enum Restorability {
    /// The run changed nothing, so there is nothing to put back.
    NothingToRestore,
    /// Everything the run changed can be put back.
    Fully,
    /// Some of it can.
    Partially {
        /// Why not all of it.
        reason: NotRestorable,
    },
    /// None of it can.
    Not {
        /// Why.
        reason: NotRestorable,
    },
}

/// Why a run's changes cannot be (fully) put back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotRestorable {
    /// No snapshot was taken before the first change.
    NoSnapshotTaken,
    /// A snapshot exists, but this build has no restore.
    RestoreNotInThisBuild,
}

impl Restorability {
    /// What this build can say. `Fully` and `Partially` need a restore this
    /// build does not have, so it never claims them.
    pub fn of(mutation_dispatched: bool, snapshot_path: Option<&str>) -> Self {
        if !mutation_dispatched {
            return Self::NothingToRestore;
        }
        Self::Not {
            reason: if snapshot_path.is_some() {
                NotRestorable::RestoreNotInThisBuild
            } else {
                NotRestorable::NoSnapshotTaken
            },
        }
    }
}

/// One run, as history records it. Every field `04-migration-and-safety.md`
/// (History) lists, in that order; the status, the restorability and whether a
/// mutation was dispatched are derived here from the steps, never supplied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    id: String,
    started_at: String,
    tool_version: String,
    #[serde(flatten)]
    subject: RunSubject,
    environment_changes: Vec<EnvironmentChange>,
    steps: Vec<RecordedStep>,
    ending: RunEnding,
    status: RunStatus,
    mutation_dispatched: bool,
    snapshot_path: Option<String>,
    restorability: Restorability,
}

impl RunRecord {
    /// Builds the record. `steps` are kept in the order given, which is the
    /// plan's. `snapshot_path` is where the `SNAPSHOT_BEFORE` step wrote its
    /// snapshot; this build performs no such step and passes `None`.
    pub fn new(
        start: RunStart,
        subject: RunSubject,
        steps: Vec<RecordedStep>,
        ending: RunEnding,
        snapshot_path: Option<String>,
    ) -> Self {
        let mutation_dispatched = steps.iter().any(|s| s.kind.is_mutating() && s.dispatched);
        // An ending is supplied, so it is checked against the steps: success
        // also needs every step verified, and a completion the steps do not
        // bear out is recorded as partial — never success (INV-03).
        let status = match RunStatus::of(&ending, mutation_dispatched) {
            RunStatus::Success
                if steps.is_empty()
                    || !steps
                        .iter()
                        .all(|s| matches!(s.result, StepResult::Verified { .. })) =>
            {
                RunStatus::Partial
            }
            status => status,
        };
        Self {
            id: start.id,
            started_at: start.started_at,
            tool_version: start.tool_version,
            subject,
            environment_changes: Vec::new(),
            status,
            restorability: Restorability::of(mutation_dispatched, snapshot_path.as_deref()),
            steps,
            ending,
            mutation_dispatched,
            snapshot_path,
        }
    }

    /// The run's identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// When it began.
    pub fn started_at(&self) -> &str {
        &self.started_at
    }

    /// What it was about.
    pub fn subject(&self) -> &RunSubject {
        &self.subject
    }

    /// The steps, in plan order.
    pub fn steps(&self) -> &[RecordedStep] {
        &self.steps
    }

    /// How the loop said it ended.
    pub fn ending(&self) -> &RunEnding {
        &self.ending
    }

    /// The outcome history states.
    pub fn status(&self) -> RunStatus {
        self.status
    }

    /// Whether any mutating step was handed to a runner. **A store takes a
    /// lock or writes an interrupted-run marker only for a run where this is
    /// true** (SE-092): a run that dispatched no mutation left nothing half
    /// done.
    pub fn mutation_dispatched(&self) -> bool {
        self.mutation_dispatched
    }

    /// Whether its changes can be put back.
    pub fn restorability(&self) -> Restorability {
        self.restorability
    }
}
