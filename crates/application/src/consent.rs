// SPDX-License-Identifier: Apache-2.0

//! Elevation consent, as decisions (pass 08 slice 4, D10; INV-06; SE-016).
//!
//! **Wired, unreachable** (work order 16 part E-3). The execution gate still
//! refuses every step whose elevation is not `NONE` (`ELEVATION_REQUIRED`,
//! `ELEVATION_UNKNOWN`), the guard halts every elevated command, and no
//! elevated command is built or run anywhere, so a cleared run has nothing to
//! ask about until part J. What lands here is everything a consent path
//! decides:
//!
//! - **Which operations ask.** [`elevation_requests`] lists every mutating
//!   command whose elevation, **re-derived from the provider command table and
//!   never read from the plan's field** (`plan.v1.md` obligation 9), is not
//!   `NONE` — `UNKNOWN` asks too (`04-migration-and-safety.md`, Privilege). A
//!   plan edited to say `NONE` does not skip the question. The program and argv
//!   a request shows are re-derived the same way, and a plan whose text
//!   differs from them is a refusal, never a request.
//! - **What consent is.** One [`ConsentGiven`] per operation, minted only by
//!   [`confirm`], which checks the typed phrase itself: consent cannot exist
//!   without the phrase. It is neither `Clone` nor `Copy`, and holds the
//!   operation it covers, so it is never cached and never inherited (INV-06).
//! - **A fresh code per question** (work order 16 part E, SR 10). The phrase
//!   carries a [`ConsentCode`] drawn for that question alone, so a line typed
//!   before the question was written cannot be its answer.
//! - **What `--yes` is.** Nothing here: no function takes it.
//! - **When a run asks** (work order 16 part E-3). [`consent_for`] decides it
//!   for a planned run: no question for a plan with nothing to ask about, none
//!   for a plan that disagrees with the table, and otherwise exactly one token
//!   per request or a halt. The **question** is put once per elevated
//!   operation, up front under the lock and before anything is written, so a
//!   refusal leaves nothing half-done; the **credential** is requested by the
//!   spawn that runs the operation, and a token covers that operation alone
//!   (`04-migration-and-safety.md`, Privilege; SE-111).
//!
//! The prompt itself — terminal checks, where it is written, how input is
//! read — is the command-line crate's `consent` module.

use crate::executor::PlannedRun;
use crate::planner::{elevation_for, package_kind_in, planned_argv};
use crate::provider_commands;
use macportsbrew_domain::{
    DomainError, Elevation, ErrorId, MigrationPlan, ProviderId, Snapshot, StepKind,
};
use std::path::Path;

/// One operation that would need administrator rights.
///
/// **Not persisted, and never in a document**: it carries the program and argv
/// as they would run, home included. A renderer shows it with every hostile
/// character escaped and the invoking user's home as `~`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElevationRequest {
    /// The step, as the plan numbers it.
    pub step: u32,
    /// The provider it changes.
    pub provider: ProviderId,
    /// The program that would run, re-derived from the detected prefix.
    pub program: String,
    /// Its argv, re-derived from the command table.
    pub argv: Vec<String>,
    /// Whether the table says it needs elevation (`true`) or nobody
    /// established whether it does (`false`).
    pub known: bool,
}

/// Why no consent was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentRefusal {
    /// Input or the prompt's stream is not a terminal: nobody could have seen
    /// the question, so nothing is read.
    NotATerminal,
    /// The person did not type the confirmation.
    Declined,
    /// There was nothing to ask about.
    NoRequests,
    /// The plan's command is not what the table and the detected prefix give
    /// for that step, so it is not shown as something to approve.
    PlanDisagrees,
    /// No fresh code could be drawn for the question, so it was not asked
    /// (SR 10): never a person's refusal.
    CodeUnavailable,
}

/// The code a confirmation phrase carries: three bytes, written as six
/// lowercase hexadecimal ASCII characters, drawn fresh for one question
/// (SR 10). Built only from bytes, so it is never empty or malformed.
#[derive(Debug, PartialEq, Eq)]
pub struct ConsentCode([u8; 3]);

impl ConsentCode {
    /// The code these three bytes write.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 3]) -> Self {
        Self(bytes)
    }
}

impl std::fmt::Display for ConsentCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c] = self.0;
        write!(f, "{a:02x}{b:02x}{c:02x}")
    }
}

/// What asking produced.
#[derive(Debug, PartialEq, Eq)]
pub enum ConsentDecision {
    /// Consent for each operation, one token apiece.
    Given(Vec<ConsentGiven>),
    /// No consent.
    Refused(ConsentRefusal),
}

/// Consent for exactly one operation. Minted only by [`confirm`]; neither
/// `Clone` nor `Copy`, so it is spent once.
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub struct ConsentGiven {
    step: u32,
    program: String,
    argv: Vec<String>,
}

impl ConsentGiven {
    /// The step it covers.
    pub fn step(&self) -> u32 {
        self.step
    }

    /// Whether it covers running `program` with `argv` for `step` — equality
    /// on the values themselves, no hash.
    pub fn covers(&self, step: u32, program: &str, argv: &[String]) -> bool {
        self.step == step && self.program == program && self.argv == argv
    }
}

/// The phrase a person types to approve `request`: ASCII, never translated,
/// naming the step, so an approval cannot be pasted onto another operation,
/// and carrying `code`, drawn for this question, so no line typed before it
/// was shown can be its answer (SR 10).
#[must_use]
pub fn confirmation_phrase(request: &ElevationRequest, code: &ConsentCode) -> String {
    format!("allow step {} {code}", request.step)
}

/// Consent for `request` when `typed` is exactly its phrase with `code`, after removing a
/// single trailing `\n` or `\r\n` and nothing else. The only way a
/// [`ConsentGiven`] is made; a word scan holds its callers to the command-line
/// crate's consent module.
#[must_use]
pub fn confirm(
    request: &ElevationRequest,
    code: &ConsentCode,
    typed: &str,
) -> Option<ConsentGiven> {
    let typed = typed
        .strip_suffix("\r\n")
        .or_else(|| typed.strip_suffix('\n'))
        .unwrap_or(typed);
    (typed == confirmation_phrase(request, code)).then(|| ConsentGiven {
        step: request.step,
        program: request.program.clone(),
        argv: request.argv.clone(),
    })
}

/// Every mutating operation of `plan` that would need elevation, re-derived
/// from the command table and the prefix `snapshot` detected (the snapshot the
/// plan was built from).
///
/// # Errors
/// [`ConsentRefusal::PlanDisagrees`] when a mutating command's program or
/// argv is not what the table and the prefix give for its step.
pub fn elevation_requests(
    plan: &MigrationPlan,
    snapshot: &Snapshot,
) -> Result<Vec<ElevationRequest>, ConsentRefusal> {
    let table = provider_commands::load().unwrap_or_default();
    let kind = package_kind_in(plan, snapshot);
    let mut requests = Vec::new();
    for step in &plan.steps {
        let (side, operation, name) = match step.kind {
            StepKind::AddDestination => (
                &plan.destination,
                "add",
                plan.destination_package.as_deref(),
            ),
            StepKind::RemoveSource => (&plan.source, "remove", Some(plan.package.name.as_str())),
            _ => continue,
        };
        if step.commands.is_empty() {
            continue;
        }
        let entry = table.get(side.as_str());
        let elevation = elevation_for(entry, operation, kind, None);
        if elevation == Elevation::None {
            continue;
        }
        let prefix = snapshot
            .providers
            .get(side)
            .and_then(|p| p.detection.prefix.as_deref());
        let (Some(entry), Some(prefix), Some(name)) = (entry, prefix, name) else {
            return Err(ConsentRefusal::PlanDisagrees);
        };
        let program = Path::new(prefix)
            .join(&entry.executable)
            .to_string_lossy()
            .into_owned();
        let argv = planned_argv(Some(entry), operation, name);
        for command in &step.commands {
            if command.program != program || command.argv != argv {
                return Err(ConsentRefusal::PlanDisagrees);
            }
        }
        requests.push(ElevationRequest {
            step: step.index,
            provider: side.clone(),
            program,
            argv,
            known: matches!(elevation, Elevation::Required { .. }),
        });
    }
    Ok(requests)
}

/// The consent a run was given, one token per elevated operation, keyed by
/// step. Neither `Clone` nor `Default`: it is made only by [`consent_for`],
/// and each token is taken once.
#[derive(Debug)]
#[must_use]
pub struct ConsentTokens {
    tokens: Vec<ConsentGiven>,
    asked: bool,
}

impl ConsentTokens {
    /// Whether a question was put to a person to obtain these tokens. `false`
    /// for a run with nothing to ask about, which holds no token.
    #[must_use]
    pub fn asked(&self) -> bool {
        self.asked
    }

    /// The token for `step`, taken out so it is spent once; `None` when there
    /// is none for that step, or it was already taken.
    pub fn take(&mut self, step: u32) -> Option<ConsentGiven> {
        let at = self.tokens.iter().position(|t| t.step == step)?;
        Some(self.tokens.remove(at))
    }
}

/// Why a run stops at its consent question, before anything is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentHalt {
    /// The person did not give the confirmation.
    Declined,
    /// Nobody could have seen the question: a stream was not a terminal.
    Unseen,
    /// No fresh code could be drawn, so nothing was asked (SR 10).
    CodeUnavailable,
    /// The plan's commands are not what the table and the detected prefix
    /// give, so nothing was put to a person as something to approve.
    PlanDisagrees,
    /// The prompt answered in a shape it never gives: tokens that are not one
    /// per request, each covering its request, or a refusal that contradicts
    /// the question it was asked (R5). A defect, never a person's refusal.
    Malformed,
}

impl ConsentHalt {
    /// The error the run ends with: a person's refusal is `USER_ABORTED`
    /// (exit 6); a question nobody could see, a missing code and a plan that
    /// disagrees are safety stops (exit 4); a malformed answer is `INTERNAL`
    /// (exit 1) — so a script tells them apart (R2, R5).
    #[must_use]
    pub fn error(self) -> DomainError {
        match self {
            Self::Declined => DomainError::new(
                ErrorId::UserAborted,
                "elevation was not allowed, so nothing was changed",
            ),
            Self::Unseen => DomainError::new(
                ErrorId::SafetyBlocked,
                "elevation needs a terminal to ask at; nothing was changed",
            ),
            Self::CodeUnavailable => DomainError::new(
                ErrorId::SafetyBlocked,
                "no confirmation code could be drawn, so elevation was not asked about; nothing \
                 was changed",
            ),
            Self::PlanDisagrees => DomainError::new(
                ErrorId::SafetyBlocked,
                "a command the plan would elevate is not what the provider table gives for it; \
                 nothing was changed",
            ),
            Self::Malformed => DomainError::new(
                ErrorId::Internal,
                "the consent prompt answered in a shape it never gives; nothing was changed",
            ),
        }
    }
}

/// The consent `run` needs, decided here and asked through `ask` (work order
/// 16 part E-3, SE-092): `ask` is called **only** when the plan, re-derived
/// against the snapshot it was built from, has an operation to ask about, and
/// never for a plan that disagrees with the table. The answer must hold
/// exactly one token per request, each covering that request's step, program
/// and argv, in order; anything else, like a refusal the prompt never gives
/// for a non-empty list, is [`ConsentHalt::Malformed`] (R5).
///
/// # Errors
/// The [`ConsentHalt`] the run stops with.
pub fn consent_for(
    run: &PlannedRun,
    ask: impl FnOnce(&[ElevationRequest]) -> ConsentDecision,
) -> Result<ConsentTokens, ConsentHalt> {
    let requests =
        elevation_requests(run.plan(), run.before()).map_err(|_| ConsentHalt::PlanDisagrees)?;
    if requests.is_empty() {
        return Ok(ConsentTokens {
            tokens: Vec::new(),
            asked: false,
        });
    }
    match ask(&requests) {
        ConsentDecision::Given(tokens) => {
            let one_for_one = tokens.len() == requests.len()
                && tokens
                    .iter()
                    .zip(&requests)
                    .all(|(t, r)| t.covers(r.step, &r.program, &r.argv));
            if one_for_one {
                Ok(ConsentTokens {
                    tokens,
                    asked: true,
                })
            } else {
                Err(ConsentHalt::Malformed)
            }
        }
        ConsentDecision::Refused(ConsentRefusal::Declined) => Err(ConsentHalt::Declined),
        ConsentDecision::Refused(ConsentRefusal::NotATerminal) => Err(ConsentHalt::Unseen),
        ConsentDecision::Refused(ConsentRefusal::CodeUnavailable) => {
            Err(ConsentHalt::CodeUnavailable)
        }
        ConsentDecision::Refused(ConsentRefusal::NoRequests | ConsentRefusal::PlanDisagrees) => {
            Err(ConsentHalt::Malformed)
        }
    }
}
