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

/// The longest path this tool will accept anywhere.
pub const MAX_PATH_LEN: usize = 4096;

/// The system directories every sanitised `PATH` ends with, in order.
///
/// In the domain because three places need the same list: the executor builds a
/// `PATH` from it, and the snapshot reader checks one against it. A second copy
/// is a second definition of what a clean `PATH` is.
pub const SYSTEM_PATH: &[&str] = &["/usr/bin", "/bin", "/usr/sbin", "/sbin"];

/// The variables every sanitised environment carries, with the only values they
/// are ever given. `PATH` and `HOME` are not here: one is checked by shape, the
/// other is a path this tool does not choose.
pub const PINNED_ENVIRONMENT: &[(&str, &str)] = &[
    ("LANG", "C"),
    ("LC_ALL", "C"),
    ("TERM", "dumb"),
    ("NO_COLOR", "1"),
];

fn path_is_sane(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_PATH_LEN
        && !text.chars().any(char::is_control)
        && !Path::new(text)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// Whether a path is one this tool will hand to the machine: absolute, bounded,
/// no `..`, no control characters.
///
/// `..` matters more than it looks. A component-wise prefix comparison does not
/// normalise it, so `/opt/../opt/homebrew/bin` is not "under `/opt/homebrew`"
/// by any structural test while the kernel resolves it there. Rejecting the
/// component outright is what makes a prefix comparison mean anything.
pub fn is_safe_path(path: &Path) -> bool {
    path.to_str()
        .is_some_and(|text| text.starts_with('/') && path_is_sane(text))
}

/// The same rule for a path read back from a capture, where redaction has
/// replaced the home directory with `~` ([`redact_home`]). Such a path is
/// absolute on the machine it came from; refusing it would make a snapshot
/// taken on a Homebrew-under-`$HOME` install unreadable by the tool that wrote
/// it.
pub fn is_safe_captured_path(text: &str) -> bool {
    (text.starts_with('/') || text == "~" || text.starts_with("~/")) && path_is_sane(text)
}

/// Every prefix a provider other than `id` is known to own or has declared.
fn taken_prefixes<'a>(
    contracts: &'a BTreeMap<String, ProviderEnvironmentContract>,
    declared: &'a [(&'a str, &'a Path)],
    id: &'a str,
) -> impl Iterator<Item = (&'a str, &'a Path)> {
    contracts
        .iter()
        .filter(move |(other, _)| other.as_str() != id)
        .flat_map(|(other, c)| {
            c.prefixes
                .iter()
                .map(move |p| (other.as_str(), p.as_path()))
        })
        .chain(declared.iter().filter(move |(o, _)| *o != id).copied())
}

/// Whether `path` lies under `prefix`, comparing components case-insensitively.
///
/// The boot volume is case-insensitive APFS by default: `/OPT/Local/bin` and
/// `/opt/local/bin` are the same directory, so a byte comparison says
/// "different prefix" while the kernel opens the same file. (`realpath` *does*
/// fold, but nothing here calls it: this comparison is lexical by design, so it
/// can be decided from the snapshot alone.) That is the same class of defeat as a `..` component - the string and
/// the machine disagreeing - and it is closed the same way, by refusing to let
/// the comparison be the looser of the two.
pub fn path_starts_with_insensitive(path: &Path, prefix: &Path) -> bool {
    let fold = |p: &Path| -> Vec<String> {
        p.components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect()
    };
    let (path, prefix) = (fold(path), fold(prefix));
    prefix.len() <= path.len() && path[..prefix.len()] == prefix[..]
}

/// What one provider's adapter could have produced, as the layer that knows
/// providers describes it.
///
/// The domain names no provider, so it cannot know where Homebrew lives or that
/// MacPorts adds no variables of its own. It is handed those facts and applies
/// the rule to them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderEnvironmentContract {
    /// Directories this provider's `PATH` may lead with - its detected prefix
    /// and the prefixes the registry knows for it. Taking the *known* ones from
    /// code and not only the detected one from the file matters: a snapshot
    /// that understates its own prefix would otherwise widen what it may say.
    pub prefixes: Vec<PathBuf>,
    /// Variables this provider's adapter adds beyond the pinned ones, with the
    /// values it sets them to.
    ///
    /// A map and not a set, because permitting a key by name is not equality:
    /// `HOMEBREW_NO_AUTO_UPDATE` is what makes a read-only command read-only,
    /// and a snapshot that set it to `"0"` was accepted while the rule claimed
    /// to assert what the builder emits.
    pub extra: BTreeMap<String, String>,
}

/// Environment variables that make one prefix visible to another's build
/// system. INV-10 names five of them - its sixth, `PATH`, is reserved rather
/// than stripped - and then says "or equivalents"; this is the equivalents, in
/// one place.
///
/// It lives in `domain` because it is a provider-neutral fact about build
/// systems, not about either provider - and because it had begun to exist in
/// three places at once. `adapters::process::environment` filters on it and
/// `adapters::system::machine` kept a reporting superset in step with it by
/// hand, while the INV-10 test named just one of these variables and a `DYLD_`
/// prefix - so a leak through `CXXFLAGS` passed the test written to catch
/// exactly that.
pub fn is_build_flag(key: &str) -> bool {
    matches!(
        key,
        "CPATH"
            | "C_INCLUDE_PATH"
            | "CPLUS_INCLUDE_PATH"
            | "LIBRARY_PATH"
            | "LD_LIBRARY_PATH"
            | "PKG_CONFIG_PATH"
            | "PKG_CONFIG_LIBDIR"
            | "LDFLAGS"
            | "CPPFLAGS"
            | "CFLAGS"
            | "CXXFLAGS"
            | "OBJCFLAGS"
            | "ACLOCAL_PATH"
            | "CMAKE_PREFIX_PATH"
    ) || key.starts_with("DYLD_")
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
    /// Refuses a snapshot stating an invocation environment no adapter could
    /// have built.
    ///
    /// `sanitised_environment` builds a clean environment, and that was taken
    /// to mean a clean environment is what a command gets. It is not, by two
    /// routes. `invocation_environment` is a deserialised field, so
    /// `--from-snapshot` can state outright what a `brew` or `port` command
    /// runs in (F-21). And the *constructed* route is no safer: the leading
    /// `PATH` is `<prefix>/bin`, the prefix is what `brew --prefix` answered,
    /// and that answer comes from a recording under `--replay` and from the
    /// provider on a live run - untrusted in both cases by the threat model.
    /// `sanitised_environment` cannot notice, because it never sees the other
    /// provider. So this runs on every snapshot, captured or loaded.
    ///
    /// The remedy is to refuse rather than to rewrite: re-sanitising would make
    /// the analysis stop being a function of the snapshot, which is the
    /// property `--from-snapshot` exists to have, and it would conceal the
    /// tampering.
    ///
    /// **What is asserted is what the builder emits**, not the absence of things
    /// to fear. Three security reviews are the reason, and the lesson had the
    /// same shape each time: every defeat after the first was a place where the
    /// rule was *wider* than the builder. The
    /// first version banned build-flag keys and the other provider's prefix,
    /// and was defeated four ways in minutes - `SUDO_ASKPASS` and `RUBYOPT` are
    /// neither and both name a program the provider will run; `/tmp/evil/bin`
    /// first on `PATH` is neither; `/opt/../opt/homebrew/bin` slips a
    /// component-wise prefix test while the kernel resolves it where you would
    /// expect. The second version asserted a shape, and every remaining defeat
    /// was a place where the shape asserted was *wider* than the shape built:
    /// `/OPT/Local` compares unequal and opens the same file, because the boot
    /// volume is case-insensitive; a `HOME` of the attacker's choosing makes
    /// `brew` read `$HOME/.homebrew/brew.env` and export `SUDO_ASKPASS` and
    /// every `HOMEBREW_*` from it, reinstating the whole refused key set; and
    /// `/usr/local/bin`, group-writable on any Mac, is "under a known Homebrew
    /// prefix". So the rule is now: exactly the variables the builder sets,
    /// exactly the values it sets them to, and a `PATH` of this provider's own
    /// directories followed by the system ones. The third round added the two
    /// that mattered most: a quieting variable was permitted by name, so a
    /// snapshot could set `HOMEBREW_NO_AUTO_UPDATE=0` and turn a read-only
    /// command into one that writes; and a prefix shallow enough to contain
    /// other things - `/opt`, or `/` - made every rule below vacuous, which was
    /// a working bypass of the whole check.
    ///
    /// Decided from the snapshot and the contract alone - no clock, no
    /// filesystem, no live environment - so a snapshotted read stays
    /// reproducible (INV-15).
    pub fn check_invocation_environments(
        &self,
        contracts: &BTreeMap<String, ProviderEnvironmentContract>,
    ) -> Result<(), crate::error::DomainError> {
        let refuse = |detail: String| {
            Err(crate::error::DomainError::new(
                crate::error::ErrorId::SnapshotUnusable,
                detail,
            ))
        };
        // Every prefix any provider lays claim to, before any of them is
        // trusted. Collected first because a provider that legitimately lives
        // somewhere the registry does not know - Homebrew under `$HOME` - would
        // otherwise be unprotected against another provider claiming the same
        // place.
        let declared: Vec<(&str, &Path)> = self
            .providers
            .iter()
            .filter_map(|(id, p)| p.detection.prefix.as_deref().map(|d| (id.as_str(), d)))
            .collect();

        for (id, provider) in &self.providers {
            let environment = &provider.invocation_environment;
            // A provider whose capture failed states nothing, because the
            // adapter never got far enough to build one. Accepting that is not
            // a hole - the planner emits no command for a provider that is not
            // present - and refusing it would make the tool reject a snapshot
            // it had just written, on exactly the broken machine someone
            // captures a snapshot from in order to get help.
            if environment.is_empty()
                && provider.detection.presence != crate::provider::ProviderPresence::Present
            {
                continue;
            }
            let contract = contracts.get(id.as_str()).cloned().unwrap_or_default();

            // Where this provider is allowed to be. A declared prefix is
            // honoured - a provider may install anywhere - but never on top of
            // somewhere another provider is known to live or has claimed, and
            // never in a shape the kernel reads differently from the way it
            // compares.
            let mut own: Vec<PathBuf> = Vec::new();
            if let Some(detected) = provider.detection.prefix.as_deref() {
                let text = detected.to_string_lossy().into_owned();
                if !is_safe_captured_path(&text) {
                    return refuse(format!(
                        "{id} declares a prefix that is not a usable directory ({text:?})"
                    ));
                }
                // A prefix shallow enough to contain other things is how the
                // containment rule below gets switched off: declare `/opt` and
                // the other provider's whole tree is "mine"; declare `/` and
                // everything is. Every real prefix names a directory of its
                // own, so requiring two components and forbidding an ancestor
                // relationship costs nothing and closes the family.
                if detected.components().count() < 2 {
                    return refuse(format!(
                        "{id} declares {text:?} as its prefix, which contains \
                         everything below it rather than naming a directory of its own"
                    ));
                }
                for system in SYSTEM_PATH {
                    if path_starts_with_insensitive(Path::new(system), detected) {
                        return refuse(format!(
                            "{id} declares {text:?}, which contains the system \
                             directory {system}"
                        ));
                    }
                }
                for (other, prefix) in taken_prefixes(contracts, &declared, id.as_str()) {
                    if path_starts_with_insensitive(detected, prefix)
                        || path_starts_with_insensitive(prefix, detected)
                    {
                        return refuse(format!(
                            "{id} claims a prefix ({text}) that overlaps {other}'s"
                        ));
                    }
                }
                own.push(detected.to_path_buf());
            } else if provider.detection.presence == crate::provider::ProviderPresence::Present {
                // A present provider was found somewhere, so the adapter has a
                // prefix and states it. Falling back to the registry's list here
                // would undo the narrowing above: a snapshot saying "present,
                // prefix unknown" would get `/usr/local` back as its own.
                return refuse(format!(
                    "{id} is present but states no prefix; every provider this tool \
                     detects reports where it was found"
                ));
            } else {
                // Absent or broken, so there is nothing to honour: the adapter
                // still leads its `PATH` with a default, which the registry
                // knows, and the planner emits no command for it either way.
                own.extend(contract.prefixes.iter().cloned());
            }

            for (key, value) in environment {
                let pinned = PINNED_ENVIRONMENT.iter().find(|(k, _)| *k == key);
                let permitted = key == "PATH"
                    || key == "HOME"
                    || pinned.is_some()
                    || contract.extra.contains_key(key);
                if !permitted {
                    return refuse(format!(
                        "{id} states {key}, which no adapter puts in a provider's \
                         environment; a snapshot may only state what this tool builds"
                    ));
                }
                if let Some((_, expected)) = pinned {
                    if value != expected {
                        return refuse(format!(
                            "{id} states {key}={value:?}, and this tool only ever \
                             sets it to {expected:?}"
                        ));
                    }
                }
                // `HOME` is redacted to `~` at capture and absent under replay,
                // so those are the only two shapes this tool produces - and the
                // tightest useful rule, because `brew` reads
                // `$HOME/.homebrew/brew.env` and exports `SUDO_ASKPASS` and any
                // `HOMEBREW_*` it finds there (observed in `bin/brew`,
                // 2026-09-03). A `HOME` a snapshot chooses hands back every key
                // the list above refuses.
                if let Some(expected) = contract.extra.get(key) {
                    if value != expected {
                        return refuse(format!(
                            "{id} states {key}={value:?}, and its adapter only ever \
                             sets it to {expected:?}"
                        ));
                    }
                }
                if key == "HOME" && value != "~" {
                    return refuse(format!(
                        "{id} states HOME={value:?}; a captured home is redacted to \
                         `~`, and a provider reads its own configuration from there"
                    ));
                }
            }
            let required = PINNED_ENVIRONMENT
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .chain(contract.extra.clone());
            for (key, expected) in required {
                if !environment.contains_key(&key) {
                    return refuse(format!(
                        "{id} states no {key}, which its environment always carries \
                         set to {expected:?}; without it a command behaves differently"
                    ));
                }
            }

            let Some(path) = environment.get("PATH") else {
                return refuse(format!(
                    "{id} states no PATH to run its commands with; every environment \
                     this tool builds has one"
                ));
            };
            let entries: Vec<&str> = path.split(':').collect();
            let split = entries.len().saturating_sub(SYSTEM_PATH.len());
            if entries.len() <= SYSTEM_PATH.len() || &entries[split..] != SYSTEM_PATH {
                return refuse(format!(
                    "{id} has a PATH that does not end with the system directories \
                     ({}); this tool builds one by prepending a provider's own \
                     directories to them",
                    SYSTEM_PATH.join(":")
                ));
            }
            for entry in &entries[..split] {
                if !is_safe_captured_path(entry) {
                    return refuse(format!(
                        "{id} has a PATH entry that is empty, relative, over-long, or \
                         reaches through `..` ({entry:?}); a command would resolve \
                         somewhere other than where the entry reads"
                    ));
                }
                // The only positive rule, and deliberately narrow: a directory
                // *this* provider was detected at. Not the prefixes the
                // registry knows for it - `/usr/local/bin` is group-writable on
                // where an Intel Homebrew created it, and `doctor` calls that an
                // error in its own right,
                // so a snapshot must not be able to put it on the `PATH` of a
                // Homebrew that lives at `/opt/homebrew`.
                for (other, prefix) in taken_prefixes(contracts, &declared, id.as_str()) {
                    if path_starts_with_insensitive(Path::new(entry), prefix) {
                        return refuse(format!(
                            "{id} has {entry:?} on its PATH, which is under {other}'s \
                             prefix (INV-10)"
                        ));
                    }
                }
                // Byte-exact, deliberately, and not the case-insensitive
                // comparison used for the anchor. The two directions need
                // opposite folds: for "may not claim another's prefix" the
                // loosest matcher is safe, because every extra match is a
                // refusal; for "this entry is under my own prefix" the
                // strictest is, because every extra match is an acceptance. The
                // adapter builds the prefix and the entry from one `PathBuf`
                // and redaction rewrites both the same way, so on a snapshot
                // this tool wrote the bytes always agree.
                if !own.iter().any(|p| Path::new(entry).starts_with(p)) {
                    return refuse(format!(
                        "{id} leads its PATH with {entry:?}, which is not a directory \
                         it was detected at; a command it runs would resolve helpers \
                         from there (INV-10)"
                    ));
                }
            }
        }
        Ok(())
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variable_inv_10_names_is_a_build_flag() {
        // INV-10's own list, minus `PATH`, which is reserved rather than
        // stripped. Three places consult this list - the executor's filter,
        // `doctor`'s reporting set and the INV-10 test - and they read it from
        // here so that adding an entry strengthens all three at once.
        for key in [
            "CPATH",
            "LIBRARY_PATH",
            "PKG_CONFIG_PATH",
            "LDFLAGS",
            "CPPFLAGS",
        ] {
            assert!(is_build_flag(key), "INV-10 names {key}");
        }
        assert!(is_build_flag("DYLD_INSERT_LIBRARIES"));
        assert!(!is_build_flag("PATH"));
        assert!(!is_build_flag("HOME"));
    }
}
