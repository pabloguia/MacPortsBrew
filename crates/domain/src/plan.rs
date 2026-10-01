// SPDX-License-Identifier: Apache-2.0

//! Migration plans as data (`04-migration-and-safety.md`, "The plan is the
//! contract"). A plan is serialisable, diffable and reviewable. Nothing in
//! this crate can execute one, and nothing in this build executes a provider
//! command from one: `migrate --yes` reaches a refusal gate
//! (`macportsbrew_application::execution_gate`) that refuses every plan this
//! build produces; the read-step loop behind it exists, is tested against
//! recordings, and is unreachable from the binary; and
//! [`THIS_BUILD_DISPATCHES_MUTATIONS`] is `false` (SE-020).
//!
//! Step kinds follow the state machine in that chapter. The verbs a step
//! would run come from provider data files, never from code, so that the
//! no-mutation grep over `src/` stays meaningful.

use crate::package::PackageIdentity;
use crate::provider::ProviderId;
use crate::reason::Reason;
use crate::recommendation::Outcome;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The kinds of step, in the order the state machine runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepKind {
    /// Take the automatic snapshot before the first mutating step.
    SnapshotBefore,
    /// Add the package under the destination provider.
    AddDestination,
    /// Verify the destination per the verification contract.
    VerifyDestination,
    /// Make the destination the primary implementation.
    SetPrimary,
    /// Verify that `PATH` now resolves to the destination.
    VerifyResolution,
    /// Remove the package from the source provider (optional; INV-01).
    RemoveSource,
    /// Verify the removal.
    VerifyRemoval,
    /// Present cleanup candidates for review (never automatic).
    CleanupReview,
    /// Append the run to history.
    RecordHistory,
    /// A human must act; the plan stops here when executed (exit 4).
    ManualReview,
}

impl StepKind {
    /// Whether executing this kind changes the machine.
    ///
    /// Exhaustive, with no wildcard, on purpose: every gate that brackets or
    /// refuses a mutation keys on this, so a kind added to the domain must be
    /// classified here before this compiles rather than defaulting to
    /// non-mutating and slipping past them (fail closed).
    pub fn is_mutating(self) -> bool {
        match self {
            StepKind::AddDestination
            | StepKind::SetPrimary
            | StepKind::RemoveSource
            | StepKind::CleanupReview => true,
            StepKind::SnapshotBefore
            | StepKind::VerifyDestination
            | StepKind::VerifyResolution
            | StepKind::VerifyRemoval
            | StepKind::RecordHistory
            | StepKind::ManualReview => false,
        }
    }
}

/// Whether a step needs elevation, as the adapter reported it (R-5: per
/// operation and per package, never per provider).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "elevation")]
pub enum Elevation {
    /// Runs as the user.
    None,
    /// Needs administrator rights, for the stated reason.
    Required {
        /// Why (a stable identifier or the provider's own statement).
        reason: String,
    },
    /// The adapter could not say; shown as such, never assumed away.
    Unknown,
}

/// How an executor establishes that a command succeeded.
///
/// Exit status is not the universal answer, and assuming it is fails *open* on
/// one of the two providers this tool exists for. Observed on this machine
/// 2026-09-08 (MacPorts 2.12.6, R-5 E22, F-33): `port -q installed <name>`
/// exits **0** for an installed port, for a real port that is absent, and for a
/// name that is not a port at all — three cases, one exit status. `brew list
/// --versions <name>` exits 1 when the name is absent, so the two providers
/// cannot share one rule.
///
/// This matters because of where it sits. `VERIFY_DESTINATION` is the step
/// INV-01 puts in front of `REMOVE_SOURCE`: an executor that reads "exit 0" as
/// "verified" would pass verification for a package that was never installed
/// and then remove the source. `MigrationPlan::check_ordering` would hold
/// perfectly while the thing it orders proved nothing.
///
/// [`Self::Unknown`] is the value a reader arrives at, never one the data
/// states — see [`PlanCommand::success`].
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "success")]
pub enum SuccessSemantic {
    /// Exit status carries the answer: zero succeeded, nonzero failed.
    ExitStatus,
    /// Exit status carries no information for this command. Success requires
    /// the package's own name in the command's standard output; an empty
    /// stdout is a failure however the process exited.
    ///
    /// **"Names" means a whole field, not a substring**, and the difference is
    /// a data-loss path rather than a nicety. This said only "the package's own
    /// name in the command's standard output" until 2026-09-19. Measured that
    /// day on this machine: of 201 installed formulae, **18 are a substring of
    /// another formula's name** — `gh` inside `ghidra`, `highway` and
    /// `libnghttp2`; `apr` inside `apr-util`; `gd` inside `gdk-pixbuf`. So a
    /// `VERIFY_DESTINATION` for `gh` whose install had failed would find "gh"
    /// in the line for `ghidra` and **pass**, and INV-01 puts precisely that
    /// step in front of `REMOVE_SOURCE` (F-51).
    ///
    /// The rule that closes it, and it is one rule for both providers because
    /// both inventories agree on the shape: **the name must equal the first
    /// whitespace-separated token of some line of stdout.** Observed
    /// 2026-09-19 — `node 26.7.0`, `cryptography 50.0.1 50.0.0` and
    /// `  autoconf @2.73_0 (active)` all carry the name as that token, and
    /// MacPorts' two-space indent is why it is "first token" rather than
    /// "line begins with". A prefix test would not have closed it: `ghidra`
    /// *begins* with `gh`.
    StdoutNamesPackage,
    /// The package's name is **not** the first whitespace-separated token of any
    /// line of a **complete** standard output. How absence is established
    /// (`VERIFY_REMOVAL`, `PACKAGE_ABSENT`).
    ///
    /// **Four conditions, and an executor must check all four before the name
    /// test**, because unlike [`Self::StdoutNamesPackage`] this semantic is
    /// satisfied by *saying nothing*, so every way of saying nothing has to be
    /// excluded first:
    ///
    /// 1. the process exited **0**;
    /// 2. its output was **not truncated**;
    /// 3. stdout is **non-empty**;
    /// 4. only then: the name is not the first token of any line.
    ///
    /// Judged by `verification::evaluate`, which also fails every semantic —
    /// exit status included — on a truncated or unfinished read: a stream cut
    /// mid-line can manufacture a first token. What it judges against is built
    /// only by `verification::expectation_for`, which addresses the step and
    /// the command by position in the plan, and binds the step kind to the
    /// semantic, the provider to the step's side, and the name to the argv and
    /// to `is_safe_provider_name` (SE-089).
    ///
    /// **The first draft of this comment stated only (3) and justified it with
    /// a claim that is false.** It said a provider "crashed, was killed, or
    /// whose output was truncated produces an empty stdout". Truncation does
    /// not: `read_bounded` returns the first `cap` bytes and sets a separate
    /// flag, and a process killed mid-write leaves whatever it had flushed. So
    /// a `brew list --versions` cut off at line 40 of 201 is non-empty, omits
    /// the name, and would have certified a removal it never saw — on the far
    /// side of `REMOVE_SOURCE`, where the package is already gone. An
    /// alphabetical inventory makes a name near the end *more* likely to be in
    /// the lost tail. Found by `security-reviewer`, 2026-09-19.
    ///
    /// Why the single-name form is not available, observed the same day —
    /// MacPorts 2.12.6, Homebrew 6.0.21. Asked about **one** name, each
    /// provider answers three different states identically:
    ///
    /// | | installed | absent | not a package |
    /// |---|---|---|---|
    /// | `port -q installed <n>` | exit 0, names it | exit 0, empty | exit 0, empty |
    /// | `brew list --versions <n>` | exit 0, names it | exit 1, empty | exit 1, empty |
    ///
    /// So "nothing came back" would mean "successfully removed". Asking for the
    /// **whole inventory** instead makes the truthful answer a large positive
    /// output, which is what conditions (1)-(3) can then be demanded of.
    ///
    /// **"Not the first token" is the exact negation of
    /// [`Self::StdoutNamesPackage`]'s rule, and the shared definition is
    /// load-bearing.** It is a **literal token comparison — never a substring,
    /// glob or regular expression.** `is_safe_provider_name` admits `.` and
    /// `+`, and real names use them (`lua@5.4`, `gtk+3`), so a name compiled as
    /// a pattern either fails to compile or matches a different package. Under
    /// a substring rule the 18 measured collisions break the pair in opposite
    /// directions: this one refuses a genuine removal of `gh` while `ghidra`
    /// remains (a false negative, safe), and its mirror falsely certifies a
    /// destination (F-51, not safe).
    ///
    /// **The stdout of such a command is never recorded.** It is the machine's
    /// complete installed inventory — 201 formulae here — which is a strong
    /// fingerprint and routinely names an employer's internal tooling. What may
    /// be written to history, a log or a report is the boolean result, the line
    /// count and the truncation flag.
    ///
    /// The cost, stated rather than discovered: removing the last package a
    /// provider holds leaves an empty inventory, which condition (3) refuses.
    /// That is a false negative — "could not verify", never "verified" — and it
    /// is the direction this project chooses every time. It also needs its own
    /// message, because it is the single-package migration and "could not
    /// verify removal" reads as "the removal failed", whose natural response is
    /// to remove it again.
    StdoutOmitsPackage,
    /// Not stated, or stated in terms this build does not recognise.
    ///
    /// The refusing value, and the default, for the reason
    /// [`PlanCommand::placeholder`] documents as its own weak point: a reader
    /// that meets a document written before this field existed, or one naming a
    /// semantic a newer build added, must not fall back to exit status — that
    /// is the fail-open this type exists to prevent. Here the fallback *is* the
    /// refusing value, which is the trick [`VerificationCheck::Unknown`] uses
    /// and `placeholder` could not.
    #[default]
    #[serde(other)]
    Unknown,
}

/// A command a step would run: argv only, never a shell string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCommand {
    /// What this command invokes: a provider, or MacPortsBrew itself.
    ///
    /// Modelled rather than inferred from the program string. INV-10's
    /// sanitised environment is a rule about **provider** invocations, and the
    /// one command in a plan that is not one used to be recognised by comparing
    /// `program` to a literal — including inside the test named after the
    /// invariant, which filtered it out and so could not see that it also
    /// carried no environment at all. A distinction the type makes cannot be
    /// filtered away by accident.
    #[serde(default)]
    pub target: CommandTarget,
    /// The program, resolved from the provider prefix at plan time.
    pub program: String,
    /// Arguments, each its own element.
    pub argv: Vec<String>,
    /// The sanitised environment the executor would set (INV-10).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    /// Elevation this command needs.
    pub elevation: Elevation,
    /// How an executor establishes that this command succeeded.
    ///
    /// Always serialised, and absent on read means [`SuccessSemantic::Unknown`]
    /// rather than exit status, so a document from a build that predates this
    /// field refuses rather than fails open.
    #[serde(default)]
    pub success: SuccessSemantic,
    /// Whether this command is an illustration rather than something an
    /// executor could run.
    ///
    /// A plan is built for a provider that is not installed too — showing what
    /// *would* happen is the point of a planning-only build — and the program
    /// then reads `<macports prefix>/bin/macports`, with the verb as
    /// `<verify>`. Those are placeholders, and until this field existed nothing
    /// distinguished them from a real command: a plan carrying them serialised,
    /// diffed, reviewed and passed `check_ordering`, which never looks at
    /// commands at all (F-20).
    ///
    /// An executor must refuse a plan containing one rather than run the steps
    /// around it, for the same reason an unrecognised verification check cannot
    /// count as satisfied: what it cannot perform is exactly the part that was
    /// unknown when the plan was built.
    ///
    /// **Always written, never skipped, and the default is the unsafe one.**
    /// That is worth stating because it is the weak point of this shape and not
    /// a detail: `false` means runnable, so a reader that ignores the field, or
    /// a document written before it existed, treats an illustration as a real
    /// command. [`VerificationCheck::Unknown`] does not have this problem — an
    /// input a reader cannot parse *becomes* the refusing value. There is no
    /// equivalent trick for a boolean without inverting its sense, which would
    /// make every real command carry a field saying so. So it is always
    /// serialised, so that a reader which looks never finds it missing, and
    /// `plan.v1.md` states the hazard in the same row rather than leaving it to
    /// be discovered.
    #[serde(default)]
    pub placeholder: bool,
}

/// What a [`PlanCommand`] invokes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "target")]
pub enum CommandTarget {
    /// A provider's own executable, resolved from its detected prefix. INV-10
    /// applies: the environment is constructed and never inherited, and the
    /// program is absolute so that correctness does not depend on `PATH`.
    #[default]
    Provider,
    /// MacPortsBrew itself, for a step it performs rather than delegates —
    /// selecting the primary provider is the only one, because the shim
    /// directory is this tool's own (SE-036).
    ///
    /// Not a provider invocation, so INV-10's sanitised environment is not what
    /// governs it. It is equally not something to resolve through `PATH`: the
    /// program here is the name a person reads in the plan, and an executor
    /// must not resolve this name through `PATH` — most likely by running the
    /// binary it is already running, or by performing the step in-process, which
    /// the executor pass settles and this type does not —
    /// which is the whole hazard `doctor` exists to warn about.
    Tool,
}

/// One verification check of the contract in `04-migration-and-safety.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "check")]
pub enum VerificationCheck {
    /// The destination provider reports the package installed and active.
    ProviderReportsActive,
    /// The provider's own integrity check passes, where offered.
    ProviderIntegrityCheck,
    /// Expected executables exist at paths the destination owns, confirmed
    /// by asking the provider.
    ExecutablesOwnedByDestination {
        /// The executables expected.
        executables: Vec<String>,
    },
    /// A liveness probe with a safe flag exits successfully.
    LivenessProbe {
        /// Executable.
        executable: String,
        /// The non-mutating flag (`--version` by default).
        flag: String,
    },
    /// No new cross-provider linkage was introduced.
    NoCrossProviderLinkage,
    /// The source provider reports the package **no longer** installed.
    ///
    /// `VERIFY_REMOVAL` used to carry `ProviderReportsActive`, which is
    /// documented as "reports the package installed and active" - so as data the
    /// plan said: verify the removal by confirming the package is still there.
    /// Nothing executes yet, so nothing acted on it; an executor would have had
    /// to either silently invert it or find nothing to run (F-06). The plan is
    /// the contract (INV-02), and a contract that says the opposite of what it
    /// means is not one.
    PackageAbsent,
    /// The destination reports the package installed **and did not at plan
    /// time**.
    ///
    /// The distinction is what makes `VERIFY_DESTINATION` a gate rather than a
    /// formality. All five contract checks are satisfied by state that predates
    /// the plan, so for a package already present at the destination a failed or
    /// no-op install still cleared the gate INV-01 depends on. This variant is
    /// emitted only where the destination was absent when the plan was built,
    /// which is exactly when "the provider now reports it" proves the install
    /// happened. Where it was already present, the plan says so instead of
    /// implying a proof it cannot give.
    PackageNewlyPresent,
    /// A check written by a build newer than this one, or a name that did not
    /// survive whatever produced this plan.
    ///
    /// **Tolerated on the way in, never satisfied.** Reading a plan that names
    /// a check this build does not know must not refuse the whole document —
    /// that is what keeps adding a variant additive. But a verification step is
    /// the gate INV-01 rests on, and `.claude/rules/domain.md` states the rule
    /// this variant would otherwise break: *an unknown is never a yes*. An
    /// executor that iterated the checks it recognised, found them satisfied
    /// and cleared the gate would re-open F-06's own defect through the door
    /// opened to make F-06's fix additive — an older build reading a newer
    /// plan, missing `PackageNewlyPresent`, and treating pre-existing state as
    /// proof of an install.
    ///
    /// So [`VerificationCheck::is_satisfiable`] is `false` here, and a step
    /// carrying one cannot pass. The precedent this variant follows is
    /// `PathFindingKind::Unknown`, and the difference matters: that one labels
    /// a `doctor` finding, where the worst case is a mislabelled warning.
    #[serde(other)]
    Unknown,
}

impl VerificationCheck {
    /// Whether this check is one an executor could ever report as passing.
    ///
    /// `false` only for [`VerificationCheck::Unknown`]. A step carrying an
    /// unrecognised check fails closed: see that variant for why a union used
    /// as a safety gate cannot treat "not understood" as "not applicable".
    #[must_use]
    pub fn is_satisfiable(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// A risk the planner detected, with the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRisk {
    /// Stable identifier (`reverse_dependencies`, `stateful`, `service`,
    /// `deprecated_destination`, `source_build`).
    pub id: String,
    /// The reason, renderable.
    pub reason: Reason,
    /// Whether this risk blocks execution (a `MANUAL_REVIEW` step) or only
    /// warns.
    pub blocking: bool,
}

/// One step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationStep {
    /// Position, from 1.
    pub index: u32,
    /// Kind.
    pub kind: StepKind,
    /// The provider this step talks to, when it talks to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    /// Commands the step would run, in order. Empty for verification-only
    /// and manual steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<PlanCommand>,
    /// Verification checks the step performs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification: Vec<VerificationCheck>,
    /// The reason this step exists, renderable.
    pub reason: Reason,
}

/// The plan for one package.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlan {
    /// Plan schema version.
    pub schema_version: u32,
    /// The package under the source provider.
    pub package: PackageIdentity,
    /// Source provider.
    pub source: ProviderId,
    /// Destination provider.
    pub destination: ProviderId,
    /// The destination package name, when a mapping was selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_package: Option<String>,
    /// Whether the source package is kept (`--retain-source`).
    pub retain_source: bool,
    /// Which provider becomes primary afterwards.
    pub primary_after: ProviderId,
    /// The recommendation this plan follows, and whether the user overrode
    /// a different one.
    pub recommendation: Outcome,
    /// The recommendation the user overrode, when they did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overrode: Option<Outcome>,
    /// Ordered steps.
    pub steps: Vec<MigrationStep>,
    /// Detected risks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<PlanRisk>,
    /// Steps that need elevation, by index, for the summary the user sees.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elevated_steps: Vec<u32>,
    /// The policy version the plan was built under.
    pub policy_version: String,
    /// Whether the build that produced this plan can execute its mutating
    /// steps: the planner writes `!THIS_BUILD_DISPATCHES_MUTATIONS`, so this is
    /// `true` for every plan this build produces and describes a compile-time
    /// property rather than a choice.
    ///
    /// **Not the boundary.** Execution must pass a refusal gate
    /// (`macportsbrew_application::execution_gate`) that refuses every plan
    /// this build produces — this field is one of its refusals
    /// (`PLAN_NOT_DRY_RUN_ONLY`, for a plan claiming otherwise), not the
    /// reason nothing runs: `migrate --yes` reaches a refusal gate that
    /// refuses every plan this build produces, and the read-step loop behind
    /// it is unreachable from the binary. The field keeps the document
    /// truthful; the gate, and the absence of any path to a provider's
    /// mutating verbs, keep the machine untouched.
    pub dry_run_only: bool,
}

/// Whether this build dispatches a plan's mutating steps. `false`: nothing
/// executes a provider command that changes the machine (SE-020).
///
/// **Read in exactly two places in product source**, and a textual test in
/// `crates/application/tests/execution_gate.rs` holds it there: the planner,
/// which writes `dry_run_only: !THIS_BUILD_DISPATCHES_MUTATIONS`, and the
/// execution gate, which refuses the first mutating step while this is
/// `false` and a plan whose `dryRunOnly` is not its negation. Flipping it is
/// not how the mutating pass begins — that pass owes the obligations
/// `plan.v1.md` lists first — and no dispatch guard may read it, so that a
/// flipped constant alone cannot reach a provider.
pub const THIS_BUILD_DISPATCHES_MUTATIONS: bool = false;

/// The current plan schema version.
pub const PLAN_SCHEMA_VERSION: u32 = 1;

impl MigrationPlan {
    /// True when any step is a manual-review stop.
    pub fn requires_manual_review(&self) -> bool {
        self.steps.iter().any(|s| s.kind == StepKind::ManualReview)
    }

    /// True when any step would change the machine.
    pub fn has_mutating_steps(&self) -> bool {
        self.steps.iter().any(|s| s.kind.is_mutating())
    }

    /// Steps that carry a command known to need administrator rights.
    ///
    /// **Not the set to gate consent on.** That is [`Self::elevated_steps`],
    /// which is Required ∪ Unknown: an elevation nobody has established is not
    /// an elevation that will not happen, and the executor must ask before a
    /// step it cannot vouch for. This set exists so that a *report* can say
    /// "these need administrator rights" and be true — the sentence that used
    /// to be said about the conservative set, which is how a plan came to tell
    /// a user that `port -q installed` would ask for a password (F-10).
    #[must_use]
    pub fn steps_requiring_elevation(&self) -> Vec<u32> {
        self.steps
            .iter()
            .filter(|s| {
                s.commands
                    .iter()
                    .any(|c| matches!(c.elevation, Elevation::Required { .. }))
            })
            .map(|s| s.index)
            .collect()
    }

    /// Steps carrying a command whose elevation nobody has established, and no
    /// command that is known to need it.
    ///
    /// Disjoint from [`Self::steps_requiring_elevation`] by construction, and
    /// together with it exactly [`Self::elevated_steps`].
    #[must_use]
    pub fn steps_with_unknown_elevation(&self) -> Vec<u32> {
        let required = self.steps_requiring_elevation();
        self.steps
            .iter()
            .filter(|s| {
                !required.contains(&s.index)
                    && s.commands
                        .iter()
                        .any(|c| matches!(c.elevation, Elevation::Unknown))
            })
            .map(|s| s.index)
            .collect()
    }

    /// Steps carrying a command that is an illustration rather than something
    /// an executor could run.
    ///
    /// Empty for a plan built against providers that were all detected. A plan
    /// with any entry here describes what *would* happen and cannot be
    /// executed; see [`PlanCommand::placeholder`].
    #[must_use]
    pub fn steps_with_placeholders(&self) -> Vec<u32> {
        self.steps
            .iter()
            .filter(|s| s.commands.iter().any(|c| c.placeholder))
            .map(|s| s.index)
            .collect()
    }

    /// Steps carrying a command that does not say how its success is
    /// established.
    ///
    /// Empty for a plan this build produced: the planner reads the semantic
    /// from `provider-commands.toml`, whose loader refuses an entry that omits
    /// one. A non-empty result means the plan was written by another build, or
    /// names a semantic this one does not recognise — and an executor must
    /// refuse it for the same reason it refuses an unrecognised verification
    /// check (`Self::verification_is_performable`): what it cannot read is
    /// exactly the part that decides whether the step passed.
    #[must_use]
    pub fn steps_with_unknown_success(&self) -> Vec<u32> {
        self.steps
            .iter()
            .filter(|s| {
                s.commands
                    .iter()
                    .any(|c| c.success == SuccessSemantic::Unknown)
            })
            .map(|s| s.index)
            .collect()
    }

    /// Whether every step's verification is one this build can perform.
    ///
    /// `false` when any step carries a check this build does not recognise. An
    /// executor must refuse such a plan rather than run the steps it does
    /// understand: the checks it cannot read are exactly the ones a newer build
    /// added because the older set was insufficient (F-06 is the worked
    /// example). Reported separately from [`Self::check_ordering`] because it is
    /// a different failure — the plan is well-formed and this build is too old
    /// for it.
    #[must_use]
    pub fn verification_is_performable(&self) -> bool {
        self.steps
            .iter()
            .all(|s| s.verification.iter().all(VerificationCheck::is_satisfiable))
    }

    /// Checks the ordering invariants a plan must satisfy (INV-01: the
    /// destination is added and verified before the source is removed) and
    /// returns the first violation as text.
    pub fn check_ordering(&self) -> Result<(), String> {
        let pos = |k: StepKind| self.steps.iter().position(|s| s.kind == k);
        let count = |k: StepKind| self.steps.iter().filter(|s| s.kind == k).count();

        // This check compares *positions*, which is only the same thing as
        // "the guard for this removal" while a plan carries one package. A
        // `MigrationPlan` does today - it names a single `package` - and the
        // planner emits one of each mutating step. But `.position()` is the
        // first occurrence, so an interleaved plan like
        // `[Snapshot, Add(A), Verify(A), Remove(A), Remove(B), Add(B), Verify(B)]`
        // would pass this while removing B before B was ever added (F-06).
        //
        // A step does not carry the package it acts on, so the pairwise check
        // that would catch it cannot be written yet. What can be written is the
        // assumption this one silently rests on. A batch plan then fails here,
        // loudly, at INV-01's enforcement point - rather than passing quietly
        // and being discovered by whatever it removes.
        for kind in [
            StepKind::AddDestination,
            StepKind::VerifyDestination,
            StepKind::RemoveSource,
        ] {
            if count(kind) > 1 {
                // Short on purpose: this reaches a user through
                // `ErrorId::Internal`, outside the i18n catalogue. The reasoning
                // is above, where a maintainer reads it; a person who sees this
                // needs to know it is a defect in the build, which the error
                // identifier's own remediation text already says.
                return Err(format!(
                    "a plan carries {} {kind:?} steps and can only be ordered for one (INV-01)",
                    count(kind)
                ));
            }
        }
        if let Some(remove) = pos(StepKind::RemoveSource) {
            let add = pos(StepKind::AddDestination)
                .ok_or("RemoveSource without AddDestination (INV-01)")?;
            let verify = pos(StepKind::VerifyDestination)
                .ok_or("RemoveSource without VerifyDestination (INV-01)")?;
            if !(add < verify && verify < remove) {
                return Err("RemoveSource precedes destination add/verify (INV-01)".into());
            }
        }
        if self.has_mutating_steps() {
            match pos(StepKind::SnapshotBefore) {
                Some(0) => {}
                _ => return Err("a mutating plan must start with SnapshotBefore".into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    /// The module's own `plan` and `step`, over a list of kinds.
    fn plan_of(kinds: &[StepKind]) -> MigrationPlan {
        plan(
            kinds
                .iter()
                .enumerate()
                .map(|(i, k)| step(u32::try_from(i).unwrap() + 1, *k))
                .collect(),
        )
    }

    #[test]
    fn invariant_01_an_interleaved_plan_is_refused_rather_than_mis_ordered() {
        // `check_ordering` compares *positions*, which is only "the guard for
        // this removal" while a plan carries one package. A step does not name
        // the package it acts on, so the pairwise check cannot be written yet —
        // and the first-position comparison would pass a plan that removes B
        // before B was ever added (F-06).
        //
        // What can be written is the assumption the check silently rested on.
        let interleaved = plan_of(&[
            StepKind::SnapshotBefore,
            StepKind::AddDestination,
            StepKind::VerifyDestination,
            StepKind::RemoveSource,
            StepKind::RemoveSource,
            StepKind::AddDestination,
            StepKind::VerifyDestination,
        ]);
        let err = interleaved
            .check_ordering()
            .expect_err("a batch plan cannot be ordered by first position");
        assert!(err.contains("INV-01"), "{err}");

        // And the single-package plan this project emits still passes.
        let ordinary = plan_of(&[
            StepKind::SnapshotBefore,
            StepKind::AddDestination,
            StepKind::VerifyDestination,
            StepKind::SetPrimary,
            StepKind::RemoveSource,
            StepKind::VerifyRemoval,
        ]);
        ordinary
            .check_ordering()
            .expect("the ordinary plan is fine");

        // The original defect, unchanged: a removal before its guard.
        let inverted = plan_of(&[
            StepKind::SnapshotBefore,
            StepKind::RemoveSource,
            StepKind::AddDestination,
            StepKind::VerifyDestination,
        ]);
        assert!(inverted.check_ordering().is_err());
    }

    #[test]
    fn an_unrecognised_check_is_read_and_never_satisfied() {
        // The tolerance added to keep a new variant additive is also a way to
        // re-open the defect the variant was added to close: an executor that
        // skipped what it did not recognise, found the rest satisfied by state
        // predating the plan, and cleared INV-01's gate. An unknown is never a
        // yes (`.claude/rules/domain.md`).
        for known in [
            VerificationCheck::ProviderReportsActive,
            VerificationCheck::PackageAbsent,
            VerificationCheck::PackageNewlyPresent,
            VerificationCheck::NoCrossProviderLinkage,
        ] {
            assert!(known.is_satisfiable(), "{known:?}");
        }
        assert!(!VerificationCheck::Unknown.is_satisfiable());

        let mut plan = plan_of(&[StepKind::SnapshotBefore, StepKind::VerifyDestination]);
        plan.steps[1].verification = vec![
            VerificationCheck::ProviderReportsActive,
            VerificationCheck::Unknown,
        ];
        assert!(
            !plan.verification_is_performable(),
            "a plan this build cannot fully verify is not one it may execute"
        );
        plan.steps[1].verification = vec![VerificationCheck::ProviderReportsActive];
        assert!(plan.verification_is_performable());
    }

    #[test]
    fn a_verification_check_from_a_newer_build_is_read_rather_than_refused() {
        // Adding a variant must stay additive. Without the unknown-tolerant
        // variant the whole plan is rejected by an older build and no analysis
        // happens at all — a breaking change wearing an additive face, which
        // this project has already made once (SE-053).
        let json = r#"{"check":"SOMETHING_A_LATER_BUILD_ADDED"}"#;
        let check: VerificationCheck = serde_json::from_str(json).expect("tolerated");
        assert_eq!(check, VerificationCheck::Unknown);

        // And the ones that exist round-trip by name.
        for (value, text) in [
            (VerificationCheck::PackageAbsent, "PACKAGE_ABSENT"),
            (
                VerificationCheck::PackageNewlyPresent,
                "PACKAGE_NEWLY_PRESENT",
            ),
        ] {
            let encoded = serde_json::to_string(&value).unwrap();
            assert!(encoded.contains(text), "{encoded}");
            assert_eq!(
                serde_json::from_str::<VerificationCheck>(&encoded).unwrap(),
                value
            );
        }
    }

    use super::*;

    fn step(index: u32, kind: StepKind) -> MigrationStep {
        MigrationStep {
            index,
            kind,
            provider: None,
            commands: vec![],
            verification: vec![],
            reason: Reason::new("test"),
        }
    }

    /// A command carrying one elevation and nothing else that matters here.
    fn cmd(elevation: Elevation) -> PlanCommand {
        PlanCommand {
            target: CommandTarget::Provider,
            program: "/p/bin/x".into(),
            argv: vec!["read".into()],
            environment: BTreeMap::new(),
            elevation,
            placeholder: false,
            success: SuccessSemantic::ExitStatus,
        }
    }

    fn cmd_with_success(success: SuccessSemantic) -> PlanCommand {
        PlanCommand {
            success,
            ..cmd(Elevation::None)
        }
    }

    #[test]
    fn an_unknown_elevation_is_reported_separately_from_a_required_one() {
        // `steps_with_unknown_elevation` had **no test at all** - zero
        // references outside its one call site in the renderer - in a crate
        // carrying a 95% coverage floor (F-36, raised by `spec-guardian`).
        //
        // It became untested rather than started that way: the only reachable
        // `unknown` in the shipped data was MacPorts' `verify`, and observing
        // that it needs no elevation turned it into a `none` (F-24). No plan
        // built from today's data can produce an `Unknown`, so the renderer arm
        // and its locale key are now exercised by nothing. They are not
        // unreachable: a provider present in `registry.rs` with no row in
        // `provider-commands.toml` yields `Unknown` on every command, and
        // nothing asserts that cannot happen.
        //
        // Constructed rather than planned, deliberately. A test that had to
        // manufacture a fake provider entry to reach this would be asserting
        // the planner's behaviour; this asserts the domain's, which is where
        // the method lives and where the property is total.
        let mut required = step(1, StepKind::AddDestination);
        required.commands = vec![cmd(Elevation::Required {
            reason: "the prefix is not user-writable".into(),
        })];
        let mut unknown = step(2, StepKind::VerifyDestination);
        unknown.commands = vec![cmd(Elevation::Unknown)];
        let mut plain = step(3, StepKind::RecordHistory);
        plain.commands = vec![cmd(Elevation::None)];

        let p = plan(vec![required, unknown, plain]);

        assert_eq!(
            p.steps_with_unknown_elevation(),
            vec![2],
            "only the step whose command is Unknown belongs here"
        );
        assert_eq!(
            p.steps_requiring_elevation(),
            vec![1],
            "and it must not appear among the steps that definitely elevate"
        );

        // The disjointness the doc comment claims, asserted rather than
        // described: a step whose commands are *both* is required, not unknown,
        // because "we know this one elevates" outranks "we do not know about
        // that one" when deciding what to warn a user about.
        let mut both = step(1, StepKind::AddDestination);
        both.commands = vec![
            cmd(Elevation::Unknown),
            cmd(Elevation::Required { reason: "r".into() }),
        ];
        let p = plan(vec![both]);
        assert_eq!(p.steps_requiring_elevation(), vec![1]);
        assert!(
            p.steps_with_unknown_elevation().is_empty(),
            "the two lists are disjoint by construction"
        );
    }

    fn plan(steps: Vec<MigrationStep>) -> MigrationPlan {
        MigrationPlan {
            schema_version: PLAN_SCHEMA_VERSION,
            package: PackageIdentity::new(ProviderId::new("a"), "x"),
            source: ProviderId::new("a"),
            destination: ProviderId::new("b"),
            destination_package: Some("x".into()),
            retain_source: false,
            primary_after: ProviderId::new("b"),
            recommendation: Outcome::MigrateTo {
                provider: ProviderId::new("b"),
            },
            overrode: None,
            steps,
            risks: vec![],
            elevated_steps: vec![],
            policy_version: "0".into(),
            dry_run_only: true,
        }
    }

    #[test]
    fn destination_before_source_is_enforced() {
        let good = plan(vec![
            step(1, StepKind::SnapshotBefore),
            step(2, StepKind::AddDestination),
            step(3, StepKind::VerifyDestination),
            step(4, StepKind::RemoveSource),
        ]);
        assert!(good.check_ordering().is_ok());
        let bad = plan(vec![
            step(1, StepKind::SnapshotBefore),
            step(2, StepKind::RemoveSource),
            step(3, StepKind::AddDestination),
            step(4, StepKind::VerifyDestination),
        ]);
        assert!(bad.check_ordering().unwrap_err().contains("INV-01"));
        let no_snapshot = plan(vec![step(1, StepKind::AddDestination)]);
        assert!(no_snapshot.check_ordering().is_err());
    }

    #[test]
    fn a_command_that_does_not_say_how_it_succeeds_is_unknown_not_exit_status() {
        // The whole point of the type. A reader meeting a plan written before
        // this field existed must not fall back to exit status: on MacPorts
        // `verify` that is a check which cannot fail, and INV-01 puts it in
        // front of `REMOVE_SOURCE` (F-33).
        //
        // Asserted through serde rather than through `Default`, because the
        // hazard is a *document*, and `#[serde(default)]` on the field is the
        // line that makes it safe.
        let without: PlanCommand = serde_json::from_str(
            r#"{"target":{"target":"PROVIDER"},"program":"/p/bin/x","argv":["read"],
                "elevation":{"elevation":"NONE"},"placeholder":false}"#,
        )
        .expect("a document predating the field must still parse");
        assert_eq!(
            without.success,
            SuccessSemantic::Unknown,
            "absent must mean unknown, never exit status"
        );

        let unrecognised: PlanCommand = serde_json::from_str(
            r#"{"target":{"target":"PROVIDER"},"program":"/p/bin/x","argv":["read"],
                "elevation":{"elevation":"NONE"},"placeholder":false,
                "success":{"success":"STDOUT_MATCHES_A_REGEX_A_NEWER_BUILD_ADDED"}}"#,
        )
        .expect("an unrecognised semantic must parse, not fail");
        assert_eq!(
            unrecognised.success,
            SuccessSemantic::Unknown,
            "a semantic this build does not know is one it cannot perform"
        );
    }

    #[test]
    fn a_stated_success_semantic_round_trips() {
        let json = serde_json::to_string(&cmd_with_success(SuccessSemantic::StdoutNamesPackage))
            .expect("serialises");
        assert!(
            json.contains(r#""success":{"success":"STDOUT_NAMES_PACKAGE"}"#),
            "the wire shape is the tagged one the other plan enums use: {json}"
        );
        let back: PlanCommand = serde_json::from_str(&json).expect("parses");
        assert_eq!(back.success, SuccessSemantic::StdoutNamesPackage);
    }

    #[test]
    fn steps_whose_command_cannot_say_how_it_succeeds_are_reported() {
        let mut stated = step(1, StepKind::AddDestination);
        stated.commands = vec![cmd_with_success(SuccessSemantic::ExitStatus)];
        let mut silent = step(2, StepKind::VerifyDestination);
        silent.commands = vec![cmd_with_success(SuccessSemantic::Unknown)];
        let mut other = step(3, StepKind::VerifyRemoval);
        other.commands = vec![cmd_with_success(SuccessSemantic::StdoutNamesPackage)];

        let p = plan(vec![stated, silent, other]);

        assert_eq!(
            p.steps_with_unknown_success(),
            vec![2],
            "only the step whose command cannot say belongs here"
        );
    }

    #[test]
    fn a_step_with_no_commands_at_all_is_not_reported_as_unknown() {
        // `VERIFY_REMOVAL` carries no command today (`plan.v1.md` executor-debt
        // item 3, which this pass does not close). A step with nothing to run
        // has nothing whose success semantic could be missing, and reporting it
        // would make the accessor fire on every plan and so mean nothing.
        let bare = step(1, StepKind::VerifyRemoval);
        assert!(bare.commands.is_empty());
        assert!(plan(vec![bare]).steps_with_unknown_success().is_empty());
    }
}
