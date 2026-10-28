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

/// The most installations of one provider a snapshot may state. The candidate
/// prefixes are a short policy list; more than this is not a capture.
const MAX_INSTALLATIONS: usize = 16;

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
    /// The provider's own configuration files that can outrank how this tool
    /// invokes it, and the keys they set (DP-027). Absent from a
    /// document written before it existed, which reads as
    /// [`ProviderConfiguration::NotRecorded`].
    #[serde(
        default,
        skip_serializing_if = "ProviderConfiguration::is_not_recorded"
    )]
    pub configuration: ProviderConfiguration,
}

/// What capture established about a provider's own configuration files.
///
/// R-15 and R-16 found that each provider reads configuration that applies
/// over the environment this tool invokes it with: Homebrew exports its
/// `brew.env` files after it, MacPorts overlays a user `macports.conf` on its
/// system file. Nothing an invocation sets can win against them, so the answer
/// DP-027 ruled is to detect them - report on reads, refuse before a mutation.
/// This is the record of that detection, a capability of any provider rather
/// than a Homebrew special case (INV-17). Key *names* only: a value can carry a
/// credential (a proxy URL), and nothing here needs one.
///
/// An unknown is never a "none": a recording or a snapshot file of one says
/// [`Self::Unknowable`], and a document from before this field says
/// [`Self::NotRecorded`]. Only a capture that read this host says [`Self::Read`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderConfiguration {
    /// Nothing could have read it: the capture was a replay of a recording,
    /// which carries no files (INV-15).
    Unknowable,
    /// The files were looked for on the captured machine. Only the ones that
    /// exist are listed, so an empty list means none was there.
    Read {
        /// Each file that exists, in the provider's reading order.
        files: Vec<ConfigurationFile>,
    },
    /// The capture did not record it - a provider that is not installed, a
    /// document older than the field, or a state this build does not know,
    /// from a newer one.
    #[default]
    #[serde(other)]
    NotRecorded,
}

impl ProviderConfiguration {
    /// Whether the document carries nothing about it.
    #[must_use]
    pub fn is_not_recorded(&self) -> bool {
        matches!(self, Self::NotRecorded)
    }

    /// Whether what was read may change how the provider runs: a file sets a
    /// key, or a file exists that could not be read. `None` when it
    /// was not read at all, so a caller has to decide what an unknown means
    /// rather than receive a "no".
    #[must_use]
    pub fn outranks(&self) -> Option<bool> {
        match self {
            Self::Read { files } => Some(files.iter().any(|f| !f.readable || !f.keys.is_empty())),
            Self::NotRecorded | Self::Unknowable => None,
        }
    }
}

/// Where a provider is installed on the captured machine: every place it was
/// found, not only the one this tool runs.
///
/// A machine can carry one package manager in two places - an Apple Silicon Mac
/// migrated from an Intel one keeps a working copy under the old prefix - and
/// the second one is invisible to a detection that stops at the first hit
/// (`04-migration-and-safety.md`, "Cross-provider contamination"). This is the
/// record of that search, a capability of any provider (INV-17); the prefixes it
/// lists come from dated policy data, never from code.
///
/// An unknown is never a "one": a capture made from a recording (a replay)
/// says [`Self::Unknowable`] (nothing was looked at, INV-15), and a document from
/// before this field says [`Self::NotRecorded`]. A snapshot file of a live
/// capture carries what that capture read. Only a capture that read this
/// host says [`Self::Read`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Installations {
    /// Nothing could have been looked at: the capture was a replay of a
    /// recording, which carries no filesystem facts (INV-15).
    Unknowable,
    /// The candidate prefixes were looked at on the captured machine.
    Read {
        /// Each candidate prefix that holds the provider's executable, in
        /// detection order. One entry means one installation.
        prefixes: Vec<String>,
    },
    /// The capture did not record it - a provider that is not installed, a
    /// document older than the field, or a state this build does not know,
    /// from a newer one.
    #[default]
    #[serde(other)]
    NotRecorded,
}

impl Installations {
    /// Whether the document carries nothing about it.
    #[must_use]
    pub fn is_not_recorded(&self) -> bool {
        matches!(self, Self::NotRecorded)
    }

    /// How many installations were found, or `None` when nobody looked, so a
    /// caller has to decide what an unknown means rather than receive a "one".
    #[must_use]
    pub fn count(&self) -> Option<usize> {
        match self {
            Self::Read { prefixes } => Some(prefixes.len()),
            Self::NotRecorded | Self::Unknowable => None,
        }
    }
}

/// One configuration file that exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationFile {
    /// Where it is, redacted: a `~/...` path never names the home.
    pub path: String,
    /// Whose file it is.
    pub scope: ConfigurationScope,
    /// Whether it could be read. A file that exists and could not be read is
    /// an unknown, never an empty file.
    pub readable: bool,
    /// Every key the provider's reader accepts from it, by name, in order.
    /// Values are never kept.
    #[serde(default)]
    pub keys: std::collections::BTreeSet<String>,
    /// Those of `keys` that decide whether the provider sends its own
    /// analytics, as the provider-configuration data names them (DP-028).
    /// The value is never kept, so this says the key is set, not which way.
    #[serde(default, skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub analytics_keys: std::collections::BTreeSet<String>,
}

/// Whose configuration a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigurationScope {
    /// The whole machine's (`/etc/...`).
    System,
    /// The installation's, under its prefix.
    Prefix,
    /// The user's, under the home.
    User,
    /// A scope this build does not know, from a newer one.
    #[serde(other)]
    Unknown,
}

/// Whether a configuration key name is one a snapshot may carry: the
/// characters both providers' readers accept in a key, bounded.
#[must_use]
pub fn is_safe_configuration_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= MAX_COMPONENT_LEN
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
    /// Where each installed provider was found, keyed by provider id: every
    /// candidate prefix that holds its executable, not only the one this tool
    /// runs. A filesystem fact, so recorded at capture for the same reason as
    /// `path_findings`: the disk that has the answer is the captured machine's
    /// (INV-15). A provider with no entry is [`Installations::NotRecorded`];
    /// read it through [`Snapshot::installations_of`]. Absent from a document
    /// written before it existed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub installations: BTreeMap<ProviderId, Installations>,
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
    /// The directory is writable by the user this tool is running as, because
    /// they own it and the owner bit is set.
    ///
    /// This is the *ordinary* shadowing shape on macOS, and the one the other
    /// two kinds miss entirely: `~/bin` at `0755`, owned by the user, ahead of
    /// `/usr/bin`. A `curl | sh` installer, an `npm` postinstall or a
    /// compromised dev dependency that drops `~/bin/git` needs no unusual
    /// permissions at all — it is already writable by the account it is
    /// running in. `04-migration-and-safety.md` ("Threat model") names it in
    /// those words.
    ///
    /// Ownership is part of the condition, not decoration. Nearly every
    /// directory has the owner-write bit set, `/usr/bin` at `0755 root:wheel`
    /// included; what distinguishes this one is that the owner is *this* user,
    /// so this user — or anything running as them — can plant a binary in it.
    OwnerWritable,
    /// A kind this build does not know, from a snapshot a newer one wrote.
    ///
    /// Without it, adding a variant is a *breaking* schema change and not the
    /// additive one it looks like: the whole snapshot is rejected on the older
    /// build with "unknown variant", and no analysis happens at all. Adding
    /// `OwnerWritable` did exactly that before this variant existed.
    /// `docs/schemas/README.md` permits adding an enumeration value without a
    /// version bump only where consumers already handle an unknown one — so
    /// here is the consumer handling it.
    #[serde(other)]
    Unknown,
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

// The control half of the one classification, as for every other field a
// snapshot keeps (`crate::text`). A format character in a path is kept like one
// in a `PATH` entry: legitimate directory names carry them, and every renderer
// escapes them.
fn path_is_sane(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_PATH_LEN
        && !text.chars().any(crate::text::is_control_class)
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

/// Cap on a name or version read out of a snapshot, matching what both
/// adapters already impose on provider output.
pub const MAX_COMPONENT_LEN: usize = 128;

fn is_safe_component(text: &str, extra: &[char]) -> bool {
    !text.is_empty()
        && text.len() <= MAX_COMPONENT_LEN
        && text != "."
        && text != ".."
        && !text.starts_with('-')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
}

/// True when `name` is one this tool will pass to a provider as an argument.
///
/// The union of what the two adapters already accept from provider output —
/// Homebrew's `python@3.13`, MacPorts' `python313` — and nothing wider. A
/// leading `-` is refused outright, which is the whole point: a name is an
/// argument, and an argument that starts with a dash is a flag.
///
/// In the domain because a name reaches a command by two routes and only one
/// of them went through an adapter. The adapters validate what a provider
/// says; `--from-snapshot` reads the same names out of a file that never met
/// one, and the planner turns them into argv elements and executable names
/// without passing any allow-list, because the allow-lists live in the
/// adapters (F-09).
pub fn is_safe_provider_name(name: &str) -> bool {
    is_safe_component(name, &['@', '+', '.', '_', '-'])
}

/// True when free text a provider reported is safe to keep and to print.
///
/// Bounded, and with no character of the [`crate::text::HostileClass::Control`]
/// class, which is all that can be asked of a version string or a description:
/// they reach a reader, never a command. Format characters and the line and
/// paragraph separators are **kept** - legitimate names carry them (a ZWJ emoji
/// sequence, ZWNJ in a Persian word), a snapshot is the record of what was
/// reported, and every human renderer escapes them
/// ([`crate::text::escape_hostile`]). One classification answers both
/// questions, so the loader and the renderer cannot disagree about which
/// characters are hostile, only about what each does with them. The
/// stricter rule was tried and refused a real machine - Homebrew reports
/// `>=4.3.0 (shallow or no git repository)` when it is installed from the `.pkg`
/// or from a shallow clone, which is neither a name nor a path but is what
/// `brew --version` says.
pub fn is_safe_provider_text(text: &str) -> bool {
    text.len() <= MAX_TEXT_LEN && !text.chars().any(crate::text::is_control_class)
}

/// Cap on free text kept from provider output.
pub const MAX_TEXT_LEN: usize = 4096;

/// The same as [`is_safe_provider_name`], for a version string, which carries a
/// few more separators. Applied where a version is a *component* - never to
/// what a provider reports about itself, which is prose.
pub fn is_safe_provider_version(version: &str) -> bool {
    is_safe_component(version, &['@', '+', '.', '_', '-', ',', '~', ':'])
}

/// Why this path cannot be a provider's prefix, or `None` when it can be.
///
/// One definition, because two checks need it and they run on different sets of
/// providers: the environment rule skips a provider whose capture failed, and
/// the name rule does not. A prefix that only the first checked was how
/// `relative/evil` and `.` reached a command's `program`.
fn prefix_shape_error(prefix: &Path) -> Option<String> {
    let text = prefix.to_string_lossy().into_owned();
    if !is_safe_captured_path(&text) {
        return Some(format!("{text:?} is not a usable directory"));
    }
    // A `~`-rooted prefix is allowed *here* and refused where it matters. It is
    // not really relative - it is an absolute path with the home elided by
    // redaction, and the privacy machinery exists so a home-installed provider
    // can be captured and read at all. But `Command::new` resolves a string
    // containing a slash against the working directory, so `~/evil/bin/port`
    // would follow the user wherever they ran the tool. `planner::command`
    // therefore refuses to emit a program that is not absolute: such a snapshot
    // can be analysed and reported, and cannot produce a command.
    if prefix.components().count() < 2 {
        return Some(format!(
            "{text:?} contains everything below it rather than naming a directory of its own"
        ));
    }
    SYSTEM_PATH
        .iter()
        .find(|system| path_starts_with_insensitive(Path::new(*system), prefix))
        .map(|system| format!("{text:?} contains the system directory {system}"))
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
    /// a snapshot that set `HOMEBREW_NO_AUTO_UPDATE` to `"0"` was accepted
    /// while the rule claimed to assert what the builder emits. (That key keeps
    /// a read from updating Homebrew; it does not keep a read offline on an
    /// empty cache, and a `brew.env` can outrank it - R-15, SE-016 item 3.)
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
    /// What the snapshot says about where a provider is installed.
    #[must_use]
    pub fn installations_of(&self, provider: &ProviderId) -> &Installations {
        static NOT_RECORDED: Installations = Installations::NotRecorded;
        self.environment
            .installations
            .get(provider)
            .unwrap_or(&NOT_RECORDED)
    }

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
    /// every `HOMEBREW_*` from it, reinstating the whole refused key set (the
    /// files in `/etc/homebrew/` and the prefix can do the same whatever `HOME`
    /// is, so this rule closes the snapshot's route, not the files' - R-15,
    /// DP-027); and
    /// `/usr/local/bin`, group-writable on any Mac, is "under a known Homebrew
    /// prefix". So the rule is now: exactly the variables the builder sets,
    /// exactly the values it sets them to, and a `PATH` of this provider's own
    /// directories followed by the system ones. The third round added the two
    /// that mattered most: a quieting variable was permitted by name, not by
    /// value, so a snapshot could replay a read under a value the builder
    /// never sets (a blanked one deletes the key in Homebrew, R-15, and the
    /// rule is exact-value rather than resting on what `0` does); and a prefix shallow enough to contain
    /// other things - `/opt`, or `/` - made every rule below vacuous, which was
    /// a working bypass of the whole check.
    ///
    /// Decided from the snapshot and the contract alone - no clock, no
    /// filesystem, no live environment - so a snapshotted read stays
    /// reproducible (INV-15).
    pub(crate) fn check_invocation_environments(
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
                if let Some(why) = prefix_shape_error(detected) {
                    return refuse(format!("{id} declares a prefix that {why}"));
                }
                // A prefix shallow enough to contain other things is how the
                // containment rule below gets switched off: declare `/opt` and
                // the other provider's whole tree is "mine"; declare `/` and
                // everything is. Every real prefix names a directory of its
                // own, so requiring two components and forbidding an ancestor
                // relationship costs nothing and closes the family.
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
                // the list above refuses. The `/etc/homebrew/` and prefix files
                // do so whatever `HOME` is; this rule is about what a snapshot
                // may claim, not about those files (R-15, DP-027).
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

    /// Refuses a snapshot whose names, versions or paths are not ones a
    /// provider could have reported.
    ///
    /// The adapters already apply these predicates to provider output. But
    /// `--from-snapshot` reads the same fields out of a file that never met an
    /// adapter, and the planner turns them into argv elements and executable
    /// names without passing any allow-list, because the allow-lists live in
    /// the adapters. "Here is a snapshot from my machine, what should I
    /// migrate?" is a workflow the documentation invites, and a package in that
    /// file named `--force` or `-o/etc/anything` is a flag the executor would
    /// hand to `brew` (F-09).
    ///
    /// Today the consequence is a misleading plan; the plan is the executor's
    /// contract, so when one exists it is flag injection. Refusing now costs
    /// nothing: every name in all six recordings passes, and the predicate is
    /// the union of what the two adapters already accept, not a new rule.
    pub(crate) fn check_names(&self) -> Result<(), crate::error::DomainError> {
        let refuse = |what: &str, value: &str| {
            Err(crate::error::DomainError::new(
                crate::error::ErrorId::SnapshotUnusable,
                format!("{what} {value:?} is not something a provider could have reported"),
            ))
        };
        let name = |what: &str, value: &str| {
            if is_safe_provider_name(value) {
                Ok(())
            } else {
                refuse(what, value)
            }
        };
        // Printable and bounded, for everything that reaches a reader rather
        // than a command. The strict command-path rule would refuse a machine
        // whose `PATH` legitimately contains `.../bin/../bin`, because `which`
        // answers with the entry as written - a fifth way to break a correct
        // machine, and the class this check keeps failing by.
        let text = |what: &str, value: &str| {
            if is_safe_provider_text(value) {
                Ok(())
            } else {
                refuse(what, value)
            }
        };
        // Where each provider was found: `doctor` prints these back, and a
        // snapshot file states whatever it likes. A provider has a handful of
        // candidate prefixes, so a long list is a forged one.
        for (id, installations) in &self.environment.installations {
            name("an installations key", id.as_str())?;
            // A record for a provider the snapshot does not carry is inert - `doctor`
            // reads it only inside a present provider - and is not refused: a
            // document with a provider deleted must reach the plan's own refusal.
            if let Installations::Read { prefixes } = installations {
                // A place named twice would read as "installed in 2 places".
                let distinct: std::collections::BTreeSet<&String> = prefixes.iter().collect();
                if distinct.len() != prefixes.len() {
                    return refuse(
                        &format!("the installation list of {id}"),
                        &prefixes.join(", "),
                    );
                }
                if prefixes.len() > MAX_INSTALLATIONS {
                    return refuse(
                        &format!("the installation list of {id}"),
                        &prefixes.len().to_string(),
                    );
                }
                for prefix in prefixes {
                    if !is_safe_captured_path(prefix) {
                        return refuse(&format!("an installation of {id}"), prefix);
                    }
                }
            }
        }
        // The machine's own `PATH`, which `doctor` prints back a line at a time.
        for entry in &self.environment.path {
            text("a PATH entry", entry)?;
        }
        for finding in &self.environment.path_findings {
            text("a PATH finding", &finding.entry)?;
        }
        // The reported build variables. The two halves are held to different
        // rules because they are different things.
        //
        // A **name** is a variable name, and `doctor` prints it back. Capture
        // only ever writes names from a fixed list - `is_build_flag`'s exact
        // names and `MANPATH` - but a file states whatever it likes, and this
        // was the one display field the check did not read. A control
        // character in a variable name is not a name any capture writes, so it
        // is refused.
        //
        // A **value** is whatever the user's shell holds, and a tab or a
        // newline in `CPPFLAGS` - or a `PKG_CONFIG_PATH` longer than any bound
        // this check could pick - is a correct machine. Refusing it would make
        // `capture` decline the machine in front of it - the class this check
        // keeps failing by. So a value is kept as recorded and unbounded beyond
        // the document's own size limit, which the reader applies before
        // parsing (`persistence::snapshot_store::read_bounded`,
        // `MAX_DOCUMENT_BYTES`); it is escaped wherever it is shown, and today
        // nothing renders one (`doctor` only searches it for a prefix).
        for variable in self.environment.reported.keys() {
            text("a reported variable", variable)?;
        }
        for (id, provider) in &self.providers {
            // The identifier, before anything that names it. A provider id is an
            // identifier, never prose: every renderer and every refusal below
            // prints it, and a key like `homebrew\u{1b}[2J` reached the terminal
            // through the `status` and `analyze` tables. And the map's key must
            // be the provider the detection says it is - two spellings of one
            // fact are how a plan gets built for the wrong provider.
            if !is_safe_provider_name(id.as_str()) {
                return refuse("a provider identifier", id.as_str());
            }
            if provider.detection.provider != *id {
                return refuse(
                    &format!("the detection filed under provider {id}"),
                    provider.detection.provider.as_str(),
                );
            }
            // The prefix, next and unconditionally. It is the one field that
            // becomes a command's `program` - `planner.rs` joins it with the
            // executable name from the table - and it was the field this check
            // originally left out while validating `detection.executable`, which
            // no command uses at all. `check_invocation_environments` does
            // constrain a prefix, but it skips a provider whose capture failed,
            // and such a provider can still be the source side of a plan, so
            // `relative/evil` and `.` reached a `REMOVE_SOURCE` step that
            // elevates.
            if let Some(prefix) = &provider.detection.prefix {
                if let Some(why) = prefix_shape_error(prefix) {
                    return Err(crate::error::DomainError::new(
                        crate::error::ErrorId::SnapshotUnusable,
                        format!("{id} declares a prefix that {why}"),
                    ));
                }
            }
            if let Some(version) = &provider.detection.version {
                // Prose, not a component: `brew --version` answers
                // `>=4.3.0 (shallow or no git repository)` on a `.pkg` install.
                if !is_safe_provider_text(version.as_str()) {
                    return refuse(&format!("{id}'s reported version"), version.as_str());
                }
            }
            if let Some(executable) = &provider.detection.executable {
                text(&format!("{id}'s executable"), &executable.to_string_lossy())?;
            }
            let inventory = &provider.inventory;
            for row in &inventory.installed {
                name(&format!("a package {id} reports"), &row.identity.name)?;
                if !is_safe_provider_version(row.version.as_str()) {
                    return refuse(
                        &format!("the version of {}", row.identity.name),
                        row.version.as_str(),
                    );
                }
                if let Some(revision) = &row.revision {
                    name(&format!("a revision of {}", row.identity.name), revision)?;
                }
                // Bounded and printable, and nothing more. Two providers fill
                // this field and they fill it with different things: MacPorts
                // writes `+universal` and the deselected `-x11`, Homebrew copies
                // a keg's `used_options`, which are literally `--with-openssl`
                // and `--HEAD`. A predicate designed from one provider's shape
                // refused the other's outright - and because this check runs
                // inside `capture`, that was the tool refusing the machine in
                // front of it. A variant reaches no command and no renderer, so
                // the rule for it is the one for everything a reader might see.
                for variant in &row.variants {
                    text(&format!("a variant of {}", row.identity.name), variant)?;
                }
                if let Some(installed_at) = &row.install_path {
                    text(
                        &format!("`install_path` of {}", row.identity.name),
                        &installed_at.to_string_lossy(),
                    )?;
                }
            }
            for requested in &inventory.requested.names {
                name(&format!("a package {id} records as requested"), requested)?;
            }
            for edge in &inventory.dependencies {
                name(&format!("a dependent {id} reports"), &edge.from)?;
                name(&format!("a dependency {id} reports"), &edge.to)?;
            }
            for key in inventory.metadata.keys() {
                name(&format!("a package {id} has metadata for"), key)?;
            }
            for key in inventory.availability.keys() {
                name(&format!("a package {id} assessed availability for"), key)?;
            }
            for (key, resolution) in &provider.resolutions {
                name(&format!("an executable {id} resolved"), key)?;
                name(
                    &format!("an executable {id} resolved"),
                    &resolution.executable,
                )?;
                for (what, value) in [
                    ("the path of", resolution.path.as_deref()),
                    (
                        "the canonical path of",
                        resolution.canonical_path.as_deref(),
                    ),
                ] {
                    if let Some(value) = value {
                        text(&format!("{what} {key}"), &value.to_string_lossy())?;
                    }
                }
            }
            // DP-027: a configuration file's path is shown by `doctor` and in a
            // plan's risks, and a key name is printed back; capture writes a
            // path from data and a name that passed this same rule, a file states
            // whatever it likes.
            if let ProviderConfiguration::Read { files } = &provider.configuration {
                for file in files {
                    if !is_safe_captured_path(&file.path) {
                        return refuse(&format!("a configuration file of {id}"), &file.path);
                    }
                    for key in &file.keys {
                        if !is_safe_configuration_key(key) {
                            return refuse(&format!("a configuration key of {id}"), key);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Every rule a deserialised or captured snapshot must satisfy, in one
    /// call.
    ///
    /// One door, because two boundaries have to apply the same set and a
    /// boundary that calls three of four checks is how a gap gets in. Both
    /// callers - `load_snapshot` and `capture` - go through this.
    pub fn check(
        &self,
        contracts: &BTreeMap<String, ProviderEnvironmentContract>,
    ) -> Result<(), crate::error::DomainError> {
        self.check_schema()?;
        self.check_names()?;
        self.check_invocation_environments(contracts)
    }

    /// Records every capability a loaded document does not mention as
    /// [`CapabilityState::Unknown`].
    ///
    /// **This is a rule about documents, and it is the opposite of the rule
    /// about values in memory.** `ProviderCapabilities::is_unknown` answers
    /// `false` for an absent entry on purpose: in memory, a capability nobody
    /// reported was never asked about, and reading that as "could not be
    /// detected" would refuse every plan built from a hand-made snapshot.
    ///
    /// A document is not in that position. A capture asks every capability and
    /// writes every answer, so a document that omits one is not a document
    /// where the question went unasked — it is one written by a build that did
    /// not have the question, and there is no way to tell from the file which
    /// answer it would have given. Absence there means the record does not say,
    /// which is what `Unknown` means.
    ///
    /// The case this closes is `SERVICE_METADATA` and DP-016(c) (F-29). The
    /// planner refuses to plan when the source cannot answer whether its
    /// packages define services, because INV-19's `MANUAL_REVIEW` reads a
    /// `bool` that renders unknown as `false` and so would silently not fire.
    /// That refusal keys on `Unknown`, and a document predating the capability
    /// carries the same `SNAPSHOT_SCHEMA_VERSION`, so it loaded and planned as
    /// though the answer were no.
    ///
    /// Read-only analysis is deliberately still possible afterwards: the
    /// capability table reports `UNKNOWN`, which is the analysis degrading and
    /// saying so rather than failing, and only the planning path refuses.
    ///
    /// **This took two attempts, and the first one is worth knowing about.**
    /// It filled every provider's table, and the fixture matrix refused it:
    /// `neither-provider-intel-macos13` failed with "snapshot round trip
    /// differs from the replay analysis". The second skipped empty tables, and
    /// `spec-guardian` demonstrated that emptying the whole table - rather
    /// than deleting one key - then walked straight past the refusal this
    /// exists to install, on input the threat model treats as hostile.
    ///
    /// Both symptoms had one cause, and it was in neither of those places: the
    /// Homebrew adapter wrote an **empty** capability table for a provider
    /// that is not installed, where the MacPorts adapter wrote every one
    /// `Unknown`s. The peer providers disagreed about the shape of "absent",
    /// and a rule written here to accommodate that disagreement could only
    /// pick which of the two defects to keep. The adapter is fixed; this rule
    /// is unconditional again, and it is a no-op on every table this build
    /// writes - which is what keeps the live and document routes identical
    /// (INV-15).
    pub fn mark_unrecorded_capabilities_unknown(&mut self) {
        for provider in self.providers.values_mut() {
            for &capability in crate::provider::Capability::all() {
                provider
                    .capabilities
                    .capabilities
                    .entry(capability)
                    .or_insert(crate::provider::CapabilityState::Unknown);
            }
        }
    }

    /// Fails when the schema version is not the one this build reads.
    pub(crate) fn check_schema(&self) -> Result<(), crate::error::DomainError> {
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

    #[test]
    fn a_configuration_state_or_scope_from_a_newer_build_reads_as_an_unknown() {
        // `docs/schemas/snapshot.v1.md` promises both fallbacks; an unknown
        // `state` - with or without fields beside it - is not recorded, which
        // `doctor` and the planner treat as unknown, never as "nothing found".
        for text in [
            r#"{"state":"FUTURE"}"#,
            r#"{"state":"FUTURE","files":[{"path":"~/x","scope":"USER","readable":true}]}"#,
        ] {
            let read: ProviderConfiguration = serde_json::from_str(text).expect(text);
            assert_eq!(read, ProviderConfiguration::NotRecorded, "{text}");
            assert_eq!(read.outranks(), None);
        }
        let file: ConfigurationFile =
            serde_json::from_str(r#"{"path":"~/x","scope":"FUTURE","readable":true,"keys":["A"]}"#)
                .expect("an unknown scope");
        assert_eq!(file.scope, ConfigurationScope::Unknown);
    }
    use super::*;

    /// The premise `planner::command`'s home-installed refusal rests on.
    ///
    /// That refusal emits **one** message — "a provider installed under the
    /// home directory" — rather than branching, and it is allowed to because a
    /// `program` that is not absolute can only have come from a `~`-rooted
    /// prefix: this function admits nothing else that is relative, and
    /// `provider_commands` refuses an absolute `executable`. A first version of
    /// the refusal carried a second arm for the other case; `spec-guardian`
    /// showed the arm was unreachable and the term selecting it untested, which
    /// is the F-19 class.
    ///
    /// So the guarantee lives here, in the crate that owns it, instead of as a
    /// dead branch there. Widen this and the refusal's message becomes a lie —
    /// this test is what fails first, and it names the site.
    #[test]
    fn is_safe_captured_path_admits_only_absolute_or_home_rooted() {
        for absolute in ["/usr/local", "/opt/local", "/opt/homebrew"] {
            assert!(is_safe_captured_path(absolute), "{absolute}");
        }
        for home in ["~", "~/homebrew", "~/opt/local"] {
            assert!(is_safe_captured_path(home), "{home}");
        }
        for relative in [
            "relative/evil",
            ".",
            "..",
            "./homebrew",
            "../homebrew",
            "usr/local",
            "",
        ] {
            assert!(
                !is_safe_captured_path(relative),
                "{relative:?} is relative and not `~`-rooted, so a plan built from it would \
                 be refused with a message naming the home directory, which it is not about \
                 (planner::command, DP-021)"
            );
        }
    }

    #[test]
    fn a_provider_name_is_an_argument_and_never_a_flag() {
        // The union of what the two adapters accept from provider output, and
        // nothing wider: every one of the 201 distinct names and 210 versions in
        // the six recordings passes, so refusing at the load boundary costs no
        // legitimate snapshot.
        for name in [
            "wget",
            "python@3.13",
            "python313",
            "gnu-sed",
            "ca_root_nss",
            "libxml2.1",
        ] {
            assert!(is_safe_provider_name(name), "{name}");
        }
        for version in ["1.2.3", "2.44.0_1", "3.13.1+build", "1.0~beta,2"] {
            assert!(is_safe_provider_version(version), "{version}");
        }
        // A leading dash is the whole point: an argument that starts with one
        // is a flag, and the planner appends a package to an argv with no `--`
        // separator in front of it.
        for hostile in [
            "--force",
            "-o/etc/anything",
            "../../../bin/sh",
            "foo bar",
            "foo;rm -rf /",
            "",
            ".",
            "..",
        ] {
            assert!(!is_safe_provider_name(hostile), "{hostile:?}");
        }
        assert!(!is_safe_provider_name(&"a".repeat(MAX_COMPONENT_LEN + 1)));
        assert!(
            !is_safe_provider_name("naïve"),
            "non-ASCII is not a port name"
        );
    }

    #[test]
    fn a_variant_and_a_reported_version_are_read_by_people_not_by_providers() {
        // Two shapes an earlier version of this rule refused, each of which is
        // what a correct machine actually reports. MacPorts records a deselected
        // variant with its sign - `libsdl @1.2.15_3-x11`, R-3 E-1, and this
        // repository's own parser asserts the sign survives - and Homebrew
        // answers `>=4.3.0 (shallow or no git repository)` when it was installed
        // from the `.pkg` or from a shallow clone. Refusing either made every
        // command on such a machine exit 8, including the one the error told the
        // user to run.
        // Two providers fill `variants` and they fill it differently: MacPorts
        // writes `+universal` and the deselected `-x11`, Homebrew copies a keg's
        // `used_options`, which are `--with-openssl` and `--HEAD`. A predicate
        // built from one shape refused the other, and refused it *at capture*,
        // so the tool declined the machine in front of it. A variant reaches no
        // command, so it is held to the rule for what a reader might see.
        for variant in [
            "-x11",
            "+universal",
            "gnutls",
            "--with-openssl",
            "--HEAD",
            "--c++11",
        ] {
            assert!(is_safe_provider_text(variant), "{variant}");
        }

        assert!(is_safe_provider_text(
            ">=4.3.0 (shallow or no git repository)"
        ));
        assert!(is_safe_provider_text("2.44.0"));
        // Free text still reaches a terminal, so control characters do not.
        assert!(!is_safe_provider_text("\u{1b}[31mPWNED\u{1b}[0m"));
        assert!(!is_safe_provider_text(&"a".repeat(MAX_TEXT_LEN + 1)));
    }

    /// A provider identifier is an identifier: every renderer and every
    /// refusal prints it. A hostile map key is refused, and so is a key that
    /// disagrees with the detection filed under it.
    #[test]
    fn a_provider_identifier_is_an_identifier_and_matches_its_detection() {
        use crate::provider::{
            ProviderCapabilities, ProviderDetection, ProviderInventory, ProviderPresence,
        };
        let contracts = BTreeMap::new();
        let provider = |detected: &str| ProviderSnapshot {
            detection: ProviderDetection {
                provider: ProviderId::new(detected),
                presence: ProviderPresence::Absent,
                version: None,
                prefix: None,
                executable: None,
                architecture: None,
                prefix_redacted: false,
            },
            capabilities: ProviderCapabilities::default(),
            inventory: ProviderInventory::default(),
            environment: BTreeMap::new(),
            invocation_environment: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            configuration: crate::ProviderConfiguration::NotRecorded,
        };
        let with = |key: &str, detected: &str| {
            let mut snapshot = snapshot_with(EnvironmentSnapshot::default());
            snapshot
                .providers
                .insert(ProviderId::new(key), provider(detected));
            snapshot.check(&contracts)
        };
        with("alpha", "alpha").expect("an ordinary identifier loads");
        for &(c, class) in crate::text::CHARACTER_SAMPLES {
            if class.is_some() {
                let key = format!("al{c}pha");
                assert!(with(&key, &key).is_err(), "{} in an id", c.escape_unicode());
            }
        }
        assert!(with("--force", "--force").is_err());
        let error = with("alpha", "beta").expect_err("a mismatched key is refused");
        assert_eq!(error.id, crate::error::ErrorId::SnapshotUnusable);
    }

    #[test]
    fn a_document_from_before_installations_reads_as_not_recorded_never_as_one() {
        let environment: EnvironmentSnapshot =
            serde_json::from_str(r#"{"path":["/usr/bin"]}"#).expect("an old environment");
        assert!(environment.installations.is_empty());
        let snapshot = snapshot_with(environment);
        let id = ProviderId::new("any-provider");
        assert_eq!(snapshot.installations_of(&id), &Installations::NotRecorded);
        assert_eq!(snapshot.installations_of(&id).count(), None);
        // And an unrecognised state from a newer build reads the same way.
        let read: Installations =
            serde_json::from_str(r#"{"state":"SOMETHING_NEWER"}"#).expect("a newer state");
        assert_eq!(read, Installations::NotRecorded);
    }

    #[test]
    fn an_empty_installations_map_is_not_serialised() {
        let text = serde_json::to_string(&snapshot_with(EnvironmentSnapshot::default())).unwrap();
        assert!(!text.contains("installations"), "{text}");
    }

    #[test]
    fn installations_round_trip_and_count() {
        let mut environment = EnvironmentSnapshot::default();
        let id = ProviderId::new("any-provider");
        environment.installations.insert(
            id.clone(),
            Installations::Read {
                prefixes: vec!["/a".into(), "/b".into()],
            },
        );
        let snapshot = snapshot_with(environment);
        let again: Snapshot =
            serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
        assert_eq!(again.installations_of(&id).count(), Some(2));
        assert_eq!(Installations::Unknowable.count(), None);
    }

    #[test]
    fn a_forged_installation_list_is_refused() {
        let id = ProviderId::new("any-provider");
        for prefixes in [
            vec!["relative/prefix".to_string()],
            vec!["/ok".to_string(), "/bad\u{1b}[2J".to_string()],
            (0..=MAX_INSTALLATIONS).map(|n| format!("/p{n}")).collect(),
            vec!["/same".to_string(), "/same".to_string()],
        ] {
            let mut environment = EnvironmentSnapshot::default();
            environment
                .installations
                .insert(id.clone(), Installations::Read { prefixes });
            assert!(
                with_provider(snapshot_with(environment))
                    .check_names()
                    .is_err(),
                "a forged list was accepted"
            );
        }
        let mut fine = EnvironmentSnapshot::default();
        fine.installations.insert(
            id,
            Installations::Read {
                prefixes: vec!["/ok".into()],
            },
        );
        assert!(with_provider(snapshot_with(fine)).check_names().is_ok());
    }

    /// The snapshot with one absent provider, `any-provider`, so an
    /// installation record has a provider to be about.
    fn with_provider(mut snapshot: Snapshot) -> Snapshot {
        use crate::provider::{
            ProviderCapabilities, ProviderDetection, ProviderInventory, ProviderPresence,
        };
        let id = ProviderId::new("any-provider");
        snapshot.providers.insert(
            id.clone(),
            ProviderSnapshot {
                detection: ProviderDetection {
                    provider: id,
                    presence: ProviderPresence::Absent,
                    version: None,
                    prefix: None,
                    executable: None,
                    architecture: None,
                    prefix_redacted: false,
                },
                capabilities: ProviderCapabilities::default(),
                inventory: ProviderInventory::default(),
                environment: BTreeMap::new(),
                invocation_environment: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                configuration: crate::ProviderConfiguration::NotRecorded,
            },
        );
        snapshot
    }

    fn snapshot_with(environment: EnvironmentSnapshot) -> Snapshot {
        use crate::machine::{Architecture, CommandLineTools, MacosVersion, Translation};
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            tool_version: "test".into(),
            policy_version: "test".into(),
            provenance: Provenance {
                captured_at: "2026-10-02T00:00:00Z".into(),
                captured_by: "snapshot.rs".into(),
                origin: "synthesised".into(),
                note: None,
            },
            machine: Machine {
                macos: MacosVersion::new(26, 0, 0),
                architecture: Architecture::Arm64,
                translation: Translation::Native,
                command_line_tools: CommandLineTools::Missing,
            },
            environment,
            providers: BTreeMap::new(),
        }
    }

    /// The loader's half of the one classification, over the shared table:
    /// the Control class is refused in every display field the check reads,
    /// `reported` names included; Format and the line separators are kept,
    /// because legitimate names carry them and a snapshot is the record. A
    /// `reported` value is kept whatever it carries and however long it is.
    #[test]
    fn the_check_refuses_the_control_class_and_keeps_format_characters_and_reported_values() {
        use crate::text::{HostileClass, CHARACTER_SAMPLES, LEGITIMATE_TEXT_WITH_FORMAT};
        let contracts = BTreeMap::new();
        let cases = |text: &str| {
            vec![
                EnvironmentSnapshot {
                    path: vec![format!("/opt/a{text}b")],
                    ..EnvironmentSnapshot::default()
                },
                EnvironmentSnapshot {
                    path_findings: vec![PathFinding {
                        entry: format!("/opt/a{text}b"),
                        index: 0,
                        kind: PathFindingKind::Duplicate,
                    }],
                    ..EnvironmentSnapshot::default()
                },
                EnvironmentSnapshot {
                    reported: BTreeMap::from([(format!("CPP{text}FLAGS"), "-I/x".to_string())]),
                    ..EnvironmentSnapshot::default()
                },
            ]
        };
        // A reported value is what the user's shell holds: kept as recorded,
        // whatever its class, and bounded.
        let value = |text: &str| EnvironmentSnapshot {
            reported: BTreeMap::from([("CPPFLAGS".to_string(), format!("-I/a{text}b"))]),
            ..EnvironmentSnapshot::default()
        };
        for &(c, class) in CHARACTER_SAMPLES {
            assert_eq!(
                is_safe_provider_text(&format!("a{c}b")),
                class != Some(HostileClass::Control),
                "{}",
                c.escape_unicode()
            );
            for environment in cases(&c.to_string()) {
                let result = snapshot_with(environment.clone()).check(&contracts);
                if class == Some(HostileClass::Control) {
                    let error = result.expect_err("a control character is refused");
                    assert_eq!(error.id, crate::error::ErrorId::SnapshotUnusable);
                } else {
                    assert!(
                        result.is_ok(),
                        "{} in {environment:?} was refused: {result:?}",
                        c.escape_unicode()
                    );
                }
            }
        }
        for &(c, _) in CHARACTER_SAMPLES {
            let environment = value(&c.to_string());
            assert!(
                snapshot_with(environment.clone()).check(&contracts).is_ok(),
                "a reported value carrying {} was refused; a correct machine has one",
                c.escape_unicode()
            );
        }
        assert!(
            snapshot_with(value(&"a".repeat(MAX_TEXT_LEN * 4)))
                .check(&contracts)
                .is_ok(),
            "a reported value is kept unbounded; the document's size limit bounds it"
        );
        for text in LEGITIMATE_TEXT_WITH_FORMAT {
            for environment in cases(text) {
                assert!(
                    snapshot_with(environment.clone()).check(&contracts).is_ok(),
                    "a legitimate name was refused: {environment:?}"
                );
            }
        }
    }

    /// One door: what a capture builds and what a load reads back are refused
    /// by the same call with the same words, so `capture` followed by
    /// `--from-snapshot` cannot disagree with the live run (INV-15). The load
    /// route is mirrored as `load_snapshot` runs it - deserialise, fill unrecorded
    /// capabilities, check; the end-to-end version with the real adapters is
    /// `crates/cli/tests/hostile_text.rs`
    /// (`invariant_15_a_hostile_reported_name_is_refused_identically_by_capture_and_by_load`).
    #[test]
    fn invariant_15_a_hostile_reported_name_is_refused_identically_at_capture_and_at_load() {
        let contracts = BTreeMap::new();
        let captured = snapshot_with(EnvironmentSnapshot {
            reported: BTreeMap::from([("LDFLAGS\u{1b}[2J".to_string(), "-L/x".to_string())]),
            ..EnvironmentSnapshot::default()
        });
        let at_capture = captured.check(&contracts).expect_err("capture refuses it");
        let file = serde_json::to_string(&captured).expect("a snapshot serialises");
        let mut loaded: Snapshot = serde_json::from_str(&file).expect("and deserialises");
        loaded.mark_unrecorded_capabilities_unknown();
        let at_load = loaded.check(&contracts).expect_err("load refuses it");
        assert_eq!(at_capture, at_load);
        assert!(
            at_load.detail.contains("a reported variable"),
            "{}",
            at_load.detail
        );
    }

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
