// SPDX-License-Identifier: Apache-2.0

//! The answer a destination check gives, once its observation is in
//! (DP-051 (a); work order 16 part H).
//!
//! Before part H a check on `VERIFY_DESTINATION` either passed or halted, and
//! DP-051's "reported unchecked" had nowhere to go. So there are three answers,
//! and the whole of the part follows from them:
//!
//! - [`CheckAnswer::Holds`]: observed, and it holds.
//! - [`CheckAnswer::Fails`]: observed, and it does not. The step halts and the
//!   source is kept.
//! - [`CheckAnswer::Unchecked`]: nobody could tell. The step does not halt, the
//!   source is **never removed**, and the run says so.
//!
//! "Could not tell" is never `Fails` and never `Holds`: an unknown is never a
//! yes, and it is not a no either. Each cause is a closed identifier and never
//! plan or provider text, so a record can keep one without redaction.

use crate::executable::ExecutableName;
use serde::{Deserialize, Serialize};

/// What one destination check found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "answer", content = "cause")]
pub enum CheckAnswer {
    /// Observed, and it holds.
    Holds,
    /// Observed, and it does not hold: the step halts.
    Fails(CheckFailure),
    /// Could not be told: the step goes on and the source is kept.
    Unchecked(UncheckedCause),
}

/// Why a destination check does not hold, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckFailure {
    /// A path the package lists does not exist.
    PathMissing,
    /// A path the package lists is a link to nothing.
    PathDangling,
    /// A dependency the package records is not in the inventory.
    DependencyMissing,
    /// The package's own record of its installation is absent, or holds only
    /// metadata.
    NoInstallationRecord,
    /// Nothing is placed under the executable's name.
    NoPlacedEntry,
    /// What is placed under the executable's name is another package's.
    AnotherPackagesEntry,
    /// More than one version is present after a fresh install.
    MoreThanOneVersion,
    /// The active record is not the version that owns the entry.
    ActiveRecordDiffers,
    /// The owner's version is not the one the snapshot says it should be.
    OwnerVersionDiffers,
    /// A file refers to another provider's prefix.
    LinksAnotherProvider,
    /// As [`CheckFailure::LinksAnotherProvider`], on a package the destination
    /// held before this run: the run did not cause it, and a source is not
    /// removed under it either.
    LinkPresentBeforeTheRun,
    /// The probe ended non-zero or on a signal.
    ProbeFailed,
}

/// Why a destination check could not be told, by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UncheckedCause {
    /// Nothing observed this: a replayed run, a run from a snapshot file, or a
    /// build with no observer for the check.
    NotObserved,
    /// The executable has no curated probe (DP-051: no curated probe is
    /// unchecked).
    NoCuratedProbe,
    /// The package has no executable to probe, or nobody knows its list, so no
    /// probe was possible and none passed vacuously.
    NoExecutableToProbe,
    /// The probe was not run, because the entry's ownership could not be told
    /// (one that did not hold halts the step instead).
    OwnershipNotTold,
    /// A file of the package could not be read as an object, whether malformed,
    /// over a cap, or a Java class whose magic is a fat header's, so nothing of
    /// the package's was run.
    MalformedFile,
    /// A provider read failed, timed out, or answered in a shape not recognised.
    ReadFailed,
    /// No allowed read lists this package's files: an inactive image, several
    /// versions with none selected, an application.
    FileListNotKnown,
    /// The package lists a path outside the prefix, which was not tested.
    ListsOutsidePrefix,
    /// The package's file list passed the read's cap.
    ListOverCap,
    /// A path's `stat` failed with something other than "does not exist".
    StatFailed,
    /// The provider's record of the package's dependencies could not be told.
    DependenciesNotTold,
    /// The entry is the package's and what it finally resolves to lies outside
    /// the package: outside its own directory, or outside the prefix.
    EntryLeavesOwner,
    /// The provider's record answered nothing for the entry, which means several
    /// things, so it says nothing about who owns it.
    OwnerNotNamed,
    /// The package was held, and no active version was, so no version can be
    /// pinned against the snapshot.
    HeldVersionNotTold,
    /// A reference lies under a documented prefix where no provider was
    /// detected, so it is nobody's for certain.
    UnderUndetectedPrefix,
    /// An `@rpath` name with no candidate that exists or lies in the destination,
    /// an `@executable_path` outside an executable, or a relative install name:
    /// the file alone cannot say.
    RpathUnresolved,
    /// An `env` or relative shebang on the executable list: what runs is decided
    /// at run time.
    InterpreterResolvedAtRunTime,
    /// A file could not be opened and read.
    UnreadableFile,
    /// The walk passed its bound, or reached a file it did not examine.
    WalkOverBound,
    /// A listed file or a name under the destination leads outside every known
    /// prefix, or a load command names something that is not a regular file.
    FileNotExamined,
    /// A path with `..` means one file written and another collapsed.
    SpellingsDisagree,
}

/// One thing a `VERIFY_DESTINATION` could not tell: which check, about which
/// executable when it was about one, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UncheckedEntry {
    /// The check, by its identifier in the plan.
    pub check: DestinationCheck,
    /// The executable, for a check asked once per executable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<ExecutableName>,
    /// Why it could not be told.
    pub cause: UncheckedCause,
}

/// The four checks DP-051 (a) added to `VERIFY_DESTINATION`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DestinationCheck {
    /// `PROVIDER_INTEGRITY_CHECK`.
    ProviderIntegrityCheck,
    /// `EXECUTABLES_OWNED_BY_DESTINATION`.
    ExecutablesOwnedByDestination,
    /// `NO_CROSS_PROVIDER_LINKAGE`.
    NoCrossProviderLinkage,
    /// `LIVENESS_PROBE`, or a recorded skip of one.
    LivenessProbe,
}

/// Why a plan records no probe for an executable (or for none).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProbeSkipCause {
    /// The table holds no row for this executable of this package.
    NoCuratedProbe,
    /// The package's executable list is empty, or nobody knows it.
    NoExecutableToProbe,
}

impl ProbeSkipCause {
    /// What a skip answers. Never `Holds`.
    #[must_use]
    pub fn unchecked(self) -> UncheckedCause {
        match self {
            Self::NoCuratedProbe => UncheckedCause::NoCuratedProbe,
            Self::NoExecutableToProbe => UncheckedCause::NoExecutableToProbe,
        }
    }
}
