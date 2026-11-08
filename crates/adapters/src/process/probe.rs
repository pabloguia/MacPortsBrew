// SPDX-License-Identifier: Apache-2.0

//! The curated liveness probe: the one way an installed package's own program
//! may be started (DP-051 (a); `docs/passes/16-liveness-design.md`; R-28).
//!
//! **Staged (work order 16 part H, H/01).** This module is the admission, the
//! table reader, the scratch directory and the answer. Nothing outside the
//! process module calls it yet, so applying it starts no program; H-3 wires it,
//! in unfrozen code, after it lands ("What H-3 owes", `docs/passes/16-plans/H.md`).
//!
//! The allowlist is untouched. `PERMITTED_PROGRAMS`, `PERMITTED_SUBCOMMANDS`,
//! `SYSTEM_PROBES` and `is_permitted_invocation` still refuse every package
//! executable; a probe reaches the spawn site only as
//! [`super::Admission::CuratedProbe`], which only [`AdmittedProbe::admit`]'s
//! product can carry, and the site asks it a separate question of shape
//! ([`is_admitted_probe_shape`]).
//!
//! **Three layers hold the closed flag set**, the table reader, `admit` and the
//! site, so a probe can be widened at none of them. **A passing probe does not
//! exclude a started server** (R-28): the controls are the observed-only rows,
//! the exact version, the closed flag set and the stateful exclusion in the
//! policy loader.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "staged (H/01): nothing unfrozen calls the admission until H-3's wiring lands"
    )
)]

use super::{Admission, CommandSpec, ExitStatus, ProcessRunner, PERMITTED_PROGRAMS, SYSTEM_PROBES};
use crate::persistence::run_lock::RunLock;
use crate::persistence::StateDir;
use macportsbrew_domain::{
    CheckAnswer, CheckFailure, ExecutableName, OwnershipFacts, UncheckedCause, PINNED_ENVIRONMENT,
    SYSTEM_PATH,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{DirBuilder, Metadata};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// The whole set of flags a curated probe may pass (condition 5). `argv = []`
/// is the server-start case, and `--init`, `--daemon` and `-D` would all pass a
/// character class, so growing this is a reviewed code change. The policy
/// loader holds the same four (`PROBE_FLAGS` in `crates/policy/src/data.rs`).
pub(crate) const PROBE_FLAGS: [&str; 4] = ["--version", "-version", "-V", "-v"];

/// The timeout of a probe, which the site enforces as a ceiling. A HYPOTHESIS
/// from one machine (R-28 finding 19), which is why a timeout is never `Fails`.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// The cap per output stream, which the site enforces as a ceiling. What is
/// read is dropped: only the exit status counts.
pub(crate) const PROBE_MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Every key a probe's environment holds, and no other (R-28 findings 8, 13).
/// `CFFIXED_USER_HOME` is not here: the design note admits it only "if
/// qualification confirms it", and the re-qualification of 2026-11-04 did not.
const PROBE_ENVIRONMENT_KEYS: [&str; 8] = [
    "HOME",
    "LANG",
    "LC_ALL",
    "NO_COLOR",
    "PATH",
    "PYTHONDONTWRITEBYTECODE",
    "TERM",
    "TMPDIR",
];

/// The scratch directory's mode: the owner, and nobody else.
const SCRATCH_MODE: u32 = 0o700;

/// The mode bit that runs a file as its owner. Named by value: the elevation
/// mechanisms' names are refused in non-comment source
/// (`invariant_06_reading_the_machine_needs_no_elevation`).
const RUNS_AS_OWNER_BIT: u32 = 0o4000;

/// The mode bit that runs a file as its group.
const RUNS_AS_GROUP_BIT: u32 = 0o2000;

/// Group and world write.
const SHARED_WRITE_BITS: u32 = 0o022;

/// Shells and interpreters, by leaf (condition 10), with or without a version
/// suffix (`python3.12`, `perl5.34`, `tclsh8.6`). Defence in depth: the table
/// holds observed compiled programs only, and `bin/foo -> libexec/python3` is
/// the case this catches. Not a complete list of interpreters, which cannot be
/// written; the rows are the control.
const EXCLUDED_LEAVES: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "mksh",
    "csh",
    "tcsh",
    "fish",
    "env",
    "python",
    "pypy",
    "perl",
    "ruby",
    "irb",
    "tclsh",
    "wish",
    "expect",
    "node",
    "nodejs",
    "deno",
    "bun",
    "php",
    "lua",
    "luajit",
    "osascript",
    "awk",
    "gawk",
    "nawk",
    "mawk",
    "xcrun",
    "arch",
    "guile",
    "julia",
    "pwsh",
    "java",
    "jshell",
    "rscript",
];

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

/// The curated table, read by this layer itself: no caller hands it a table,
/// as no caller hands `prefixes.rs` a prefix. `crates/adapters` cannot reach
/// `crates/policy`, so this is a second, strict reader of the same file; an
/// agreement test against the policy loader belongs in `crates/cli/tests` (H-3),
/// through [`curated_probe_rows`].
const TABLE_SOURCE: &str = include_str!("../../../../data/policy/liveness-probes.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableFile {
    #[serde(default)]
    probe: Vec<Row>,
}

/// One row: an observation, keyed `(provider, package, executable)`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::process) struct Row {
    provider: String,
    package: String,
    executable: String,
    argv: Vec<String>,
    observed_version: String,
    source: String,
    read_on: String,
}

/// Parses the table strictly, failing closed on anything the policy loader
/// would refuse that can be read from this file alone: a key twice, an empty
/// provider or package, an executable outside its name class, `argv` other than
/// one flag of [`PROBE_FLAGS`], no version, no source, a date not `YYYY-MM-DD`.
/// The stateful exclusion needs `stateful-packages.toml` and stays the policy
/// loader's, which refuses to load at all on a stateful row.
fn parse_table(text: &str) -> Result<Vec<Row>, String> {
    let file: TableFile = toml::from_str(text).map_err(|e| format!("malformed: {e}"))?;
    let mut keys = BTreeSet::new();
    for row in &file.probe {
        let key = format!("({}, {}, {})", row.provider, row.package, row.executable);
        if !keys.insert((&row.provider, &row.package, &row.executable)) {
            return Err(format!("{key} appears twice"));
        }
        if row.provider.is_empty() || row.package.is_empty() {
            return Err(format!("{key} names no provider or no package"));
        }
        if ExecutableName::parse(&row.executable).is_err() {
            return Err(format!("{key} names no executable"));
        }
        if !is_one_permitted_flag(&row.argv) {
            return Err(format!(
                "{key} passes other than one flag of the closed set"
            ));
        }
        if row.observed_version.is_empty() {
            return Err(format!("{key} says no version it was observed at"));
        }
        if row.source.trim().is_empty() || !is_iso_date(&row.read_on) {
            return Err(format!("{key} carries no source or no date"));
        }
    }
    Ok(file.probe)
}

/// `YYYY-MM-DD`, by shape.
fn is_iso_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// The embedded table, parsed once. A malformed table answers every probe
/// `NO_CURATED_PROBE`, never a guess; `the_shipped_table_parses` holds it.
fn table() -> Result<&'static [Row], &'static str> {
    use std::sync::OnceLock;
    static PARSED: OnceLock<Result<Vec<Row>, String>> = OnceLock::new();
    PARSED
        .get_or_init(|| parse_table(TABLE_SOURCE))
        .as_deref()
        .map_err(String::as_str)
}

/// Whether `argv` is exactly one element of [`PROBE_FLAGS`].
fn is_one_permitted_flag(argv: &[String]) -> bool {
    matches!(argv, [flag] if PROBE_FLAGS.contains(&flag.as_str()))
}

/// A copy of one row of the table this module admits against, for reading only.
/// Building one admits nothing: [`AdmittedProbe::admit`] reads its own parsed
/// table, never one of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CuratedProbeRow {
    /// The destination provider.
    pub provider: String,
    /// The destination package.
    pub package: String,
    /// The executable's leaf.
    pub executable: String,
    /// The one flag.
    pub argv: Vec<String>,
    /// The version it was observed at, revision included.
    pub observed_version: String,
    /// What was seen.
    pub source: String,
    /// When, `YYYY-MM-DD`.
    pub read_on: String,
}

/// The rows of the embedded table as this module parsed them, or why it did
/// not parse. **Parameterless and read-only**, so it is not the seam the design
/// note forbids (one that accepts a table or a prefix): it exists for the
/// agreement test against the policy loader in `crates/cli/tests`, which is why
/// it is `pub`.
pub fn curated_probe_rows() -> Result<Vec<CuratedProbeRow>, String> {
    table().map_err(str::to_owned).map(|rows| {
        rows.iter()
            .map(|row| CuratedProbeRow {
                provider: row.provider.clone(),
                package: row.package.clone(),
                executable: row.executable.clone(),
                argv: row.argv.clone(),
                observed_version: row.observed_version.clone(),
                source: row.source.clone(),
                read_on: row.read_on.clone(),
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------
// What the adapter supplies
// ---------------------------------------------------------------------------

/// What a provider adapter answers about itself for its probe, **from itself**:
/// its own id, its own scope and runner, its own detected prefix, and its own
/// owner rule run now. No caller hands any of these in, so no route lets a plan
/// or a test name a prefix or a table (`docs/passes/16-liveness-design.md`). H-3
/// implements it for each adapter, in unfrozen code.
///
/// `pub(crate)`, and deliberately not reachable from `crates/*/tests`: a `pub`
/// seam accepting a prefix would ship.
pub(crate) trait ProbeOwner {
    /// The adapter's own provider id: condition 4's row key.
    fn provider(&self) -> &str;
    /// Whether the filesystem describes this machine (condition 1).
    fn is_this_machine(&self) -> bool;
    /// The adapter's own runner (condition 1 asks it `is_replay` and
    /// `may_run_probes`).
    fn runner(&self) -> &dyn ProcessRunner;
    /// The prefix this adapter detected.
    fn prefix(&self) -> &Path;
    /// The owner rule, run now, on `placed`, the entry `admit` built from typed
    /// parts and is about to canonicalise (conditions 6 and 7): the same answer
    /// `EXECUTABLES_OWNED_BY_DESTINATION` reads, judged of that very file.
    fn entry_owner(&self, package: &str, leaf: &ExecutableName, placed: &Path) -> OwnershipFacts;
    /// Whether `canonical`, what `placed` finally resolves to, is the package's
    /// by the owner rule (condition 8): inside its keg where the rule is a
    /// path, and where the rule is a query, asked of `placed` **and**
    /// `canonical`, both answering the package. Not merely under the prefix,
    /// which `admit` requires as a floor of its own.
    fn target_is_packages(
        &self,
        package: &str,
        placed: &Path,
        canonical: &Path,
    ) -> Result<bool, UncheckedCause>;
}

// ---------------------------------------------------------------------------
// Who this process is
// ---------------------------------------------------------------------------

/// This process's effective uid, read as the owner of a file it just made
/// (`crate::system::machine::effective_uid`): never a caller's word for it.
fn current_euid() -> Option<u32> {
    crate::system::machine::effective_uid()
}

#[cfg(test)]
thread_local! {
    /// Makes condition 2 see root, in a test only, so dropping that condition can
    /// be shown to reach a spawn: nothing else here can make this process root.
    static TREAT_AS_ROOT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Condition 2's question.
fn is_root(euid: u32) -> bool {
    #[cfg(test)]
    if TREAT_AS_ROOT.with(std::cell::Cell::get) {
        return true;
    }
    euid == 0
}

// ---------------------------------------------------------------------------
// Identity of the target
// ---------------------------------------------------------------------------

/// What `lstat` says of the canonical target: recorded at `admit`, re-read
/// immediately before spawn (in `system.rs`) and after exit (here). Any
/// difference is `Unchecked (PROBE_TARGET_CHANGED)`, never `Holds`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::process) struct TargetIdentity {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
    mode: u32,
}

impl TargetIdentity {
    fn of(meta: &Metadata) -> Self {
        Self {
            dev: meta.dev(),
            ino: meta.ino(),
            size: meta.size(),
            mtime: meta.mtime(),
            mtime_nsec: meta.mtime_nsec(),
            ctime: meta.ctime(),
            ctime_nsec: meta.ctime_nsec(),
            mode: meta.mode(),
        }
    }

    /// One `lstat` of `path`, never following a link.
    pub(in crate::process) fn read(path: &Path) -> Option<Self> {
        std::fs::symlink_metadata(path).ok().map(|m| Self::of(&m))
    }
}

// ---------------------------------------------------------------------------
// The admission
// ---------------------------------------------------------------------------

/// A probe that passed all eleven conditions. Private fields, no `Clone`, no
/// `Default`, no serde: the only way to one is [`AdmittedProbe::admit`], and
/// the only thing it becomes is a spec built and run inside this module.
pub(crate) struct AdmittedProbe {
    /// The canonical target, which is what runs.
    program: PathBuf,
    /// The placed leaf, passed as `argv[0]`: `lzcat` is a link to `xz`.
    arg0: String,
    /// The row's one flag.
    argv: Vec<String>,
    /// `<prefix>/bin`, the first entry of the probe's `PATH`.
    destination_bin: PathBuf,
    /// The canonical target's identity at admission.
    target: TargetIdentity,
    /// This process's effective uid, as read at admission.
    euid: u32,
}

impl AdmittedProbe {
    /// Refuses unless every condition of `docs/passes/16-liveness-design.md`
    /// holds; each refusal is a named cause, and a refusal starts nothing.
    ///
    /// The owner rule's answer is read once and tested for condition 7 before
    /// condition 6: a version is only the package's to compare once the entry
    /// is the package's. Everything else is in the note's order.
    pub(in crate::process) fn admit(
        table: &[Row],
        owner: &dyn ProbeOwner,
        package: &str,
        leaf: &str,
        argv: &[String],
    ) -> Result<Self, UncheckedCause> {
        // 1. This machine, and a live runner that may start a probe (INV-15): a
        // replay, a test double and a recording runner all refuse here, before
        // anything is read or written.
        let runner = owner.runner();
        if !owner.is_this_machine() || runner.is_replay() || !runner.may_run_probes() {
            return Err(UncheckedCause::NotObserved);
        }
        // 2. Never as root (INV-06). The uid is this process's own.
        let Some(euid) = current_euid() else {
            return Err(UncheckedCause::ProbeNotStarted);
        };
        if is_root(euid) {
            return Err(UncheckedCause::ProbeAsRoot);
        }
        // 3. The name class (§3 item 10).
        let Ok(leaf) = ExecutableName::parse(leaf) else {
            return Err(UncheckedCause::ProbeLeafRefused);
        };
        // 4. A row for exactly this key, and the plan's argv is the row's.
        let Some(row) = table.iter().find(|row| {
            row.provider == owner.provider()
                && row.package == package
                && row.executable == leaf.as_str()
        }) else {
            return Err(UncheckedCause::NoCuratedProbe);
        };
        if row.argv != argv {
            return Err(UncheckedCause::ProbeArgvNotTheRows);
        }
        // 5. One flag of the closed set, here as in the reader and at the site.
        if !is_one_permitted_flag(argv) {
            return Err(UncheckedCause::ProbeArgvNotPermitted);
        }
        // 7. The placed entry, from typed parts, is the package's by the owner
        // rule run now, on that entry.
        let Ok(Some(placed)) =
            crate::destination_reads::placed_entry(owner.prefix(), leaf.as_str())
        else {
            return Err(UncheckedCause::ProbeEntryNotThePackages);
        };
        let version = match owner.entry_owner(package, &leaf, &placed) {
            OwnershipFacts::Owner { name, version, .. } if name == package => version,
            OwnershipFacts::Owner { .. }
            | OwnershipFacts::NoEntry
            | OwnershipFacts::NotOwned
            | OwnershipFacts::OwnerLeavesItsDirectory { .. }
            | OwnershipFacts::CouldNotTell(_) => {
                return Err(UncheckedCause::ProbeEntryNotThePackages)
            }
        };
        // 6. Exactly the observed version, revision included.
        if version != row.observed_version {
            return Err(UncheckedCause::ProbeVersionNotQualified);
        }
        // 8. What it resolves to lies under the destination's prefix, by
        // component (a floor of this module's own), and is the package's by the
        // owner rule.
        let Ok(canonical) = std::fs::canonicalize(&placed) else {
            return Err(UncheckedCause::ProbeTargetNotThePackages);
        };
        if !super::is_safe_path(&canonical)
            || !lies_under(&canonical, owner.prefix())
            || owner.target_is_packages(package, &placed, &canonical) != Ok(true)
        {
            return Err(UncheckedCause::ProbeTargetNotThePackages);
        }
        // 9. On one `lstat` of the canonical target: a regular file, neither
        // mode bit that runs it as someone else, owned by this uid or root, no
        // group or world write (INV-06).
        let Ok(meta) = std::fs::symlink_metadata(&canonical) else {
            return Err(UncheckedCause::ProbeTargetModeRefused);
        };
        if target_mode_refused(meta.file_type().is_file(), meta.uid(), meta.mode(), euid) {
            return Err(UncheckedCause::ProbeTargetModeRefused);
        }
        let target = TargetIdentity::of(&meta);
        // 10. Neither leaf is a provider program, a system probe, a shell or an
        // interpreter.
        let canonical_leaf = canonical.file_name().and_then(|n| n.to_str());
        if is_excluded_leaf(leaf.as_str()) || canonical_leaf.is_none_or(is_excluded_leaf) {
            return Err(UncheckedCause::ProbeProgramExcluded);
        }
        // 11. Only a Mach-O executable for this build's architecture, read on a
        // handle that is the file `lstat` described.
        target_head_admits(&canonical, &target)?;
        Ok(Self {
            program: canonical,
            arg0: leaf.as_str().to_owned(),
            argv: argv.to_vec(),
            destination_bin: owner.prefix().join("bin"),
            target,
            euid,
        })
    }
}

/// Whether `path` is, or lies under, `prefix` by component, as `prefix` is
/// written or canonically.
fn lies_under(path: &Path, prefix: &Path) -> bool {
    let within = |p: &Path| path == p || crate::listing::is_under(path, p);
    within(prefix) || std::fs::canonicalize(prefix).is_ok_and(|real| within(&real))
}

/// Condition 9, pure, so the owner case is tested without a second user.
fn target_mode_refused(is_file: bool, uid: u32, mode: u32, euid: u32) -> bool {
    !is_file
        || mode & (RUNS_AS_OWNER_BIT | RUNS_AS_GROUP_BIT) != 0
        || (uid != euid && uid != 0)
        || mode & SHARED_WRITE_BITS != 0
}

/// Condition 10, on one leaf: a provider program, a system probe, or a shell or
/// interpreter with or without a version suffix, compared without case (the
/// default macOS volume does not tell `Bash` from `bash`).
fn is_excluded_leaf(leaf: &str) -> bool {
    let lower = leaf.to_ascii_lowercase();
    if PERMITTED_PROGRAMS.contains(&lower.as_str()) || SYSTEM_PROBES.contains(&lower.as_str()) {
        return true;
    }
    EXCLUDED_LEAVES.iter().any(|base| {
        lower.strip_prefix(base).is_some_and(|rest| {
            rest.chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        })
    })
}

/// Condition 11, and the handle half of condition 9: **an allowlist of what a
/// read shows**. Opens the target, checks the handle is the file `lstat`
/// described (`PROBE_TARGET_CHANGED` if not), and admits it only if its bytes,
/// read through that handle and bounded by `crate::macho`'s own limits, are
/// either **a thin native 64-bit Mach-O executable for this build's
/// architecture**, or **a big-endian fat file every slice of which is a Mach-O
/// executable and one of which is for this build's architecture**
/// ([`is_executable_for_this_build`]).
///
/// Anything else is refused with the reserved `PROBE_TARGET_IS_ENV_SCRIPT`,
/// because what the kernel cannot exec it hands to `/bin/sh` (`ENOEXEC`, review
/// round 3): a script of any kind, text with no `#!`, an empty or one-byte file,
/// a dylib, a bundle, an object, an archive, another architecture, a
/// little-endian fat file, a fat file with any slice that is not an executable,
/// a malformed or over-limit header, and a read error. Condition 9 already
/// passed on the `lstat`, so its cause would say something false of a read
/// failure. (The cause's name now under-describes the refusal: a naming item for
/// H-3, no cause is added here.)
fn target_head_admits(canonical: &Path, target: &TargetIdentity) -> Result<(), UncheckedCause> {
    let refused = UncheckedCause::ProbeTargetIsEnvScript;
    let file = std::fs::File::open(canonical).map_err(|_| refused)?;
    let meta = file.metadata().map_err(|_| refused)?;
    if meta.dev() != target.dev || meta.ino() != target.ino {
        return Err(UncheckedCause::ProbeTargetChanged);
    }
    if is_executable_for_this_build(&file, meta.size()) {
        Ok(())
    } else {
        Err(refused)
    }
}

/// `MH_MAGIC_64` as it lies on disk for this build's byte order (both macOS
/// architectures are little-endian): the only thin magic admitted.
const NATIVE_THIN_MAGIC: [u8; 4] = 0xfeed_facf_u32.to_le_bytes();

/// `FAT_MAGIC` and `FAT_MAGIC_64`, big-endian on disk: the only fat magics the
/// kernel execs. `crate::macho::classify` also reads the byte-swapped
/// `FAT_CIGAM` forms for the linkage reader; the kernel does not, and a file it
/// cannot exec runs as a shell script (review round 4).
const KERNEL_FAT_MAGICS: [[u8; 4]; 2] =
    [0xcafe_babe_u32.to_be_bytes(), 0xcafe_babf_u32.to_be_bytes()];

/// Whether `file`, of `len` bytes, is what condition 11 admits, by
/// `crate::macho`'s three stages with every read positioned (`pread`) on the one
/// handle and bounded: the head by `macho::HEAD_BYTES`, each slice header by
/// `SLICE_HEADER_BYTES`, each window by what `plan_slice` answers (at most
/// `MAX_SIZEOFCMDS` and a header), and at most `MAX_SLICES` slices.
///
/// - A thin file: its raw magic is native `MH_MAGIC_64`, and its one slice is
///   an `MH_EXECUTE` for `THIS_BUILD_CPUTYPE`.
/// - A fat file: its raw magic is big-endian `FAT_MAGIC` or `FAT_MAGIC_64`;
///   **every** slice is a thin native `MH_MAGIC_64` `MH_EXECUTE` (no nested fat,
///   no dylib, no text), because the kernel, not this reader, picks the slice
///   (`x86_64h` before `x86_64` on this CPU, the native slice for a child under
///   Rosetta); and at least one slice is for `THIS_BUILD_CPUTYPE`.
///
/// Any error, short read or other shape is a no.
fn is_executable_for_this_build(file: &std::fs::File, len: u64) -> bool {
    use crate::macho::{
        classify, plan_slice, read_slice, Classified, ExaminedType, SlicePlan, HEAD_BYTES,
        SLICE_HEADER_BYTES, THIS_BUILD_CPUTYPE,
    };
    let read = |offset: u64, want: u64| -> Option<Vec<u8>> {
        let want = usize::try_from(want).ok()?;
        let mut bytes = vec![0u8; want];
        let mut done = 0;
        while done < want {
            let at = offset.checked_add(u64::try_from(done).ok()?)?;
            match std::os::unix::fs::FileExt::read_at(file, &mut bytes[done..], at) {
                Ok(0) => return None,
                Ok(n) => done += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return None,
            }
        }
        Some(bytes)
    };
    let head_len = len.min(u64::try_from(HEAD_BYTES).unwrap_or(u64::MAX));
    let Some(head) = read(0, head_len) else {
        return false;
    };
    let magic = head.get(..4);
    let ranges = match classify(&head, len) {
        Ok(Classified::Thin(range)) if magic == Some(&NATIVE_THIN_MAGIC[..]) => vec![range],
        Ok(Classified::Fat(ranges)) if KERNEL_FAT_MAGICS.iter().any(|m| magic == Some(&m[..])) => {
            ranges
        }
        Ok(
            Classified::Thin(_)
            | Classified::Fat(_)
            | Classified::NotObject
            | Classified::Skipped(_)
            | Classified::Script(_),
        )
        | Err(_) => return false,
    };
    let header_bytes = u64::try_from(SLICE_HEADER_BYTES).unwrap_or(u64::MAX);
    // The `cputype` of each slice, when it is a thin native `MH_EXECUTE`.
    let executable_cputype = |range: &crate::macho::SliceRange| -> Option<u32> {
        let slice_head = read(range.offset, range.size.min(header_bytes))?;
        if slice_head.get(..4) != Some(&NATIVE_THIN_MAGIC[..]) {
            return None;
        }
        let Ok(SlicePlan::Window { len: window_len }) = plan_slice(&slice_head, range.size) else {
            return None;
        };
        let refs = read_slice(&read(range.offset, window_len)?).ok()?;
        (refs.file_type == ExaminedType::Execute).then_some(refs.cputype)
    };
    let cputypes: Option<Vec<u32>> = ranges.iter().map(executable_cputype).collect();
    cputypes.is_some_and(|cputypes| cputypes.contains(&THIS_BUILD_CPUTYPE))
}

// ---------------------------------------------------------------------------
// The spec, and the site's question
// ---------------------------------------------------------------------------

impl CommandSpec {
    /// The only producer of [`Admission::CuratedProbe`]. Consumes the admitted
    /// probe, so a spec is built once per admission, inside this module.
    fn curated_probe(probe: AdmittedProbe, scratch: &Path) -> Self {
        let scratch_text = scratch.to_string_lossy().into_owned();
        let mut path = vec![probe.destination_bin.to_string_lossy().into_owned()];
        path.extend(SYSTEM_PATH.iter().map(|p| (*p).to_owned()));
        let mut environment = BTreeMap::new();
        environment.insert("PATH".to_owned(), path.join(":"));
        for (key, value) in PINNED_ENVIRONMENT {
            environment.insert((*key).to_owned(), (*value).to_owned());
        }
        environment.insert("HOME".to_owned(), scratch_text.clone());
        environment.insert("TMPDIR".to_owned(), scratch_text);
        environment.insert("PYTHONDONTWRITEBYTECODE".to_owned(), "1".to_owned());
        Self {
            program: probe.program,
            argv: probe.argv,
            environment,
            timeout: PROBE_TIMEOUT,
            max_output_bytes: PROBE_MAX_OUTPUT_BYTES,
            replay_only: false,
            admission: Admission::CuratedProbe {
                scratch: scratch.to_path_buf(),
                arg0: probe.arg0,
                target: probe.target,
            },
        }
    }
}

/// The site's question for a curated probe: **shape only**, and never the
/// test-build widening. It cannot prove prefix membership, which `admit`
/// established; it refuses anything a spec could have been widened to after
/// admission (`with_timeout` is `pub`): a timeout or cap over the constants, an
/// environment whose keys are not exactly [`PROBE_ENVIRONMENT_KEYS`] or whose
/// values are not the probe's, a flag outside the set, a program or `arg0` that
/// is a provider program, a system probe, a shell or an interpreter.
pub(in crate::process) fn is_admitted_probe_shape(spec: &CommandSpec) -> bool {
    let Admission::CuratedProbe { scratch, arg0, .. } = &spec.admission else {
        return false;
    };
    let program_leaf = spec.program.file_name().and_then(|n| n.to_str());
    let plain = |path: &Path| {
        super::is_safe_path(path) && !path.components().any(|c| c == Component::ParentDir)
    };
    plain(&spec.program)
        && plain(scratch)
        && program_leaf.is_some_and(|leaf| !is_excluded_leaf(leaf))
        && ExecutableName::parse(arg0).is_ok()
        && !is_excluded_leaf(arg0)
        && is_one_permitted_flag(&spec.argv)
        && spec.timeout <= PROBE_TIMEOUT
        && spec.max_output_bytes <= PROBE_MAX_OUTPUT_BYTES
        && is_probe_environment(&spec.environment, scratch)
}

/// Exactly the probe's environment: those keys and no other; the pinned
/// values; `HOME` and `TMPDIR` the scratch; `PATH` one absolute directory named
/// `bin` and then the system path, so no other provider's prefix is on it.
fn is_probe_environment(environment: &BTreeMap<String, String>, scratch: &Path) -> bool {
    if !environment
        .keys()
        .map(String::as_str)
        .eq(PROBE_ENVIRONMENT_KEYS)
    {
        return false;
    }
    let scratch_text = scratch.to_string_lossy();
    let pinned = PINNED_ENVIRONMENT
        .iter()
        .all(|(key, value)| environment.get(*key).map(String::as_str) == Some(*value));
    let path_holds = environment.get("PATH").is_some_and(|path| {
        let mut entries = path.split(':');
        let first = entries.next().map(Path::new);
        first.is_some_and(|bin| {
            super::is_safe_path(bin) && bin.file_name().is_some_and(|n| n == "bin")
        }) && entries.eq(SYSTEM_PATH.iter().copied())
    });
    pinned
        && path_holds
        && environment.get("HOME").map(String::as_str) == Some(&*scratch_text)
        && environment.get("TMPDIR").map(String::as_str) == Some(&*scratch_text)
        && environment
            .get("PYTHONDONTWRITEBYTECODE")
            .map(String::as_str)
            == Some("1")
}

// ---------------------------------------------------------------------------
// How it ended
// ---------------------------------------------------------------------------

/// How a curated probe ended, as the runner saw it, and nothing it printed.
///
/// Opaque outside the process module: no runner elsewhere can build one, so
/// none can claim a probe ran (`ProcessRunner::run_probe`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeRun {
    pub(in crate::process) kind: ProbeRunKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::process) enum ProbeRunKind {
    /// Nothing was observed: a replay, a test double, or a recording runner.
    NotObserved,
    /// The site refused the spec.
    Refused,
    /// The target's identity differed immediately before spawn.
    TargetChanged,
    /// The spawn failed.
    NotStarted,
    /// It started and ended, by `status`, with its streams ending `ending`.
    Ended {
        status: ExitStatus,
        ending: super::system::StreamEnd,
    },
}

impl ProbeRun {
    pub(in crate::process) fn not_observed() -> Self {
        Self {
            kind: ProbeRunKind::NotObserved,
        }
    }

    pub(in crate::process) fn refused() -> Self {
        Self {
            kind: ProbeRunKind::Refused,
        }
    }

    pub(in crate::process) fn target_changed() -> Self {
        Self {
            kind: ProbeRunKind::TargetChanged,
        }
    }

    pub(in crate::process) fn not_started() -> Self {
        Self {
            kind: ProbeRunKind::NotStarted,
        }
    }

    pub(in crate::process) fn ended(status: ExitStatus, ending: super::system::StreamEnd) -> Self {
        Self {
            kind: ProbeRunKind::Ended { status, ending },
        }
    }
}

/// The answer (`docs/passes/16-liveness-design.md`, "What the answer is"): an
/// exhaustive match on how it ended, with no wildcard and not `success()` plus a
/// flag. `unchanged` is the post-exit identity re-read; a target that changed
/// is `Unchecked (PROBE_TARGET_CHANGED)` whatever ended, since what ran cannot
/// be told.
fn answer(run: &ProbeRun, unchanged: bool) -> CheckAnswer {
    use super::system::StreamEnd;
    let observed = match &run.kind {
        ProbeRunKind::NotObserved => CheckAnswer::Unchecked(UncheckedCause::NotObserved),
        ProbeRunKind::Refused | ProbeRunKind::NotStarted => {
            CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
        }
        ProbeRunKind::TargetChanged => CheckAnswer::Unchecked(UncheckedCause::ProbeTargetChanged),
        ProbeRunKind::Ended { status, ending } => match status {
            ExitStatus::Exited { code: 0 } => match ending {
                StreamEnd::Complete => CheckAnswer::Holds,
                StreamEnd::OverCap => CheckAnswer::Unchecked(UncheckedCause::ProbeOutputOverCap),
                StreamEnd::ReadError => CheckAnswer::Unchecked(UncheckedCause::ProbeEndNotRead),
                StreamEnd::GaveUp => CheckAnswer::Unchecked(UncheckedCause::ProbeLeftAProcess),
            },
            ExitStatus::Exited { .. } | ExitStatus::Signalled { signal: Some(_) } => {
                CheckAnswer::Fails(CheckFailure::ProbeFailed)
            }
            ExitStatus::Signalled { signal: None } | ExitStatus::Cancelled => {
                CheckAnswer::Unchecked(UncheckedCause::ProbeEndNotRead)
            }
            ExitStatus::TimedOut => CheckAnswer::Unchecked(UncheckedCause::ProbeTimedOut),
            ExitStatus::NotStarted { .. } => {
                CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
            }
        },
    };
    match observed {
        CheckAnswer::Holds | CheckAnswer::Fails(_) if !unchanged => {
            CheckAnswer::Unchecked(UncheckedCause::ProbeTargetChanged)
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// The scratch directory
// ---------------------------------------------------------------------------

/// One probe's scratch directory, removed when dropped. A removal failure does
/// not change the answer; the next run sweeps leftovers under its lock.
struct Scratch {
    path: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Whether `probes` is the private directory `StateDir::ensure` keeps: a real
/// directory, not a link, owned by `euid`, open to nobody else. A planted link
/// or file there refuses.
fn probes_directory_holds(probes: &Path, euid: u32) -> bool {
    std::fs::symlink_metadata(probes).is_ok_and(|meta| {
        let open_to_others = meta.mode() & 0o077 != 0;
        meta.file_type().is_dir() && meta.uid() == euid && !open_to_others
    })
}

/// Whether `name` has exactly the shape [`scratch_name`] gives:
/// `probe-<decimal pid>-<six lowercase hex>`.
fn is_scratch_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("probe-") else {
        return false;
    };
    let Some((pid, bytes)) = rest.split_once('-') else {
        return false;
    };
    !pid.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && bytes.len() == 6
        && bytes
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Removes what a crashed run or a surviving daemon left under `probes`, and
/// **only** that: an entry whose name is exactly a scratch name and that `lstat`
/// shows as a directory owned by `euid` at mode `0700`. If anything else is
/// there, nothing is removed and the probe is refused, because the state root
/// can be an override such as `$HOME` and its `probes` the user's own. Safe only
/// under the run's lock, which is why [`run_curated_probe`] takes it.
fn sweep_leftovers(probes: &Path, euid: u32) -> Result<(), ()> {
    // Assumes probes run one at a time in this process; running them in parallel
    // would need this to skip this pid's own scratch names.
    let entries = std::fs::read_dir(probes).map_err(|_| ())?;
    let mut leftovers = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ())?;
        let path = entry.path();
        let name_holds = entry.file_name().to_str().is_some_and(is_scratch_name);
        let meta = std::fs::symlink_metadata(&path).map_err(|_| ())?;
        let ours =
            meta.file_type().is_dir() && meta.uid() == euid && meta.mode() & 0o777 == SCRATCH_MODE;
        if !(name_holds && ours) {
            return Err(());
        }
        leftovers.push(path);
    }
    for path in leftovers {
        // `remove_dir_all` removes a link inside as a link; it follows none.
        let _ = std::fs::remove_dir_all(path);
    }
    Ok(())
}

/// Makes `probes/<name>` at [`SCRATCH_MODE`], failing if anything is there, and
/// checks what was made: a directory, owned by `euid`, which is not root.
fn make_scratch(probes: &Path, name: &str, euid: u32) -> Option<Scratch> {
    let path = probes.join(name);
    DirBuilder::new().mode(SCRATCH_MODE).create(&path).ok()?;
    let scratch = Scratch { path };
    let meta = std::fs::symlink_metadata(&scratch.path).ok()?;
    let holds = meta.file_type().is_dir()
        && meta.uid() == euid
        && meta.uid() != 0
        && meta.mode() & 0o777 == SCRATCH_MODE;
    holds.then_some(scratch)
}

/// A fresh name: this process and three bytes of the system's random device.
fn scratch_name() -> Option<String> {
    let bytes = crate::entropy::fresh_bytes()?;
    Some(format!(
        "probe-{}-{:02x}{:02x}{:02x}",
        std::process::id(),
        bytes[0],
        bytes[1],
        bytes[2]
    ))
}

/// Every provider prefix this layer knows: the detected one and every known
/// provider's documented candidates.
fn known_prefixes(detected: &Path) -> Vec<PathBuf> {
    let mut prefixes = vec![detected.to_path_buf()];
    for id in crate::registry::known_provider_ids() {
        prefixes.extend(crate::registry::candidate_prefixes(&id));
    }
    prefixes
}

/// Whether `root` is, or lies under, any provider's documented or detected
/// prefix, as written or canonically. The state directory's override can point
/// anywhere, and scratch under a prefix would let a probe write into it.
fn state_root_under_a_prefix(root: &Path, detected: &Path) -> bool {
    let real = std::fs::canonicalize(root).ok();
    let roots = std::iter::once(root).chain(real.as_deref());
    known_prefixes(detected)
        .iter()
        .any(|prefix| roots.clone().any(|r| lies_under(r, prefix)))
}

/// Whether `lock` is this state root's run lock.
fn lock_is_this_roots(lock: &RunLock, state: &StateDir) -> bool {
    lock.path() == state.root().join(crate::persistence::run_lock::LOCK_FILE)
}

// ---------------------------------------------------------------------------
// The whole probe
// ---------------------------------------------------------------------------

/// Runs the curated probe of `leaf` of `package` with `argv`, on `owner`'s
/// adapter, under the run's lock, and answers. The table is the embedded one;
/// nothing here accepts a table or a prefix from a caller, and the uid is this
/// process's own.
pub(crate) fn run_curated_probe(
    owner: &dyn ProbeOwner,
    state: &StateDir,
    lock: &RunLock,
    package: &str,
    leaf: &str,
    argv: &[String],
) -> CheckAnswer {
    match table() {
        Ok(rows) => probe_with(rows, owner, state, lock, package, leaf, argv),
        Err(_) => CheckAnswer::Unchecked(UncheckedCause::NoCuratedProbe),
    }
}

fn probe_with(
    rows: &[Row],
    owner: &dyn ProbeOwner,
    state: &StateDir,
    lock: &RunLock,
    package: &str,
    leaf: &str,
    argv: &[String],
) -> CheckAnswer {
    let admitted = match AdmittedProbe::admit(rows, owner, package, leaf, argv) {
        Ok(admitted) => admitted,
        Err(cause) => return CheckAnswer::Unchecked(cause),
    };
    let euid = admitted.euid;
    let not_started = CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted);
    if lock.verify_still_current().is_err()
        || !lock_is_this_roots(lock, state)
        || state_root_under_a_prefix(state.root(), owner.prefix())
    {
        return not_started;
    }
    let probes = state.probes();
    if !probes_directory_holds(&probes, euid) || sweep_leftovers(&probes, euid).is_err() {
        return not_started;
    }
    let Some(scratch) = scratch_name().and_then(|name| make_scratch(&probes, &name, euid)) else {
        return not_started;
    };
    let program = admitted.program.clone();
    let target = admitted.target;
    let spec = CommandSpec::curated_probe(admitted, &scratch.path);
    let run = owner.runner().run_probe(&spec);
    let unchanged = TargetIdentity::read(&program) == Some(target);
    drop(scratch);
    answer(&run, unchanged)
}
#[cfg(test)]
mod tests;
