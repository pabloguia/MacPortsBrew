// SPDX-License-Identifier: Apache-2.0

//! Snapshots (`02-domain-model.md`, "Snapshot"): a complete, serialisable
//! description of the machine and both providers, sufficient to reproduce an
//! analysis offline. The input to `analyze --from-snapshot` (INV-15), the
//! basis of every fixture, and the restore safety net of later passes.

use crate::machine::Machine;
use crate::provider::{ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory};
use crate::resolution::Resolution;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The snapshot schema version. Loading a different major fails loudly.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Everything captured about one provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSnapshot {
    /// Detection result.
    pub detection: ProviderDetection,
    /// Capability table.
    pub capabilities: ProviderCapabilities,
    /// The inventory. Empty when the provider is absent or broken.
    pub inventory: ProviderInventory,
    /// Reported environment (paths, variables the prefix implies). Names
    /// facts about the installation; not what the executor sets.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
    /// The sanitised environment the adapter invokes the provider with
    /// (INV-10): built from scratch, provider-quieting variables set,
    /// build-flag variables absent. A plan command carries exactly this.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub invocation_environment: BTreeMap<String, String>,
    /// Resolutions of the executables the inventory names, keyed by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resolutions: BTreeMap<String, Resolution>,
}

/// The shell environment as observed, redacted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    /// `PATH` entries in order, with the home directory replaced by `~`.
    #[serde(default)]
    pub path: Vec<String>,
    /// The build-affecting variables `05-environment` says to report, when
    /// set, redacted the same way. Values of other variables are never
    /// captured.
    #[serde(default)]
    pub reported: BTreeMap<String, String>,
    /// What was wrong with the `PATH` entries at capture time, in entry
    /// order.
    ///
    /// Recorded rather than recomputed: deciding that a directory is
    /// world-writable means asking a filesystem, and the filesystem that has
    /// the answer is the captured machine's, not the one replaying the
    /// snapshot later (INV-15). Whoever captures fills this in; everything
    /// downstream reads it.
    #[serde(default)]
    pub path_findings: Vec<PathFinding>,
}

/// What is wrong with one `PATH` entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PathFindingKind {
    /// The same entry appeared earlier. Harmless in itself, but it hides which
    /// copy actually resolves and it is what a duplicated shell block leaves
    /// behind (INV-16).
    Duplicate,
    /// The entry is not an absolute path, so it could not be inspected. The
    /// shell resolves it against the working directory, which makes what runs
    /// depend on where the user stands — the shadowing case in the threat
    /// model. An empty entry (`::`, or a trailing `:`) is this finding: the
    /// shell reads it as the working directory. A `~` entry with no home to
    /// expand it against lands here too.
    Relative,
    /// No directory exists at the entry, or what is there is not a directory.
    Missing,
    /// Every user on the machine may create files in the directory, so anybody
    /// may decide what `git` means for this user.
    WorldWritable,
    /// Every member of the directory's group may create files in it. This is
    /// the ordinary macOS case rather than the exotic one: `/usr/local` and
    /// `/usr/local/bin` are commonly `root:admin` and group-writable, which is
    /// where Homebrew's Intel prefix lives, so every admin account on the
    /// machine can place a binary there.
    GroupWritable,
}

/// One finding about one `PATH` entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathFinding {
    /// The entry as it appears in `PATH`, redacted.
    pub entry: String,
    /// Its zero-based position, so a caller can reason about order.
    pub index: usize,
    /// What is wrong with it.
    pub kind: PathFindingKind,
}

/// Replaces the home directory with `~`, wherever it appears.
///
/// `06-cli-and-ux.md` ("Privacy and diagnostics") forbids home paths and
/// usernames in anything the user might attach to an issue, and a captured
/// snapshot is exactly that. Redaction happens at capture, so a home path is
/// never written down in the first place.
///
/// Every occurrence is replaced, not just a leading one: a `PATH` entry is a
/// whole path, but `CPPFLAGS` is `-I/Users/ada/.local/include` and
/// `PKG_CONFIG_PATH` is a colon-separated list, and a leading-only rule would
/// let the home path through in both. The match must end a path component, so
/// `/Users/adamant` is not inside `/Users/ada`. Where the two rules disagree
/// this errs toward redacting: over-redaction costs a reader some context,
/// under-redaction publishes a username.
///
/// `06-cli-and-ux.md` names usernames separately from home paths, so the home
/// directory's own last component is then replaced wherever it appears as a
/// whole component — `/Volumes/build/ada/include` keeps a username that
/// redacting the home path alone would miss. This matches what
/// `scripts/capture-fixture.sh` does with `id -un`, so the two capture routes
/// have the same privacy strength.
///
/// It lives in the domain rather than in an adapter because two layers redact:
/// the system adapter, for the environment it reports, and the application's
/// capture, for every path a provider hands back. One implementation, so the
/// two cannot drift into disagreeing about what a home path is.
pub fn redact_home(value: &str, home: &HomeRedaction) -> String {
    match home {
        HomeRedaction::Known(home) => redact_known_home(value, home),
        HomeRedaction::Unresolved => redact_home_by_shape(value),
        HomeRedaction::Recorded => value.to_string(),
    }
}

/// Why a home directory is, or is not, available to redact against.
///
/// This used to be an `Option<&Path>`, and could not carry the distinction that
/// matters: `None` meant both *"a replayed recording, redacted when it was
/// captured"* and *"a live run whose `HOME` is unset"*. The two need opposite
/// treatment, and giving the second the first's treatment published the user's
/// home path — measured at 39 occurrences in one `doctor --report` run, username
/// included, in a file `06-cli-and-ux.md` invites them to attach to an issue.
///
/// Making it a type rather than a boolean is deliberate: the reason is what
/// decides the behaviour, so the reason is what the call site must state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HomeRedaction {
    /// The home directory is known. Redact it, and the user name with it.
    Known(PathBuf),
    /// A live run whose home could not be resolved — `HOME` unset, or set to a
    /// relative path. Redact by shape: publishing the path is the worse failure,
    /// and over-redacting a path that merely looks like a home costs nothing.
    Unresolved,
    /// A replayed recording. It was redacted when it was captured, and redacting
    /// it again against *this* machine would let a host fact reach a replayed
    /// analysis (INV-15), so the right answer is to leave it exactly as it is.
    Recorded,
}

impl HomeRedaction {
    /// The policy for a live run, given whatever `HOME` yielded.
    pub fn live(home: Option<PathBuf>) -> Self {
        home.map_or(Self::Unresolved, Self::Known)
    }

    /// How a reader should read the redaction of anything produced under this
    /// policy. Three states, deliberately not a boolean: `Recorded` is not
    /// grounded, it is *unknown* — a recording or a snapshot file carries no
    /// record of how it was redacted when it was captured, so a run reading one
    /// cannot claim the home was known. Collapsing that into "grounded" is what
    /// a boolean did, and it produced a confident false claim on exactly the
    /// path the leak came from: a snapshot captured with `HOME` unset is
    /// redacted heuristically, and a later `--from-snapshot --report` stamped it
    /// as grounded.
    ///
    /// `SCREAMING_SNAKE_CASE` per `docs/schemas/README.md`.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Known(_) => "GROUNDED",
            Self::Unresolved => "HEURISTIC",
            Self::Recorded => "AS_CAPTURED",
        }
    }

    /// The home directory itself, for the callers that need a real path rather
    /// than a redaction rule — expanding a leading `~` before a directory can be
    /// inspected is the one such caller. `Unresolved` and `Recorded` both yield
    /// `None`, and correctly: there is no path to expand against in either.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Known(home) => Some(home),
            Self::Unresolved | Self::Recorded => None,
        }
    }
}

fn redact_known_home(value: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    if home.is_empty() {
        return value.to_string();
    }
    let redacted = replace_component(value, home, "~");
    match home.rsplit('/').next().filter(|u| u.len() > 1) {
        Some(user) => replace_component(&redacted, user, "USER"),
        None => redacted,
    }
}

/// Directories under `/Users` that macOS creates and that name no user.
///
/// `Deleted Users` is deliberately absent: `is_component_break` treats the space
/// as a boundary, so the name extracted from `/Users/Deleted Users/bob/...` is
/// `Deleted`, and listing the two-word form here matched nothing while the real
/// user name went unredacted. It is handled by the space check in
/// [`redact_home_by_shape`] instead.
const NOT_A_USER_HOME: &[&str] = &["Shared", "Guest", "Localized"];

/// Names that are ordinary path components far more often than they are user
/// names. The bare-name pass rewrites a name *everywhere* in a value, so a home
/// called `local` would turn `/opt/local/bin` — the MacPorts prefix, the single
/// most load-bearing fact in a diagnostic report — into `/opt/USER/bin`. The
/// home path itself is still redacted for these; only the bare-name pass is
/// skipped, which is the trade that keeps the report readable without
/// publishing the home.
const AMBIGUOUS_AS_A_BARE_NAME: &[&str] = &[
    "local",
    "opt",
    "bin",
    "sbin",
    "lib",
    "libexec",
    "usr",
    "var",
    "etc",
    "tmp",
    "share",
    "src",
    "include",
    "Library",
    "Applications",
    "System",
    "Users",
    "Volumes",
    "Developer",
    "Public",
];

/// Redacts home paths by their shape, for the case where the home directory
/// could not be resolved at all.
///
/// `redact_home` used to return the value unchanged when `home` was `None`,
/// which conflated two situations that are not alike. Under `--replay` the home
/// is deliberately absent because a recording carries no live paths, and doing
/// nothing is right. On a live run the home is absent whenever `HOME` is unset
/// or relative — routine under `cron`, `launchd` and some CI — and doing
/// nothing published the user's home path into a file
/// `06-cli-and-ux.md` ("Privacy and diagnostics") invites them to attach to an
/// issue. Measured before this existed: a `doctor --report` run with `HOME`
/// unset carried 39 occurrences of the real path, username included.
///
/// So the fallback redacts what it can still recognise without being told:
/// `/Users/<name>` is where macOS puts a home, and the name is then redacted as
/// a whole component the same way the grounded path does it. It is a heuristic
/// and it fails in the safe direction — it over-redacts a path that merely looks
/// like a home, and it cannot see a home somewhere else entirely, which is why
/// the diagnostic report also records which of the two redactions it applied
/// rather than letting the reader assume.
fn redact_home_by_shape(value: &str) -> String {
    const ROOT: &str = "/Users/";
    // A bound on distinct names, not on the value: the loop below is linear in
    // the input, but each name costs two more passes over the whole string, so
    // an adversarial value full of distinct `/Users/<name>` segments would be
    // quadratic. Provider output is untrusted and bounded only at 64 MiB, and a
    // value with more than this many distinct home names is not a value whose
    // legibility is worth defending.
    const MAX_NAMES: usize = 32;
    let mut names: BTreeSet<&str> = BTreeSet::new();
    let mut rest = value;
    while let Some(at) = rest.find(ROOT) {
        // No leading-boundary test. `ROOT` begins with `/`, so it carries its
        // own boundary — the same reasoning `replace_component` documents — and
        // requiring another one before it would miss `-I/Users/ada/.local`,
        // which is what a compiler-flag variable actually looks like and is
        // exactly the shape the grounded path was built to catch.
        let after = &rest[at + ROOT.len()..];
        let end = after.find(is_component_break).unwrap_or(after.len());
        let name = &after[..end];
        // A space ends the extracted name but not the component, so `Deleted
        // Users` and any other multi-word directory would yield a fragment.
        // Rewriting a fragment mangles the path and leaves the real name behind,
        // which is worse than leaving it alone and reporting `HEURISTIC`.
        let multi_word = after[end..].starts_with(' ');
        if !name.is_empty()
            && !multi_word
            && !NOT_A_USER_HOME.contains(&name)
            && names.len() < MAX_NAMES
        {
            names.insert(name);
        }
        rest = after;
    }
    let mut redacted = value.to_string();
    for name in names {
        // The home path always goes, whatever the name is: a one-character home
        // is still a home, and the length filter below is about the *bare-name*
        // pass only — which is where `redact_known_home` applies it too.
        redacted = replace_component(&redacted, &format!("{ROOT}{name}"), "~");
        if name.len() > 1 && !AMBIGUOUS_AS_A_BARE_NAME.contains(&name) {
            redacted = replace_component(&redacted, name, "USER");
        }
    }
    redacted
}

/// Replaces `needle` with `replacement` wherever `needle` is a whole path
/// component, so a longer name that merely contains it survives:
/// `/Users/adamant` is not `/Users/ada`, and `/opt/prada` is not the user
/// `ada`.
///
/// A needle that starts with `/` carries its own leading boundary, which is
/// what lets `-I/Users/ada/x` be redacted even though `I` precedes the match.
/// A bare name needs a boundary on both sides.
fn replace_component(value: &str, needle: &str, replacement: &str) -> String {
    let needs_leading_boundary = !needle.starts_with('/');
    let mut redacted = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find(needle) {
        let before_ok = !needs_leading_boundary
            || rest[..at]
                .chars()
                .next_back()
                .is_none_or(is_component_break);
        let after = &rest[at + needle.len()..];
        let after_ok = after.chars().next().is_none_or(is_component_break);
        if before_ok && after_ok {
            redacted.push_str(&rest[..at]);
            redacted.push_str(replacement);
            rest = after;
        } else {
            let consumed = at + needle.len();
            redacted.push_str(&rest[..consumed]);
            rest = &rest[consumed..];
        }
    }
    redacted.push_str(rest);
    redacted
}

/// True for a character that cannot be part of the same path component.
fn is_component_break(c: char) -> bool {
    !c.is_alphanumeric() && c != '_' && c != '-' && c != '.'
}

/// Provenance of a capture (`07-quality-and-testing.md`, "Fixtures").
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    /// Capture timestamp, RFC 3339 UTC.
    pub captured_at: String,
    /// The command that captured it.
    pub captured_by: String,
    /// `captured` from a live machine, or `synthesised` from documentation
    /// with the note that says so.
    pub origin: String,
    /// Free-text note (which machine, which documents).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Schema version.
    pub schema_version: u32,
    /// MacPortsBrew version that captured it.
    pub tool_version: String,
    /// Policy version in effect at capture.
    pub policy_version: String,
    /// Provenance.
    pub provenance: Provenance,
    /// The machine.
    pub machine: Machine,
    /// The shell environment, redacted.
    #[serde(default)]
    pub environment: EnvironmentSnapshot,
    /// One entry per provider the tool knows, present or not.
    pub providers: BTreeMap<ProviderId, ProviderSnapshot>,
}

impl Snapshot {
    /// Fails when the schema version is not the one this build reads.
    pub fn check_schema(&self) -> Result<(), crate::error::DomainError> {
        if self.schema_version == SNAPSHOT_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(crate::error::DomainError::new(
                crate::error::ErrorId::SnapshotUnusable,
                format!(
                    "snapshot schema version {} is not the supported {}",
                    self.schema_version, SNAPSHOT_SCHEMA_VERSION
                ),
            ))
        }
    }
}
