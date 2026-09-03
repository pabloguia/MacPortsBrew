// SPDX-License-Identifier: Apache-2.0

//! Migration plans as data (`04-migration-and-safety.md`, "The plan is the
//! contract"). A plan is serialisable, diffable and reviewable. Nothing in
//! this crate can execute one: the executor is a later pass and the
//! foundation pass ships no mutating code path (SE-020).
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
    pub fn is_mutating(self) -> bool {
        matches!(
            self,
            StepKind::AddDestination
                | StepKind::SetPrimary
                | StepKind::RemoveSource
                | StepKind::CleanupReview
        )
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
    /// `true` in this pass: the plan can be shown but never executed.
    pub dry_run_only: bool,
}

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

    /// Checks the ordering invariants a plan must satisfy (INV-01: the
    /// destination is added and verified before the source is removed) and
    /// returns the first violation as text.
    pub fn check_ordering(&self) -> Result<(), String> {
        let pos = |k: StepKind| self.steps.iter().position(|s| s.kind == k);
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
}
