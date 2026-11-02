// SPDX-License-Identifier: Apache-2.0

//! The executables a package puts on the machine, and where they land.
//!
//! Part F of work order 16 (`docs/passes/16-plans/F.md`, Design §1). A
//! primary provider is chosen per *executable*, not per package name, so the
//! verification contract (`04-migration-and-safety.md`, checks 3 and 4) needs
//! the list a provider's own metadata gives: `rg` for `ripgrep`, `glibtool` for
//! `libtool` (DP-047 (b)).
//!
//! Three answers are kept apart because folding any two of them is the defect
//! DP-032 fixed for `defines_service`:
//!
//! * [`ExecutableList::NotRecorded`] - nobody asked;
//! * [`ExecutableList::Unknown`] - asked, and the answer could not be
//!   established, with the reason;
//! * [`ExecutableList::Known`] - asked and answered, possibly with no entry.
//!
//! An empty `Known` is a real answer ("this package ships no executable") and
//! only an adapter that parsed a non-empty listing may produce it.

use crate::error::ErrorId;
use crate::text::hostile_class;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The longest name accepted, in bytes: the macOS `NAME_MAX`.
pub const MAX_EXECUTABLE_NAME_BYTES: usize = 255;

/// More executables than this in one package make the list
/// [`UnknownReason::TooLarge`] rather than a list (F.md Design §2).
pub const MAX_EXECUTABLES: usize = 4096;

/// A validated executable leaf name: what a shell looks up on `PATH`.
///
/// The one name class for every reader of an executable name - the adapter
/// parsers, `which`, the planner and the gate (F.md §1, SR-11): non-empty, one
/// path component (no `/`), not `.` or `..`, no leading `-`, at most
/// [`MAX_EXECUTABLE_NAME_BYTES`] bytes, and no character of any
/// [`crate::HostileClass`]. Deserialising goes through [`Self::parse`], so a
/// document carrying a bad name fails to load rather than loading it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExecutableName(String);

/// Why a candidate name is not an [`ExecutableName`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameRefusal {
    /// The empty string.
    Empty,
    /// Contains `/`: a path, not a leaf.
    Separator,
    /// `.` or `..`.
    DotOrDotDot,
    /// Starts with `-`, which a command would read as a flag.
    LeadingDash,
    /// Longer than [`MAX_EXECUTABLE_NAME_BYTES`].
    TooLong,
    /// Carries a character of a [`crate::HostileClass`] (NUL included).
    Hostile,
    /// Carries U+FFFD: what a lossy decoding put where the provider's bytes
    /// were not UTF-8, so the name on disk is not this one.
    Replacement,
}

impl fmt::Display for NameRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            NameRefusal::Empty => "empty",
            NameRefusal::Separator => "contains a path separator",
            NameRefusal::DotOrDotDot => "is `.` or `..`",
            NameRefusal::LeadingDash => "starts with `-`",
            NameRefusal::TooLong => "is longer than 255 bytes",
            NameRefusal::Hostile => "carries a control, format or line-separator character",
            NameRefusal::Replacement => "carries U+FFFD, the mark of bytes that were not UTF-8",
        };
        f.write_str(text)
    }
}

impl ExecutableName {
    /// Validates `name` against the one name class.
    pub fn parse(name: &str) -> Result<Self, NameRefusal> {
        if name.is_empty() {
            return Err(NameRefusal::Empty);
        }
        if name.len() > MAX_EXECUTABLE_NAME_BYTES {
            return Err(NameRefusal::TooLong);
        }
        if name.contains('/') {
            return Err(NameRefusal::Separator);
        }
        if name == "." || name == ".." {
            return Err(NameRefusal::DotOrDotDot);
        }
        if name.starts_with('-') {
            return Err(NameRefusal::LeadingDash);
        }
        if name.chars().any(|c| hostile_class(c).is_some()) {
            return Err(NameRefusal::Hostile);
        }
        if name.contains(char::REPLACEMENT_CHARACTER) {
            return Err(NameRefusal::Replacement);
        }
        Ok(Self(name.to_string()))
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExecutableName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for ExecutableName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|why| {
            format!(
                "`{}` is not an executable name: it {why}",
                crate::text::escape_hostile(&value)
            )
        })
    }
}

impl From<ExecutableName> for String {
    fn from(value: ExecutableName) -> Self {
        value.0
    }
}

/// Defines [`ListReason`] and [`ListReason::all`] from one list, so a variant
/// cannot exist without being in `all()`, and the collision test and every
/// other reader of `all()` see it. The serialised spelling is serde's
/// `SCREAMING_SNAKE_CASE` of each identifier, unchanged.
macro_rules! list_reasons {
    ($($(#[$meta:meta])* $variant:ident,)+) => {
        /// A reason an adapter itself names, as opposed to an [`ErrorId`] a
        /// read failed with. Spelled so that no member collides with an
        /// [`ErrorId`] - `no_reason_collides_with_an_error_id` holds that over
        /// [`ListReason::all`], because [`UnknownReason`] is told apart by its
        /// spelling alone.
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum ListReason {
            $($(#[$meta])* $variant,)+
        }

        impl ListReason {
            /// Every reason, in declaration order. Generated by
            /// `list_reasons!` from the same list as the enum, so it holds
            /// every variant by construction: there is no second list to
            /// forget to extend.
            pub fn all() -> &'static [ListReason] {
                &[$(ListReason::$variant,)+]
            }
        }
    };
}

list_reasons! {
    /// Nothing was asked: the default.
    NotRecorded,
    /// The package is not installed under this provider, so nothing it ships
    /// and nowhere it would land can be read.
    NotInstalled,
    /// Installed but not active: no read is made (F.md SR2-N5).
    Inactive,
    /// A cask: no allow-listed read gives its file list (R-21).
    CaskNotCovered,
    /// Several installed kegs and no linked one, so which keg a listing would
    /// describe is not known (F.md SR-10). No read is made.
    AmbiguousKeg,
    /// More than [`MAX_EXECUTABLES`] executables.
    TooLarge,
    /// A listed entry failed [`ExecutableName::parse`]. The whole list is
    /// unknown, never a list with the entry removed.
    NameClass,
    /// A valid listing with no line under the provider's detected prefix
    /// (F.md SR2-N2).
    NothingUnderPrefix,
    /// One leaf name sits directly under both `bin` and `sbin`, so which one a
    /// shell reaches depends on the `PATH` order of the two, and a single
    /// directory cannot be stated for it.
    AmbiguousDirectory,
    /// Installed, not kept off the linked directories by the provider, and
    /// not linked either: whether its names reach `PATH` is not established
    /// (a placement reason only).
    NotLinkedUnestablished,
    /// Several installed versions of one package, and which one the
    /// provider's listing read would describe has not been observed
    /// (`docs/spec/09-research-checklist.md`, R-24). No read is made. The
    /// provider-neutral counterpart of [`ListReason::AmbiguousKeg`].
    AmbiguousInstalledVersion,
    /// A placement only: the adapter placed an executable in a `PATH` entry,
    /// and the capture's own search of `PATH` contradicts it - nothing was
    /// found there, or it was found only further along (F.md SR L-1: an
    /// inference such as "the linked keg's names are linked" is checked
    /// against what resolved, never trusted over it).
    ContradictedByResolution,
}

/// Why an [`ExecutableList`] or a [`Placement`] is unknown: a reason the
/// adapter names, or the [`ErrorId`] its read failed with. Serialised as the
/// bare identifier either way (`"INACTIVE"`, `"PROVIDER_TIMEOUT"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UnknownReason {
    /// A reason of the adapter's own.
    Listed(ListReason),
    /// The read answered with this error.
    Read(ErrorId),
}

impl From<ListReason> for UnknownReason {
    fn from(value: ListReason) -> Self {
        UnknownReason::Listed(value)
    }
}

impl From<ErrorId> for UnknownReason {
    fn from(value: ErrorId) -> Self {
        UnknownReason::Read(value)
    }
}

/// The executables a package ships, from its provider's own file list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutableList {
    /// Never asked.
    #[default]
    NotRecorded,
    /// Asked, and not established.
    Unknown {
        /// Why.
        reason: UnknownReason,
    },
    /// Asked and answered. Empty only from a non-empty, fully parsed listing
    /// with no direct `bin`/`sbin` child (F.md SR2-N1).
    Known {
        /// The names, ordered.
        executables: BTreeSet<ExecutableName>,
    },
}

impl ExecutableList {
    /// An unknown list.
    pub fn unknown(reason: impl Into<UnknownReason>) -> Self {
        ExecutableList::Unknown {
            reason: reason.into(),
        }
    }

    /// The list a fully parsed listing produced, from the leaf names it found.
    ///
    /// All or nothing: one name that fails [`ExecutableName::parse`] makes the
    /// whole list [`ListReason::NameClass`], and more than
    /// [`MAX_EXECUTABLES`] distinct names make it [`ListReason::TooLarge`].
    pub fn from_leaves<'a>(leaves: impl IntoIterator<Item = &'a str>) -> Self {
        let mut executables = BTreeSet::new();
        for leaf in leaves {
            let Ok(name) = ExecutableName::parse(leaf) else {
                return Self::unknown(ListReason::NameClass);
            };
            executables.insert(name);
            if executables.len() > MAX_EXECUTABLES {
                return Self::unknown(ListReason::TooLarge);
            }
        }
        ExecutableList::Known { executables }
    }

    /// The names, when the list is known.
    pub fn known(&self) -> Option<&BTreeSet<ExecutableName>> {
        match self {
            ExecutableList::Known { executables } => Some(executables),
            ExecutableList::NotRecorded | ExecutableList::Unknown { .. } => None,
        }
    }

    /// Whether nobody asked: what a snapshot leaves out.
    pub fn is_not_recorded(&self) -> bool {
        matches!(self, ExecutableList::NotRecorded)
    }
}

/// Where an installed package's executables land, filled by each adapter from
/// detection only (F.md §1, SG-1). The absence of a field never means "on
/// `PATH`".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Placement {
    /// Each executable is reachable in the named directory - the detected
    /// prefix's `bin` or `sbin` - per executable, because a package can ship
    /// into both.
    OnPathDir {
        /// Executable to the directory it lands in, ordered.
        dirs: BTreeMap<ExecutableName, String>,
    },
    /// The provider keeps this package off its linked directories (a
    /// keg-only formula, for one), with the provider's stated reason.
    NotLinked {
        /// The provider's reason, as it states it.
        reason: String,
    },
    /// Not established.
    Unknown {
        /// Why.
        reason: UnknownReason,
    },
}

impl Default for Placement {
    fn default() -> Self {
        Placement::unknown(ListReason::NotRecorded)
    }
}

impl Placement {
    /// An unknown placement.
    pub fn unknown(reason: impl Into<UnknownReason>) -> Self {
        Placement::Unknown {
            reason: reason.into(),
        }
    }

    /// Whether nobody asked: the default, which a snapshot leaves out.
    pub fn is_not_recorded(&self) -> bool {
        *self == Placement::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::CHARACTER_SAMPLES;

    #[test]
    fn executable_name_refuses_a_slash_a_dot_dot_a_leading_dash_and_every_hostile_class() {
        for (bad, why) in [
            ("", NameRefusal::Empty),
            ("bin/rg", NameRefusal::Separator),
            ("/rg", NameRefusal::Separator),
            ("rg/", NameRefusal::Separator),
            (".", NameRefusal::DotOrDotDot),
            ("..", NameRefusal::DotOrDotDot),
            ("-rf", NameRefusal::LeadingDash),
            ("--version", NameRefusal::LeadingDash),
            ("rg\0", NameRefusal::Hostile),
            ("rg\n", NameRefusal::Hostile),
            ("r\u{202e}g", NameRefusal::Hostile),
            ("r\u{200b}g", NameRefusal::Hostile),
            ("r\u{2028}g", NameRefusal::Hostile),
            ("r\u{fffd}g", NameRefusal::Replacement),
        ] {
            assert_eq!(ExecutableName::parse(bad), Err(why), "{bad:?}");
        }
        assert_eq!(
            ExecutableName::parse(&"a".repeat(256)),
            Err(NameRefusal::TooLong)
        );
        // Every character the shared table classes as hostile is refused, so
        // the name class cannot drift from `HostileClass`.
        for &(character, class) in CHARACTER_SAMPLES {
            if character == '/' {
                continue;
            }
            let name = format!("a{character}b");
            assert_eq!(
                ExecutableName::parse(&name).is_ok(),
                class.is_none(),
                "{:?}",
                character.escape_unicode().to_string()
            );
        }
        for good in [
            "rg",
            "glibtool",
            "pkg-config",
            "7z",
            "python3.13",
            "c++",
            "..x",
            "x-",
            "lzcat",
        ] {
            assert_eq!(
                ExecutableName::parse(good).map(|n| n.as_str().to_string()),
                Ok(good.to_string())
            );
        }
        assert!(ExecutableName::parse(&"a".repeat(255)).is_ok());
    }

    #[test]
    fn a_bad_name_fails_to_deserialise_rather_than_loading() {
        assert!(serde_json::from_str::<ExecutableName>("\"rg\"").is_ok());
        for bad in ["\"\"", "\"a/b\"", "\"..\"", "\"-x\"", "\"a\\u0000\""] {
            assert!(
                serde_json::from_str::<ExecutableName>(bad).is_err(),
                "{bad}"
            );
        }
        let list = r#"{"state":"KNOWN","executables":["rg","../x"]}"#;
        assert!(serde_json::from_str::<ExecutableList>(list).is_err());
    }

    #[test]
    fn one_bad_entry_makes_the_whole_list_unknown() {
        assert_eq!(
            ExecutableList::from_leaves(["rg", "-x", "fd"]),
            ExecutableList::unknown(ListReason::NameClass),
            "never a list with the entry removed"
        );
        assert_eq!(
            ExecutableList::from_leaves(["rg", "a\u{202e}b"]),
            ExecutableList::unknown(ListReason::NameClass)
        );
        let known = ExecutableList::from_leaves(["rg", "fd", "rg"]);
        assert_eq!(
            known
                .known()
                .unwrap()
                .iter()
                .map(ExecutableName::as_str)
                .collect::<Vec<_>>(),
            vec!["fd", "rg"]
        );
    }

    #[test]
    fn more_than_the_cap_is_too_large() {
        let names: Vec<String> = (0..=MAX_EXECUTABLES).map(|i| format!("x{i}")).collect();
        assert_eq!(
            ExecutableList::from_leaves(names.iter().map(String::as_str)),
            ExecutableList::unknown(ListReason::TooLarge)
        );
        assert!(
            ExecutableList::from_leaves(names[..MAX_EXECUTABLES].iter().map(String::as_str))
                .known()
                .is_some()
        );
    }

    #[test]
    fn an_empty_known_list_and_not_recorded_are_different_answers() {
        let empty = ExecutableList::from_leaves(std::iter::empty());
        assert_eq!(
            empty,
            ExecutableList::Known {
                executables: BTreeSet::new()
            }
        );
        assert_ne!(empty, ExecutableList::NotRecorded);
        assert_ne!(empty, ExecutableList::unknown(ListReason::NotInstalled));
        assert_eq!(ExecutableList::default(), ExecutableList::NotRecorded);
        assert_eq!(ExecutableList::NotRecorded.known(), None);
        assert_eq!(
            serde_json::to_string(&empty).unwrap(),
            r#"{"state":"KNOWN","executables":[]}"#
        );
        assert_eq!(
            serde_json::to_string(&ExecutableList::NotRecorded).unwrap(),
            r#"{"state":"NOT_RECORDED"}"#
        );
    }

    #[test]
    fn the_serialised_shapes_are_tagged_by_state() {
        assert_eq!(
            serde_json::to_string(&ExecutableList::unknown(ListReason::Inactive)).unwrap(),
            r#"{"state":"UNKNOWN","reason":"INACTIVE"}"#
        );
        assert_eq!(
            serde_json::to_string(&ExecutableList::unknown(ErrorId::ProviderTimeout)).unwrap(),
            r#"{"state":"UNKNOWN","reason":"PROVIDER_TIMEOUT"}"#
        );
        let placement = Placement::OnPathDir {
            dirs: [(
                ExecutableName::parse("rg").unwrap(),
                "/opt/local/bin".to_string(),
            )]
            .into_iter()
            .collect(),
        };
        let json = serde_json::to_string(&placement).unwrap();
        assert_eq!(
            json,
            r#"{"state":"ON_PATH_DIR","dirs":{"rg":"/opt/local/bin"}}"#
        );
        assert_eq!(serde_json::from_str::<Placement>(&json).unwrap(), placement);
        assert_eq!(
            serde_json::to_string(&Placement::default()).unwrap(),
            r#"{"state":"UNKNOWN","reason":"NOT_RECORDED"}"#
        );
        for reason in [
            UnknownReason::Listed(ListReason::NothingUnderPrefix),
            UnknownReason::Read(ErrorId::ProviderOutputChanged),
        ] {
            let text = serde_json::to_string(&reason).unwrap();
            assert_eq!(
                serde_json::from_str::<UnknownReason>(&text).unwrap(),
                reason
            );
        }
    }

    #[test]
    fn no_reason_collides_with_an_error_id() {
        // `UnknownReason` is untagged: a reason that serialised like an error
        // id would read back as the other one.
        let ids: BTreeSet<String> = ErrorId::all()
            .iter()
            .map(|id| serde_json::to_string(id).unwrap())
            .collect();
        // From `all()`, which an exhaustive `match` keeps whole, so a reason
        // added later is checked without editing this test.
        for reason in ListReason::all() {
            let text = serde_json::to_string(reason).unwrap();
            assert!(!ids.contains(&text), "{text} is also an error id");
            assert_eq!(
                serde_json::from_str::<UnknownReason>(&text).unwrap(),
                UnknownReason::Listed(*reason)
            );
        }
    }

    #[test]
    fn known_is_none_for_unknown_and_not_recorded() {
        // An unknown list answered as an empty set would read as "ships no
        // executable" - the vacuous NoExecutables (SR2-N1).
        assert_eq!(ExecutableList::unknown(ListReason::Inactive).known(), None);
        assert_eq!(
            ExecutableList::unknown(ErrorId::ProviderTimeout).known(),
            None
        );
        assert_eq!(ExecutableList::NotRecorded.known(), None);
        assert_eq!(
            ExecutableList::from_leaves(std::iter::empty()).known(),
            Some(&BTreeSet::new())
        );
    }

    #[test]
    fn list_reason_all_is_the_whole_enum_without_duplicates() {
        // `all()` and the enum come from one `list_reasons!` list, so
        // completeness holds by construction; what is pinned here is the
        // serialised spelling of every reason, which is a compatibility
        // surface and must not move with the macro.
        let distinct: BTreeSet<_> = ListReason::all().iter().collect();
        assert_eq!(distinct.len(), ListReason::all().len());
        let spelled: Vec<String> = ListReason::all()
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect();
        assert_eq!(
            spelled,
            [
                "\"NOT_RECORDED\"",
                "\"NOT_INSTALLED\"",
                "\"INACTIVE\"",
                "\"CASK_NOT_COVERED\"",
                "\"AMBIGUOUS_KEG\"",
                "\"TOO_LARGE\"",
                "\"NAME_CLASS\"",
                "\"NOTHING_UNDER_PREFIX\"",
                "\"AMBIGUOUS_DIRECTORY\"",
                "\"NOT_LINKED_UNESTABLISHED\"",
                "\"AMBIGUOUS_INSTALLED_VERSION\"",
                "\"CONTRADICTED_BY_RESOLUTION\"",
            ]
        );
    }
}
