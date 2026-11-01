// SPDX-License-Identifier: Apache-2.0

//! Part F-2b through the real binary: `snapshot [<package>...]`, the
//! fail-closed load rule over every document this repository holds or writes,
//! and the analysis unchanged by the move onto `Snapshot::resolution_of`
//! (`docs/passes/16-plans/F.md`, Design §1 and §3, SG2-N6, SG2-Q3, SR-13).
//!
//! Where a read needs a listing, the captured recording is copied into a
//! scratch directory with the listing appended, shaped after the R-21
//! transcripts as F-2a's tests do in memory; nothing under `fixtures/` is
//! written, and the copy is removed afterwards.

use macportsbrew_adapters::persistence::load_snapshot;
use macportsbrew_domain::{ExecutableList, ProviderId, ResolutionState, Snapshot};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const TODAY: &str = "2026-09-02";

static NEXT: AtomicU32 = AtomicU32::new(0);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/cli has a grandparent")
        .to_path_buf()
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-f2b-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn run(scratch: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", scratch.join("state"))
        .env_remove("MACPORTSBREW_LANG")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("spawning the binary")
}

fn recordings() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(root().join("fixtures/recordings"))
        .expect("the recordings directory")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    found.sort();
    assert!(found.len() >= 7, "the recordings were not found");
    found
}

/// Replays `recording` into a snapshot file, naming `packages`.
fn snapshot_of(recording: &Path, packages: &[&str], dir: &Path) -> (PathBuf, Output) {
    let out = dir.join("snapshot.json");
    let mut args = vec![
        "--replay",
        recording.to_str().expect("UTF-8 path"),
        "snapshot",
        "--out",
        out.to_str().expect("UTF-8 path"),
        "--today",
        TODAY,
    ];
    args.extend_from_slice(packages);
    let output = run(dir, &args);
    (out, output)
}

#[test]
fn every_committed_snapshot_loads_under_the_fail_closed_rule() {
    // SG2-N6: the load rule tightens v1, so every snapshot this repository
    // holds, and every one the fixture matrix writes from a recording, must
    // still load through the loader that now runs it.
    let committed = root().join("fixtures/snapshots");
    let mut checked = 0;
    for entry in std::fs::read_dir(&committed)
        .expect("fixtures/snapshots")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            load_snapshot(&path)
                .unwrap_or_else(|e| panic!("{} no longer loads: {e:?}", path.display()));
            checked += 1;
        }
    }
    assert!(checked >= 1, "no committed snapshot was found");
    for recording in recordings() {
        let dir = scratch("load");
        let (out, output) = snapshot_of(&recording, &[], &dir);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}: {}",
            recording.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        load_snapshot(&out).unwrap_or_else(|e| {
            panic!(
                "the snapshot written from {} does not load: {e:?}",
                recording.display()
            )
        });
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The live snapshot with its per-provider resolutions also written into the
/// PATH map, as a capture of this build writes both: `FOUND`, no index.
fn with_the_path_map(mut snapshot: Snapshot) -> Snapshot {
    let found: Vec<_> = snapshot
        .providers
        .values()
        .flat_map(|p| p.resolutions.clone())
        .collect();
    for (name, resolution) in found {
        snapshot.environment.resolutions.insert(
            macportsbrew_domain::ExecutableName::parse(&name).expect("a resolved name"),
            ResolutionState::Found {
                resolution,
                path_index: None,
            },
        );
    }
    snapshot
}

#[test]
fn analyze_and_doctor_goldens_are_byte_identical_without_named_packages() {
    // SG2-Q3: analysis moved onto `resolution_of`. The fixture matrix holds
    // the replayed goldens byte-identical; a replay resolves nothing, so this
    // covers the route those goldens cannot - the captured live snapshot,
    // whose per-provider map has 66 resolutions. Read as it was committed
    // (no PATH map) and as this build writes it (both maps), `analyze` and
    // `doctor` say the same bytes.
    let live = root().join("fixtures/snapshots/both-providers-intel-macos26-live.json");
    let original = load_snapshot(&live).expect("the live snapshot loads");
    assert!(original.environment.resolutions.is_empty());
    let both = with_the_path_map(original.clone());
    assert!(both.environment.resolutions.len() >= 60);
    let dir = scratch("golden");
    let copy = dir.join("both-maps.json");
    std::fs::write(&copy, serde_json::to_string_pretty(&both).unwrap()).unwrap();
    load_snapshot(&copy).expect("the two maps agree, so the copy loads");
    for command in [&["analyze"][..], &["doctor"][..]] {
        let outputs: Vec<Output> = [live.as_path(), copy.as_path()]
            .iter()
            .map(|file| {
                let mut args = vec!["--from-snapshot", file.to_str().unwrap(), "--json"];
                args.extend_from_slice(command);
                args.extend_from_slice(&["--today", TODAY]);
                run(&dir, &args)
            })
            .collect();
        assert_eq!(
            outputs[0].status.code(),
            outputs[1].status.code(),
            "{command:?}"
        );
        assert!(!outputs[0].stdout.is_empty(), "{command:?} wrote nothing");
        assert!(
            outputs[0].stdout == outputs[1].stdout,
            "{command:?} differs once the PATH map is present"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The captured two-provider recording, with one listing per provider for
/// `xz` appended, written into `dir`.
fn recording_with_listings(dir: &Path) -> PathBuf {
    let source = root().join("fixtures/recordings/both-providers-intel-macos26.json");
    let mut recording: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(source).unwrap()).unwrap();
    let listing = |program: &str, argv: &[&str], stdout: &str| {
        serde_json::json!({
            "program": program, "argv": argv, "environment": {},
            "status": {"status": "EXITED", "code": 0},
            "stdout": stdout, "stderr": "", "truncated": false, "durationMs": 0
        })
    };
    let invocations = recording["invocations"].as_array_mut().unwrap();
    invocations.push(listing(
        "brew",
        &["list", "--formula", "xz"],
        "/usr/local/Cellar/xz/5.8.3/bin/lzcat\n/usr/local/Cellar/xz/5.8.3/bin/xz\n\
         /usr/local/Cellar/xz/5.8.3/lib/liblzma.5.dylib\n",
    ));
    invocations.push(listing(
        "port",
        &["-q", "contents", "xz"],
        "  /opt/local/bin/lzcat\n  /opt/local/bin/xz\n  /opt/local/lib/liblzma.5.dylib\n",
    ));
    let path = dir.join("with-listings.json");
    std::fs::write(&path, serde_json::to_string(&recording).unwrap()).unwrap();
    path
}

#[test]
fn snapshot_sends_the_inventory_name_and_nothing_for_an_unknown_one() {
    let dir = scratch("named");
    let recording = recording_with_listings(&dir);
    let (out, output) = snapshot_of(&recording, &["xz", "nosuch"], &dir);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    // Read for the installed row in each provider, with the inventory's name:
    // the replay answers only the argv the recording holds, so a list here is
    // proof the argv was exactly `xz`.
    let snapshot = load_snapshot(&out).expect("the snapshot loads");
    for provider in ["homebrew", "macports"] {
        let row = &snapshot.providers[&ProviderId::new(provider)]
            .inventory
            .metadata["xz"];
        assert_eq!(
            row.executable_list,
            ExecutableList::from_leaves(["lzcat", "xz"]),
            "{provider}"
        );
    }
    // A name no inventory holds selected nothing (no invocation for it is
    // recorded, and nothing failed), and the user is told.
    assert!(
        stderr.contains("No provider has nosuch installed"),
        "{stderr}"
    );
    assert!(!stderr.contains("No provider has xz installed"), "{stderr}");
    // Every other row is as before: nobody asked.
    let other = &snapshot.providers[&ProviderId::new("homebrew")]
        .inventory
        .metadata;
    assert!(other
        .iter()
        .filter(|(n, _)| *n != "xz")
        .all(|(_, m)| m.executable_list.is_not_recorded()));

    // An argument that begins with `-` is never a package, even after `--`.
    let (_, refused) = snapshot_of(&recording, &["--", "-x"], &dir);
    let said = String::from_utf8_lossy(&refused.stderr);
    assert_ne!(refused.status.code(), Some(0), "{said}");
    assert!(said.contains("begins with `-`"), "{said}");

    // A name echoed back is escaped (SR2-N6): no raw escape sequence reaches
    // the terminal, from the not-installed notice or from the refusal.
    for args in [&["x\u{1b}[2J"][..], &["--", "-\u{1b}[2J"][..]] {
        let (_, output) = snapshot_of(&recording, args, &dir);
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(
            !said.contains('\u{1b}'),
            "a raw ESC reached stderr: {said:?}"
        );
        assert!(
            said.contains("\\u{1b}"),
            "the name is shown escaped: {said:?}"
        );
    }

    // Under `--from-snapshot` nothing is captured, so naming a package is a
    // usage error rather than a list silently not read.
    let (_, from_file) = {
        let target = dir.join("again.json");
        let output = run(
            &dir,
            &[
                "--from-snapshot",
                out.to_str().unwrap(),
                "snapshot",
                "--out",
                target.to_str().unwrap(),
                "xz",
            ],
        );
        (target, output)
    };
    let said = String::from_utf8_lossy(&from_file.stderr);
    assert_ne!(from_file.status.code(), Some(0), "{said}");
    assert!(said.contains("snapshot file"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether any object in `value` has `"kind": "executables"`.
fn has_executables_evidence(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            map.get("kind").and_then(|k| k.as_str()) == Some("executables")
                || map.values().any(has_executables_evidence)
        }
        serde_json::Value::Array(items) => items.iter().any(has_executables_evidence),
        _ => false,
    }
}

#[test]
fn a_named_snapshot_adds_executables_evidence_to_the_mapping() {
    // What naming a package changes in an analysis (spec-guardian M-1): the
    // two `xz` rows carry equal known lists, which is mapping evidence. `xz` is
    // a dependency, so the rows are shown under `--advanced`. A
    // snapshot that names nothing carries none, as before F-2b.
    let dir = scratch("evidence");
    let recording = recording_with_listings(&dir);
    let analysed = |packages: &[&str]| -> serde_json::Value {
        let sub = dir.join(packages.join("-") + "x");
        std::fs::create_dir_all(&sub).unwrap();
        let (file, output) = snapshot_of(&recording, packages, &sub);
        assert_eq!(output.status.code(), Some(0));
        let analysis = run(
            &sub,
            &[
                "--from-snapshot",
                file.to_str().unwrap(),
                "--json",
                "--advanced",
                "analyze",
                "--today",
                TODAY,
            ],
        );
        serde_json::from_slice(&analysis.stdout).expect("analyze --json")
    };
    assert!(has_executables_evidence(&analysed(&["xz"])));
    assert!(!has_executables_evidence(&analysed(&[])));
    let _ = std::fs::remove_dir_all(&dir);
}
