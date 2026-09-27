// SPDX-License-Identifier: Apache-2.0

//! How a verification step's read is judged (`plan.v1.md`, "What an executor
//! inherits", obligations 3 and 4). Pure: a plan, a step and a command go in,
//! an [`Expectation`] comes out; an expectation and what the process produced
//! go in, a [`Verdict`] comes out. Nothing here runs anything.
//!
//! Mix-ups are made unwritable rather than tested for. The name a check reads
//! is a [`DestinationName`] or a [`SourceName`], and only
//! [`MigrationPlan::destination_name`] and [`MigrationPlan::source_name`] can
//! make one, so a destination check built from the source name does not
//! compile. An [`Expectation`] has no constructor but [`expectation_for`], so
//! none can be written by hand, and that function takes the step and the
//! command as positions in the plan it borrows, so neither can be written by
//! hand either. The provider a check talks to is bound there too: a
//! `VERIFY_DESTINATION` addressed to the source provider is refused, because a
//! package held by both would otherwise be "verified" by the provider it is
//! leaving (INV-08).
//!
//! **Every rule fails closed.** A process that did not finish, and an output
//! that was truncated, certify nothing whatever they contain: a stream cut
//! mid-line can manufacture a first token — `ghidra 1.0` cut after two bytes
//! reads `gh`.

use crate::plan::{CommandTarget, MigrationPlan, StepKind, SuccessSemantic, VerificationCheck};
use crate::snapshot::is_safe_provider_name;
use serde::{Deserialize, Serialize};

/// The destination package's name, as the destination provider spells it.
///
/// No public constructor: [`MigrationPlan::destination_name`] is the only way
/// to obtain one, so it cannot be built from the source name by mistake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestinationName<'p>(&'p str);

impl<'p> DestinationName<'p> {
    /// The name text.
    pub fn as_str(&self) -> &'p str {
        self.0
    }
}

/// The source package's name, as the source provider spells it.
///
/// No public constructor: [`MigrationPlan::source_name`] is the only way to
/// obtain one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceName<'p>(&'p str);

impl<'p> SourceName<'p> {
    /// The name text.
    pub fn as_str(&self) -> &'p str {
        self.0
    }
}

impl MigrationPlan {
    /// The destination package's name, when the plan selected one.
    ///
    /// `None` for a plan without a mapping, and for an empty name — which a
    /// destination check refuses ([`ExpectationError::NoDestinationName`])
    /// rather than defaulting to the source name, which is a different
    /// provider's spelling (`python@3.13` against `python313`).
    pub fn destination_name(&self) -> Option<DestinationName<'_>> {
        self.destination_package
            .as_deref()
            .filter(|n| !n.is_empty())
            .map(DestinationName)
    }

    /// The source package's name.
    pub fn source_name(&self) -> SourceName<'_> {
        SourceName(&self.package.name)
    }
}

/// What a verification command's read must show for its step to pass.
///
/// **Opaque, and [`expectation_for`] is its only constructor.** Every binding
/// that function makes — step kind to semantic, provider to side, name to
/// argv, and the name's own safety — would be void if a caller could write the
/// result by hand. So the variants are private and this type is read through
/// accessors only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expectation<'p>(Kind<'p>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind<'p> {
    /// `VERIFY_DESTINATION` over [`SuccessSemantic::ExitStatus`]: exit 0, from
    /// a command whose last argv element is this package.
    ExitZero(DestinationName<'p>),
    /// `VERIFY_DESTINATION` over [`SuccessSemantic::StdoutNamesPackage`]: the
    /// name is the first token of some line. The exit status is not consulted.
    Names(DestinationName<'p>),
    /// `VERIFY_REMOVAL` over [`SuccessSemantic::StdoutOmitsPackage`]: a clean,
    /// complete, non-empty inventory in which the name is no line's first
    /// token.
    Omits(SourceName<'p>),
}

impl<'p> Expectation<'p> {
    /// The semantic this expectation judges by.
    pub fn semantic(&self) -> SuccessSemantic {
        match self.0 {
            Kind::ExitZero(_) => SuccessSemantic::ExitStatus,
            Kind::Names(_) => SuccessSemantic::StdoutNamesPackage,
            Kind::Omits(_) => SuccessSemantic::StdoutOmitsPackage,
        }
    }

    /// The destination name, for a destination check.
    pub fn destination(&self) -> Option<DestinationName<'p>> {
        match self.0 {
            Kind::ExitZero(name) | Kind::Names(name) => Some(name),
            Kind::Omits(_) => None,
        }
    }

    /// The source name, for a removal check.
    pub fn source(&self) -> Option<SourceName<'p>> {
        match self.0 {
            Kind::ExitZero(_) | Kind::Names(_) => None,
            Kind::Omits(name) => Some(name),
        }
    }
}

/// Why a command cannot be judged at all. Each is a refusal, never a pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpectationError {
    /// No step at that position in the plan.
    StepNotInPlan,
    /// No command at that position in the step.
    CommandNotInStep,
    /// The step is neither `VERIFY_DESTINATION` nor `VERIFY_REMOVAL`.
    NotAVerificationStep,
    /// The command is an illustration, not something that can be run
    /// ([`crate::plan::PlanCommand::placeholder`]).
    PlaceholderCommand,
    /// The command invokes something other than a provider. A verification
    /// read asks a provider about its own inventory; nothing else can answer.
    NotAProviderCommand,
    /// The step carries a verification check this build does not recognise,
    /// which can never count as satisfied
    /// ([`crate::plan::VerificationCheck::Unknown`]).
    UnrecognisedCheck,
    /// The command does not say how its success is established.
    SuccessUnknown,
    /// The step talks to a provider other than the one its kind requires: a
    /// destination check to anything but the destination, a removal check to
    /// anything but the source, or either to no provider at all.
    WrongProvider,
    /// The semantic cannot certify this kind of step (a removal certified by
    /// exit status or by presence, a destination by absence).
    SemanticNotValidForStep,
    /// A destination check on a plan that selected no destination name.
    NoDestinationName,
    /// The name the check reads fails [`is_safe_provider_name`]. An empty or
    /// whitespace name is the dangerous case: it is no line's first token, so
    /// a removal check reading it would pass on any inventory at all.
    UnsafePackageName,
    /// An exit-status destination check whose last argv element is not the
    /// destination package: its exit status answers a different question.
    ArgvDoesNotNameDestination,
    /// A removal check whose argv names either of **this plan's** packages —
    /// the source's spelling or the destination's. The inventory form is the
    /// only one whose silence means anything (obligation 3). That no name at
    /// all is appended to this verb is the loader's refusal, over the data;
    /// this one is over the plan in hand.
    ArgvNamesAPackage,
}

/// Derives what command `command` of step `step` of `plan` must show, both
/// positions counted from 0 in [`MigrationPlan::steps`] and
/// [`crate::plan::MigrationStep::commands`].
///
/// **Positions, not references, so that the step and the command cannot come
/// from anywhere but this plan.** A signature taking a `&MigrationStep` let a
/// caller write a step by hand — its fields are public — of the right kind and
/// provider with no `verification`, which cleared the unrecognised-check
/// refusal and made "the command is the step's own" a check against the
/// caller's own step (spec-guardian). An index out of range is refused
/// ([`ExpectationError::StepNotInPlan`], [`ExpectationError::CommandNotInStep`]).
///
/// The only constructor of [`Expectation`], and where every binding the plan's
/// data does not make by itself is made: the step kind to a semantic, the step's provider to the side of the migration its
/// kind reads, the name to the argv, and the name to
/// [`is_safe_provider_name`]. It also refuses what can never be run or never
/// be satisfied: a placeholder, a command that is not a provider's, a step
/// carrying an unrecognised check, and an unknown semantic.
pub fn expectation_for(
    plan: &MigrationPlan,
    step: usize,
    command: usize,
) -> Result<Expectation<'_>, ExpectationError> {
    let step = plan
        .steps
        .get(step)
        .ok_or(ExpectationError::StepNotInPlan)?;
    let cmd = step
        .commands
        .get(command)
        .ok_or(ExpectationError::CommandNotInStep)?;
    let side = match step.kind {
        StepKind::VerifyDestination => &plan.destination,
        StepKind::VerifyRemoval => &plan.source,
        StepKind::SnapshotBefore
        | StepKind::AddDestination
        | StepKind::SetPrimary
        | StepKind::VerifyResolution
        | StepKind::RemoveSource
        | StepKind::CleanupReview
        | StepKind::RecordHistory
        | StepKind::ManualReview => return Err(ExpectationError::NotAVerificationStep),
    };
    if cmd.placeholder {
        return Err(ExpectationError::PlaceholderCommand);
    }
    match cmd.target {
        CommandTarget::Provider => {}
        CommandTarget::Tool => return Err(ExpectationError::NotAProviderCommand),
    }
    if !step
        .verification
        .iter()
        .all(VerificationCheck::is_satisfiable)
    {
        return Err(ExpectationError::UnrecognisedCheck);
    }
    if cmd.success == SuccessSemantic::Unknown {
        return Err(ExpectationError::SuccessUnknown);
    }
    if step.provider.as_ref() != Some(side) {
        return Err(ExpectationError::WrongProvider);
    }
    match (step.kind, cmd.success) {
        (StepKind::VerifyDestination, SuccessSemantic::ExitStatus) => {
            let name = safe_destination(plan)?;
            if cmd.argv.last().map(String::as_str) != Some(name.as_str()) {
                return Err(ExpectationError::ArgvDoesNotNameDestination);
            }
            Ok(Expectation(Kind::ExitZero(name)))
        }
        (StepKind::VerifyDestination, SuccessSemantic::StdoutNamesPackage) => {
            Ok(Expectation(Kind::Names(safe_destination(plan)?)))
        }
        (StepKind::VerifyRemoval, SuccessSemantic::StdoutOmitsPackage) => {
            let source = plan.source_name();
            if !is_safe_provider_name(source.as_str()) {
                return Err(ExpectationError::UnsafePackageName);
            }
            let destination = plan.destination_name();
            let names_one = cmd
                .argv
                .iter()
                .any(|a| a == source.as_str() || destination.is_some_and(|d| a == d.as_str()));
            if names_one {
                return Err(ExpectationError::ArgvNamesAPackage);
            }
            Ok(Expectation(Kind::Omits(source)))
        }
        _ => Err(ExpectationError::SemanticNotValidForStep),
    }
}

/// The destination name, present and safe.
fn safe_destination(plan: &MigrationPlan) -> Result<DestinationName<'_>, ExpectationError> {
    let name = plan
        .destination_name()
        .ok_or(ExpectationError::NoDestinationName)?;
    if !is_safe_provider_name(name.as_str()) {
        return Err(ExpectationError::UnsafePackageName);
    }
    Ok(name)
}

/// How the process ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessEnd {
    /// It exited with this status.
    Exited {
        /// The exit status.
        code: i32,
    },
    /// It was killed, timed out, or never reported a status.
    DidNotFinish,
}

/// What a verification command produced, as the executor read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadOutput<'a> {
    /// How the process ended.
    pub end: ProcessEnd,
    /// Its standard output, up to the read cap.
    pub stdout: &'a str,
    /// Whether the cap was reached, so that `stdout` is a prefix.
    pub truncated: bool,
}

/// The judgement on one read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The read shows what the expectation requires.
    Passed,
    /// It does not, for this reason.
    Failed(FailureCause),
}

/// Why a read certifies nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailureCause {
    /// The process did not finish.
    DidNotFinish,
    /// The output was cut at the read cap.
    Truncated,
    /// The process exited nonzero where exit 0 is required.
    NonZeroExit,
    /// The output carried no line with a token.
    EmptyOutput,
    /// The name a destination check requires is no line's first token.
    NameAbsent,
    /// The name a removal check requires gone is still a line's first token.
    NamePresent,
}

/// Judges `output` against `expectation`.
///
/// Every semantic fails on [`ProcessEnd::DidNotFinish`] and on truncation
/// first — exit status too, which reads no stdout but cannot vouch for a read
/// that was cut or never ended. Name-in-stdout ignores the exit status;
/// absence demands exit 0, a complete output and a non-empty one, each checked
/// on its own, before it asks about the name at all — it is the one semantic
/// satisfied by saying nothing.
pub fn evaluate(expectation: &Expectation<'_>, output: &ReadOutput<'_>) -> Verdict {
    let code = match output.end {
        ProcessEnd::DidNotFinish => return Verdict::Failed(FailureCause::DidNotFinish),
        ProcessEnd::Exited { code } => code,
    };
    if output.truncated {
        return Verdict::Failed(FailureCause::Truncated);
    }
    match expectation.0 {
        Kind::ExitZero(_) => {
            if code != 0 {
                return Verdict::Failed(FailureCause::NonZeroExit);
            }
            Verdict::Passed
        }
        Kind::Names(name) => {
            if token_lines(output.stdout) == 0 {
                return Verdict::Failed(FailureCause::EmptyOutput);
            }
            if !stdout_names(output.stdout, name.as_str()) {
                return Verdict::Failed(FailureCause::NameAbsent);
            }
            Verdict::Passed
        }
        Kind::Omits(name) => {
            if code != 0 {
                return Verdict::Failed(FailureCause::NonZeroExit);
            }
            // Condition 2, completeness, is the truncation branch above.
            if token_lines(output.stdout) == 0 {
                return Verdict::Failed(FailureCause::EmptyOutput);
            }
            if stdout_names(output.stdout, name.as_str()) {
                return Verdict::Failed(FailureCause::NamePresent);
            }
            Verdict::Passed
        }
    }
}

/// Whether `name` is the first whitespace-separated token of some line of
/// `stdout`.
///
/// Leading whitespace is skipped, because one provider indents its inventory.
/// The comparison is literal — never a substring, prefix, glob or pattern:
/// `gh` is not named by a line for `ghidra`, and `.` and `+` in a name are
/// characters, not operators (`SuccessSemantic::StdoutNamesPackage`).
pub fn stdout_names(stdout: &str, name: &str) -> bool {
    !name.is_empty()
        && stdout
            .lines()
            .any(|line| line.split_whitespace().next() == Some(name))
}

/// The number of lines carrying at least one token.
fn token_lines(stdout: &str) -> usize {
    stdout
        .lines()
        .filter(|line| line.split_whitespace().next().is_some())
        .count()
}

/// What may be recorded about a read: the result, the line count and the
/// truncation flag.
///
/// **There is no field for the output, by construction.** A removal check's
/// stdout is the machine's complete installed inventory, a strong fingerprint
/// (obligation 3), and a type that cannot hold it cannot disclose it. The fields
/// are private and [`ReadEvidence::of`] is the only constructor, so `passed`
/// cannot disagree with `cause`, nor the counts with the read they describe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadEvidence {
    passed: bool,
    line_count: usize,
    truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<FailureCause>,
}

impl ReadEvidence {
    /// The recordable part of a judged read.
    pub fn of(output: &ReadOutput<'_>, verdict: Verdict) -> Self {
        let cause = match verdict {
            Verdict::Passed => None,
            Verdict::Failed(cause) => Some(cause),
        };
        Self {
            passed: cause.is_none(),
            line_count: output.stdout.lines().count(),
            truncated: output.truncated,
            cause,
        }
    }

    /// Whether the read passed.
    pub fn passed(&self) -> bool {
        self.passed
    }

    /// Lines of stdout read, up to the cap.
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Whether the read was cut at the cap.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Why it failed, when it did.
    pub fn cause(&self) -> Option<FailureCause> {
        self.cause
    }
}
