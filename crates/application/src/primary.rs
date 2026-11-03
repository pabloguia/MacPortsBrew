// SPDX-License-Identifier: Apache-2.0

//! The primary decision (`05-environment-and-primary-provider.md`, "Setting
//! the primary", the MUST step list 1–6; F.md Design §4; SE-112).
//!
//! A pure function of the snapshot: one rule for both directions (INV-17),
//! one `PATH` — the snapshot's — for both sides (SG-10), and nothing that
//! reorders `PATH` (INV-12) or transfers ownership (INV-11). An unknown is
//! never a yes: every fact this rule cannot establish refuses, and only an
//! observed `NOT_ON_PATH` counts as "no hit" (SG-2).

use macportsbrew_domain::{
    ExecutableList, ExecutableName, ExecutableOwner, PackageMetadata, Placement, PrimaryDecision,
    PrimaryReason, ProviderId, ResolutionState, Snapshot,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What the decision is asked about: one source package, and the one
/// destination package its 1→1 mapping selected.
#[derive(Clone, Debug)]
pub struct PrimaryQuestion<'a> {
    /// The provider being moved from.
    pub source: &'a ProviderId,
    /// The source package, by the source provider's name for it.
    pub source_package: &'a str,
    /// The provider being moved to.
    pub destination: &'a ProviderId,
    /// The destination package, when a mapping selected one.
    pub destination_package: Option<&'a str>,
    /// Whether the destination provider already has that package installed.
    pub destination_installed: bool,
    /// Whether the mapping is one package to one package.
    pub one_to_one: bool,
}

fn metadata<'s>(
    snapshot: &'s Snapshot,
    provider: &ProviderId,
    name: &str,
) -> Option<&'s PackageMetadata> {
    snapshot
        .providers
        .get(provider)
        .and_then(|p| p.inventory.metadata.get(name))
}

/// The list the source package's metadata records, `NOT_RECORDED` without one.
fn list_of<'s>(
    snapshot: &'s Snapshot,
    provider: &ProviderId,
    name: &str,
) -> Option<&'s ExecutableList> {
    metadata(snapshot, provider, name).map(|m| &m.executable_list)
}

/// The decision for `question`, from `snapshot` alone.
#[must_use]
pub fn decide(snapshot: &Snapshot, question: &PrimaryQuestion<'_>) -> PrimaryDecision {
    let path_used = snapshot.environment.path.clone();
    let refuse_package = |reason| PrimaryDecision::Refused {
        executables: BTreeSet::new(),
        reasons: BTreeMap::new(),
        package: Some(reason),
        path_used: path_used.clone(),
    };
    let Some(destination_package) = question.destination_package.filter(|_| question.one_to_one)
    else {
        return refuse_package(PrimaryReason::PrimaryMappingNotOneToOne);
    };
    // Step 1: the source's list, never the package name in its place.
    let Some(list) =
        list_of(snapshot, question.source, question.source_package).and_then(ExecutableList::known)
    else {
        return refuse_package(PrimaryReason::ExecutablesUnknown);
    };
    if list.is_empty() {
        return PrimaryDecision::NoExecutables;
    }
    let destination = question
        .destination_installed
        .then(|| metadata(snapshot, question.destination, destination_package))
        .flatten();
    let reasons: BTreeMap<ExecutableName, PrimaryReason> = list
        .iter()
        .filter_map(|e| {
            decide_one(snapshot, question, destination_package, destination, e)
                .map(|reason| (e.clone(), reason))
        })
        .collect();
    // All or nothing per package (05 MUST): one refusal refuses the plan.
    if reasons.is_empty() {
        PrimaryDecision::NothingToChange {
            executables: list.clone(),
            path_used,
        }
    } else {
        PrimaryDecision::Refused {
            executables: list.clone(),
            reasons,
            package: None,
            path_used,
        }
    }
}

/// Steps 2–6 for one executable: `None` when the destination package already
/// wins or is predicted to, else why not.
fn decide_one(
    snapshot: &Snapshot,
    question: &PrimaryQuestion<'_>,
    destination_package: &str,
    destination: Option<&PackageMetadata>,
    executable: &ExecutableName,
) -> Option<PrimaryReason> {
    // Step 2: never resolved is never "not on PATH".
    let winner = match snapshot.resolution_of(executable) {
        ResolutionState::NotObserved => return Some(PrimaryReason::PrimaryResolutionUnknown),
        ResolutionState::NotOnPath => None,
        ResolutionState::Found {
            resolution,
            path_index,
        } => {
            match &resolution.owner {
                ExecutableOwner::Provider { provider, package }
                    if provider == question.destination =>
                {
                    // Step 3, with the package compared (SR-3): `None` is not
                    // the destination package.
                    if package.as_deref() != Some(destination_package) {
                        return Some(PrimaryReason::OwnedByAnotherPackage);
                    }
                    if !resolution.authoritative {
                        return Some(PrimaryReason::WinnerNotAuthoritative);
                    }
                    // "Already wins" is an observation only where the shell's
                    // search order is: a winner with no recorded index (a
                    // relative or empty entry before it, or an older document)
                    // is unpredictable here too (SR-5; security review of F-3,
                    // M-2). And it is the destination's only if the destination
                    // is installed: a row that says otherwise is inconsistent
                    // and refused the same way in both directions (spec-guardian
                    // review of F-3, R-1; 05 "What this reaches").
                    if path_index.is_none() {
                        return Some(PrimaryReason::PrimaryUnpredictable);
                    }
                    if !question.destination_installed {
                        return Some(PrimaryReason::PrimaryDestinationNotInstalled);
                    }
                    return None;
                }
                ExecutableOwner::Provider { .. } => {
                    if !resolution.authoritative {
                        return Some(PrimaryReason::WinnerNotAuthoritative);
                    }
                }
                // Step 4 (SG-8, Tier 4): a system tool, another user install, or
                // a file no provider claims (capture records `/usr/bin/m4` as an
                // `UNKNOWN` owner: every provider was asked and none owns it)
                // wins today and is not displaced by this release. "Any
                // non-provider owner" is the plan's wording; a provider's
                // prefix-matched file is the separate reason above (SG2-N4).
                ExecutableOwner::System
                | ExecutableOwner::External
                | ExecutableOwner::MacportsbrewShim
                | ExecutableOwner::Unknown => {
                    return Some(PrimaryReason::NonProviderEntryWins);
                }
            }
            Some(path_index)
        }
    };
    // Step 5 (DP-055, default (a)): an uninstalled destination is refused in
    // both directions, before its placement is ever compared.
    if !question.destination_installed {
        return Some(PrimaryReason::PrimaryDestinationNotInstalled);
    }
    let Some(destination) = destination else {
        return Some(PrimaryReason::PrimaryUnpredictable);
    };
    // "Does the destination ship `e`" before its placement (05 step 5, in this
    // order). With its list not known, a `NOT_LINKED` placement says nothing
    // about `e`, so it is not evidence a shim is needed (security review of
    // F-3, L-1).
    let Some(ships) = destination.executable_list.known() else {
        return Some(PrimaryReason::PrimaryUnpredictable);
    };
    if !ships.contains(executable) {
        return Some(PrimaryReason::DestinationLacksExecutable);
    }
    // A surviving `ON_PATH_DIR` is the adapter's statement, not an
    // observation (security review of F-2b, L-4): it is only ever compared
    // with the captured winner, never taken as proof of a win by itself.
    let dir = match &destination.placement {
        Placement::NotLinked { .. } => return Some(PrimaryReason::PrimaryNeedsShim),
        Placement::Unknown { .. } => return Some(PrimaryReason::PrimaryUnpredictable),
        Placement::OnPathDir { dirs } => match dirs.get(executable) {
            Some(dir) => dir,
            None => return Some(PrimaryReason::PrimaryUnpredictable),
        },
    };
    // Step 6: `d` on the snapshot's own `PATH`, with the relative-entry rule
    // (SR-4).
    let d = match destination_index(&snapshot.environment.path, dir) {
        DirectoryIndex::At(d) => d,
        DirectoryIndex::Unpredictable => return Some(PrimaryReason::PrimaryUnpredictable),
        DirectoryIndex::Absent => return Some(PrimaryReason::PrimaryNeedsShim),
    };
    // The destination says `e` is linked at `d`, and the winner reached here
    // is not the destination's own (step 3 answered that). Where `d` comes
    // before the winner, or nothing was found at all, the capture contradicts
    // the placement: the shell would have found the destination's copy first.
    // A contradiction is never a predicted win (security review of F-3, L-2;
    // 05 "What this reaches").
    match winner {
        None => Some(PrimaryReason::PrimaryUnpredictable),
        // A winner with no recorded index is unpredictable, never index 0
        // (SR-5; mutant 15).
        Some(None) => Some(PrimaryReason::PrimaryUnpredictable),
        Some(Some(w)) => match d.cmp(&w) {
            std::cmp::Ordering::Less => Some(PrimaryReason::PrimaryUnpredictable),
            std::cmp::Ordering::Greater => Some(PrimaryReason::PrimaryNeedsShim),
            std::cmp::Ordering::Equal => Some(PrimaryReason::SameDirectory),
        },
    }
}

/// Where the destination's directory stands on the decision's `PATH`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryIndex {
    /// The first absolute entry equal to it, component-wise, with no relative
    /// or empty entry before it.
    At(u32),
    /// A relative or empty entry comes before the first match: a shell would
    /// search it first, so the order is not established (SR-4).
    Unpredictable,
    /// No absolute entry equals it.
    Absent,
}

/// The index `d` of F.md Design §4 step 6, on `entries` as the snapshot
/// records them: home-redacted, so an entry written `~` or `~/…` was absolute
/// before redaction and counts as one, as in `recorded_index_holds`.
#[must_use]
pub fn destination_index(entries: &[String], dir: &str) -> DirectoryIndex {
    let absolute = |e: &str| e.starts_with('/') || e == "~" || e.starts_with("~/");
    let wanted = Path::new(dir);
    let mut skipped = false;
    for (index, entry) in entries.iter().enumerate() {
        if !absolute(entry) {
            skipped = true;
            continue;
        }
        if Path::new(entry).components().eq(wanted.components()) {
            if skipped {
                return DirectoryIndex::Unpredictable;
            }
            return u32::try_from(index).map_or(DirectoryIndex::Unpredictable, DirectoryIndex::At);
        }
    }
    DirectoryIndex::Absent
}

/// Executables an **installed** destination ships outside the source's list
/// whose winner today is not a provider's file (SG-8): listed in the plan as a
/// non-blocking risk, so the person reads that those names stay the system's.
#[must_use]
pub fn system_tools_keeping_their_names(
    snapshot: &Snapshot,
    question: &PrimaryQuestion<'_>,
) -> BTreeSet<ExecutableName> {
    let (Some(destination_package), true) =
        (question.destination_package, question.destination_installed)
    else {
        return BTreeSet::new();
    };
    let Some(ships) = list_of(snapshot, question.destination, destination_package)
        .and_then(ExecutableList::known)
    else {
        return BTreeSet::new();
    };
    let source =
        list_of(snapshot, question.source, question.source_package).and_then(ExecutableList::known);
    ships
        .iter()
        .filter(|e| source.is_none_or(|s| !s.contains(*e)))
        .filter(|e| match snapshot.resolution_of(e) {
            ResolutionState::Found { resolution, .. } => matches!(
                resolution.owner,
                ExecutableOwner::System | ExecutableOwner::External | ExecutableOwner::Unknown
            ),
            ResolutionState::NotOnPath | ResolutionState::NotObserved => false,
        })
        .cloned()
        .collect()
}
