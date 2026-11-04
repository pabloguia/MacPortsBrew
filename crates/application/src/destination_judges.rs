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
    InstalledPackage, IntegrityBasis, IntegrityFacts, LinkageFacts, MigrationPlan, OwnershipFacts,
    Snapshot, StepKind, UncheckedCause, VerificationCheck,
};

/// What the paired snapshot says about the destination package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeldBefore {
    /// No row of it: this run's install is what would put it there.
    NotHeld,
    /// Rows of it, and none active, so no version to pin against.
    OnlyInactive,
    /// A row active, whose version the adapter states in the owner's shape.
    Active,
}

/// What the plan and its paired snapshot expect of an owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerExpectation<'a> {
    /// The destination package.
    pub package: &'a str,
    /// The step carries `PACKAGE_NEWLY_PRESENT`: the snapshot held none of it.
    pub newly_present: bool,
    /// What the snapshot held.
    pub held: HeldBefore,
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
                CheckAnswer::Unchecked(UncheckedCause::EntryLeavesOwner)
            } else {
                CheckAnswer::Fails(CheckFailure::AnotherPackagesEntry)
            }
        }
        OwnershipFacts::Owner {
            name,
            version,
            active,
            versions_present,
            held,
        } => {
            if name != expected.package {
                return CheckAnswer::Fails(CheckFailure::AnotherPackagesEntry);
            }
            if active.as_deref() != Some(version.as_str()) {
                return CheckAnswer::Fails(CheckFailure::ActiveRecordDiffers);
            }
            if expected.newly_present {
                return if *versions_present == 1 {
                    CheckAnswer::Holds
                } else {
                    CheckAnswer::Fails(CheckFailure::MoreThanOneVersion)
                };
            }
            match (expected.held, held) {
                (HeldBefore::Active, Some(held)) if held == version => CheckAnswer::Holds,
                (HeldBefore::Active, Some(_)) => {
                    CheckAnswer::Fails(CheckFailure::OwnerVersionDiffers)
                }
                // Nothing to pin against is not "it is the version it was".
                (HeldBefore::Active, None)
                | (HeldBefore::OnlyInactive | HeldBefore::NotHeld, _) => {
                    CheckAnswer::Unchecked(UncheckedCause::HeldVersionNotTold)
                }
            }
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
    // The two that stop a probe come first, so that no other cause can hide one
    // and let a program of this package run.
    if facts.malformed_file {
        return CheckAnswer::Unchecked(UncheckedCause::MalformedFile);
    }
    if facts.walk_over_bound {
        return CheckAnswer::Unchecked(UncheckedCause::WalkOverBound);
    }
    if let Some(cause) = facts.list_unusable {
        return CheckAnswer::Unchecked(cause);
    }
    if facts.unreadable_file {
        return CheckAnswer::Unchecked(UncheckedCause::UnreadableFile);
    }
    if facts.file_not_examined {
        return CheckAnswer::Unchecked(UncheckedCause::FileNotExamined);
    }
    if facts.spellings_disagree {
        return CheckAnswer::Unchecked(UncheckedCause::SpellingsDisagree);
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
        // Held at all, active or not: a link an inactive image left was not made
        // by this run.
        DestinationFacts::Linkage(facts) => judge_linkage(
            held_before(plan, run.before()) != HeldBefore::NotHeld,
            facts,
        ),
        DestinationFacts::Ownership(facts) => {
            let Some(package) = plan.destination_package.as_deref() else {
                return CheckAnswer::Unchecked(UncheckedCause::NotObserved);
            };
            judge_ownership(
                &OwnerExpectation {
                    package,
                    newly_present: newly_present(plan),
                    held: held_before(plan, run.before()),
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

fn rows<'a>(plan: &MigrationPlan, before: &'a Snapshot) -> Vec<&'a InstalledPackage> {
    let Some(name) = plan.destination_package.as_deref() else {
        return Vec::new();
    };
    before
        .providers
        .get(&plan.destination)
        .map_or_else(Vec::new, |provider| {
            provider
                .inventory
                .installed
                .iter()
                .filter(|row| row.identity.name == name)
                .collect()
        })
}

fn held_before(plan: &MigrationPlan, before: &Snapshot) -> HeldBefore {
    let rows = rows(plan, before);
    if rows.iter().any(|row| row.active) {
        HeldBefore::Active
    } else if rows.is_empty() {
        HeldBefore::NotHeld
    } else {
        HeldBefore::OnlyInactive
    }
}

/// The snapshot's active row of the destination package, which the adapter that
/// answers for it reads the comparable version from.
#[must_use]
pub fn held_row(run: &PlannedRun) -> Option<&InstalledPackage> {
    rows(run.plan(), run.before())
        .into_iter()
        .find(|row| row.active)
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
            held: Some("5.8.3_0".to_owned()),
        }
    }

    const FRESH: OwnerExpectation<'_> = OwnerExpectation {
        package: "xz",
        newly_present: true,
        held: HeldBefore::NotHeld,
    };

    const HELD: OwnerExpectation<'_> = OwnerExpectation {
        package: "xz",
        newly_present: false,
        held: HeldBefore::Active,
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
            Unchecked(UncheckedCause::EntryLeavesOwner)
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
    fn a_package_held_only_inactive_has_no_version_to_pin_and_never_holds() {
        let only_inactive = OwnerExpectation {
            held: HeldBefore::OnlyInactive,
            ..HELD
        };
        let holds = owner("xz", "5.8.3_0", Some("5.8.3_0"), 1);
        assert_eq!(
            judge_ownership(&only_inactive, &holds),
            CheckAnswer::Unchecked(UncheckedCause::HeldVersionNotTold)
        );
        // Neither is an adapter that could not state the version.
        let unstated = OwnershipFacts::Owner {
            name: "xz".to_owned(),
            version: "5.8.3_0".to_owned(),
            active: Some("5.8.3_0".to_owned()),
            versions_present: 1,
            held: None,
        };
        assert_eq!(
            judge_ownership(&HELD, &unstated),
            CheckAnswer::Unchecked(UncheckedCause::HeldVersionNotTold)
        );
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
        let cases: [(Set, UncheckedCause); 9] = [
            (
                |f| f.file_not_examined = true,
                UncheckedCause::FileNotExamined,
            ),
            (
                |f| f.spellings_disagree = true,
                UncheckedCause::SpellingsDisagree,
            ),
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

    /// What stops a probe outranks every other cause, or the other cause would
    /// let a program of this package run.
    #[test]
    fn a_cause_that_stops_the_probe_is_never_hidden_by_another() {
        let everything = LinkageFacts {
            malformed_file: true,
            walk_over_bound: true,
            unreadable_file: true,
            file_not_examined: true,
            list_unusable: Some(UncheckedCause::ListsOutsidePrefix),
            ..LinkageFacts::default()
        };
        assert_eq!(
            judge_linkage(false, &everything),
            CheckAnswer::Unchecked(UncheckedCause::MalformedFile)
        );
        let over = LinkageFacts {
            malformed_file: false,
            ..everything
        };
        assert_eq!(
            judge_linkage(false, &over),
            CheckAnswer::Unchecked(UncheckedCause::WalkOverBound)
        );
    }

    #[test]
    fn nothing_to_load_holds_on_its_own_stated_basis() {
        let none = DestinationFacts::Linkage(LinkageFacts::default());
        assert_eq!(
            judge_linkage(false, &LinkageFacts::default()),
            CheckAnswer::Holds
        );
        assert_eq!(basis_of(&none), Some(CheckBasis::NoLoadCommands));
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
