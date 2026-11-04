// SPDX-License-Identifier: Apache-2.0

//! What an adapter observed for a destination check, before anyone judged it
//! (work order 16 part H; R-25, R-26, R-27).
//!
//! An adapter reads; `application` judges, one judge per check, and never
//! branches on a provider id (INV-17). These types are that boundary: counts and
//! closed answers, no verdict. A provider that cannot answer says
//! [`DestinationFacts::NotObserved`], which is also the default of
//! `ProviderRead::destination_facts`, and that starts no program.
//!
//! [`PrefixTable`] lives here rather than in an adapter because "what counts as
//! under a prefix" must be one rule for the walk and for any test of it, and it
//! is pure: bytes in, an answer out.

use crate::destination_check::UncheckedCause;
use crate::executable::ExecutableName;

/// What a destination check asks of a provider.
#[derive(Clone, Copy, Debug)]
pub enum DestinationQuery<'a> {
    /// The placed entry of `leaf`: whose is it, at which version?
    Ownership {
        /// The executable, already of its name class.
        leaf: &'a ExecutableName,
    },
    /// The provider's record of the package against the disk.
    Integrity,
    /// The references the package's files carry.
    Linkage {
        /// Where each provider is, as the registry knows it.
        prefixes: &'a PrefixTable,
    },
}

/// What a provider observed, or that it observed nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestinationFacts {
    /// Nothing was observed: a replayed run, a run from a snapshot file, or a
    /// provider with no such read. Never a yes.
    NotObserved,
    /// The answer to [`DestinationQuery::Ownership`].
    Ownership(OwnershipFacts),
    /// The answer to [`DestinationQuery::Integrity`].
    Integrity(IntegrityFacts),
    /// The answer to [`DestinationQuery::Linkage`].
    Linkage(LinkageFacts),
}

/// Who owns a placed entry, by the provider's own record (R-26).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnershipFacts {
    /// Nothing usable is placed under the name: absent, or a dangling link.
    NoEntry,
    /// The provider's record names no package for the entry.
    NotOwned,
    /// A one-hop link is the package's, and what it finally resolves to lies
    /// outside the package's own directory (R-26 finding 14).
    OwnerLeavesItsDirectory {
        /// The package the first hop names.
        owner: String,
    },
    /// The provider's record names this package at this version.
    Owner {
        /// The package, compared and never joined into a path.
        name: String,
        /// The version the entry belongs to, in the adapter's own shape.
        version: String,
        /// The version the provider holds active, when it holds one.
        active: Option<String>,
        /// How many versions of the package are present.
        versions_present: usize,
    },
    /// The read could not tell.
    CouldNotTell(UncheckedCause),
}

/// What a provider's file list says, and how strong that is (R-25).
///
/// One is a stored record and the other a walk of the disk at the moment of
/// asking, so "every listed path exists" means something only of the first. Both
/// are carried, neutrally, so the difference is reported rather than averaged
/// away (R-25 finding 1, DP-057).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntegrityBasis {
    /// A record the provider stored at installation. A missing file can be
    /// found.
    RecordedFileList,
    /// A list generated from the disk when asked. **A file absent from it is not
    /// detected**: the basis never claims completeness.
    GeneratedFileList,
}

/// The neutral basis a `HOLDS` rests on, for `--json` and the catalogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckBasis {
    /// As [`IntegrityBasis::RecordedFileList`].
    RecordedFileList,
    /// As [`IntegrityBasis::GeneratedFileList`].
    GeneratedFileList,
    /// Load commands and shebangs were read, and nothing named another
    /// provider's prefix.
    ReferencesExamined,
    /// The package holds no loadable file and no `env` shebang on its executable
    /// list, so nothing was left unevaluated (DP-057's declared reading).
    NoLoadCommands,
}

/// The package's own record of its installation, where the provider keeps one
/// separately from the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallationRecord {
    /// This provider keeps none apart from its file list. A declared asymmetry.
    NotKept,
    /// Present, with at least one real entry beside it.
    Present,
    /// Absent, or only metadata is there.
    Absent,
}

/// The dependencies the provider recorded for this installation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DependencyRecord {
    /// This provider offers no readable record of them. A declared asymmetry
    /// (R-25 finding 14).
    NotKept,
    /// Every recorded dependency is in the inventory (an honest none included).
    AllPresent,
    /// A recorded dependency is not in the inventory.
    Missing,
    /// The record could not be told: an absent field is not "none".
    NotTold,
}

/// The facts of `PROVIDER_INTEGRITY_CHECK`, one provider's worth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrityFacts {
    /// How strong "every listed path exists" is here.
    pub basis: IntegrityBasis,
    /// Why the list itself could not be used, when it could not. Nothing was
    /// tested then, so the counts below say nothing.
    pub list_unusable: Option<UncheckedCause>,
    /// Paths whose `lstat` said they do not exist.
    pub missing: usize,
    /// Links whose `stat` said they lead nowhere.
    pub dangling: usize,
    /// Paths whose `stat` failed with anything else, which proves nothing.
    pub stat_failed: usize,
    /// Listed paths outside the prefix. Not `stat`ed.
    pub outside_prefix: usize,
    /// The record of the installation.
    pub record: InstallationRecord,
    /// The recorded dependencies.
    pub dependencies: DependencyRecord,
}

/// The facts of `NO_CROSS_PROVIDER_LINKAGE` (R-27).
// Each bool is one thing the walk can report, and more than one can hold at once;
// a judge reads them in its own order, so an enum would lose what it needs.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkageFacts {
    /// Why the list itself could not be used, when it could not.
    pub list_unusable: Option<UncheckedCause>,
    /// `MH_EXECUTE`, `MH_DYLIB` and `MH_BUNDLE` files examined.
    pub loadable_files: usize,
    /// `MH_OBJECT` and `ar` files, skipped and counted.
    pub skipped_files: usize,
    /// A reference lies under a prefix a provider was detected at, other than
    /// the destination's.
    pub links_detected_provider: bool,
    /// A reference lies under a candidate prefix no provider was detected at.
    pub under_undetected_prefix: bool,
    /// An `@rpath` name with no candidate, or an `@executable_path` that cannot
    /// be resolved lexically.
    pub rpath_unresolved: bool,
    /// A file this reader cannot answer for.
    pub malformed_file: bool,
    /// A file that could not be opened and read.
    pub unreadable_file: bool,
    /// The walk passed a bound, or a reference reached a file not examined.
    pub walk_over_bound: bool,
    /// An `env` shebang on the package's executable list.
    pub env_shebang_on_list: bool,
    /// `env` shebangs elsewhere in the list, counted (R-27 finding 12).
    pub env_shebang_elsewhere: usize,
}

/// What a probe observed. This slice runs none: only `NotRun` can come back,
/// until the admission lands (`docs/passes/16-liveness-design.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeObservation {
    /// No program was started, and why.
    NotRun(UncheckedCause),
}

/// What runs a curated probe. **Not a supertrait of `ProviderRead`:** any holder
/// of that trait could otherwise start a third-party program past the gate, the
/// consent and the run lock (the design note). Obtained only from the registry's
/// destination sites.
pub trait DestinationProbe {
    /// Runs `argv` on the placed `leaf` of `package`.
    fn liveness_probe(
        &self,
        package: &str,
        leaf: &ExecutableName,
        argv: &[String],
    ) -> ProbeObservation;
}

/// Which side a path lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrefixClass {
    /// The destination's own prefix.
    Destination,
    /// A prefix another provider was detected at: `Fails`.
    DetectedOther,
    /// A documented candidate where no provider was detected: `Unchecked`,
    /// because `/usr/local/lib/x.dylib` on a machine with no provider there is
    /// nobody's (R-27).
    UndetectedCandidate,
}

/// Every prefix that counts, by class. Component-wise, case-insensitive, longest
/// match (R-26 finding 7; an upper-case prefix is inferred, not observed).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrefixTable {
    entries: Vec<(Vec<Vec<u8>>, PrefixClass)>,
}

impl PrefixTable {
    /// Adds `path` as `class`. A path already there keeps the class it has: the
    /// registry adds the destination and the detected first, so a candidate
    /// never weakens them. A path that does not normalise is not added.
    pub fn add(&mut self, path: &[u8], class: PrefixClass) {
        let Some(normal) = normalise(None, path) else {
            return;
        };
        let key = components_folded(&normal);
        if key.is_empty() || self.entries.iter().any(|(k, _)| *k == key) {
            return;
        }
        self.entries.push((key, class));
    }

    /// The class of `path`, which must already be absolute and normal: the
    /// longest prefix it lies under, or `None` for a path under none of them. A
    /// prefix itself is under itself.
    #[must_use]
    pub fn class_of(&self, path: &[u8]) -> Option<PrefixClass> {
        let path = components_folded(path);
        self.entries
            .iter()
            .filter(|(key, _)| path.len() >= key.len() && path.iter().zip(key).all(|(a, b)| a == b))
            .max_by_key(|(key, _)| key.len())
            .map(|(_, class)| *class)
    }
}

fn components_folded(path: &[u8]) -> Vec<Vec<u8>> {
    path.split(|b| *b == b'/')
        .filter(|c| !c.is_empty())
        .map(<[u8]>::to_ascii_lowercase)
        .collect()
}

/// `path` as an absolute path with no empty, `.` or `..` component, joined to
/// `base` when it is relative. `..` at the root stays at the root, as the kernel
/// has it. `None` for a relative path with no base, or one holding a NUL. Bytes
/// throughout: an install name is compared, never decoded.
#[must_use]
pub fn normalise(base: Option<&[u8]>, path: &[u8]) -> Option<Vec<u8>> {
    if path.contains(&0) {
        return None;
    }
    let joined: Vec<u8> = if path.first() == Some(&b'/') {
        path.to_vec()
    } else {
        let mut joined = base?.to_vec();
        joined.push(b'/');
        joined.extend_from_slice(path);
        joined
    };
    if joined.first() != Some(&b'/') {
        return None;
    }
    let mut kept: Vec<&[u8]> = Vec::new();
    for component in joined.split(|b| *b == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                kept.pop();
            }
            real => kept.push(real),
        }
    }
    let mut normal = Vec::with_capacity(joined.len());
    for component in &kept {
        normal.push(b'/');
        normal.extend_from_slice(component);
    }
    if normal.is_empty() {
        normal.push(b'/');
    }
    Some(normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> PrefixTable {
        let mut table = PrefixTable::default();
        table.add(b"/opt/local", PrefixClass::Destination);
        table.add(b"/usr/local", PrefixClass::DetectedOther);
        table.add(b"/opt/homebrew", PrefixClass::UndetectedCandidate);
        table.add(b"/opt/local/nested", PrefixClass::UndetectedCandidate);
        table
    }

    #[test]
    fn under_is_by_component_and_not_by_string() {
        let table = table();
        assert_eq!(table.class_of(b"/opt/local2/lib/x"), None);
        assert_eq!(
            table.class_of(b"/opt/local/lib/x"),
            Some(PrefixClass::Destination)
        );
    }

    #[test]
    fn under_folds_case_and_prefers_the_longest() {
        let table = table();
        assert_eq!(
            table.class_of(b"/USR/Local/lib/x"),
            Some(PrefixClass::DetectedOther)
        );
        assert_eq!(
            table.class_of(b"/opt/local/nested/lib/x"),
            Some(PrefixClass::UndetectedCandidate)
        );
    }

    #[test]
    fn a_candidate_does_not_weaken_what_was_detected() {
        let mut table = table();
        table.add(b"/usr/local", PrefixClass::UndetectedCandidate);
        assert_eq!(
            table.class_of(b"/usr/local/x"),
            Some(PrefixClass::DetectedOther)
        );
    }

    #[test]
    fn normalising_resolves_dots_and_refuses_what_it_cannot_anchor() {
        assert_eq!(
            normalise(None, b"/opt/local/lib/../../../usr/local/x").as_deref(),
            Some(&b"/usr/local/x"[..])
        );
        assert_eq!(
            normalise(Some(b"/a/b"), b"../c").as_deref(),
            Some(&b"/a/c"[..])
        );
        assert_eq!(normalise(None, b"lib/x"), None);
        assert_eq!(normalise(None, b"/a\0/b"), None);
        assert_eq!(normalise(None, b"/../..").as_deref(), Some(&b"/"[..]));
    }
}
