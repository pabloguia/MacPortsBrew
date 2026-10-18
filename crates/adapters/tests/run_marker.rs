// SPDX-License-Identifier: Apache-2.0

//! The interrupted-run marker (`persistence::run_marker`, pass 08 slice 2): it
//! is written under the lock and never replaced, read without being trusted and
//! without creating anything, and cleared only by the run that wrote it.
//! Every test runs in a scratch state directory (INV-05).

use macportsbrew_adapters::persistence::run_lock::{acquire, RunLock, LOCK_FILE};
use macportsbrew_adapters::persistence::run_marker::{
    clear, marker_state, peek, read, write, ClearOutcome, MARKER_FILE, MAX_MARKER_BYTES,
};
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_domain::{ErrorId, InterruptedRun, MarkerState};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

const FIRST: &str = "20261005T120000Z-00000000000000aa";
const SECOND: &str = "20261005T130000Z-00000000000000bb";

struct Scratch {
    base: PathBuf,
    state: StateDir,
    uid: u32,
    lock: Option<RunLock>,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "mpb-marker-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        state.ensure().unwrap();
        let uid = std::fs::metadata(&base).unwrap().uid();
        let lock = Some(acquire(&state, uid).unwrap());
        Self {
            base,
            state,
            uid,
            lock,
        }
    }

    fn lock(&self) -> &RunLock {
        self.lock.as_ref().unwrap()
    }

    fn path(&self) -> PathBuf {
        self.state.root().join(MARKER_FILE)
    }

    fn read(&self) -> Result<Option<InterruptedRun>, macportsbrew_domain::DomainError> {
        read(self.lock(), &self.state, self.uid)
    }

    fn write(&self, id: &str) -> Result<(), macportsbrew_domain::DomainError> {
        write(
            self.lock(),
            &self.state,
            &InterruptedRun::new(id).unwrap(),
            self.uid,
        )
    }

    fn clear(&self, id: &str) -> Result<ClearOutcome, macportsbrew_domain::DomainError> {
        clear(
            self.lock(),
            &self.state,
            &InterruptedRun::new(id).unwrap(),
            self.uid,
        )
    }

    /// Plants raw text as the marker, privately.
    fn plant(&self, text: &[u8]) {
        std::fs::write(self.path(), text).unwrap();
        std::fs::set_permissions(self.path(), std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.state.root())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        self.lock.take();
        std::fs::remove_dir_all(&self.base).ok();
    }
}

fn canonical(id: &str) -> String {
    format!("{{\"schemaVersion\":2,\"runId\":\"{id}\",\"snapshotFile\":null}}\n")
}

#[test]
fn a_written_marker_is_read_back_as_the_run_that_wrote_it() {
    let s = Scratch::new("roundtrip");
    assert_eq!(s.read().unwrap(), None);
    s.write(FIRST).unwrap();
    assert_eq!(s.read().unwrap().unwrap().run_id(), FIRST);
    assert_eq!(std::fs::read_to_string(s.path()).unwrap(), canonical(FIRST));
    let meta = std::fs::metadata(s.path()).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(meta.nlink(), 1);
}

#[test]
fn a_marker_is_never_replaced_and_the_refusal_names_the_run_it_found() {
    let s = Scratch::new("never-replaced");
    s.write(FIRST).unwrap();
    let error = s.write(SECOND).unwrap_err();
    assert_eq!(error.id, ErrorId::RunInterrupted);
    assert_eq!(error.id.exit_code(), 4);
    assert_eq!(
        error.detail, FIRST,
        "the detail is the run id and nothing else"
    );
    assert_eq!(std::fs::read_to_string(s.path()).unwrap(), canonical(FIRST));
    // The same run again is also a collision, not a lost reply of its own.
    assert_eq!(s.write(FIRST).unwrap_err().id, ErrorId::RunInterrupted);
}

#[test]
fn an_unusable_marker_in_the_way_is_unusable_not_interrupted() {
    let s = Scratch::new("write-over-unusable");
    s.plant(b"not a marker");
    assert_eq!(s.write(SECOND).unwrap_err().id, ErrorId::HistoryUnusable);
    assert_eq!(std::fs::read(s.path()).unwrap(), b"not a marker");
}

#[test]
fn reading_creates_nothing_and_a_missing_marker_is_none() {
    let s = Scratch::new("creates-nothing");
    let before = s.entries();
    assert_eq!(s.read().unwrap(), None);
    assert_eq!(s.entries(), before);
    assert!(!s.path().exists());
    assert!(before.contains(&LOCK_FILE.to_string()));
}

#[test]
fn anything_that_is_not_exactly_what_the_writer_leaves_is_unusable() {
    let s = Scratch::new("grammar");
    let good = canonical(FIRST);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("no trailing newline", good.trim_end().as_bytes().to_vec()),
        ("two newlines", format!("{good}\n").into_bytes()),
        ("leading space", format!(" {good}").into_bytes()),
        ("a byte order mark", format!("\u{feff}{good}").into_bytes()),
        (
            "an unknown field",
            good.replace('}', ",\"extra\":1}").into_bytes(),
        ),
        ("schema 1", good.replace(":2,", ":1,").into_bytes()),
        ("schema 3", good.replace(":2,", ":3,").into_bytes()),
        (
            "a snapshot name that is a path",
            good.replace("null", "\"../x.json\"").into_bytes(),
        ),
        (
            "a duplicate key",
            good.replace('{', "{\"schemaVersion\":1,").into_bytes(),
        ),
        ("trailing data", format!("{good}{{}}").into_bytes()),
        (
            "a NUL in the id",
            good.replace("00aa", "00a\u{0}").into_bytes(),
        ),
        ("not UTF-8", vec![0xff, 0xfe, 0xfd]),
        (
            "a path as the id",
            good.replace(FIRST, "../../etc/passwd").into_bytes(),
        ),
        (
            "a separator in the id",
            good.replace(FIRST, "a/b").into_bytes(),
        ),
        ("an empty id", good.replace(FIRST, "").into_bytes()),
        (
            "an upper-case id",
            good.replace("00aa", "00AA").into_bytes(),
        ),
        ("empty", Vec::new()),
        ("an array", b"[]\n".to_vec()),
    ];
    for (what, text) in cases {
        s.plant(&text);
        let error = s.read().unwrap_err();
        assert_eq!(error.id, ErrorId::HistoryUnusable, "{what}");
        assert!(
            !error.detail.contains(s.base.to_str().unwrap()),
            "{what}: {}",
            error.detail
        );
        assert_eq!(
            std::fs::read(s.path()).unwrap(),
            text,
            "{what}: left in place"
        );
    }
}

#[test]
fn a_marker_of_the_wrong_size_mode_or_kind_is_unusable() {
    let s = Scratch::new("shape");
    // Oversized.
    s.plant(&vec![b' '; usize::try_from(MAX_MARKER_BYTES).unwrap() + 1]);
    assert_eq!(s.read().unwrap_err().id, ErrorId::HistoryUnusable);
    assert_eq!(s.clear(FIRST).unwrap_err().id, ErrorId::HistoryUnusable);
    assert!(s.path().exists());
    // Open to others.
    s.plant(canonical(FIRST).as_bytes());
    for mode in [0o644, 0o660, 0o606] {
        std::fs::set_permissions(s.path(), std::fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(
            s.read().unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{mode:o}"
        );
        assert_eq!(
            s.clear(FIRST).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{mode:o}"
        );
    }
    // A directory.
    std::fs::remove_file(s.path()).unwrap();
    std::fs::create_dir(s.path()).unwrap();
    assert_eq!(s.read().unwrap_err().id, ErrorId::HistoryUnusable);
    std::fs::remove_dir(s.path()).unwrap();
    // A symbolic reference, dangling or not, is not followed.
    let target = s.base.join("elsewhere");
    std::fs::write(&target, canonical(FIRST)).unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink(&target, s.path()).unwrap();
    assert_eq!(s.read().unwrap_err().id, ErrorId::HistoryUnusable);
    let refused = s.clear(FIRST).unwrap_err();
    assert_eq!(refused.id, ErrorId::HistoryUnusable);
    // Refused for what it is on every host — `clear` looks before it opens, as
    // `read` does — never followed and never reported as "replaced" (slice 4
    // review, L3).
    assert!(
        refused.detail.contains("symbolic reference"),
        "{}",
        refused.detail
    );
    assert!(target.exists(), "nothing was removed through the reference");
}

#[test]
fn a_marker_left_at_two_links_by_a_crash_still_reads_and_still_clears() {
    let s = Scratch::new("two-links");
    s.write(FIRST).unwrap();
    let leftover = s
        .state
        .root()
        .join(format!(".{MARKER_FILE}.4242.00000000deadbeef.partial"));
    std::fs::hard_link(s.path(), &leftover).unwrap();
    assert_eq!(std::fs::metadata(s.path()).unwrap().nlink(), 2);
    assert_eq!(s.read().unwrap().unwrap().run_id(), FIRST);
    assert_eq!(s.clear(FIRST).unwrap(), ClearOutcome::Cleared);
}

#[test]
fn clearing_removes_only_the_runs_own_marker() {
    let s = Scratch::new("clear");
    assert_eq!(s.clear(FIRST).unwrap(), ClearOutcome::AlreadyAbsent);
    s.write(FIRST).unwrap();
    assert_eq!(s.clear(SECOND).unwrap(), ClearOutcome::NotOurs);
    assert!(s.path().exists(), "another run's marker is left in place");
    assert_eq!(s.clear(FIRST).unwrap(), ClearOutcome::Cleared);
    assert!(!s.path().exists());
    assert_eq!(s.read().unwrap(), None);
    // A second clear of a finished run is not an error.
    assert_eq!(s.clear(FIRST).unwrap(), ClearOutcome::AlreadyAbsent);
}

#[test]
fn an_unreadable_marker_is_not_cleared() {
    let s = Scratch::new("clear-unusable");
    s.plant(b"{\"schemaVersion\":1}\n");
    assert_eq!(s.clear(FIRST).unwrap_err().id, ErrorId::HistoryUnusable);
    assert!(s.path().exists());
}

#[test]
fn nothing_touches_the_marker_once_the_lock_file_is_gone_from_under_the_run() {
    let s = Scratch::new("lock-gone");
    s.write(FIRST).unwrap();
    std::fs::remove_file(s.state.root().join(LOCK_FILE)).unwrap();
    assert_eq!(s.write(SECOND).unwrap_err().id, ErrorId::HistoryUnusable);
    assert_eq!(s.clear(FIRST).unwrap_err().id, ErrorId::HistoryUnusable);
    assert!(
        s.path().exists(),
        "the marker was not cleared without a lock"
    );
}

#[test]
fn a_run_after_a_cleared_run_finds_no_marker_and_writes_its_own() {
    let s = Scratch::new("sequential");
    s.write(FIRST).unwrap();
    s.clear(FIRST).unwrap();
    assert_eq!(s.read().unwrap(), None);
    s.write(SECOND).unwrap();
    assert_eq!(s.read().unwrap().unwrap().run_id(), SECOND);
}

/// The code of a source file with its comment lines removed.
fn code_of(file: &str) -> String {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap();
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
        // The crate's own name is not a provider.
        .replace("macportsbrew", "")
}

#[test]
fn invariant_17_the_lock_and_the_marker_name_no_provider() {
    // The lock is MacPortsBrew-wide, so it is by choice, not by the layering
    // rule, that neither module names a provider: a run is excluded the same
    // way whichever direction it migrates in.
    for file in [
        "src/persistence/run_lock.rs",
        "src/persistence/run_marker.rs",
    ] {
        let code = code_of(file);
        for word in ["homebrew", "macports", "brew", "cellar", "/opt/local"] {
            assert!(!code.contains(word), "{file} names {word}");
        }
    }
}

// --- Reading without the lock (pass 08 slice 5, D12) --------------------------

#[test]
fn peek_sees_what_read_sees_without_the_lock_and_creates_nothing() {
    let s = Scratch::new("peek");
    let before = s.entries();
    assert_eq!(peek(&s.state, s.uid).unwrap(), None);
    assert_eq!(marker_state(&s.state, s.uid), MarkerState::Absent);
    assert_eq!(s.entries(), before, "reading created nothing");

    s.write(FIRST).unwrap();
    let seen = peek(&s.state, s.uid).unwrap().unwrap();
    assert_eq!(seen.run_id(), FIRST);
    assert_eq!(seen, s.read().unwrap().unwrap());
    assert_eq!(
        marker_state(&s.state, s.uid),
        MarkerState::Names(FIRST.into())
    );
}

#[test]
fn a_marker_names_the_snapshot_the_run_took_first() {
    let s = Scratch::new("peek-snapshot");
    let marker = InterruptedRun::new(FIRST)
        .unwrap()
        .with_snapshot("snapshot-20261005T120000Z.json")
        .unwrap();
    write(s.lock(), &s.state, &marker, s.uid).unwrap();
    let seen = peek(&s.state, s.uid).unwrap().unwrap();
    assert_eq!(seen.snapshot_file(), Some("snapshot-20261005T120000Z.json"));
    // Clearing goes by the run, not by the snapshot the marker was built with.
    let by_id = InterruptedRun::new(FIRST).unwrap();
    assert_eq!(
        clear(s.lock(), &s.state, &by_id, s.uid).unwrap(),
        ClearOutcome::Cleared
    );
}

#[test]
fn something_that_is_not_a_marker_is_unreadable_and_never_absent() {
    // Fail closed: reading a marker as absent is how an unfinished run gets a
    // successor on top of it.
    for (what, text) in [
        ("garbage", b"not json".to_vec()),
        (
            "an older schema",
            b"{\"schemaVersion\":1,\"runId\":\"20261005T120000Z-00000000000000aa\"}\n".to_vec(),
        ),
        ("an empty file", Vec::new()),
    ] {
        let s = Scratch::new("peek-garbage");
        s.plant(&text);
        assert_eq!(
            peek(&s.state, s.uid).unwrap_err().id,
            ErrorId::HistoryUnusable,
            "{what}"
        );
        assert_eq!(
            marker_state(&s.state, s.uid),
            MarkerState::Unreadable,
            "{what}"
        );
    }
}

#[test]
fn a_state_directory_others_can_write_is_unreadable_not_empty() {
    let s = Scratch::new("peek-open-root");
    s.write(FIRST).unwrap();
    std::fs::set_permissions(s.state.root(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        peek(&s.state, s.uid).unwrap_err().id,
        ErrorId::HistoryUnusable
    );
    assert_eq!(marker_state(&s.state, s.uid), MarkerState::Unreadable);
    std::fs::set_permissions(s.state.root(), std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn a_state_directory_that_is_not_there_holds_no_marker_and_stays_not_there() {
    let s = Scratch::new("peek-missing");
    let missing =
        StateDir::resolve(None, Some(s.base.join("elsewhere").to_str().unwrap())).unwrap();
    assert_eq!(peek(&missing, s.uid).unwrap(), None);
    assert_eq!(marker_state(&missing, s.uid), MarkerState::Absent);
    assert!(!missing.root().exists(), "reading created nothing");
}
