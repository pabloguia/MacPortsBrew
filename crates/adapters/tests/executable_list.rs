// SPDX-License-Identifier: Apache-2.0

//! Part F's executable-list reads, both providers, over the captured machine.
//!
//! The inventory every adapter here reads is
//! `fixtures/recordings/both-providers-intel-macos26.json` (captured
//! 2026-09-07). That recording has **no** listing read in it - no recording
//! does yet (`docs/passes/16-plans/F.md`, "Fixtures"; the recapture waits on
//! `F/05`). So each test appends, **in memory only**, the one listing
//! invocation it needs, shaped after the R-21 transcripts (E-4: `port -q
//! contents` is two-space-indented absolute paths; E-9: `brew list --formula`
//! is unindented keg paths). Nothing is written under `fixtures/`, and the
//! appended invocation never claims to be a capture.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::homebrew::HomebrewAdapter;
use macportsbrew_adapters::macports::MacPortsAdapter;
use macportsbrew_adapters::process::{
    ExitStatus, Invocation, RecordedInvocation, Recording, ReplayRunner,
};
use macportsbrew_domain::{
    ErrorId, ExecutableList, ExecutableName, ListReason, PackageIdentity, Placement, ProviderId,
    ProviderRead, UnknownReason,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const FIXTURE: &str =
    include_str!("../../../fixtures/recordings/both-providers-intel-macos26.json");

fn listing(program: &str, argv: &[&str], code: i32, stdout: &str) -> RecordedInvocation {
    RecordedInvocation {
        invocation: Invocation {
            program: program.into(),
            argv: argv.iter().map(|a| (*a).to_string()).collect(),
            environment: BTreeMap::new(),
            status: ExitStatus::Exited { code },
            stdout: stdout.into(),
            stderr: String::new(),
            truncated: false,
            duration_ms: 0,
        },
    }
}

fn runner(extra: Vec<RecordedInvocation>) -> ReplayRunner {
    let mut recording = Recording::from_json(FIXTURE).expect("the captured fixture parses");
    recording.invocations.extend(extra);
    ReplayRunner::new(recording)
}

fn homebrew(runner: &ReplayRunner) -> HomebrewAdapter<'_> {
    HomebrewAdapter::new(
        runner,
        PathBuf::from("/usr/local"),
        None,
        vec!["tahoe".into(), "all".into()],
        FilesystemScope::Elsewhere,
    )
}

fn macports(runner: ReplayRunner) -> MacPortsAdapter<ReplayRunner> {
    MacPortsAdapter::new(
        runner,
        PathBuf::from("/opt/local"),
        None,
        FilesystemScope::Elsewhere,
    )
}

fn brew(name: &str) -> PackageIdentity {
    PackageIdentity::new(ProviderId::new("homebrew"), name)
}

fn port(name: &str) -> PackageIdentity {
    PackageIdentity::new(ProviderId::new("macports"), name)
}

fn names(list: &ExecutableList) -> Vec<&str> {
    list.known()
        .expect("a known list")
        .iter()
        .map(ExecutableName::as_str)
        .collect()
}

fn dirs(placement: &Placement) -> BTreeMap<String, String> {
    match placement {
        Placement::OnPathDir { dirs } => dirs
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.clone()))
            .collect(),
        other => panic!("expected ON_PATH_DIR, got {other:?}"),
    }
}

const BREW_XZ: &str = "/usr/local/Cellar/xz/5.8.3/bin/lzcat
/usr/local/Cellar/xz/5.8.3/bin/xz
/usr/local/Cellar/xz/5.8.3/include/lzma.h
/usr/local/Cellar/xz/5.8.3/lib/liblzma.5.dylib
/usr/local/Cellar/xz/5.8.3/share/man/man1/xz.1
";

const PORT_XZ: &str = "  /opt/local/bin/lzcat
  /opt/local/bin/xz
  /opt/local/include/lzma.h
  /opt/local/lib/liblzma.5.dylib
  /opt/local/share/man/man1/xz.1.gz
";

#[test]
fn placement_is_filled_from_detection_for_both_providers() {
    // Homebrew: `xz` has a linked keg (5.8.3), so its executables land in the
    // detected prefix's `bin`.
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        BREW_XZ,
    )]);
    let (list, placement) = homebrew(&r).executables(&brew("xz")).unwrap();
    assert_eq!(names(&list), vec!["lzcat", "xz"]);
    assert_eq!(
        dirs(&placement),
        [
            ("lzcat".into(), "/usr/local/bin".into()),
            ("xz".into(), "/usr/local/bin".into())
        ]
        .into_iter()
        .collect()
    );

    // MacPorts: active, and its own listing places each name directly under
    // the detected prefix's `bin`.
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        PORT_XZ,
    )]));
    let (list, placement) = m.executables(&port("xz")).unwrap();
    assert_eq!(names(&list), vec!["lzcat", "xz"]);
    assert_eq!(
        dirs(&placement),
        [
            ("lzcat".into(), "/opt/local/bin".into()),
            ("xz".into(), "/opt/local/bin".into())
        ]
        .into_iter()
        .collect()
    );

    // Homebrew keg-only with one keg (`m4`): read, and NOT_LINKED with
    // Homebrew's own reason - never "on PATH" for want of a field.
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "m4"],
        0,
        "/usr/local/Cellar/m4/1.4.21/bin/m4\n/usr/local/Cellar/m4/1.4.21/share/info/m4.info\n",
    )]);
    let (list, placement) = homebrew(&r).executables(&brew("m4")).unwrap();
    assert_eq!(names(&list), vec!["m4"]);
    assert!(
        matches!(placement, Placement::NotLinked { ref reason } if !reason.is_empty()),
        "{placement:?}"
    );

    // Not installed is UNKNOWN { NOT_INSTALLED } for both, with no read.
    let r = runner(Vec::new());
    let not_installed = UnknownReason::Listed(ListReason::NotInstalled);
    assert_eq!(
        homebrew(&r).executables(&brew("ripgrep")).unwrap(),
        (
            ExecutableList::unknown(not_installed),
            Placement::unknown(not_installed)
        )
    );
    let m = macports(runner(Vec::new()));
    assert_eq!(
        m.executables(&port("ripgrep")).unwrap(),
        (
            ExecutableList::unknown(not_installed),
            Placement::unknown(not_installed)
        )
    );
    assert!(r.consulted().iter().all(|c| !c.contains("list --formula")));
    assert!(m
        .runner()
        .consulted()
        .iter()
        .all(|c| !c.contains("contents")));
}

#[test]
fn invariant_20_bin_sbin_and_cellar_come_from_detection_and_the_keg_from_linked_keg() {
    // The Cellar is `brew --cellar`'s answer and the prefix the detected one:
    // run the same listing against a recording whose `--cellar` says
    // somewhere else, and the keg paths no longer belong to the keg the read
    // was for. A literal `/usr/local/Cellar` would accept them.
    let mut recording = Recording::from_json(FIXTURE).unwrap();
    for recorded in &mut recording.invocations {
        if recorded.invocation.program == "brew" && recorded.invocation.argv == ["--cellar"] {
            recorded.invocation.stdout = "/opt/elsewhere/Cellar\n".into();
        }
    }
    recording
        .invocations
        .push(listing("brew", &["list", "--formula", "xz"], 0, BREW_XZ));
    let r = ReplayRunner::new(recording);
    let (list, _) = homebrew(&r).executables(&brew("xz")).unwrap();
    assert_eq!(
        list,
        ExecutableList::unknown(ErrorId::ProviderOutputChanged),
        "the keg comes from detection, not a literal"
    );

    // `sbin` is the detected prefix's, for both providers.
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        "/usr/local/Cellar/xz/5.8.3/sbin/xzd\n/usr/local/Cellar/xz/5.8.3/bin/xz\n",
    )]);
    let (_, placement) = homebrew(&r).executables(&brew("xz")).unwrap();
    assert_eq!(dirs(&placement)["xzd"], "/usr/local/sbin");
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "  /opt/local/sbin/xzd\n  /opt/local/bin/xz\n",
    )]));
    let (_, placement) = m.executables(&port("xz")).unwrap();
    assert_eq!(dirs(&placement)["xzd"], "/opt/local/sbin");

    // The prefix is the detected one, for both providers (SG F-2): build each
    // adapter on a prefix no literal names, and the placement directories and
    // the MacPorts prefix filter follow it. A hard-coded `/usr/local` or
    // `/opt/local` fails here.
    let elsewhere = PathBuf::from("/opt/elsewhere");
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        "/usr/local/Cellar/xz/5.8.3/bin/xz\n/usr/local/Cellar/xz/5.8.3/sbin/xzd\n",
    )]);
    let adapter = HomebrewAdapter::new(
        &r,
        elsewhere.clone(),
        None,
        vec!["tahoe".into(), "all".into()],
        FilesystemScope::Elsewhere,
    );
    let (_, placement) = adapter.executables(&brew("xz")).unwrap();
    assert_eq!(
        dirs(&placement),
        [
            ("xz".into(), "/opt/elsewhere/bin".into()),
            ("xzd".into(), "/opt/elsewhere/sbin".into())
        ]
        .into_iter()
        .collect()
    );
    let m = MacPortsAdapter::new(
        runner(vec![listing(
            "port",
            &["-q", "contents", "xz"],
            0,
            "  /opt/elsewhere/bin/xz\n  /opt/elsewhere/sbin/xzd\n",
        )]),
        elsewhere.clone(),
        None,
        FilesystemScope::Elsewhere,
    );
    let (_, placement) = m.executables(&port("xz")).unwrap();
    assert_eq!(
        dirs(&placement),
        [
            ("xz".into(), "/opt/elsewhere/bin".into()),
            ("xzd".into(), "/opt/elsewhere/sbin".into())
        ]
        .into_iter()
        .collect()
    );
    let m = MacPortsAdapter::new(
        runner(vec![listing("port", &["-q", "contents", "xz"], 0, PORT_XZ)]),
        elsewhere,
        None,
        FilesystemScope::Elsewhere,
    );
    let nothing = UnknownReason::Listed(ListReason::NothingUnderPrefix);
    assert_eq!(
        m.executables(&port("xz")).unwrap().0,
        ExecutableList::unknown(nothing),
        "a listing under `/opt/local` is not under the detected prefix"
    );

    // The keg is the one `linked_keg` names. A listing of `htop`'s keg that
    // is not 3.5.3 (the linked one) is a different keg's listing.
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "htop"],
        0,
        "/usr/local/Cellar/htop/3.5.2/bin/htop\n",
    )]);
    assert_eq!(
        homebrew(&r).executables(&brew("htop")).unwrap().0,
        ExecutableList::unknown(ErrorId::ProviderOutputChanged)
    );
}

#[test]
fn a_keg_path_outside_the_linked_keg_is_provider_output_changed() {
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        "/usr/local/Cellar/xz/5.8.3/bin/xz\n/usr/local/Cellar/other/1.0/bin/evil\n",
    )]);
    let (list, placement) = homebrew(&r).executables(&brew("xz")).unwrap();
    let changed = UnknownReason::Read(ErrorId::ProviderOutputChanged);
    assert_eq!(list, ExecutableList::unknown(changed));
    assert_eq!(placement, Placement::unknown(changed));
}

#[test]
fn several_kegs_without_a_linked_keg_is_ambiguous_without_a_read() {
    // `pydantic`: two kegs, none linked, not keg-only. `libffi`: two kegs,
    // none linked, keg-only - its placement is still known.
    let r = runner(Vec::new());
    let adapter = homebrew(&r);
    let ambiguous = UnknownReason::Listed(ListReason::AmbiguousKeg);
    assert_eq!(
        adapter.executables(&brew("pydantic")).unwrap(),
        (
            ExecutableList::unknown(ambiguous),
            Placement::unknown(ambiguous)
        )
    );
    let (list, placement) = adapter.executables(&brew("libffi")).unwrap();
    assert_eq!(list, ExecutableList::unknown(ambiguous));
    assert!(
        matches!(placement, Placement::NotLinked { .. }),
        "{placement:?}"
    );
    assert!(
        r.consulted().iter().all(|c| !c.contains("list --formula")),
        "no read: {:?}",
        r.consulted()
    );
}

#[test]
fn a_cask_answers_unknown_without_a_read() {
    let r = runner(Vec::new());
    let cask = UnknownReason::Listed(ListReason::CaskNotCovered);
    assert_eq!(
        homebrew(&r).executables(&brew("meld")).unwrap(),
        (ExecutableList::unknown(cask), Placement::unknown(cask))
    );
    assert!(r.consulted().iter().all(|c| !c.contains("list --formula")));
}

#[test]
fn homebrew_exit_0_with_empty_stdout_is_provider_output_changed() {
    let r = runner(vec![listing("brew", &["list", "--formula", "xz"], 0, "")]);
    let (list, _) = homebrew(&r).executables(&brew("xz")).unwrap();
    assert_eq!(
        list,
        ExecutableList::unknown(ErrorId::ProviderOutputChanged),
        "an empty answer is never KNOWN{{}}"
    );
}

#[test]
fn macports_exit_0_with_empty_stdout_is_provider_output_changed() {
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "",
    )]));
    let (list, _) = m.executables(&port("xz")).unwrap();
    assert_eq!(
        list,
        ExecutableList::unknown(ErrorId::ProviderOutputChanged)
    );
}

#[test]
fn no_executables_only_from_a_non_empty_listing_with_no_bin_children() {
    // Both providers: a real listing with files, none directly in bin/sbin.
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        "/usr/local/Cellar/xz/5.8.3/lib/liblzma.5.dylib\n/usr/local/Cellar/xz/5.8.3/libexec/bin/inner\n",
    )]);
    let (list, _) = homebrew(&r).executables(&brew("xz")).unwrap();
    assert_eq!(
        list,
        ExecutableList::Known {
            executables: BTreeSet::new()
        }
    );
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "  /opt/local/lib/liblzma.5.dylib\n  /opt/local/share/doc/xz/README\n",
    )]));
    let (list, placement) = m.executables(&port("xz")).unwrap();
    assert_eq!(
        list,
        ExecutableList::Known {
            executables: BTreeSet::new()
        }
    );
    assert_eq!(
        placement,
        Placement::OnPathDir {
            dirs: BTreeMap::new()
        }
    );
}

#[test]
fn a_listing_with_nothing_under_the_prefix_is_unknown_nothing_under_prefix() {
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "  /Applications/MacPorts/Thing.app/Contents/Info.plist\n",
    )]));
    let nothing = UnknownReason::Listed(ListReason::NothingUnderPrefix);
    assert_eq!(
        m.executables(&port("xz")).unwrap(),
        (
            ExecutableList::unknown(nothing),
            Placement::unknown(nothing)
        )
    );
}

#[test]
fn a_sibling_named_prefix_is_not_under_the_prefix() {
    // `/opt/localx` starts with the string `/opt/local` but is not under it,
    // component by component: a string-prefix `is_under` would read these
    // as the port's own executables.
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "  /opt/localx/bin/xz\n  /opt/localx/share/doc/xz/README\n",
    )]));
    let nothing = UnknownReason::Listed(ListReason::NothingUnderPrefix);
    assert_eq!(
        m.executables(&port("xz")).unwrap(),
        (
            ExecutableList::unknown(nothing),
            Placement::unknown(nothing)
        )
    );
}

#[test]
fn a_sibling_keg_version_is_outside_the_keg() {
    // `5.8.3x` starts with the string `5.8.3` but is another keg: a
    // string-prefix `is_under` would accept a listing of it.
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        "/usr/local/Cellar/xz/5.8.3x/bin/xz\n",
    )]);
    let changed = UnknownReason::Read(ErrorId::ProviderOutputChanged);
    assert_eq!(
        homebrew(&r).executables(&brew("xz")).unwrap(),
        (
            ExecutableList::unknown(changed),
            Placement::unknown(changed)
        )
    );
}

#[test]
fn a_contents_line_without_the_indent_is_provider_output_changed() {
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "  /opt/local/bin/xz\n/opt/local/bin/lzcat\n",
    )]));
    let (list, _) = m.executables(&port("xz")).unwrap();
    assert_eq!(
        list,
        ExecutableList::unknown(ErrorId::ProviderOutputChanged)
    );
}

#[test]
fn only_direct_entries_of_bin_and_sbin_are_executables_for_both_adapters() {
    let r = runner(vec![listing(
        "brew",
        &["list", "--formula", "xz"],
        0,
        "/usr/local/Cellar/xz/5.8.3/bin/xz\n/usr/local/Cellar/xz/5.8.3/bin/sub/deep\n/usr/local/Cellar/xz/5.8.3/libexec/bin/inner\n",
    )]);
    assert_eq!(
        names(&homebrew(&r).executables(&brew("xz")).unwrap().0),
        vec!["xz"]
    );
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        0,
        "  /opt/local/bin/xz\n  /opt/local/bin/sub/deep\n  /opt/local/libexec/bin/inner\n  /opt/local/binx/no\n",
    )]));
    assert_eq!(names(&m.executables(&port("xz")).unwrap().0), vec!["xz"]);
}

#[test]
fn a_provider_that_answered_badly_is_unknown_for_that_package_and_says_why() {
    // MacPorts' `Registry error` (R-21 E-4) is exit 1.
    let m = macports(runner(vec![listing(
        "port",
        &["-q", "contents", "xz"],
        1,
        "",
    )]));
    assert_eq!(
        m.executables(&port("xz")).unwrap().0,
        ExecutableList::unknown(ErrorId::ProviderUnhealthy)
    );
    let r = runner(vec![listing("brew", &["list", "--formula", "xz"], 1, "")]);
    assert_eq!(
        homebrew(&r).executables(&brew("xz")).unwrap().0,
        ExecutableList::unknown(ErrorId::ProviderUnhealthy)
    );
}

#[test]
fn a_replayed_missing_invocation_is_unknown_and_the_capture_completes() {
    // No listing in the recording: the replay runner's `Internal` is a
    // per-package unknown, and the next package is still answered.
    let r = runner(Vec::new());
    let adapter = homebrew(&r);
    assert_eq!(
        adapter.executables(&brew("xz")).unwrap().0,
        ExecutableList::unknown(ErrorId::Internal)
    );
    assert!(adapter.executables(&brew("meld")).is_ok());
    let m = macports(runner(Vec::new()));
    assert_eq!(
        m.executables(&port("xz")).unwrap().0,
        ExecutableList::unknown(ErrorId::Internal)
    );
    assert!(m.executables(&port("htop")).is_ok());
}
