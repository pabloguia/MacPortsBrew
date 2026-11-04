// SPDX-License-Identifier: Apache-2.0

//! One judge per destination check, over facts (work order 16 part H).
//!
//! Pure, and no judge sees a provider id, so one rule answers both directions
//! (INV-17): what differs between providers is which facts an adapter can offer,
//! and that is stated in the facts. "Could not tell" is never `Fails` and never
//! `Holds`, and an observed failure outranks anything that could not be told.

use crate::executor::PlannedRun;
use macportsbrew_domain::{
    CheckAnswer, CheckBasis, CheckFailure, DependencyRecord, DestinationFacts, InstallationRecord,
    IntegrityBasis, IntegrityFacts, LinkageFacts, MigrationPlan, OwnershipFacts, Snapshot,
    StepKind, UncheckedCause, VerificationCheck,
};

/// What the plan and its paired snapshot expect of an owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerExpectation<'a> {
    /// The destination package.
    pub package: &'a str,
    /// The step carries `PACKAGE_NEWLY_PRESENT`: the snapshot held none of it.
    pub newly_present: bool,
    /// The version the snapshot held active, when it held one.
    pub held_version: Option<&'a str>,
}

/// `EXECUTABLES_OWNED_BY_DESTINATION` (R-26). "The active version `v` owns this
/// path", pinned as far as a snapshot can: **an inference, stated as one**, since
/// no provider ties a version to a run (SE-107 L2).
#[must_use]
pub fn judge_ownership(expected: &OwnerExpectation<'_>, facts: &OwnershipFacts) -> CheckAnswer {
    match facts {
        OwnershipFacts::CouldNotTell(cause) => CheckAnswer::Unchecked(*cause),
        OwnershipFacts::NoEntry => CheckAnswer::Fails(CheckFailure::NoPlacedEntry),
        OwnershipFacts::NotOwned => CheckAnswer::Fails(CheckFailure::AnotherPackagesEntry),
        OwnershipFacts::OwnerLeavesItsDirectory { owner } => {
            if owner == expected.package {
                CheckAnswer::Unchecked(UncheckedCause::OwnerLeavesKeg)
            } else {
                CheckAnswer::Fails(CheckFailure::AnotherPackagesEntry)
            }
        }
        OwnershipFacts::Owner {
            name,
            version,
            active,
            versions_present,
        } => {
            if name != expected.package {
                return CheckAnswer::Fails(CheckFailure::AnotherPackagesEntry);
            }
            if active.as_deref() != Some(version.as_str()) {
                return CheckAnswer::Fails(CheckFailure::ActiveRecordDiffers);
            }
            if expected.newly_present && *versions_present != 1 {
                return CheckAnswer::Fails(CheckFailure::MoreThanOneVersion);
            }
            if !expected.newly_present && expected.held_version.is_some_and(|held| held != version)
            {
                return CheckAnswer::Fails(CheckFailure::OwnerVersionDiffers);
            }
            CheckAnswer::Holds
        }
    }
}

/// `PROVIDER_INTEGRITY_CHECK` (R-25). Never asserts content integrity, provenance,
/// or that the package runs.
#[must_use]
pub fn judge_integrity(facts: &IntegrityFacts) -> CheckAnswer {
    if let Some(cause) = facts.list_unusable {
        return CheckAnswer::Unchecked(cause);
    }
    if facts.missing > 0 {
        return CheckAnswer::Fails(CheckFailure::PathMissing);
    }
    if facts.dangling > 0 {
        return CheckAnswer::Fails(CheckFailure::PathDangling);
    }
    if facts.dependencies == DependencyRecord::Missing {
        return CheckAnswer::Fails(CheckFailure::DependencyMissing);
    }
    if facts.record == InstallationRecord::Absent {
        return CheckAnswer::Fails(CheckFailure::NoInstallationRecord);
    }
    if facts.stat_failed > 0 {
        return CheckAnswer::Unchecked(UncheckedCause::StatFailed);
    }
    if facts.outside_prefix > 0 {
        return CheckAnswer::Unchecked(UncheckedCause::ListsOutsidePrefix);
    }
    if facts.dependencies == DependencyRecord::NotTold {
        return CheckAnswer::Unchecked(UncheckedCause::DependenciesNotTold);
    }
    CheckAnswer::Holds
}

/// `NO_CROSS_PROVIDER_LINKAGE` (R-27): "no reference to another provider's prefix
/// present at verification", evidence about INV-07 and not the invariant. A link
/// found fails whether or not this run caused it, because a source is not removed
/// under it either; the name says which it was (R-27 finding 9). No check here
/// reads one provider's artifact as satisfying the other's (INV-08).
#[must_use]
pub fn judge_linkage(held_before: bool, facts: &LinkageFacts) -> CheckAnswer {
    if facts.links_detected_provider {
        return CheckAnswer::Fails(if held_before {
            CheckFailure::LinkPresentBeforeTheRun
        } else {
            CheckFailure::LinksAnotherProvider
        });
    }
    if let Some(cause) = facts.list_unusable {
        return CheckAnswer::Unchecked(cause);
    }
    if facts.malformed_file {
        return CheckAnswer::Unchecked(UncheckedCause::MalformedFile);
    }
    if facts.unreadable_file {
        return CheckAnswer::Unchecked(UncheckedCause::UnreadableFile);
    }
    if facts.walk_over_bound {
        return CheckAnswer::Unchecked(UncheckedCause::WalkOverBound);
    }
    if facts.under_undetected_prefix {
        return CheckAnswer::Unchecked(UncheckedCause::UnderUndetectedPrefix);
    }
    if facts.rpath_unresolved {
        return CheckAnswer::Unchecked(UncheckedCause::RpathUnresolved);
    }
    if facts.env_shebang_on_list {
        return CheckAnswer::Unchecked(UncheckedCause::InterpreterResolvedAtRunTime);
    }
    CheckAnswer::Holds
}

/// The neutral basis a `HOLDS` on these facts rests on, or `None` for facts with
/// no such statement to make.
#[must_use]
pub fn basis_of(facts: &DestinationFacts) -> Option<CheckBasis> {
    match facts {
        DestinationFacts::Integrity(facts) => Some(match facts.basis {
            IntegrityBasis::RecordedFileList => CheckBasis::RecordedFileList,
            IntegrityBasis::GeneratedFileList => CheckBasis::GeneratedFileList,
        }),
        DestinationFacts::Linkage(facts) if facts.loadable_files == 0 => {
            Some(CheckBasis::NoLoadCommands)
        }
        DestinationFacts::Linkage(_) => Some(CheckBasis::ReferencesExamined),
        DestinationFacts::Ownership(_) | DestinationFacts::NotObserved => None,
    }
}

/// The answer `facts` gives for the `VERIFY_DESTINATION` of `run`: the plan and
/// its paired snapshot supply what a judge expects. Facts of nobody's check, and
/// no facts, are `Unchecked(NOT_OBSERVED)`, never a yes.
#[must_use]
pub fn answer_from_facts(run: &PlannedRun, facts: &DestinationFacts) -> CheckAnswer {
    let plan = run.plan();
    match facts {
        DestinationFacts::NotObserved => CheckAnswer::Unchecked(UncheckedCause::NotObserved),
        DestinationFacts::Integrity(facts) => judge_integrity(facts),
        DestinationFacts::Linkage(facts) => {
            judge_linkage(held_version(plan, run.before()).is_some(), facts)
        }
        DestinationFacts::Ownership(facts) => {
            let Some(package) = plan.destination_package.as_deref() else {
                return CheckAnswer::Unchecked(UncheckedCause::NotObserved);
            };
            let held = held_version(plan, run.before());
            judge_ownership(
                &OwnerExpectation {
                    package,
                    newly_present: newly_present(plan),
                    held_version: held.as_deref(),
                },
                facts,
            )
        }
    }
}

fn newly_present(plan: &MigrationPlan) -> bool {
    plan.steps
        .iter()
        .filter(|step| step.kind == StepKind::VerifyDestination)
        .any(|step| {
            step.verification
                .iter()
                .any(|check| matches!(check, VerificationCheck::PackageNewlyPresent))
        })
}

/// The version the snapshot held active for the destination package, in the
/// shape an owner answer has: a version with its revision. One provider's version
/// already carries it and the other's does not, so it is added only if absent; no
/// provider is asked which it is. A linked keg older than the formula's revision
/// reads as different, which fails in the safe direction.
fn held_version(plan: &MigrationPlan, before: &Snapshot) -> Option<String> {
    let name = plan.destination_package.as_deref()?;
    let provider = before.providers.get(&plan.destination)?;
    let row = provider
        .inventory
        .installed
        .iter()
        .find(|row| row.identity.name == name && row.active)?;
    let version = row.version.to_string();
    Some(match &row.revision {
        Some(revision) if !version.ends_with(&format!("_{revision}")) => {
            format!("{version}_{revision}")
        }
        _ => version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(name: &str, version: &str, active: Option<&str>, present: usize) -> OwnershipFacts {
        OwnershipFacts::Owner {
            name: name.to_owned(),
            version: version.to_owned(),
            active: active.map(str::to_owned),
            versions_present: present,
        }
    }

    const FRESH: OwnerExpectation<'_> = OwnerExpectation {
        package: "xz",
        newly_present: true,
        held_version: None,
    };

    const HELD: OwnerExpectation<'_> = OwnerExpectation {
        package: "xz",
        newly_present: false,
        held_version: Some("5.8.3_0"),
    };

    #[test]
    fn invariant_17_the_ownership_judge_answers_each_case_from_facts_alone() {
        use CheckAnswer::{Fails, Holds, Unchecked};
        let holds = owner("xz", "5.8.3_0", Some("5.8.3_0"), 1);
        assert_eq!(judge_ownership(&FRESH, &holds), Holds);
        assert_eq!(judge_ownership(&HELD, &holds), Holds);
        assert_eq!(
            judge_ownership(&FRESH, &OwnershipFacts::NoEntry),
            Fails(CheckFailure::NoPlacedEntry)
        );
        assert_eq!(
            judge_ownership(&FRESH, &OwnershipFacts::NotOwned),
            Fails(CheckFailure::AnotherPackagesEntry)
        );
        assert_eq!(
            judge_ownership(&FRESH, &owner("openssl@3", "3", Some("3"), 1)),
            Fails(CheckFailure::AnotherPackagesEntry)
        );
        assert_eq!(
            judge_ownership(&FRESH, &owner("xz", "5", Some("6"), 1)),
            Fails(CheckFailure::ActiveRecordDiffers)
        );
        assert_eq!(
            judge_ownership(&FRESH, &owner("xz", "5", None, 1)),
            Fails(CheckFailure::ActiveRecordDiffers)
        );
        assert_eq!(
            judge_ownership(&FRESH, &owner("xz", "5", Some("5"), 2)),
            Fails(CheckFailure::MoreThanOneVersion)
        );
        assert_eq!(
            judge_ownership(&HELD, &owner("xz", "5.8.2_0", Some("5.8.2_0"), 2)),
            Fails(CheckFailure::OwnerVersionDiffers)
        );
        assert_eq!(
            judge_ownership(
                &FRESH,
                &OwnershipFacts::OwnerLeavesItsDirectory {
                    owner: "xz".to_owned()
                }
            ),
            Unchecked(UncheckedCause::OwnerLeavesKeg)
        );
        assert_eq!(
            judge_ownership(
                &FRESH,
                &OwnershipFacts::OwnerLeavesItsDirectory {
                    owner: "node".to_owned()
                }
            ),
            Fails(CheckFailure::AnotherPackagesEntry)
        );
        assert_eq!(
            judge_ownership(
                &FRESH,
                &OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed)
            ),
            Unchecked(UncheckedCause::ReadFailed)
        );
    }

    fn integrity(basis: IntegrityBasis) -> IntegrityFacts {
        IntegrityFacts {
            basis,
            list_unusable: None,
            missing: 0,
            dangling: 0,
            stat_failed: 0,
            outside_prefix: 0,
            record: InstallationRecord::NotKept,
            dependencies: DependencyRecord::NotKept,
        }
    }

    /// One judge, the same answer whichever provider's shape of facts it is
    /// handed: a provider that keeps no record is simply not asked about one.
    #[test]
    fn invariant_17_the_integrity_judge_answers_both_bases_alike() {
        use CheckAnswer::{Fails, Holds, Unchecked};
        for basis in [
            IntegrityBasis::RecordedFileList,
            IntegrityBasis::GeneratedFileList,
        ] {
            let clean = integrity(basis);
            assert_eq!(judge_integrity(&clean), Holds);
            let with = |change: fn(&mut IntegrityFacts)| {
                let mut facts = clean.clone();
                change(&mut facts);
                judge_integrity(&facts)
            };
            assert_eq!(with(|f| f.missing = 1), Fails(CheckFailure::PathMissing));
            assert_eq!(with(|f| f.dangling = 1), Fails(CheckFailure::PathDangling));
            assert_eq!(
                with(|f| f.dependencies = DependencyRecord::Missing),
                Fails(CheckFailure::DependencyMissing)
            );
            assert_eq!(
                with(|f| f.record = InstallationRecord::Absent),
                Fails(CheckFailure::NoInstallationRecord)
            );
            assert_eq!(
                with(|f| f.stat_failed = 1),
                Unchecked(UncheckedCause::StatFailed)
            );
            assert_eq!(
                with(|f| f.outside_prefix = 1),
                Unchecked(UncheckedCause::ListsOutsidePrefix)
            );
            assert_eq!(
                with(|f| f.dependencies = DependencyRecord::NotTold),
                Unchecked(UncheckedCause::DependenciesNotTold)
            );
            assert_eq!(
                with(|f| f.list_unusable = Some(UncheckedCause::ListOverCap)),
                Unchecked(UncheckedCause::ListOverCap)
            );
            // An observed failure outranks what could not be told.
            assert_eq!(
                with(|f| {
                    f.missing = 1;
                    f.stat_failed = 1;
                }),
                Fails(CheckFailure::PathMissing)
            );
        }
    }

    #[test]
    fn invariant_07_a_reference_to_another_provider_fails_and_says_whose_run_it_was() {
        let linked = LinkageFacts {
            links_detected_provider: true,
            loadable_files: 1,
            ..LinkageFacts::default()
        };
        assert_eq!(
            judge_linkage(false, &linked),
            CheckAnswer::Fails(CheckFailure::LinksAnotherProvider)
        );
        assert_eq!(
            judge_linkage(true, &linked),
            CheckAnswer::Fails(CheckFailure::LinkPresentBeforeTheRun)
        );
        // Nothing that could not be told hides it.
        let also = LinkageFacts {
            malformed_file: true,
            walk_over_bound: true,
            ..linked
        };
        assert_eq!(
            judge_linkage(false, &also),
            CheckAnswer::Fails(CheckFailure::LinksAnotherProvider)
        );
    }

    #[test]
    fn invariant_08_each_unknown_of_linkage_is_unchecked_and_none_holds() {
        type Set = fn(&mut LinkageFacts);
        let cases: [(Set, UncheckedCause); 7] = [
            (|f| f.malformed_file = true, UncheckedCause::MalformedFile),
            (|f| f.unreadable_file = true, UncheckedCause::UnreadableFile),
            (|f| f.walk_over_bound = true, UncheckedCause::WalkOverBound),
            (
                |f| f.under_undetected_prefix = true,
                UncheckedCause::UnderUndetectedPrefix,
            ),
            (
                |f| f.rpath_unresolved = true,
                UncheckedCause::RpathUnresolved,
            ),
            (
                |f| f.env_shebang_on_list = true,
                UncheckedCause::InterpreterResolvedAtRunTime,
            ),
            (
                |f| f.list_unusable = Some(UncheckedCause::FileListNotKnown),
                UncheckedCause::FileListNotKnown,
            ),
        ];
        for (set, cause) in cases {
            let mut facts = LinkageFacts::default();
            set(&mut facts);
            assert_eq!(judge_linkage(false, &facts), CheckAnswer::Unchecked(cause));
        }
    }

    #[test]
    fn nothing_to_load_holds_on_its_own_stated_basis() {
        let none = DestinationFacts::Linkage(LinkageFacts::default());
        assert_eq!(
            judge_linkage(false, &LinkageFacts::default()),
            CheckAnswer::Holds
        );
        assert_eq!(basis_of(&none), Some(CheckBasis::NoLoadCommands));
        // An `env` shebang off the list is counted and decides nothing.
        let elsewhere = LinkageFacts {
            env_shebang_elsewhere: 3,
            ..LinkageFacts::default()
        };
        assert_eq!(judge_linkage(false, &elsewhere), CheckAnswer::Holds);
    }

    #[test]
    fn the_generated_basis_is_its_own_and_never_the_recorded_one() {
        assert_eq!(
            basis_of(&DestinationFacts::Integrity(integrity(
                IntegrityBasis::GeneratedFileList
            ))),
            Some(CheckBasis::GeneratedFileList)
        );
        assert_eq!(basis_of(&DestinationFacts::NotObserved), None);
    }
}
