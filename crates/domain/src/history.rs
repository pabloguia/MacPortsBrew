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
//! **A record is read back only through [`RunRecord::from_stored`]**, which
//! re-derives every derived field and refuses a record that disagrees with
//! itself (INV-03): a file never supplies its own status. The store that
//! writes and reads the file is in the adapters (`persistence::history_store`);
//! nothing in this build dispatches a mutating step, so the shipped binary
//! writes no record yet (pass 08, slice 1).

use crate::text::is_control_class;
use crate::{DomainError, FailureCause};
use crate::{ErrorId, Outcome, PackageIdentity, ProviderId, ReadEvidence, StepKind, Version};
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnvironmentChange {}

/// How a mutating command ended, which is all a record keeps of it: never
/// what it printed (shell rule 5). An exit status is not a step's success
/// (`04-migration-and-safety.md`): the verification step after it judges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "end")]
pub enum CommandEnd {
    /// It exited with this status.
    Exited {
        /// The exit status.
        code: i32,
    },
    /// It was killed, timed out, or never reported a status.
    DidNotFinish,
}

impl CommandEnd {
    /// Whether it exited 0.
    pub fn exited_zero(self) -> bool {
        self == Self::Exited { code: 0 }
    }
}

impl From<crate::ProcessEnd> for CommandEnd {
    fn from(end: crate::ProcessEnd) -> Self {
        match end {
            crate::ProcessEnd::Exited { code } => Self::Exited { code },
            crate::ProcessEnd::DidNotFinish => Self::DidNotFinish,
        }
    }
}

/// What became of one step, as history keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "result")]
pub enum StepResult {
    /// A verification step whose every check was evaluated and passed.
    Verified {
        /// One per command, in order.
        evidence: Vec<ReadEvidence>,
    },
    /// `SNAPSHOT_BEFORE` or `RECORD_HISTORY` on a run that did not perform it:
    /// the read-only loop, or a bracket the run never reached. A record that
    /// dispatched a mutation never states this for `SNAPSHOT_BEFORE`
    /// ([`RunRecord::from_stored`]).
    NotPerformedInThisBuild,
    /// `SNAPSHOT_BEFORE`: the snapshot was written durably and read back before
    /// the first change. Its file name is the record's `snapshot_path`.
    SnapshotTaken,
    /// `RECORD_HISTORY`: this record, written. **A record never certifies its
    /// own write** (pass 08 slice 5, D3): the step is stored pending, and what
    /// confirms the record is a read-back under the run lock followed by the
    /// interrupted-run marker being cleared. A record whose id the marker still
    /// names is not confirmed.
    RecordPendingReadBack,
    /// A mutating step with no command, passed over and not dispatched.
    NotDispatchedMutating,
    /// A mutating step whose every command exited 0. Carried out, and not
    /// thereby a success: the verification step after it judges.
    Performed {
        /// One per command, in order; every one exited 0.
        ends: Vec<CommandEnd>,
    },
    /// The step stopped the run.
    Halted {
        /// The error the halt reports — an identifier, never plan text.
        error: ErrorId,
        /// The reads made before it stopped.
        evidence: Vec<ReadEvidence>,
        /// How the mutating commands that ran before it stopped ended. Empty
        /// for a read step, and left out of the stored document when empty.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        ends: Vec<CommandEnd>,
    },
    /// The run stopped before this step.
    NotReached,
}

impl StepResult {
    /// Whether the step was carried out: a verification that passed, or a
    /// mutation every command of which exited 0.
    pub fn carried_out(&self) -> bool {
        matches!(
            self,
            Self::Verified { .. }
                | Self::Performed { .. }
                | Self::SnapshotTaken
                | Self::RecordPendingReadBack
        )
    }
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
    /// that kept a read, or the end of a mutating command, was dispatched
    /// whatever the caller says.
    pub fn new(
        index: u32,
        kind: StepKind,
        provider: Option<ProviderId>,
        result: StepResult,
        halted_after_dispatch: bool,
    ) -> Self {
        let dispatched = match &result {
            StepResult::Verified { .. } | StepResult::Performed { .. } => true,
            StepResult::Halted { evidence, ends, .. } => {
                halted_after_dispatch || !evidence.is_empty() || !ends.is_empty()
            }
            StepResult::NotPerformedInThisBuild
            | StepResult::SnapshotTaken
            | StepResult::RecordPendingReadBack
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// Whether what a run changed can be put back, **derived on read** from the
/// record, the snapshot it names and the record's standing
/// (`application::history::restorability`). Never stored: it depends on a file
/// that can disappear after the record was written (history schema 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "restorable")]
pub enum Restorability {
    /// The run changed nothing, so there is nothing to put back.
    NothingToRestore,
    /// A reinstall plan can be built from the snapshot the run took. This is
    /// not a promise that every artifact is still available or that versions
    /// match: the plan shows where they differ (04, "Undoing a migration").
    FromSnapshot,
    /// A plan can be built, but the machine's state after the run is not known
    /// well enough to trust it unreviewed.
    Partially {
        /// Why not all of it.
        reason: NotRestorable,
    },
    /// No plan is offered.
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
    /// The snapshot the record names is not in the state directory.
    SnapshotMissing,
    /// The snapshot is there but cannot be trusted or read.
    SnapshotUnreadable,
    /// The snapshot is readable but is not the one this run took.
    SnapshotNotThisRuns,
    /// The record is not confirmed: the interrupted-run marker still names it
    /// (04: a record not known to be durable is never offered for restore).
    RecordNotConfirmed,
    /// Whether the record is confirmed cannot be established: the marker is
    /// unreadable.
    StandingUnknown,
    /// A mutating command did not finish or exited non-zero: the machine's
    /// state after it is not known.
    OutcomeUncertain,
}

/// What the interrupted-run marker says, as a reader found it. There is one
/// marker per state directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkerState {
    /// None is there.
    Absent,
    /// It names this run.
    Names(String),
    /// Something is there that cannot be read as a marker, or the state
    /// directory could not be trusted: fail closed.
    Unreadable,
}

/// Whether a record is known to have been published and read back
/// (pass 08 slice 5, D3): the marker is the confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecordStanding {
    /// No marker names it, and the marker was readable.
    Confirmed,
    /// The marker names it: published, not confirmed.
    Unconfirmed,
    /// The marker could not be read, so nothing is known.
    Unknown,
}

impl MarkerState {
    /// The standing the marker gives the record with this id.
    #[must_use]
    pub fn standing_of(&self, record_id: &str) -> RecordStanding {
        match self {
            Self::Absent => RecordStanding::Confirmed,
            Self::Names(id) if id == record_id => RecordStanding::Unconfirmed,
            Self::Names(_) => RecordStanding::Confirmed,
            Self::Unreadable => RecordStanding::Unknown,
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
        // also needs every step carried out (verified, or a mutation
        // performed), and a completion the steps do not bear out is recorded
        // as partial — never success (INV-03).
        let status = match RunStatus::of(&ending, mutation_dispatched) {
            RunStatus::Success
                if steps.is_empty() || !steps.iter().all(|s| s.result.carried_out()) =>
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

    /// The version of the tool that ran.
    pub fn tool_version(&self) -> &str {
        &self.tool_version
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

    /// The file name of the snapshot the run took first, when it took one.
    pub fn snapshot_path(&self) -> Option<&str> {
        self.snapshot_path.as_deref()
    }
}

// --- Read-back --------------------------------------------------------------

/// The schema version a stored record carries (`docs/schemas/history.v2.md`).
pub const HISTORY_SCHEMA_VERSION: u32 = 2;

/// Characters in a compact UTC stamp, `YYYYMMDDTHHMMSSZ`.
const STAMP_CHARS: usize = 16;
/// Lowercase hex characters after the stamp in a run id.
const RUN_ID_SUFFIX_CHARS: usize = 16;
/// The longest stamp a snapshot file name may carry; the adapters' writer
/// emits no longer one, and a test there holds the two together.
pub const SNAPSHOT_STAMP_MAX_CHARS: usize = 32;

/// Whether `id` is a run id: `<YYYYMMDDTHHMMSSZ>-<16 lowercase hex>`. The one
/// grammar for an id a record states and an id a user types, checked before
/// either becomes part of a path (pass 08).
pub fn is_run_id(id: &str) -> bool {
    let Some((stamp, suffix)) = id.split_once('-') else {
        return false;
    };
    is_compact_stamp(stamp)
        && suffix.len() == RUN_ID_SUFFIX_CHARS
        && suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `name` is a file name the snapshot writer can emit:
/// `snapshot-<1..=32 ASCII alphanumerics>.json`. A record keeps only this name,
/// so no record can point outside the snapshot directory (no separator, no
/// `..`, nothing absolute, nothing empty).
pub fn is_snapshot_file_name(name: &str) -> bool {
    name.strip_prefix("snapshot-")
        .and_then(|rest| rest.strip_suffix(".json"))
        .is_some_and(|stamp| {
            !stamp.is_empty()
                && stamp.len() <= SNAPSHOT_STAMP_MAX_CHARS
                && stamp.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// `YYYYMMDDTHHMMSSZ`, digits where digits go.
fn is_compact_stamp(stamp: &str) -> bool {
    let b = stamp.as_bytes();
    b.len() == STAMP_CHARS
        && b.iter().enumerate().all(|(i, c)| match i {
            8 => *c == b'T',
            15 => *c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

/// The compact stamp of an RFC 3339 UTC instant written as
/// `YYYY-MM-DDTHH:MM:SSZ`, or `None` for anything else.
fn compact_of(started_at: &str) -> Option<String> {
    let b = started_at.as_bytes();
    let shaped = b.len() == 20
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 | 16 => *c == b':',
            19 => *c == b'Z',
            _ => c.is_ascii_digit(),
        });
    shaped.then(|| {
        started_at
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect()
    })
}

/// The envelope a record is stored in: the schema version, then the record.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEnvelope<'a> {
    schema_version: u32,
    #[serde(flatten)]
    record: &'a RunRecord,
}

impl RunRecord {
    /// The record as it is stored, under [`HISTORY_SCHEMA_VERSION`].
    pub fn stored(&self) -> StoredEnvelope<'_> {
        StoredEnvelope {
            schema_version: HISTORY_SCHEMA_VERSION,
            record: self,
        }
    }

    /// Reads a stored record back. **Nothing derived is taken from the file**
    /// (INV-03): the steps' dispatch, the status, whether a mutation was
    /// dispatched and the restorability are derived again exactly as
    /// [`RunRecord::new`] derives them, and a record whose stored values
    /// disagree is refused — disagreement is corruption or tampering, never
    /// something to overwrite silently.
    ///
    /// Also refused: another schema version; an id outside the run-id grammar
    /// or naming another instant than `startedAt`; a `startedAt` that is not
    /// `YYYY-MM-DDTHH:MM:SSZ`; a Control-class character in any string (a
    /// Format character such as a joiner is a legal part of a package name,
    /// as in a snapshot); a snapshot path that is anything but a snapshot file
    /// name; evidence whose result disagrees with its cause. The detail names
    /// the field, never its value.
    pub fn from_stored(stored: StoredRunRecord) -> Result<Self, DomainError> {
        let unusable =
            |what: &str| DomainError::new(ErrorId::HistoryUnusable, format!("the record {what}"));
        if stored.schema_version != HISTORY_SCHEMA_VERSION {
            return Err(unusable("has a schema version this build does not read"));
        }
        if stored.strings().any(|s| s.chars().any(is_control_class)) {
            return Err(unusable("contains a control character"));
        }
        if !is_run_id(&stored.id) {
            return Err(unusable("has an id outside the run-id grammar"));
        }
        let Some(compact) = compact_of(&stored.started_at) else {
            return Err(unusable("has a start time that is not RFC 3339 UTC"));
        };
        if !stored.id.starts_with(&compact) {
            return Err(unusable(
                "has an id naming another instant than its start time",
            ));
        }
        if stored
            .snapshot_path
            .as_deref()
            .is_some_and(|name| !is_snapshot_file_name(name))
        {
            return Err(unusable(
                "has a snapshot path that is not a snapshot file name",
            ));
        }

        // The field is required, and a non-empty list cannot be parsed: no
        // variant exists in this build. Matching on it says so to the reader.
        if let Some(change) = stored.environment_changes.first() {
            match *change {}
        }

        let mut steps = Vec::with_capacity(stored.steps.len());
        for step in stored.steps {
            let result = step.result.into_result().ok_or_else(|| {
                unusable("has evidence that disagrees with its cause or with its step's result")
            })?;
            let recorded = RecordedStep::new(
                step.index,
                step.kind,
                step.provider,
                result,
                step.dispatched,
            );
            // A manual review is never carried out: it stops the run, so it is
            // never dispatched and never verified.
            if step.kind == StepKind::ManualReview
                && (step.dispatched || matches!(recorded.result, StepResult::Verified { .. }))
            {
                return Err(unusable("states a manual review that was carried out"));
            }
            // A mutation's ends are written only on the two steps this build
            // ever hands to a mutation site (slice 4 review, security 4).
            let mutating_ends = match &recorded.result {
                StepResult::Performed { .. } => true,
                StepResult::Halted { ends, .. } => !ends.is_empty(),
                _ => false,
            };
            if mutating_ends
                && !matches!(step.kind, StepKind::AddDestination | StepKind::RemoveSource)
            {
                return Err(unusable(
                    "states a mutation's result on a step no mutation runs on",
                ));
            }
            // The two brackets state their own results, and only on their own
            // step; a record never certifies its own write (D3), so a
            // `RECORD_HISTORY` is never stored verified.
            let on_bracket = |kind: StepKind| step.kind == kind;
            match &recorded.result {
                StepResult::SnapshotTaken if !on_bracket(StepKind::SnapshotBefore) => {
                    return Err(unusable(
                        "states a snapshot taken by a step that takes none",
                    ));
                }
                StepResult::RecordPendingReadBack if !on_bracket(StepKind::RecordHistory) => {
                    return Err(unusable(
                        "states a pending record on a step that writes none",
                    ));
                }
                StepResult::Verified { .. } if on_bracket(StepKind::RecordHistory) => {
                    return Err(unusable("states its own write verified"));
                }
                _ => {}
            }
            if recorded.dispatched != step.dispatched {
                return Err(unusable("states a step's dispatch its result contradicts"));
            }
            steps.push(recorded);
        }

        let record = RunRecord::new(
            RunStart {
                id: stored.id,
                started_at: stored.started_at,
                tool_version: stored.tool_version,
            },
            RunSubject {
                plan_schema_version: stored.plan_schema_version,
                policy_version: stored.policy_version,
                package: PackageIdentity::new(stored.package.provider, stored.package.name),
                direction: Direction {
                    source: stored.direction.source,
                    destination: stored.direction.destination,
                },
                destination_package: stored.destination_package,
                versions: RecordedVersions {
                    source: stored.versions.source,
                    destination: stored.versions.destination,
                },
                action: stored.action.into_outcome(),
                overrode: stored.overrode.map(StoredOutcome::into_outcome),
                retain_source: stored.retain_source,
                primary_after: stored.primary_after,
            },
            steps,
            stored.ending.into_ending(),
            stored.snapshot_path,
        );
        if record.mutation_dispatched != stored.mutation_dispatched {
            return Err(unusable("states a mutation dispatch its steps contradict"));
        }
        // 04: a snapshot is taken before the first change. A record that
        // dispatched a mutation without one, or that names a snapshot no step
        // took, contradicts itself.
        let took_snapshot = record
            .steps
            .iter()
            .any(|s| matches!(s.result, StepResult::SnapshotTaken));
        if record.mutation_dispatched && !took_snapshot {
            return Err(unusable(
                "states a mutation dispatched without a snapshot taken first",
            ));
        }
        if took_snapshot != record.snapshot_path.is_some() {
            return Err(unusable("states a snapshot its steps contradict"));
        }
        if record.status != stored.status {
            return Err(unusable("states a status its steps contradict"));
        }
        Ok(record)
    }
}

/// A record exactly as a file states it, before [`RunRecord::from_stored`]
/// has checked anything. Its fields are private: the only thing to do with
/// one is hand it to `from_stored`. Every level refuses unknown fields.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredRunRecord {
    schema_version: u32,
    id: String,
    started_at: String,
    tool_version: String,
    plan_schema_version: u32,
    policy_version: String,
    package: StoredPackage,
    direction: StoredDirection,
    destination_package: Option<String>,
    versions: StoredVersions,
    action: StoredOutcome,
    overrode: Option<StoredOutcome>,
    retain_source: bool,
    primary_after: ProviderId,
    environment_changes: Vec<EnvironmentChange>,
    steps: Vec<StoredStep>,
    ending: StoredEnding,
    status: RunStatus,
    mutation_dispatched: bool,
    snapshot_path: Option<String>,
}

impl StoredRunRecord {
    /// Every string the record states, for the control-character check.
    fn strings(&self) -> impl Iterator<Item = &str> {
        let fixed = [
            Some(self.id.as_str()),
            Some(self.started_at.as_str()),
            Some(self.tool_version.as_str()),
            Some(self.policy_version.as_str()),
            Some(self.package.provider.as_str()),
            Some(self.package.name.as_str()),
            Some(self.direction.source.as_str()),
            Some(self.direction.destination.as_str()),
            self.destination_package.as_deref(),
            self.versions.source.as_ref().map(Version::as_str),
            self.versions.destination.as_ref().map(Version::as_str),
            outcome_provider(&self.action),
            self.overrode.as_ref().and_then(outcome_provider),
            Some(self.primary_after.as_str()),
            self.snapshot_path.as_deref(),
        ];
        fixed.into_iter().flatten().chain(
            self.steps
                .iter()
                .filter_map(|s| s.provider.as_ref().map(ProviderId::as_str)),
        )
    }
}

/// The provider an outcome names, when it names one.
fn outcome_provider(outcome: &StoredOutcome) -> Option<&str> {
    match outcome {
        StoredOutcome::Keep { provider } | StoredOutcome::MigrateTo { provider } => {
            Some(provider.as_str())
        }
        StoredOutcome::KeepBoth { primary } => Some(primary.as_str()),
        StoredOutcome::UseRuntimeManager {}
        | StoredOutcome::ManualReview {}
        | StoredOutcome::NoAction {} => None,
    }
}

/// [`Outcome`] as a record states it: refuses unknown keys, unit variants
/// included (see the note above [`StoredEnding`]).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "kind", deny_unknown_fields)]
enum StoredOutcome {
    Keep { provider: ProviderId },
    MigrateTo { provider: ProviderId },
    KeepBoth { primary: ProviderId },
    UseRuntimeManager {},
    ManualReview {},
    NoAction {},
}

impl StoredOutcome {
    fn into_outcome(self) -> Outcome {
        match self {
            Self::Keep { provider } => Outcome::Keep { provider },
            Self::MigrateTo { provider } => Outcome::MigrateTo { provider },
            Self::KeepBoth { primary } => Outcome::KeepBoth { primary },
            Self::UseRuntimeManager {} => Outcome::UseRuntimeManager,
            Self::ManualReview {} => Outcome::ManualReview,
            Self::NoAction {} => Outcome::NoAction,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredPackage {
    provider: ProviderId,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredDirection {
    source: ProviderId,
    destination: ProviderId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredVersions {
    source: Option<Version>,
    destination: Option<Version>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredStep {
    index: u32,
    kind: StepKind,
    provider: Option<ProviderId>,
    dispatched: bool,
    result: StoredStepResult,
}

#[derive(Debug, Deserialize)]
#[serde(
    rename_all = "SCREAMING_SNAKE_CASE",
    tag = "result",
    deny_unknown_fields
)]
enum StoredStepResult {
    Verified {
        evidence: Vec<StoredEvidence>,
    },
    NotPerformedInThisBuild {},
    SnapshotTaken {},
    RecordPendingReadBack {},
    NotDispatchedMutating {},
    Performed {
        ends: Vec<StoredCommandEnd>,
    },
    Halted {
        error: ErrorId,
        evidence: Vec<StoredEvidence>,
        #[serde(default)]
        ends: Vec<StoredCommandEnd>,
    },
    NotReached {},
}

/// [`CommandEnd`] as a record states it; struct variants refuse unknown keys
/// (see the note above [`StoredEnding`]).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "end", deny_unknown_fields)]
enum StoredCommandEnd {
    Exited { code: i32 },
    DidNotFinish {},
}

impl StoredCommandEnd {
    fn into_end(self) -> CommandEnd {
        match self {
            Self::Exited { code } => CommandEnd::Exited { code },
            Self::DidNotFinish {} => CommandEnd::DidNotFinish,
        }
    }
}

impl StoredStepResult {
    /// The result, or `None` when a piece of evidence contradicts itself.
    fn into_result(self) -> Option<StepResult> {
        let evidence = |list: Vec<StoredEvidence>| {
            list.into_iter()
                .map(StoredEvidence::into_evidence)
                .collect::<Option<Vec<_>>>()
        };
        Some(match self {
            Self::Verified { evidence: e } => {
                let evidence = evidence(e)?;
                // The writer marks a step verified only when every read
                // passed (`judge_step` halts on the first failure).
                if !evidence.iter().all(ReadEvidence::passed) {
                    return None;
                }
                StepResult::Verified { evidence }
            }
            Self::NotPerformedInThisBuild {} => StepResult::NotPerformedInThisBuild,
            Self::SnapshotTaken {} => StepResult::SnapshotTaken,
            Self::RecordPendingReadBack {} => StepResult::RecordPendingReadBack,
            Self::NotDispatchedMutating {} => StepResult::NotDispatchedMutating,
            Self::Performed { ends } => {
                let ends: Vec<CommandEnd> =
                    ends.into_iter().map(StoredCommandEnd::into_end).collect();
                // The writer marks a mutation performed only when it ran at
                // least one command and every one exited 0.
                if ends.is_empty() || !ends.iter().all(|e| e.exited_zero()) {
                    return None;
                }
                StepResult::Performed { ends }
            }
            Self::Halted {
                error,
                evidence: e,
                ends,
            } => StepResult::Halted {
                error,
                evidence: evidence(e)?,
                ends: ends.into_iter().map(StoredCommandEnd::into_end).collect(),
            },
            Self::NotReached {} => StepResult::NotReached,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredEvidence {
    passed: bool,
    line_count: usize,
    truncated: bool,
    #[serde(default)]
    cause: Option<FailureCause>,
}

impl StoredEvidence {
    fn into_evidence(self) -> Option<ReadEvidence> {
        ReadEvidence::from_recorded(self.passed, self.line_count, self.truncated, self.cause)
    }
}

// Unit variants are written as struct variants here because serde accepts
// unknown keys beside the tag of an internally tagged *unit* variant even
// under `deny_unknown_fields`; an empty struct variant refuses them.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "kind", deny_unknown_fields)]
enum StoredEnding {
    Completed {},
    PartialSuccess { step: u32, error: ErrorId },
    Halted { step: u32, error: ErrorId },
    Refused { refusals: usize },
}

impl StoredEnding {
    fn into_ending(self) -> RunEnding {
        match self {
            Self::Completed {} => RunEnding::Completed,
            Self::PartialSuccess { step, error } => RunEnding::PartialSuccess { step, error },
            Self::Halted { step, error } => RunEnding::Halted { step, error },
            Self::Refused { refusals } => RunEnding::Refused { refusals },
        }
    }
}

// --- What a consulted history says about one migration ----------------------

/// Which migration a record is about, for recognising a re-run (INV-16):
/// the package under its source provider, the destination provider and the
/// destination package. Direction-neutral — nothing here knows which provider
/// is which (INV-17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationKey {
    /// The package, under its source provider.
    pub package: PackageIdentity,
    /// Where it was to go.
    pub destination: ProviderId,
    /// The destination package name.
    pub destination_package: Option<String>,
}

impl MigrationKey {
    /// Whether `record` is about this migration.
    pub fn matches(&self, record: &RunRecord) -> bool {
        let subject = record.subject();
        subject.package == self.package
            && subject.direction.source == self.package.provider
            && subject.direction.destination == self.destination
            && subject.destination_package == self.destination_package
    }
}

/// What a consulted history says about one [`MigrationKey`], as the store
/// read it: newest first, stopping at the first record for the key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryView {
    newest_stamp: Option<String>,
    latest_for_key: Vec<RunRecord>,
    marker: MarkerState,
}

impl HistoryView {
    /// A history with no record at all.
    pub fn empty() -> Self {
        Self {
            newest_stamp: None,
            latest_for_key: Vec::new(),
            marker: MarkerState::Absent,
        }
    }

    /// The store's answer. `newest_stamp` is the compact stamp of the newest
    /// record of any key; `latest_for_key` is every record for the key that
    /// carries the newest stamp among that key's records — more than one is a
    /// tie the reader cannot order, which is never recognised (D1).
    ///
    /// `marker` is what the interrupted-run marker said when the view was read:
    /// a record is recognised as done only when the marker does not name it
    /// (pass 08 slice 5, H1), and one that cannot be read recognises nothing.
    pub fn new(
        newest_stamp: Option<String>,
        latest_for_key: Vec<RunRecord>,
        marker: MarkerState,
    ) -> Self {
        Self {
            newest_stamp,
            latest_for_key,
            marker,
        }
    }

    /// What the marker said when the view was read.
    pub fn marker(&self) -> &MarkerState {
        &self.marker
    }

    /// The standing of one of this view's records.
    pub fn standing_of(&self, record: &RunRecord) -> RecordStanding {
        self.marker.standing_of(record.id())
    }

    /// The compact stamp of the newest record of any key.
    pub fn newest_stamp(&self) -> Option<&str> {
        self.newest_stamp.as_deref()
    }

    /// The key's latest records (one, unless the stamp ties).
    pub fn latest_for_key(&self) -> &[RunRecord] {
        &self.latest_for_key
    }
}

/// The compact stamp a run id begins with, when it is one.
pub fn run_id_stamp(id: &str) -> Option<&str> {
    is_run_id(id).then(|| &id[..STAMP_CHARS])
}
