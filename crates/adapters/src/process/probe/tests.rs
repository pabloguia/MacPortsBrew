// SPDX-License-Identifier: Apache-2.0
#![cfg(test)]

use super::*;
use crate::persistence::run_lock;
use crate::process::system::{spawn_alone, StreamEnd};
use crate::process::{Recording, RecordingProvenance, ReplayRunner, SystemRunner};
use std::cell::{Cell, RefCell};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::rc::Rc;

const PROVIDER: &str = "test-provider";
const PACKAGE: &str = "pkg";
/// Not on `PERMITTED_IN_TESTS`, so the allowlist refuses it in a test build too.
const LEAF: &str = "mpb-probe-target";
const VERSION: &str = "1.0_0";
/// A target condition 11 admits: an empty text stands for a copy of
/// `/usr/bin/true`, a Mach-O that exits 0 whatever it is given. (A copy of
/// `/usr/bin/touch` or `/bin/test` is killed by the system at exec, observed
/// 2026-11-08, so neither can be a placed target.)
const ADMISSIBLE: &str = "";

fn flag() -> Vec<String> {
    vec!["--version".to_owned()]
}

fn row(provider: &str, package: &str, executable: &str, argv: &[&str], version: &str) -> Row {
    Row {
        provider: provider.to_owned(),
        package: package.to_owned(),
        executable: executable.to_owned(),
        argv: argv.iter().map(|a| (*a).to_owned()).collect(),
        observed_version: version.to_owned(),
        source: "Observed in a test".to_owned(),
        read_on: "2026-11-08".to_owned(),
    }
}

fn the_row() -> Vec<Row> {
    vec![row(PROVIDER, PACKAGE, LEAF, &["--version"], VERSION)]
}

fn write_executable(path: &Path, text: &str) {
    if text.is_empty() {
        std::fs::copy("/usr/bin/true", path).expect("copy");
    } else {
        std::fs::write(path, text).expect("write");
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("mode");
}

/// A scratch machine: a prefix with a placed link to a keg file, and a state
/// directory under the lock, both with a space in their path.
struct Fixture {
    base: PathBuf,
    prefix: PathBuf,
    canonical: PathBuf,
    state: StateDir,
    euid: u32,
    lock: Option<RunLock>,
}

impl Fixture {
    fn new(name: &str, script: &str) -> Self {
        let base = std::env::temp_dir().join(format!("mpb probe {name} {}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let prefix = base.join("prefix");
        let keg = prefix
            .join("Cellar")
            .join(PACKAGE)
            .join(VERSION)
            .join("bin");
        std::fs::create_dir_all(&keg).expect("keg");
        std::fs::create_dir_all(prefix.join("bin")).expect("bin");
        let canonical = keg.join(LEAF);
        write_executable(&canonical, script);
        symlink(&canonical, prefix.join("bin").join(LEAF)).expect("placed");
        let state = StateDir::resolve(
            None,
            Some(
                &base
                    .join("state dir, long enough to matter")
                    .to_string_lossy(),
            ),
        )
        .expect("state");
        state.ensure().expect("ensure");
        let euid = std::fs::metadata(state.root()).expect("root").uid();
        let lock = run_lock::acquire(&state, euid).expect("lock");
        Self {
            canonical: std::fs::canonicalize(&canonical).expect("canonical"),
            base,
            prefix,
            state,
            euid,
            lock: Some(lock),
        }
    }

    fn place(&self, leaf: &str, target: &Path) {
        symlink(target, self.prefix.join("bin").join(leaf)).expect("placed");
    }

    fn unplace(&self, leaf: &str) {
        std::fs::remove_file(self.prefix.join("bin").join(leaf)).expect("unplace");
    }

    fn lock(&self) -> &RunLock {
        self.lock.as_ref().expect("held")
    }

    fn probes_left(&self) -> usize {
        std::fs::read_dir(self.state.probes()).map_or(0, Iterator::count)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.lock.take();
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// The live runner, counting the probes it was asked to start.
struct Counting {
    inner: SystemRunner,
    started: Rc<Cell<usize>>,
    claims_replay: bool,
    /// A file to change once the probe has ended, as a program that rewrote
    /// itself would.
    after: Option<PathBuf>,
}

impl ProcessRunner for Counting {
    fn run(
        &self,
        spec: &CommandSpec,
    ) -> Result<crate::process::Invocation, macportsbrew_domain::DomainError> {
        self.inner.run(spec)
    }
    fn is_replay(&self) -> bool {
        self.claims_replay
    }
    fn run_probe(&self, spec: &CommandSpec) -> ProbeRun {
        self.started.set(self.started.get() + 1);
        let run = self.inner.run_probe(spec);
        if let Some(path) = &self.after {
            let mut bytes = std::fs::read(path).expect("read");
            bytes.push(0);
            std::fs::write(path, bytes).expect("rewrite");
        }
        run
    }
    fn may_run_probes(&self) -> bool {
        self.inner.may_run_probes()
    }
}

/// The adapter's side, as a test double over the fixture.
struct Owner {
    prefix: PathBuf,
    runner: Box<dyn ProcessRunner>,
    started: Rc<Cell<usize>>,
    provider: &'static str,
    this_machine: bool,
    entry: OwnershipFacts,
    target: Result<bool, UncheckedCause>,
    entry_asked: RefCell<Vec<PathBuf>>,
    target_asked: RefCell<Vec<(PathBuf, PathBuf)>>,
}

impl Owner {
    fn of(fx: &Fixture) -> Self {
        let started = Rc::new(Cell::new(0));
        Self {
            prefix: fx.prefix.clone(),
            runner: Box::new(Counting {
                inner: SystemRunner::new(),
                started: Rc::clone(&started),
                claims_replay: false,
                after: None,
            }),
            started,
            provider: PROVIDER,
            this_machine: true,
            entry: owned(PACKAGE, VERSION),
            target: Ok(true),
            entry_asked: RefCell::new(Vec::new()),
            target_asked: RefCell::new(Vec::new()),
        }
    }

    fn started(&self) -> usize {
        self.started.get()
    }
}

fn owned(name: &str, version: &str) -> OwnershipFacts {
    OwnershipFacts::Owner {
        name: name.to_owned(),
        version: version.to_owned(),
        active: Some(version.to_owned()),
        versions_present: 1,
        held: None,
    }
}

impl ProbeOwner for Owner {
    fn provider(&self) -> &str {
        self.provider
    }
    fn is_this_machine(&self) -> bool {
        self.this_machine
    }
    fn runner(&self) -> &dyn ProcessRunner {
        self.runner.as_ref()
    }
    fn prefix(&self) -> &Path {
        &self.prefix
    }
    fn entry_owner(&self, _package: &str, _leaf: &ExecutableName, placed: &Path) -> OwnershipFacts {
        self.entry_asked.borrow_mut().push(placed.to_path_buf());
        self.entry.clone()
    }
    fn target_is_packages(
        &self,
        _package: &str,
        placed: &Path,
        canonical: &Path,
    ) -> Result<bool, UncheckedCause> {
        self.target_asked
            .borrow_mut()
            .push((placed.to_path_buf(), canonical.to_path_buf()));
        self.target
    }
}

fn probe(fx: &Fixture, owner: &Owner, table: &[Row], leaf: &str, argv: &[String]) -> CheckAnswer {
    probe_with(table, owner, &fx.state, fx.lock(), PACKAGE, leaf, argv)
}

/// Asserts `cause`, that nothing was started and no scratch was left. Each
/// refusal test starts from the passing fixture and changes one thing, so
/// dropping its condition reaches the runner.
fn refused(fx: &Fixture, owner: &Owner, answer: CheckAnswer, cause: UncheckedCause) {
    assert_eq!(answer, CheckAnswer::Unchecked(cause));
    assert_eq!(owner.started(), 0, "{cause:?}: a probe was started");
    assert_eq!(fx.probes_left(), 0, "{cause:?}: scratch was left");
}

// ----- the passing case -----

#[test]
fn a_probe_that_passes_every_condition_holds_and_leaves_no_scratch() {
    let _alone = spawn_alone();
    let fx = Fixture::new("passes", ADMISSIBLE);
    let owner = Owner::of(&fx);
    assert_eq!(
        probe(&fx, &owner, &the_row(), LEAF, &flag()),
        CheckAnswer::Holds
    );
    assert_eq!(owner.started(), 1);
    assert_eq!(fx.probes_left(), 0, "the scratch directory is gone");
    let placed = fx.prefix.join("bin").join(LEAF);
    assert_eq!(
        *owner.entry_asked.borrow(),
        vec![placed.clone()],
        "the rule judged the placed entry"
    );
    assert_eq!(
        *owner.target_asked.borrow(),
        vec![(placed, fx.canonical.clone())]
    );
}

// ----- the eleven conditions -----

#[test]
fn condition_1_another_machine_or_a_replay_runs_nothing() {
    let fx = Fixture::new("c1", ADMISSIBLE);
    let mut owner = Owner::of(&fx);
    owner.this_machine = false;
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::NotObserved);
    let mut owner = Owner::of(&fx);
    owner.runner = Box::new(ReplayRunner::new(recording_answering(LEAF)));
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::NotObserved);
}

#[test]
fn condition_1_the_admission_asks_the_runner_itself() {
    // A runner that says it is a replay and would still start a probe.
    let _alone = spawn_alone();
    let fx = Fixture::new("c1-runner", ADMISSIBLE);
    let mut owner = Owner::of(&fx);
    owner.runner = Box::new(Counting {
        inner: SystemRunner::new(),
        started: Rc::clone(&owner.started),
        claims_replay: true,
        after: None,
    });
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::NotObserved);
}

#[test]
fn condition_1_a_recording_runner_touches_nothing_of_the_state_directory() {
    let _alone = spawn_alone();
    let fx = Fixture::new("c1-recording", ADMISSIBLE);
    // A leftover the sweep would remove: it is still there afterwards.
    let leftover = fx.state.probes().join("probe-1-abcdef");
    DirBuilder::new()
        .mode(0o700)
        .create(&leftover)
        .expect("leftover");
    let mut owner = Owner::of(&fx);
    let recording = Rc::new(SystemRunner::recording());
    owner.runner = Box::new(Counting {
        inner: SystemRunner::recording(),
        started: Rc::clone(&owner.started),
        claims_replay: false,
        after: None,
    });
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    assert_eq!(answer, CheckAnswer::Unchecked(UncheckedCause::NotObserved));
    assert_eq!(owner.started(), 0);
    assert!(leftover.exists(), "a capture swept the state directory");
    assert_eq!(fx.probes_left(), 1);
    // And through the trait's forwarding, nothing reaches a record.
    assert!(!recording.may_run_probes());
    assert!(recording.take_recorded().is_empty());
}

#[test]
fn invariant_06_a_probe_is_never_admitted_as_root() {
    let fx = Fixture::new("c2", ADMISSIBLE);
    let owner = Owner::of(&fx);
    TREAT_AS_ROOT.with(|root| root.set(true));
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    TREAT_AS_ROOT.with(|root| root.set(false));
    refused(&fx, &owner, answer, UncheckedCause::ProbeAsRoot);
}

#[test]
fn condition_3_a_leaf_outside_the_name_class_is_refused() {
    let fx = Fixture::new("c3", ADMISSIBLE);
    fx.place("-mpb", &fx.canonical);
    let owner = Owner::of(&fx);
    let table = vec![row(PROVIDER, PACKAGE, "-mpb", &["--version"], VERSION)];
    let answer = probe(&fx, &owner, &table, "-mpb", &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeLeafRefused);
}

#[test]
fn condition_4_the_row_is_for_exactly_this_key_and_this_argv() {
    let fx = Fixture::new("c4", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let answer = probe(&fx, &owner, &[], LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::NoCuratedProbe);
    for table in [
        vec![row(
            "other-provider",
            PACKAGE,
            LEAF,
            &["--version"],
            VERSION,
        )],
        vec![row(
            PROVIDER,
            "other-package",
            LEAF,
            &["--version"],
            VERSION,
        )],
        vec![row(
            PROVIDER,
            PACKAGE,
            "other-leaf",
            &["--version"],
            VERSION,
        )],
    ] {
        let answer = probe(&fx, &owner, &table, LEAF, &flag());
        refused(&fx, &owner, answer, UncheckedCause::NoCuratedProbe);
    }
    let other = vec!["-V".to_owned()];
    let answer = probe(&fx, &owner, &the_row(), LEAF, &other);
    refused(&fx, &owner, answer, UncheckedCause::ProbeArgvNotTheRows);
}

#[test]
fn condition_5_a_flag_outside_the_closed_set_is_refused_even_when_a_row_says_it() {
    let fx = Fixture::new("c5", ADMISSIBLE);
    let owner = Owner::of(&fx);
    for argv in [&["--daemon"][..], &[], &["--version", "--version"]] {
        let table = vec![row(PROVIDER, PACKAGE, LEAF, argv, VERSION)];
        let plan: Vec<String> = argv.iter().map(|a| (*a).to_owned()).collect();
        let answer = probe(&fx, &owner, &table, LEAF, &plan);
        refused(&fx, &owner, answer, UncheckedCause::ProbeArgvNotPermitted);
    }
}

#[test]
fn condition_6_only_the_exact_observed_version_revision_included() {
    let fx = Fixture::new("c6", ADMISSIBLE);
    for version in ["1.0_1", "1.0", "2.0_0"] {
        let mut owner = Owner::of(&fx);
        owner.entry = owned(PACKAGE, version);
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(
            &fx,
            &owner,
            answer,
            UncheckedCause::ProbeVersionNotQualified,
        );
    }
}

#[test]
fn condition_7_the_placed_entry_is_the_packages_by_the_owner_rule_now() {
    let fx = Fixture::new("c7", ADMISSIBLE);
    for entry in [
        owned("another", VERSION),
        OwnershipFacts::NoEntry,
        OwnershipFacts::NotOwned,
        OwnershipFacts::OwnerLeavesItsDirectory {
            owner: PACKAGE.to_owned(),
        },
        OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed),
    ] {
        let mut owner = Owner::of(&fx);
        owner.entry = entry;
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(
            &fx,
            &owner,
            answer,
            UncheckedCause::ProbeEntryNotThePackages,
        );
    }
    // Nothing placed, whatever the rule says.
    fx.unplace(LEAF);
    let owner = Owner::of(&fx);
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(
        &fx,
        &owner,
        answer,
        UncheckedCause::ProbeEntryNotThePackages,
    );
}

#[test]
fn condition_8_what_it_resolves_to_is_the_packages_and_under_the_prefix() {
    let fx = Fixture::new("c8", ADMISSIBLE);
    for target in [Ok(false), Err(UncheckedCause::ReadFailed)] {
        let mut owner = Owner::of(&fx);
        owner.target = target;
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(
            &fx,
            &owner,
            answer,
            UncheckedCause::ProbeTargetNotThePackages,
        );
    }
    // The floor: outside the destination's prefix, whatever the rule says.
    let outside = fx.base.join(LEAF);
    write_executable(&outside, ADMISSIBLE);
    fx.unplace(LEAF);
    fx.place(LEAF, &outside);
    let owner = Owner::of(&fx);
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(
        &fx,
        &owner,
        answer,
        UncheckedCause::ProbeTargetNotThePackages,
    );
    assert!(lies_under(
        Path::new("/opt/local/bin/x"),
        Path::new("/opt/local")
    ));
    assert!(!lies_under(
        Path::new("/opt/local2/bin/x"),
        Path::new("/opt/local")
    ));
}

#[test]
fn condition_9_a_target_of_the_wrong_type_mode_or_owner_is_refused() {
    let fx = Fixture::new("c9", ADMISSIBLE);
    let owner = Owner::of(&fx);
    for mode in [0o775, 0o757, 0o4755, 0o2755] {
        std::fs::set_permissions(&fx.canonical, std::fs::Permissions::from_mode(mode))
            .expect("mode");
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(&fx, &owner, answer, UncheckedCause::ProbeTargetModeRefused);
    }
    // A directory is not a regular file.
    std::fs::set_permissions(&fx.canonical, std::fs::Permissions::from_mode(0o755)).expect("mode");
    fx.unplace(LEAF);
    fx.place(LEAF, fx.canonical.parent().expect("keg bin"));
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeTargetModeRefused);
    // The owner, which a test cannot make another user: pure.
    assert!(!target_mode_refused(true, 501, 0o755, 501));
    assert!(!target_mode_refused(true, 0, 0o755, 501));
    assert!(target_mode_refused(true, 502, 0o755, 501));
    assert!(target_mode_refused(false, 501, 0o755, 501));
}

#[test]
fn invariant_05_a_probe_never_starts_a_provider_program() {
    let fx = Fixture::new("c10", ADMISSIBLE);
    let owner = Owner::of(&fx);
    for leaf in ["bash", "brew", "sw_vers", "Python3.12"] {
        fx.place(leaf, &fx.canonical);
        let table = vec![row(PROVIDER, PACKAGE, leaf, &["--version"], VERSION)];
        let answer = probe(&fx, &owner, &table, leaf, &flag());
        refused(&fx, &owner, answer, UncheckedCause::ProbeProgramExcluded);
    }
    // The canonical leaf too: `bin/foo -> libexec/python3`.
    let interpreter = fx.canonical.with_file_name("perl5.34");
    std::fs::rename(&fx.canonical, &interpreter).expect("rename");
    fx.unplace(LEAF);
    fx.place(LEAF, &interpreter);
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeProgramExcluded);
    // The canonical leaf a provider's own program.
    let provider_program = interpreter.with_file_name("brew");
    std::fs::rename(&interpreter, &provider_program).expect("rename");
    fx.unplace(LEAF);
    fx.place(LEAF, &provider_program);
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeProgramExcluded);
    std::fs::rename(&provider_program, &fx.canonical).expect("restore");
    fx.unplace(LEAF);
    fx.place(LEAF, &fx.canonical);
    // A text target with no `#!`, which the kernel would hand to `/bin/sh`,
    // naming a provider's program on the probe's `PATH`.
    std::fs::write(&fx.canonical, "brew --version\nport version\n").expect("write");
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeTargetIsEnvScript);
    std::fs::copy("/usr/bin/true", &fx.canonical).expect("restore");
    // And at the site, whatever admitted it: the program, the canonical
    // leaf it is, and `arg0`.
    let (spec, _scratch) = admitted_spec(&fx, &owner);
    assert!(crate::process::may_spawn(&spec));
    for program in [
        "/opt/homebrew/bin/brew",
        "/opt/local/bin/port",
        "/usr/sbin/sysctl",
    ] {
        let mut s = spec.clone();
        s.program = PathBuf::from(program);
        assert!(!crate::process::may_spawn(&s), "{program}");
    }
    for arg0 in ["brew", "port", "sw_vers", "bash"] {
        let mut s = spec.clone();
        if let Admission::CuratedProbe { arg0: a, .. } = &mut s.admission {
            *a = arg0.to_owned();
        }
        assert!(!crate::process::may_spawn(&s), "{arg0}");
    }
    for leaf in [
        "sh",
        "tclsh8.6",
        "ENV",
        "port",
        "xcode-select",
        "node22",
        "ruby3.3",
    ] {
        assert!(is_excluded_leaf(leaf), "{leaf}");
    }
    for leaf in [
        "xz",
        "htop",
        "gm4",
        "pkg-config",
        "sha1sum",
        "shasum",
        "tree",
        "env-x",
    ] {
        assert!(!is_excluded_leaf(leaf), "{leaf}");
    }
}

#[test]
fn condition_11_only_a_macho_executable_for_this_build_is_admitted() {
    let runner_link = std::env::temp_dir().join(format!("mpb-runner-{}", std::process::id()));
    let _ = std::fs::remove_file(&runner_link);
    symlink("/bin/sh", &runner_link).expect("runner link");
    let shebang_through_link = format!("#!{}\n", runner_link.display());
    for (name, head, cause) in [
        (
            "c11-env",
            "#!/usr/bin/env sh\n",
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
        ("c11-rel", "#!sh\n", UncheckedCause::ProbeTargetIsEnvScript),
        (
            "c11-shell",
            "#!/bin/sh\n",
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
        (
            "c11-unresolved",
            "#!/mpb-nowhere/bash\n",
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
        (
            "c11-linked",
            shebang_through_link.as_str(),
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
        (
            "c11-other",
            "#!/opt/local/bin/mpb-interpreter\n",
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
        (
            "c11-other2",
            "#!/usr/local/bin/mpb-interpreter\n",
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
        (
            "c11-absolute",
            "#!/usr/bin/true\n",
            UncheckedCause::ProbeTargetIsEnvScript,
        ),
    ] {
        let fx = Fixture::new(name, head);
        let owner = Owner::of(&fx);
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(&fx, &owner, answer, cause);
    }
    let _ = std::fs::remove_file(&runner_link);
    // No `#!` at all, or not at its very start: macOS's spawn runs such a
    // file with `/bin/sh` on `ENOEXEC`. And files too short to be anything.
    let header = |cputype: u32, file_type: u32| -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [0xfeed_facf_u32, cputype, 3, file_type, 0, 0, 0, 0] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    };
    let other_cputype = if crate::macho::THIS_BUILD_CPUTYPE == 0x0100_0007 {
        0x0100_000c
    } else {
        0x0100_0007
    };
    let bodies: [(&str, Vec<u8>); 8] = [
        ("c11-text", b"brew --version\n".to_vec()),
        ("c11-bom", b"\xef\xbb\xbf#!/bin/sh\nexit 0\n".to_vec()),
        ("c11-space", b" #!/bin/sh\nexit 0\n".to_vec()),
        ("c11-newline", b"\n#!/bin/sh\nexit 0\n".to_vec()),
        ("c11-empty", Vec::new()),
        ("c11-one-byte", b"x".to_vec()),
        ("c11-dylib", header(crate::macho::THIS_BUILD_CPUTYPE, 6)),
        ("c11-other-arch", header(other_cputype, 2)),
    ];
    for (name, bytes) in bodies {
        let fx = Fixture::new(name, ADMISSIBLE);
        std::fs::write(&fx.canonical, &bytes).expect("write");
        let owner = Owner::of(&fx);
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(&fx, &owner, answer, UncheckedCause::ProbeTargetIsEnvScript);
    }
    // Fat files, built by hand: the kernel, not this reader, picks the slice,
    // and execs only a big-endian fat header.
    let this = crate::macho::THIS_BUILD_CPUTYPE;
    let good = header(this, 2);
    let fat = |big_endian: bool, nfat: u32, slices: &[(u32, u32, Vec<u8>)], payload: &[u8]| {
        let word = |w: u32| {
            if big_endian {
                w.to_be_bytes()
            } else {
                w.to_le_bytes()
            }
        };
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&word(0xcafe_babe));
        bytes.extend_from_slice(&word(nfat));
        let mut place = Vec::new();
        for index in 0..usize::try_from(nfat).unwrap_or(0) {
            let which = index % slices.len();
            let (cputype, subtype, data) = &slices[which];
            let offset = 4096 * (1 + u32::try_from(which).unwrap_or(0));
            for w in [
                *cputype,
                *subtype,
                offset,
                u32::try_from(data.len()).unwrap_or(0),
                12,
            ] {
                bytes.extend_from_slice(&word(w));
            }
            place.push((offset, data.clone()));
        }
        bytes.extend_from_slice(payload);
        for (offset, data) in place {
            let at = usize::try_from(offset).unwrap_or(0);
            if bytes.len() < at + data.len() {
                bytes.resize(at + data.len(), 0);
            }
            bytes[at..at + data.len()].copy_from_slice(&data);
        }
        bytes
    };
    let x86_64h = 8;
    let swapped: Vec<u8> = [0xfeed_facf_u32, this, 3, 2, 0, 0, 0, 0]
        .iter()
        .flat_map(|w| w.to_be_bytes())
        .collect();
    let fats: [(&str, Vec<u8>); 7] = [
        ("c11-thin-swapped", swapped.clone()),
        (
            "c11-fat-swapped-slice",
            fat(true, 1, &[(this, 3, swapped)], b""),
        ),
        (
            "c11-fat-cigam",
            fat(
                false,
                10,
                &[(this, 3, good.clone())],
                b"\necho ran; exit 0\n",
            ),
        ),
        (
            "c11-fat-text",
            fat(
                true,
                2,
                &[
                    (this, 3, good.clone()),
                    (this, x86_64h, b"echo ran; exit 0\n".to_vec()),
                ],
                b"",
            ),
        ),
        (
            "c11-fat-dylib",
            fat(
                true,
                2,
                &[(this, 3, good.clone()), (this, x86_64h, header(this, 6))],
                b"",
            ),
        ),
        (
            "c11-fat-junk-other",
            fat(
                true,
                2,
                &[
                    (this, 3, good.clone()),
                    (
                        other_cputype,
                        0,
                        b"junk junk junk junk junk junk junk".to_vec(),
                    ),
                ],
                b"",
            ),
        ),
        (
            "c11-fat-nested",
            fat(
                true,
                2,
                &[
                    (this, 3, good.clone()),
                    (
                        other_cputype,
                        0,
                        fat(
                            true,
                            1,
                            &[(other_cputype, 0, header(other_cputype, 2))],
                            b"",
                        ),
                    ),
                ],
                b"",
            ),
        ),
    ];
    for (name, bytes) in fats {
        let fx = Fixture::new(name, ADMISSIBLE);
        std::fs::write(&fx.canonical, &bytes).expect("write");
        let owner = Owner::of(&fx);
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        refused(&fx, &owner, answer, UncheckedCause::ProbeTargetIsEnvScript);
    }
    // The controls. The copy of `/usr/bin/true` is admitted: a big-endian fat
    // file whose two slices (`x86_64`, `arm64e`) are both `MH_EXECUTE`, one for
    // this build. So are a big-endian fat of two hand-built executables, one
    // for this build, and the thin hand-built header for this build's
    // architecture, which shows the other-architecture header above was
    // refused for its architecture alone. The hand-built ones carry no
    // `LC_MAIN`: the kernel would refuse them (`EBADMACHO`, so
    // `PROBE_NOT_STARTED`, no shell), and they are only ever admitted here,
    // never run.
    let fx = Fixture::new("c11-this-arch", ADMISSIBLE);
    let owner = Owner::of(&fx);
    assert!(AdmittedProbe::admit(&the_row(), &owner, PACKAGE, LEAF, &flag()).is_ok());
    let both = fat(
        true,
        2,
        &[
            (this, 3, good.clone()),
            (other_cputype, 0, header(other_cputype, 2)),
        ],
        b"",
    );
    std::fs::write(&fx.canonical, both).expect("write");
    assert!(AdmittedProbe::admit(&the_row(), &owner, PACKAGE, LEAF, &flag()).is_ok());
    std::fs::write(&fx.canonical, header(this, 2)).expect("write");
    assert!(AdmittedProbe::admit(&the_row(), &owner, PACKAGE, LEAF, &flag()).is_ok());
    assert_eq!(owner.started(), 0);
    // A head that cannot be read is condition 11's, not condition 9's.
    let fx = Fixture::new("c11-unreadable", ADMISSIBLE);
    std::fs::set_permissions(&fx.canonical, std::fs::Permissions::from_mode(0o311)).expect("mode");
    let owner = Owner::of(&fx);
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeTargetIsEnvScript);
}

#[test]
fn condition_11_reads_the_head_only_of_the_file_lstat_described() {
    // The race between the `lstat` and the open, made by hand: the handle is
    // another file than the identity admitted, so nothing is read off it.
    let fx = Fixture::new("c11-handle", ADMISSIBLE);
    let other = fx.base.join("other");
    write_executable(&other, ADMISSIBLE);
    let admitted = TargetIdentity::read(&other).expect("identity");
    assert_eq!(
        target_head_admits(&fx.canonical, &admitted),
        Err(UncheckedCause::ProbeTargetChanged)
    );
    let own = TargetIdentity::read(&fx.canonical).expect("identity");
    assert_eq!(target_head_admits(&fx.canonical, &own), Ok(()));
}

// ----- the lock, the state root, the scratch directory -----

#[test]
fn the_lock_must_be_this_state_roots() {
    let fx = Fixture::new("lock", ADMISSIBLE);
    let other = StateDir::resolve(None, Some(&fx.base.join("other state").to_string_lossy()))
        .expect("state");
    other.ensure().expect("ensure");
    let lock = run_lock::acquire(&other, fx.euid).expect("lock");
    let owner = Owner::of(&fx);
    let answer = probe_with(&the_row(), &owner, &fx.state, &lock, PACKAGE, LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::ProbeNotStarted);
}

#[test]
fn the_scratch_directory_is_fresh_private_and_refused_when_planted() {
    let fx = Fixture::new("scratch", ADMISSIBLE);
    let probes = fx.state.probes();
    let made = make_scratch(&probes, "probe-a", fx.euid).expect("made");
    let meta = std::fs::symlink_metadata(&made.path).expect("meta");
    assert_eq!(meta.mode() & 0o777, 0o700);
    let path = made.path.clone();
    drop(made);
    assert!(!path.exists(), "removed when dropped");
    std::fs::write(probes.join("probe-file"), "x").expect("file");
    assert!(make_scratch(&probes, "probe-file", fx.euid).is_none());
    symlink(&fx.base, probes.join("probe-link")).expect("link");
    assert!(make_scratch(&probes, "probe-link", fx.euid).is_none());
    assert!(make_scratch(&probes, "probe-b", fx.euid + 1).is_none());
    assert!(
        !probes.join("probe-b").exists(),
        "a refused scratch is removed"
    );
    assert!(is_scratch_name(&scratch_name().expect("name")));
    for name in ["probe-1-abcdef", "probe-123456-000fff"] {
        assert!(is_scratch_name(name), "{name}");
    }
    for name in [
        "probe-1-ABCDEF",
        "probe--abcdef",
        "probe-1-abcde",
        "probe-1-abcdefa",
        "probe-x-abcdef",
        "probe-1abcdef",
        "notes",
        "probe-1-abcdef/",
    ] {
        assert!(!is_scratch_name(name), "{name}");
    }
}

#[test]
fn only_a_scratch_shaped_leftover_is_swept_and_a_link_is_not_followed() {
    let _alone = spawn_alone();
    let fx = Fixture::new("sweep", ADMISSIBLE);
    let probes = fx.state.probes();
    let outside = fx.base.join("outside");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("keep"), "x").expect("keep");
    let old = probes.join("probe-77-0a0b0c");
    DirBuilder::new().mode(0o700).create(&old).expect("old");
    std::fs::create_dir_all(old.join("deep")).expect("deep");
    symlink(&outside, old.join("link")).expect("link");
    let owner = Owner::of(&fx);
    assert_eq!(
        probe(&fx, &owner, &the_row(), LEAF, &flag()),
        CheckAnswer::Holds
    );
    assert_eq!(fx.probes_left(), 0);
    assert!(outside.join("keep").exists(), "the link was followed");
}

#[test]
fn anything_else_under_probes_is_left_alone_and_refuses_the_probe() {
    let fx = Fixture::new("foreign", ADMISSIBLE);
    let probes = fx.state.probes();
    let leftover = probes.join("probe-77-0a0b0c");
    DirBuilder::new()
        .mode(0o700)
        .create(&leftover)
        .expect("leftover");
    let file = probes.join("notes.txt");
    let dir = probes.join("mine");
    let loose = probes.join("probe-78-0a0b0c");
    let private = probes.join("mine-too");
    let link = probes.join("probe-79-0a0b0c");
    for planted in 0..5 {
        match planted {
            0 => std::fs::write(&file, "the user's").expect("file"),
            1 => std::fs::create_dir_all(dir.join("inside")).expect("dir"),
            2 => DirBuilder::new().mode(0o755).create(&loose).expect("loose"),
            3 => DirBuilder::new()
                .mode(0o700)
                .create(&private)
                .expect("private"),
            _ => symlink(&fx.base, &link).expect("link"),
        }
        let owner = Owner::of(&fx);
        let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
        assert_eq!(
            answer,
            CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted),
            "{planted}"
        );
        assert_eq!(owner.started(), 0);
        assert!(leftover.exists(), "{planted}: something was removed");
        match planted {
            0 => {
                assert_eq!(std::fs::read_to_string(&file).expect("kept"), "the user's");
                std::fs::remove_file(&file).expect("clean");
            }
            1 => {
                assert!(dir.join("inside").exists());
                std::fs::remove_dir_all(&dir).expect("clean");
            }
            2 => {
                assert!(loose.exists());
                std::fs::remove_dir(&loose).expect("clean");
            }
            3 => {
                assert!(private.exists());
                std::fs::remove_dir(&private).expect("clean");
            }
            _ => {
                assert!(std::fs::symlink_metadata(&link).is_ok());
                std::fs::remove_file(&link).expect("clean");
            }
        }
    }
}

#[test]
fn a_planted_probes_directory_or_a_state_root_under_a_prefix_starts_nothing() {
    let fx = Fixture::new("planted", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let probes = fx.state.probes();
    std::fs::remove_dir_all(&probes).expect("unprobes");
    symlink(&fx.base, &probes).expect("planted link");
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    assert_eq!(
        answer,
        CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
    );
    std::fs::remove_file(&probes).expect("unplant");
    std::fs::write(&probes, "x").expect("planted file");
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    assert_eq!(
        answer,
        CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
    );
    std::fs::remove_file(&probes).expect("unplant");
    DirBuilder::new()
        .mode(0o700)
        .create(&probes)
        .expect("probes");
    std::fs::set_permissions(&probes, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    assert_eq!(
        answer,
        CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
    );
    std::fs::remove_dir(&probes).expect("unprobes");
    let empty = fx.base.join("empty");
    DirBuilder::new().mode(0o700).create(&empty).expect("empty");
    symlink(&empty, &probes).expect("planted link to an empty directory");
    let answer = probe(&fx, &owner, &the_row(), LEAF, &flag());
    assert_eq!(
        answer,
        CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
    );
    let inside = StateDir::resolve(
        None,
        Some(&fx.prefix.join("var").join("state").to_string_lossy()),
    )
    .expect("state");
    inside.ensure().expect("ensure");
    let lock = run_lock::acquire(&inside, fx.euid).expect("lock");
    let answer = probe_with(&the_row(), &owner, &inside, &lock, PACKAGE, LEAF, &flag());
    assert_eq!(
        answer,
        CheckAnswer::Unchecked(UncheckedCause::ProbeNotStarted)
    );
    assert_eq!(owner.started(), 0);
    assert!(state_root_under_a_prefix(
        Path::new("/opt/local/var/macportsbrew"),
        Path::new("/x")
    ));
    assert!(state_root_under_a_prefix(
        Path::new("/x/y"),
        Path::new("/x")
    ));
    assert!(!state_root_under_a_prefix(
        Path::new("/x2/y"),
        Path::new("/x")
    ));
}

#[test]
fn a_target_changed_by_the_run_is_never_holds() {
    // The runner changes the target once the probe has ended, as a program
    // that rewrote itself would: the identity read after exit differs.
    let _alone = spawn_alone();
    let fx = Fixture::new("self-edit", ADMISSIBLE);
    let mut owner = Owner::of(&fx);
    owner.runner = Box::new(Counting {
        inner: SystemRunner::new(),
        started: Rc::clone(&owner.started),
        claims_replay: false,
        after: Some(fx.canonical.clone()),
    });
    assert_eq!(
        probe(&fx, &owner, &the_row(), LEAF, &flag()),
        CheckAnswer::Unchecked(UncheckedCause::ProbeTargetChanged)
    );
    assert_eq!(owner.started(), 1);
    assert_eq!(fx.probes_left(), 0);
}

// ----- how it runs, on the runner itself -----

/// An admitted probe built by hand around `program`, as `admit` would for
/// a target it passed: for the runner's own behaviour, which a shell script
/// shows and condition 11 would refuse to admit.
fn direct(fx: &Fixture, program: &Path, arg0: &str) -> (CommandSpec, Scratch) {
    let admitted = AdmittedProbe {
        program: program.to_path_buf(),
        arg0: arg0.to_owned(),
        argv: flag(),
        destination_bin: fx.prefix.join("bin"),
        target: TargetIdentity::read(program).expect("identity"),
        euid: fx.euid,
    };
    let scratch = make_scratch(&fx.state.probes(), "probe-1-000000", fx.euid).expect("scratch");
    let spec = CommandSpec::curated_probe(admitted, &scratch.path);
    (spec, scratch)
}

fn shell(fx: &Fixture, body: &str) -> PathBuf {
    let path = fx.base.join("shell-target");
    write_executable(&path, &format!("#!/bin/sh\n{body}\n"));
    path
}

fn run_direct(fx: &Fixture, program: &Path, arg0: &str) -> CheckAnswer {
    let (spec, _scratch) = direct(fx, program, arg0);
    let target = TargetIdentity::read(program);
    let run = SystemRunner::new().run_probe(&spec);
    answer(&run, TargetIdentity::read(program) == target)
}

#[test]
fn invariant_10_a_probe_runs_in_exactly_its_environment() {
    let _alone = spawn_alone();
    let fx = Fixture::new("env", ADMISSIBLE);
    let probes = fx.state.probes();
    let body = format!(
        "[ \"$HOME\" = \"$TMPDIR\" ] || exit 11\n\
             [ -d \"$HOME\" ] || exit 12\n\
             [ \"$(cd \"$HOME\" && pwd -P)\" = \"$(pwd -P)\" ] || exit 13\n\
             [ \"$(stat -f %Lp \"$HOME\")\" = 700 ] || exit 14\n\
             [ \"$LANG$LC_ALL$TERM$NO_COLOR$PYTHONDONTWRITEBYTECODE\" = CCdumb11 ] || exit 15\n\
             [ \"$1\" = --version ] && [ $# = 1 ] || exit 16\n\
             case \"$HOME\" in '{probes}'/probe-*) ;; *) exit 17 ;; esac\n\
             [ \"${{PATH%%:*}}\" = '{bin}' ] || exit 18\n\
             [ \"${{PATH#*:}}\" = /usr/bin:/bin:/usr/sbin:/sbin ] || exit 19\n\
             exit 0",
        probes = probes.display(),
        bin = fx.prefix.join("bin").display(),
    );
    let program = shell(&fx, &body);
    assert_eq!(run_direct(&fx, &program, LEAF), CheckAnswer::Holds);
    // And the spec itself, key by key: nothing more reaches the kernel.
    let owner = Owner::of(&fx);
    let admitted =
        AdmittedProbe::admit(&the_row(), &owner, PACKAGE, LEAF, &flag()).expect("admitted");
    let scratch = Path::new("/private/var/scratch with space");
    let spec = CommandSpec::curated_probe(admitted, scratch);
    let expected: BTreeMap<String, String> = [
        ("HOME", scratch.to_str().unwrap_or_default().to_owned()),
        ("LANG", "C".to_owned()),
        ("LC_ALL", "C".to_owned()),
        ("NO_COLOR", "1".to_owned()),
        (
            "PATH",
            format!(
                "{}:/usr/bin:/bin:/usr/sbin:/sbin",
                fx.prefix.join("bin").display()
            ),
        ),
        ("PYTHONDONTWRITEBYTECODE", "1".to_owned()),
        ("TERM", "dumb".to_owned()),
        ("TMPDIR", scratch.to_str().unwrap_or_default().to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    assert_eq!(spec.environment, expected);
    assert!(!spec
        .environment
        .keys()
        .any(|k| macportsbrew_domain::is_build_flag(k)));
    assert_eq!(spec.program, fx.canonical, "the canonical path runs");
    assert_eq!(spec.timeout, PROBE_TIMEOUT);
    assert_eq!(spec.max_output_bytes, PROBE_MAX_OUTPUT_BYTES);
}

#[test]
fn arg0_is_the_placed_leaf_while_the_canonical_path_runs() {
    // `/bin/test` dispatches on its `argv[0]`: as `[` it requires a closing
    // `]` and exits 2; under any other name `test --version` is true. So a
    // run with `arg0` `[` that fails proves `arg0` reached the program, and a
    // runner passing the program path as `argv[0]` holds.
    let _alone = spawn_alone();
    let fx = Fixture::new("arg0", ADMISSIBLE);
    let test = Path::new("/bin/test");
    assert_eq!(
        run_direct(&fx, test, "["),
        CheckAnswer::Fails(CheckFailure::ProbeFailed)
    );
    assert_eq!(run_direct(&fx, test, "test-named"), CheckAnswer::Holds);
}

#[test]
fn the_answer_of_each_ending_on_a_real_program() {
    let _alone = spawn_alone();
    for (name, body, expected) in [
        (
            "exit1",
            "exit 1",
            CheckAnswer::Fails(CheckFailure::ProbeFailed),
        ),
        (
            "signal",
            "kill -TERM $$",
            CheckAnswer::Fails(CheckFailure::ProbeFailed),
        ),
        (
            "overcap",
            "head -c 70000 /dev/zero\nexit 0",
            CheckAnswer::Unchecked(UncheckedCause::ProbeOutputOverCap),
        ),
        (
            "held",
            "sleep 20 &\nexit 0",
            CheckAnswer::Unchecked(UncheckedCause::ProbeLeftAProcess),
        ),
    ] {
        let fx = Fixture::new(name, ADMISSIBLE);
        let program = shell(&fx, body);
        assert_eq!(run_direct(&fx, &program, LEAF), expected, "{name}");
    }
}

#[test]
fn a_timeout_is_unchecked_and_never_fails() {
    let _alone = spawn_alone();
    let fx = Fixture::new("timeout", ADMISSIBLE);
    let program = shell(&fx, "sleep 10\nexit 0");
    let (spec, _scratch) = direct(&fx, &program, LEAF);
    // Narrower than the ceiling is allowed; wider is not (below).
    let spec = spec.with_timeout(Duration::from_millis(300));
    let run = SystemRunner::new().run_probe(&spec);
    assert_eq!(
        answer(&run, true),
        CheckAnswer::Unchecked(UncheckedCause::ProbeTimedOut)
    );
}

/// Admits the fixture's target, then builds the spec over a fresh scratch,
/// as `probe_with` does.
fn admitted_spec(fx: &Fixture, owner: &Owner) -> (CommandSpec, Scratch) {
    let admitted =
        AdmittedProbe::admit(&the_row(), owner, PACKAGE, LEAF, &flag()).expect("admitted");
    let scratch = make_scratch(&fx.state.probes(), "probe-2-000000", fx.euid).expect("scratch");
    let spec = CommandSpec::curated_probe(admitted, &scratch.path);
    (spec, scratch)
}

#[test]
fn a_target_changed_between_admission_and_spawn_is_never_started() {
    let _alone = spawn_alone();
    let fx = Fixture::new("changed", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let (spec, _scratch) = admitted_spec(&fx, &owner);
    std::fs::write(&fx.canonical, "#!/usr/bin/true\n# replaced\n").expect("write");
    let run = SystemRunner::new().run_probe(&spec);
    assert_eq!(run, ProbeRun::target_changed());
    assert_eq!(
        answer(&run, false),
        CheckAnswer::Unchecked(UncheckedCause::ProbeTargetChanged)
    );
}

#[test]
fn a_link_re_pointed_after_admission_still_runs_what_was_admitted() {
    let _alone = spawn_alone();
    let fx = Fixture::new("repoint", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let (spec, _scratch) = admitted_spec(&fx, &owner);
    let other = fx.base.join("other");
    write_executable(&other, "#!/usr/bin/false\n");
    fx.unplace(LEAF);
    fx.place(LEAF, &other);
    let run = SystemRunner::new().run_probe(&spec);
    assert_eq!(
        answer(&run, true),
        CheckAnswer::Holds,
        "the admitted target ran"
    );
}

// ----- the site -----

#[test]
fn the_site_refuses_a_probe_widened_after_admission() {
    let fx = Fixture::new("site", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let (spec, _scratch) = admitted_spec(&fx, &owner);
    assert!(crate::process::may_spawn(&spec), "the admitted spec passes");
    assert!(crate::process::may_spawn(
        &spec.clone().with_timeout(Duration::from_secs(1))
    ));
    let mut widened: Vec<(&str, CommandSpec)> = Vec::new();
    widened.push((
        "timeout",
        spec.clone()
            .with_timeout(PROBE_TIMEOUT + Duration::from_millis(1)),
    ));
    let mut s = spec.clone();
    s.max_output_bytes += 1;
    widened.push(("cap", s));
    let mut s = spec.clone();
    s.environment
        .insert("DYLD_INSERT_LIBRARIES".into(), "/x".into());
    widened.push(("extra variable", s));
    let mut s = spec.clone();
    s.environment.remove("TMPDIR");
    widened.push(("missing variable", s));
    let mut s = spec.clone();
    s.environment
        .insert("HOME".into(), "/Users/somebody".into());
    widened.push(("real home", s));
    let mut s = spec.clone();
    let path = s.environment["PATH"].clone();
    s.environment
        .insert("PATH".into(), format!("/opt/local/bin:{path}"));
    widened.push(("other provider on PATH", s));
    let mut s = spec.clone();
    s.environment.insert("LANG".into(), "en_US.UTF-8".into());
    widened.push(("pinned value", s));
    for argv in [
        vec![],
        vec!["--daemon".to_owned()],
        vec!["-v".to_owned(), "-V".to_owned()],
    ] {
        let mut s = spec.clone();
        s.argv = argv;
        widened.push(("argv", s));
    }
    let mut s = spec.clone();
    s.program = PathBuf::from("/opt/homebrew/bin/brew");
    widened.push(("provider program", s));
    let mut s = spec.clone();
    s.program = PathBuf::from("relative/target");
    widened.push(("relative program", s));
    let mut s = spec.clone();
    s.program = fx.prefix.join("bin").join("..").join("bin").join(LEAF);
    widened.push(("traversal", s));
    for arg0 in ["port", "sh", "a/b", "-x"] {
        let mut s = spec.clone();
        if let Admission::CuratedProbe { arg0: a, .. } = &mut s.admission {
            *a = arg0.to_owned();
        }
        widened.push(("arg0", s));
    }
    for (what, spec) in &widened {
        assert!(!crate::process::may_spawn(spec), "{what} was admitted");
    }
}

#[test]
fn the_allowlist_refuses_a_probe_shaped_spec_and_run_refuses_a_probe() {
    let _alone = spawn_alone();
    let fx = Fixture::new("allowlist", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let (spec, _scratch) = admitted_spec(&fx, &owner);
    let mut listed = spec.clone();
    listed.admission = Admission::Allowlist;
    assert!(!crate::process::may_spawn(&listed));
    let inv = SystemRunner::new().run(&listed).expect("record");
    assert!(
        matches!(inv.status, ExitStatus::NotStarted { .. }),
        "{:?}",
        inv.status
    );
    let recording = SystemRunner::recording();
    let inv = recording.run(&spec).expect("record");
    assert!(matches!(inv.status, ExitStatus::NotStarted { .. }));
    assert!(inv.environment.is_empty() && inv.argv.is_empty());
    assert!(recording.take_recorded().is_empty());
    assert_eq!(recording.run_probe(&spec), ProbeRun::not_observed());
    assert!(recording.take_recorded().is_empty());
    assert_eq!(SystemRunner::new().run_probe(&listed), ProbeRun::refused());
    let mut replayed = spec.clone();
    replayed.replay_only = true;
    assert_eq!(
        SystemRunner::new().run_probe(&replayed),
        ProbeRun::refused()
    );
}

fn recording_answering(leaf: &str) -> Recording {
    Recording {
        schema_version: crate::process::recording::RECORDING_SCHEMA_VERSION,
        provenance: RecordingProvenance {
            macos: "15".into(),
            architecture: "arm64".into(),
            provider_versions: vec![],
            captured_at: "2026-11-08T00:00:00Z".into(),
            captured_by: "test".into(),
            origin: "synthesised".into(),
            note: None,
        },
        invocations: vec![crate::process::RecordedInvocation {
            invocation: crate::process::Invocation {
                program: leaf.into(),
                argv: flag(),
                environment: BTreeMap::new(),
                status: ExitStatus::Exited { code: 0 },
                stdout: "recorded".into(),
                stderr: String::new(),
                truncated: false,
                duration_ms: 1,
            },
        }],
    }
}

#[test]
fn invariant_15_a_replay_never_runs_a_probe_at_the_spawn_site() {
    let fx = Fixture::new("replay", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let (spec, _scratch) = admitted_spec(&fx, &owner);
    let replay = ReplayRunner::new(recording_answering(LEAF));
    assert!(replay.run(&spec).is_err(), "a recording answered a probe");
    assert!(replay.consulted().is_empty());
    assert_eq!(replay.run_probe(&spec), ProbeRun::not_observed());
    assert!(!replay.may_run_probes());
}

// ----- the answer table -----

#[test]
fn every_row_of_the_answer_table() {
    use CheckAnswer::{Fails, Holds, Unchecked};
    use UncheckedCause as U;
    let ended = |status, ending| ProbeRun::ended(status, ending);
    let exited = |code| ExitStatus::Exited { code };
    let failed = Fails(CheckFailure::ProbeFailed);
    for (run, unchanged, expected) in [
        (ended(exited(0), StreamEnd::Complete), true, Holds),
        (
            ended(exited(0), StreamEnd::Complete),
            false,
            Unchecked(U::ProbeTargetChanged),
        ),
        (ended(exited(1), StreamEnd::Complete), true, failed),
        (
            ended(exited(1), StreamEnd::Complete),
            false,
            Unchecked(U::ProbeTargetChanged),
        ),
        (ended(exited(255), StreamEnd::GaveUp), true, failed),
        (
            ended(
                ExitStatus::Signalled { signal: Some(9) },
                StreamEnd::Complete,
            ),
            true,
            failed,
        ),
        (
            ended(ExitStatus::Signalled { signal: None }, StreamEnd::Complete),
            true,
            Unchecked(U::ProbeEndNotRead),
        ),
        (
            ended(ExitStatus::Cancelled, StreamEnd::Complete),
            true,
            Unchecked(U::ProbeEndNotRead),
        ),
        (
            ended(ExitStatus::TimedOut, StreamEnd::Complete),
            true,
            Unchecked(U::ProbeTimedOut),
        ),
        (
            ended(
                ExitStatus::NotStarted {
                    error: String::new(),
                },
                StreamEnd::Complete,
            ),
            true,
            Unchecked(U::ProbeNotStarted),
        ),
        (
            ended(exited(0), StreamEnd::GaveUp),
            true,
            Unchecked(U::ProbeLeftAProcess),
        ),
        (
            ended(exited(0), StreamEnd::OverCap),
            true,
            Unchecked(U::ProbeOutputOverCap),
        ),
        (
            ended(exited(0), StreamEnd::ReadError),
            true,
            Unchecked(U::ProbeEndNotRead),
        ),
        (ProbeRun::not_observed(), true, Unchecked(U::NotObserved)),
        (ProbeRun::refused(), true, Unchecked(U::ProbeNotStarted)),
        (ProbeRun::not_started(), true, Unchecked(U::ProbeNotStarted)),
        (
            ProbeRun::target_changed(),
            true,
            Unchecked(U::ProbeTargetChanged),
        ),
        (ProbeRun::not_observed(), false, Unchecked(U::NotObserved)),
    ] {
        assert_eq!(
            answer(&run, unchanged),
            expected,
            "{run:?} unchanged={unchanged}"
        );
    }
    assert_eq!(StreamEnd::OverCap.max(StreamEnd::GaveUp), StreamEnd::GaveUp);
    assert_eq!(
        StreamEnd::Complete.max(StreamEnd::ReadError),
        StreamEnd::ReadError
    );
}

// ----- the table -----

#[test]
fn the_whole_probe_reads_only_the_embedded_table() {
    let fx = Fixture::new("embedded", ADMISSIBLE);
    let owner = Owner::of(&fx);
    let answer = run_curated_probe(&owner, &fx.state, fx.lock(), PACKAGE, LEAF, &flag());
    refused(&fx, &owner, answer, UncheckedCause::NoCuratedProbe);
}

#[test]
fn the_shipped_table_parses_and_holds_only_closed_flags() {
    let rows = table().expect("the shipped table parses");
    assert!(!rows.is_empty());
    for row in rows {
        assert!(is_one_permitted_flag(&row.argv), "{row:?}");
        assert!(ExecutableName::parse(&row.executable).is_ok(), "{row:?}");
    }
    // The read-only copy is the same rows, in the same order.
    let copies = curated_probe_rows().expect("rows");
    assert_eq!(copies.len(), rows.len());
    for (copy, row) in copies.iter().zip(rows) {
        assert_eq!(
            (
                &copy.provider,
                &copy.package,
                &copy.executable,
                &copy.argv,
                &copy.observed_version,
                &copy.source,
                &copy.read_on
            ),
            (
                &row.provider,
                &row.package,
                &row.executable,
                &row.argv,
                &row.observed_version,
                &row.source,
                &row.read_on
            )
        );
    }
}

#[test]
fn invariant_20_every_probe_row_is_dated() {
    for row in table().expect("parses") {
        assert!(is_iso_date(&row.read_on), "{row:?}");
        assert!(!row.source.trim().is_empty(), "{row:?}");
    }
}

#[test]
fn the_reader_fails_closed() {
    let good = "[[probe]]\nprovider = \"p\"\npackage = \"k\"\nexecutable = \"e\"\n\
                    argv = [\"--version\"]\nobserved_version = \"1\"\nsource = \"s\"\n\
                    read_on = \"2026-11-08\"\n";
    assert_eq!(parse_table(good).map(|r| r.len()), Ok(1));
    let bad = [
        format!("{good}{good}"),
        good.replace("provider = \"p\"", "provider = \"\""),
        good.replace("package = \"k\"", "package = \"\""),
        good.replace("executable = \"e\"", "executable = \"a/b\""),
        good.replace("[\"--version\"]", "[]"),
        good.replace("[\"--version\"]", "[\"--daemon\"]"),
        good.replace("[\"--version\"]", "[\"-v\", \"-V\"]"),
        good.replace("observed_version = \"1\"", "observed_version = \"\""),
        good.replace("source = \"s\"", "source = \" \""),
        good.replace("2026-11-08", "8 Nov 2026"),
        format!("{good}path = \"/x\"\n"),
        "probe = 3".to_owned(),
    ];
    for text in bad {
        assert!(parse_table(&text).is_err(), "{text}");
    }
}
