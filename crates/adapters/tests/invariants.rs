// SPDX-License-Identifier: Apache-2.0

//! Invariant tests for the adapter layer.
//!
//! `01-invariants.md` requires every invariant to have at least one test named
//! after it, and a change that touches one to be able to point at that test.
//! `crates/policy/tests/invariants.rs` does this for the engine; this file
//! does it for the adapters, whose invariants are mostly about what the code
//! is *not* allowed to reach for.
//!
//! The source scans below follow the pattern of
//! `invariant_20_no_provider_deadline_is_written_into_source` in the policy
//! crate: a doc comment saying "this is the only clock read" is precisely the
//! kind of claim that cannot fail a test, so it is asserted against the source
//! text instead.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::homebrew::{self, HomebrewAdapter};
use macportsbrew_adapters::macports::MacPortsAdapter;
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_adapters::process::ReplayRunner;
use macportsbrew_adapters::system::{is_reported, SystemMachine};
use macportsbrew_domain::{
    Capability, CapabilityState, ExecutableOwner, HomeRedaction, MachineRead, ProviderRead,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "macportsbrew-inv-{tag}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The workspace root, from this crate's manifest directory.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root must be reachable")
}

/// Every `.rs` file under every crate's `src/`, as `(path, text)`.
fn sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let crates = workspace().join("crates");
    let mut stack = vec![crates];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.components().any(|c| c.as_os_str() == "src")
            {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    found.push((path, text));
                }
            }
        }
    }
    assert!(found.len() > 10, "the source scan found almost nothing");
    found
}

/// Lines of `text` that contain `needle` and are not a comment.
fn code_lines<'a>(text: &'a str, needle: &str) -> Vec<&'a str> {
    text.lines()
        .filter(|line| line.contains(needle))
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with('*')
        })
        .collect()
}

#[test]
fn invariant_14_the_wall_clock_is_read_in_exactly_one_place() {
    // INV-14 / INV-15: a recommendation is a pure function of its inputs. A
    // second clock read would make an analysis depend on when it ran, and no
    // other test would notice.
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        let is_the_one_place = path.ends_with("persistence/time.rs");
        for line in code_lines(&text, "SystemTime::now") {
            if !is_the_one_place {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the wall clock is read outside persistence::time::now_secs:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn invariant_14_the_environment_is_read_in_exactly_one_place() {
    // Same reasoning: an adapter that consulted the shell directly would make
    // a replayed snapshot depend on the shell replaying it.
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        let is_the_one_place = path.ends_with("system/machine.rs");
        for line in code_lines(&text, "env::var") {
            if !is_the_one_place {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the environment is read outside system::machine:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn invariant_15_a_replayed_reader_never_consults_the_live_filesystem() {
    // `which` resolves against the machine's own `PATH`. Under replay the
    // machine is a recording, and the filesystem is the replaying host's, so
    // answering from it would make the same snapshot produce different output
    // on two machines.
    let dir = scratch("replay-which");
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("tool"), b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(
        bin.join("tool"),
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();

    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    let replayed = SystemMachine::new(
        Box::new(ReplayRunner::new(recording)),
        Some(bin.to_string_lossy().into_owned()),
        BTreeMap::new(),
        HomeRedaction::Recorded,
    );
    assert!(replayed.is_replay());
    assert_eq!(
        replayed.which("tool"),
        None,
        "a replayed reader answered from the replaying host's filesystem"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn invariant_15_a_reader_of_another_machine_answers_nothing_from_this_disk() {
    // The companion to the test above, and the one that would have caught
    // F-25. Every read guarded there is given a filesystem that *would* answer
    // it: a cache index that exists, a real symlink to follow, a service
    // directory holding a valid plist, an executable where the prefix probe
    // looks. Then the reader is told the disk is not the machine, and every
    // answer must be the unknown one — not the negative one, which would be a
    // different lie (`02-domain-model.md`, "an unknown is never a yes").
    //
    // It is deliberately *not* guarded on `runner.is_replay()`: the runner
    // below replays, and the sibling assertions at the end use the same
    // recording over the same tree with `ThisMachine` and must all answer.
    // That is the distinction F-25 is about, and a test that used one flag for
    // both would pass with the bug restored.
    // Canonicalised up front: on macOS `temp_dir()` sits under `/var`, which is
    // a symlink to `/private/var`, so a path built here and a path this code
    // canonicalises would never compare equal — and the probe found that made
    // the root-canonicalisation case below silently vacuous.
    let root = scratch("elsewhere");
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(&root).unwrap();
    let bin = root.join("bin");
    let cellar = root.join("Cellar/wget/1.25.0/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&cellar).unwrap();
    std::fs::write(cellar.join("wget"), b"").unwrap();
    std::fs::write(bin.join("brew"), b"#!/bin/sh\n").unwrap();
    // Both providers' probes look for their own executable, so the tree
    // must answer both or one assertion below is vacuous — which is what
    // removing the MacPorts guard proved before this line existed.
    std::fs::write(bin.join("port"), b"#!/bin/sh\n").unwrap();
    std::os::unix::fs::symlink(cellar.join("wget"), bin.join("wget")).unwrap();

    let home = root.join("home");
    // The exact path `offline_index_path` builds, keyed on the first platform
    // tag, so the control half below is a real Available and not an accident.
    let index = home.join("Library/Caches/Homebrew/api/internal");
    std::fs::create_dir_all(&index).unwrap();
    std::fs::write(index.join("packages.tahoe.jws.json"), b"{}").unwrap();

    std::fs::create_dir_all(root.join("etc/macports")).unwrap();
    let services = root.join("etc/LaunchDaemons/org.macports.nginx");
    std::fs::create_dir_all(&services).unwrap();
    std::fs::write(services.join("org.macports.nginx.plist"), b"<plist/>").unwrap();

    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    let runner = ReplayRunner::new(recording);

    // --- the prefix probe -------------------------------------------------
    assert_eq!(
        homebrew::detect_prefix(std::slice::from_ref(&root), FilesystemScope::Elsewhere),
        None,
        "a prefix was probed on a disk that is not the machine"
    );
    assert_eq!(
        MacPortsAdapter::<&ReplayRunner>::detect_prefix(
            std::slice::from_ref(&root),
            FilesystemScope::Elsewhere
        ),
        None,
        "a prefix was probed on a disk that is not the machine"
    );

    // --- the service scan -------------------------------------------------
    let scanned = macportsbrew_adapters::macports::service::scan(
        &root,
        FilesystemScope::Elsewhere,
        &[("nginx".to_string(), true)].into_iter().collect(),
        &registry_agrees,
    );
    assert!(
        !scanned.is_known(),
        "a service directory on this host was read for another machine"
    );

    // --- the capability probe and the resolution --------------------------
    let elsewhere = HomebrewAdapter::new(
        &runner,
        root.clone(),
        Some(home.clone()),
        vec!["tahoe".to_string()],
        FilesystemScope::Elsewhere,
    );
    assert_eq!(
        elsewhere.capabilities().unwrap().capabilities[&Capability::OfflineMetadata],
        CapabilityState::Unknown,
        "the cached index on this host answered for another machine"
    );
    let resolved = elsewhere.resolution(&bin.join("wget")).unwrap();
    assert_eq!(
        resolved.canonical_path, None,
        "a symlink on this host was followed for another machine"
    );
    // Not `Unknown`: classifying a path against the *recorded* prefix is pure
    // computation over recorded evidence and stays correct. What must not
    // happen is a `Provider` claim, which here is only reachable by following
    // the symlink into the Cellar — that is the live-disk read (F-25 review).
    assert!(
        !matches!(resolved.owner, ExecutableOwner::Provider { .. }),
        "a symlink on this host was followed into a Cellar for another machine"
    );
    assert!(
        !resolved.authoritative,
        "an answer reached without following the link claimed to be authoritative"
    );

    let mp_elsewhere =
        MacPortsAdapter::new(&runner, root.clone(), None, FilesystemScope::Elsewhere);
    assert_eq!(
        mp_elsewhere
            .resolution(&bin.join("wget"))
            .unwrap()
            .canonical_path,
        None,
        "a symlink on this host was followed for another machine"
    );

    // --- the providers' own configuration (DP-027) --------------------------
    // Both readers, given files that would answer: the prefix file and the
    // user file for Homebrew, the user file for MacPorts.
    let brew_env = root.join("etc/homebrew");
    std::fs::create_dir_all(&brew_env).unwrap();
    std::fs::write(brew_env.join("brew.env"), b"HOMEBREW_NO_ANALYTICS=\n").unwrap();
    std::fs::create_dir_all(home.join(".macports")).unwrap();
    std::fs::write(home.join(".macports/macports.conf"), b"portdbpath /x\n").unwrap();
    for provider in ["homebrew", "macports"] {
        assert_eq!(
            macportsbrew_adapters::configuration::read(
                provider,
                &root,
                Some(&home),
                FilesystemScope::Elsewhere
            ),
            macportsbrew_domain::ProviderConfiguration::Unknowable,
            "{provider}'s configuration on this host was read for another machine"
        );
        assert_eq!(
            macportsbrew_adapters::configuration::read(
                provider,
                &root,
                Some(&home),
                FilesystemScope::ThisMachine
            )
            .outranks(),
            Some(true),
            "the control: on this machine {provider}'s file is read"
        );
    }

    // --- the cask receipt ---------------------------------------------------
    // Added when the receipt read landed (DP-012). The point of this test is
    // that it enumerates *every* guarded read; F-25 exists because one was
    // missed, and a new one arriving unlisted is the same failure again.
    let caskroom = root.join("Caskroom");
    let meta = caskroom.join("meld").join(".metadata");
    std::fs::create_dir_all(&meta).unwrap();
    std::fs::write(
        meta.join("INSTALL_RECEIPT.json"),
        br#"{"installed_on_request": true}"#,
    )
    .unwrap();
    assert_eq!(
        homebrew::cask_requested(&caskroom, "meld", FilesystemScope::Elsewhere),
        None,
        "a cask receipt on this host was read for another machine"
    );
    assert_eq!(
        homebrew::cask_requested(&caskroom, "meld", FilesystemScope::ThisMachine),
        Some(true),
        "the same receipt on the same tree, declared as the machine"
    );

    // --- and the same tree, declared as the machine, answers everything ---
    // Without this half the assertions above would pass on a reader that had
    // simply stopped working.
    assert_eq!(
        homebrew::detect_prefix(std::slice::from_ref(&root), FilesystemScope::ThisMachine),
        Some(root.clone()),
        "the same probe on the same tree, declared as the machine"
    );
    assert!(
        macportsbrew_adapters::macports::service::scan(
            &root,
            FilesystemScope::ThisMachine,
            &[("nginx".to_string(), true)].into_iter().collect(),
            &registry_agrees,
        )
        .defines_service("nginx"),
        "the same scan on the same tree, declared as the machine"
    );
    let here = HomebrewAdapter::new(
        &runner,
        root.clone(),
        Some(home.clone()),
        vec!["tahoe".to_string()],
        FilesystemScope::ThisMachine,
    );
    assert_eq!(
        here.capabilities().unwrap().capabilities[&Capability::OfflineMetadata],
        CapabilityState::Available,
        "the same index on the same tree, declared as the machine"
    );
    assert!(
        here.resolution(&bin.join("wget"))
            .unwrap()
            .canonical_path
            .is_some(),
        "the same symlink on the same tree, declared as the machine"
    );
    assert_eq!(
        MacPortsAdapter::<&ReplayRunner>::detect_prefix(
            std::slice::from_ref(&root),
            FilesystemScope::ThisMachine
        ),
        Some(root.clone()),
        "the same probe on the same tree, declared as the machine"
    );
    let mp_here = MacPortsAdapter::new(&runner, root.clone(), None, FilesystemScope::ThisMachine);
    assert!(
        mp_here
            .resolution(&bin.join("wget"))
            .unwrap()
            .canonical_path
            .is_some(),
        "the same symlink on the same tree, declared as the machine"
    );

    // --- the root canonicalisation, on its own -----------------------------
    // The guard above is invisible with a recorded Cellar that is nowhere near
    // this tree, which is how the probe caught it undefended. Here the
    // recording's `--cellar` is a *symlink* into the tree, and the executable
    // is asked about by its real path, so the only thing that can change the
    // answer is whether the root itself is resolved.
    let cellar_link = root.join("cellar-link");
    std::os::unix::fs::symlink(root.join("Cellar"), &cellar_link).unwrap();
    let linked = ReplayRunner::new(synthetic_cellar(
        &cellar_link.display().to_string(),
        &root.join("Caskroom").display().to_string(),
    ));
    let keg = root.join("Cellar/wget/1.25.0/bin/wget");
    let via_link_elsewhere = HomebrewAdapter::new(
        &linked,
        root.clone(),
        None,
        vec!["tahoe".to_string()],
        FilesystemScope::Elsewhere,
    );
    assert!(
        !matches!(
            via_link_elsewhere.resolution(&keg).unwrap().owner,
            ExecutableOwner::Provider { .. }
        ),
        "a Cellar symlink on this host was resolved for another machine"
    );
    let via_link_here = HomebrewAdapter::new(
        &linked,
        root.clone(),
        None,
        vec!["tahoe".to_string()],
        FilesystemScope::ThisMachine,
    );
    assert!(
        matches!(
            via_link_here.resolution(&keg).unwrap().owner,
            ExecutableOwner::Provider { .. }
        ),
        "the same symlinked Cellar, declared as the machine, must resolve"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn invariant_16_ensuring_the_state_directory_is_idempotent() {
    // Re-running a successful operation changes nothing further: same tree,
    // same modes, same contents, nothing accumulated. The ownership probe
    // `ensure` writes and removes does move the root's mtime, which is why
    // that is not what is compared.
    let home = scratch("idempotent");
    let dir = StateDir::resolve(Some(&home), None).unwrap();
    dir.ensure().unwrap();

    let marker = dir.history().join("run.json");
    macportsbrew_adapters::persistence::write_atomic(&marker, b"{}").unwrap();

    let listing = |d: &StateDir| -> Vec<(PathBuf, u32, Vec<String>)> {
        d.directories()
            .iter()
            .map(|path| {
                let mode = <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::mode(
                    &std::fs::metadata(path).unwrap().permissions(),
                ) & 0o777;
                let mut names: Vec<String> = std::fs::read_dir(path)
                    .unwrap()
                    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect();
                names.sort();
                (path.clone(), mode, names)
            })
            .collect()
    };

    let before = listing(&dir);
    dir.ensure().unwrap();
    dir.ensure().unwrap();
    let after = listing(&dir);

    assert_eq!(
        before, after,
        "a repeated ensure() changed a tree it had already made correct"
    );
    assert_eq!(std::fs::read(&marker).unwrap(), b"{}");
    // The ownership probe is not left behind.
    for (path, _, names) in &after {
        assert!(
            !names.iter().any(|n| n.contains("owner-probe")),
            "{} kept a probe file",
            path.display()
        );
    }

    std::fs::remove_dir_all(&home).ok();
}

#[test]
fn invariant_10_no_invocation_in_a_recording_carries_a_build_flag() {
    // Every invocation runs in an environment built from scratch. Anything that
    // inherited `PKG_CONFIG_PATH` would be the contamination INV-10 forbids -
    // and INV-10 is about the *provider* invocations most of all, which is what
    // the earlier name and the earlier filter both missed.
    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    // Every invocation, not the four probes. This used to `continue` past
    // anything that was not `sw_vers`, `uname`, `sysctl` or `xcode-select` -
    // which is to say, past every `brew` and `port` invocation in the
    // recording. INV-10 is about `brew` and `port`; the probes are the part it
    // was not written for (F-19).
    let mut checked = 0usize;
    let mut providers = 0usize;
    for recorded in &recording.invocations {
        let invocation = &recorded.invocation;
        checked += 1;
        if matches!(invocation.program.as_str(), "brew" | "port") {
            providers += 1;
        }
        for name in invocation.environment.keys() {
            assert!(
                !is_reported(name) || name == "MANPATH",
                "{} ran with {name} set",
                invocation.program
            );
        }
    }
    assert!(checked > 4, "the fixture has more than the four probes");
    assert!(
        providers > 0,
        "a fixture with no provider invocation cannot exercise INV-10 at all"
    );
}

#[test]
fn invariant_06_reading_the_machine_needs_no_elevation() {
    // Discovery must work with no elevation at all. The four probes are
    // unprivileged system tools, and nothing in the adapter layer names an
    // elevation mechanism outside a comment.
    let mut offenders = Vec::new();
    for (path, text) in sources() {
        for needle in ["sudo", "doas", "setuid", "seteuid", "AuthorizationExecute"] {
            for line in code_lines(&text, needle) {
                offenders.push(format!("{}: {}", path.display(), line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an elevation mechanism appears in non-comment source:\n{}",
        offenders.join("\n")
    );

    let fixture = workspace().join("fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording =
        macportsbrew_adapters::persistence::load_recording(&fixture).expect("fixture must load");
    let reader = SystemMachine::new(
        Box::new(ReplayRunner::new(recording)),
        None,
        BTreeMap::new(),
        HomeRedaction::Recorded,
    );
    reader.machine().expect("the machine reads unprivileged");
}

/// A two-answer recording: just `--cellar` and `--caskroom`, which is all
/// `owner_of` consults. Built in-test for one case, not a machine capture.
fn synthetic_cellar(cellar: &str, caskroom: &str) -> macportsbrew_adapters::process::Recording {
    use macportsbrew_adapters::process::{
        ExitStatus, Invocation, RecordedInvocation, Recording, RecordingProvenance,
    };
    let one = |argv: &[&str], stdout: &str| RecordedInvocation {
        invocation: Invocation {
            program: "brew".into(),
            argv: argv.iter().map(ToString::to_string).collect(),
            environment: BTreeMap::new(),
            status: ExitStatus::Exited { code: 0 },
            stdout: stdout.to_string(),
            stderr: String::new(),
            truncated: false,
            duration_ms: 0,
        },
    };
    Recording {
        schema_version: 1,
        provenance: RecordingProvenance {
            macos: "26.6.2".into(),
            architecture: "x86_64".into(),
            provider_versions: vec![],
            captured_at: "2026-09-05T00:00:00Z".into(),
            captured_by: "crates/adapters/tests/invariants.rs".into(),
            origin: "synthesised".into(),
            note: Some("two answers for the owner_of root guard; not a capture".into()),
        },
        invocations: vec![one(&["--cellar"], cellar), one(&["--caskroom"], caskroom)],
    }
}

/// A registry that names each item's port after the item itself, as MacPorts'
/// does for a port that keeps the default `startupitem.name`.
fn registry_agrees(plist: &Path) -> Option<String> {
    let file = plist.file_name()?.to_str()?;
    Some(
        file.strip_prefix("org.macports.")?
            .strip_suffix(".plist")?
            .to_string(),
    )
}
