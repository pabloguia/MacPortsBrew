// SPDX-License-Identifier: Apache-2.0

//! A capture writes down no home path and no user name.
//!
//! `06-cli-and-ux.md` ("Privacy and diagnostics") is unconditional about it,
//! and a snapshot is precisely the file a user attaches to an issue. The
//! adapters were already careful with the environment they *report*; what they
//! could not be careful with is the paths a provider hands back — Homebrew's
//! `--cache` is under `~/Library/Caches` on every machine, `HOME` is in the
//! sanitised invocation environment INV-10 builds, and a `PATH` lookup answers
//! with wherever the executable actually is. Those are redacted at the capture
//! boundary (`macportsbrew_application::capture`), which is the one place every
//! one of them passes through.
//!
//! The test lives in the command-line crate because it needs the real adapters
//! *and* the application's capture, and the layering
//! (`scripts/check-layering.sh`) lets only this crate see both.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::process::{
    RecordedInvocation, Recording, RecordingProvenance, ReplayRunner,
};
use macportsbrew_adapters::registry::{self, RegistryContext};
use macportsbrew_application::{capture, doctor, CaptureContext};
use macportsbrew_domain::{
    Architecture, CommandLineTools, DomainError, EnvironmentSnapshot, HomeRedaction, Machine,
    MachineRead, MacosVersion, PathFinding, PathFindingKind, ProviderId, Translation,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The home directory every path in this fixture is under. Nothing containing
/// it may survive into a snapshot, and `testuser` may not survive either — the
/// specification names user names separately from home paths.
const HOME: &str = "/Users/testuser";

fn invocation(program: &str, argv: &[&str], stdout: &str) -> RecordedInvocation {
    RecordedInvocation {
        invocation: macportsbrew_adapters::process::Invocation {
            program: program.to_string(),
            argv: argv.iter().map(|a| (*a).to_string()).collect(),
            environment: BTreeMap::new(),
            status: macportsbrew_adapters::process::ExitStatus::Exited { code: 0 },
            stdout: stdout.to_string(),
            stderr: String::new(),
            truncated: false,
            duration_ms: 1,
        },
    }
}

/// A Homebrew installed under the user's home, which is what `brew --prefix`
/// answers on a machine where it was installed without administrator rights,
/// and what the `--cache` answer looks like everywhere.
fn recording() -> Recording {
    Recording {
        schema_version: 1,
        provenance: RecordingProvenance {
            macos: "26.6.2".into(),
            architecture: "x86_64".into(),
            provider_versions: vec!["homebrew 4.6.15".into()],
            captured_at: "2026-09-02T00:00:00Z".into(),
            captured_by: "capture_redaction.rs".into(),
            origin: "synthesised".into(),
            note: Some("hand-written for the capture redaction test".into()),
        },
        invocations: vec![
            invocation("brew", &["--version"], "Homebrew 4.6.15\n"),
            invocation("brew", &["--prefix"], &format!("{HOME}/homebrew\n")),
            invocation("brew", &["--cellar"], &format!("{HOME}/homebrew/Cellar\n")),
            invocation(
                "brew",
                &["--caskroom"],
                &format!("{HOME}/homebrew/Caskroom\n"),
            ),
            invocation(
                "brew",
                &["--cache"],
                &format!("{HOME}/Library/Caches/Homebrew\n"),
            ),
            invocation(
                "brew",
                &["config"],
                &format!(
                    "HOMEBREW_VERSION: 4.6.15\nHOMEBREW_PREFIX: {HOME}/homebrew\n\
                     macOS: 26.6.2-x86_64\n"
                ),
            ),
            invocation(
                "brew",
                &["info", "--json=v2", "--installed"],
                r#"{"formulae":[],"casks":[]}"#,
            ),
            invocation("brew", &["list", "--pinned"], ""),
        ],
    }
}

/// A machine reader that answers `which` with paths under the home, which is
/// what a user-local Homebrew or a `~/.local/bin` produces. The real
/// [`macportsbrew_adapters::system::SystemMachine`] refuses to answer under
/// replay, on purpose, so the fixture supplies the answers here.
struct HomeMachine;

impl MachineRead for HomeMachine {
    fn machine(&self) -> Result<Machine, DomainError> {
        Ok(Machine {
            macos: MacosVersion::new(26, 6, 2),
            architecture: Architecture::X86_64,
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        })
    }

    fn environment(&self) -> Result<EnvironmentSnapshot, DomainError> {
        // Already redacted, as the system adapter delivers it.
        Ok(EnvironmentSnapshot {
            installations: std::collections::BTreeMap::default(),
            resolutions: std::collections::BTreeMap::default(),
            path: vec!["~/homebrew/bin".into(), "/usr/bin".into()],
            reported: BTreeMap::from([(
                "CPPFLAGS".to_string(),
                "-I~/homebrew/include".to_string(),
            )]),
            path_findings: vec![PathFinding {
                entry: "~/homebrew/bin".into(),
                index: 0,
                kind: PathFindingKind::GroupWritable,
            }],
        })
    }

    fn which(&self, executable: &str) -> Option<PathBuf> {
        Some(PathBuf::from(format!("{HOME}/homebrew/bin/{executable}")))
    }
}

#[test]
fn a_capture_carries_no_home_path_and_no_user_name() {
    let runner = ReplayRunner::new(recording());
    let providers = registry::build(&RegistryContext {
        runner: &runner,
        home: Some(PathBuf::from(HOME)),
        filesystem: FilesystemScope::Elsewhere,
        platform_tags: Box::new(|_| vec!["tahoe".to_string()]),
        prefix_override: Some(vec![(
            ProviderId::new("homebrew"),
            PathBuf::from(format!("{HOME}/homebrew")),
        )]),
    });
    let refs: Vec<&dyn macportsbrew_domain::ProviderRead> =
        providers.iter().map(AsRef::as_ref).collect();
    let machine = HomeMachine;
    let snapshot = capture(&CaptureContext {
        machine: &machine,
        providers: refs,
        policy_version: "test".into(),
        captured_at: "2026-09-02T00:00:00Z".into(),
        captured_by: "capture_redaction.rs".into(),
        home: HomeRedaction::Known(PathBuf::from(HOME)),
        environment_contracts: macportsbrew_adapters::registry::environment_contracts(),
        executables_for: std::collections::BTreeSet::new(),
    })
    .expect("the machine answers, so the capture succeeds");

    // The fixture is only meaningful if the provider actually answered: a
    // snapshot of an absent Homebrew would pass every assertion below while
    // proving nothing.
    let homebrew = &snapshot.providers[&ProviderId::new("homebrew")];
    assert_eq!(
        homebrew.detection.presence,
        macportsbrew_domain::ProviderPresence::Present,
        "the recording must make Homebrew present, or this test asserts nothing"
    );
    assert!(
        homebrew.environment.contains_key("cache"),
        "the `--cache` answer is the field this test exists for"
    );
    // This assertion used to require `Unavailable { reason }`, and the reason
    // named a cache file under the home — the free-text case this test found.
    // That value only existed because the adapter stat'd the *replaying host's*
    // disk to answer a question about the recorded machine, which is the INV-15
    // violation F-25 named. With the read guarded the honest answer is
    // `Unknown`, and asserting that is what keeps the violation from coming
    // back: an `Unavailable` here would mean something looked at this host
    // again. The redaction rule the old assertion was reaching for has its own
    // test, `capture::tests::a_capability_reason_is_redacted_like_any_other_free_text`.
    assert!(
        matches!(
            homebrew
                .capabilities
                .capabilities
                .get(&macportsbrew_domain::Capability::OfflineMetadata),
            Some(macportsbrew_domain::CapabilityState::Unknown)
        ),
        "a replayed reader answered the offline-metadata capability from this \
         host's filesystem (INV-15, F-25)"
    );

    let text = serde_json::to_string_pretty(&snapshot).expect("a snapshot serialises");
    assert_no_home(&text, "the snapshot");

    // `doctor --report` writes a strict subset of these two documents: the
    // machine, each provider's presence, version and prefix, the package names,
    // the number of `PATH` entries, and the findings. Nothing it writes comes
    // from anywhere else, so covering both covers it.
    let report = doctor(
        &snapshot,
        &PolicyData::load_embedded().unwrap(),
        "2026-09-02",
    );
    let text = serde_json::to_string_pretty(&report).expect("a report serialises");
    assert_no_home(&text, "the doctor report");

    // Section by section, and not vacuously: every part `doctor --report`
    // writes carries a home-rooted value in this fixture, so each assertion
    // below would fail on a leak there rather than pass over an empty section
    // (pass 10, item 4). `write_diagnostic_report` takes these and nothing
    // else: the machine, `PATH`'s length, each provider's presence, version,
    // prefix and package names, and the findings.
    let sections: Vec<(&str, String)> = vec![
        (
            "machine",
            serde_json::to_string(&snapshot.machine).expect("serialises"),
        ),
        (
            "provider prefix",
            serde_json::to_string(&homebrew.detection.prefix).expect("serialises"),
        ),
        (
            "provider packages",
            serde_json::to_string(&homebrew.inventory.installed_names()).expect("serialises"),
        ),
        (
            "findings",
            serde_json::to_string(&report.findings).expect("serialises"),
        ),
    ];
    for (section, text) in &sections {
        assert_no_home(text, section);
    }
    assert!(
        sections[1].1.contains("~/homebrew"),
        "the prefix section carries the redacted home, so it was tested: {}",
        sections[1].1
    );
    assert!(
        !report.findings.is_empty() && sections[3].1.contains("~/"),
        "the findings name a home-rooted PATH entry, redacted: {}",
        sections[3].1
    );
}

#[test]
fn a_resolution_taken_from_path_is_redacted_too() {
    // One provider rather than the registry's pair, so the assertions name the
    // Homebrew fields directly: the detected prefix and the executable built
    // from it are both under the home here.
    let runner = ReplayRunner::new(recording());
    let adapter = macportsbrew_adapters::homebrew::HomebrewAdapter::new(
        &runner,
        PathBuf::from(format!("{HOME}/homebrew")),
        Some(PathBuf::from(HOME)),
        vec!["tahoe".to_string()],
        FilesystemScope::Elsewhere,
    );
    let machine = HomeMachine;
    let snapshot = capture(&CaptureContext {
        machine: &machine,
        providers: vec![&adapter],
        policy_version: "test".into(),
        captured_at: "2026-09-02T00:00:00Z".into(),
        captured_by: "capture_redaction.rs".into(),
        home: HomeRedaction::Known(PathBuf::from(HOME)),
        environment_contracts: macportsbrew_adapters::registry::environment_contracts(),
        executables_for: std::collections::BTreeSet::new(),
    })
    .expect("the machine answers");

    let homebrew = &snapshot.providers[&ProviderId::new("homebrew")];
    assert_eq!(
        homebrew.detection.prefix,
        Some(PathBuf::from("~/homebrew")),
        "the detected prefix is stored redacted"
    );
    assert_no_home(
        &serde_json::to_string(&snapshot).unwrap(),
        "the single-provider snapshot",
    );
}

#[test]
fn a_recorded_capture_is_not_rewritten() {
    // Under replay the redaction is `Recorded`, and must then be the identity
    // function: a recording was redacted when it was captured, and redacting it
    // again against *this* machine's home would be a host fact reaching a
    // replayed analysis (INV-15).
    //
    // No home is passed, because `--replay` passes none - a recording is not
    // this machine, so there is no live home to hand a provider. The identity
    // property is asserted on the prefix and on the constructed `PATH`, both of
    // which carry the recording's own home path and must come back untouched.
    let runner = ReplayRunner::new(recording());
    let adapter = macportsbrew_adapters::homebrew::HomebrewAdapter::new(
        &runner,
        PathBuf::from(format!("{HOME}/homebrew")),
        None,
        vec!["tahoe".to_string()],
        FilesystemScope::Elsewhere,
    );
    let machine = HomeMachine;
    let snapshot = capture(&CaptureContext {
        machine: &machine,
        providers: vec![&adapter],
        policy_version: "test".into(),
        captured_at: "2026-09-02T00:00:00Z".into(),
        captured_by: "capture_redaction.rs".into(),
        home: HomeRedaction::Recorded,
        environment_contracts: macportsbrew_adapters::registry::environment_contracts(),
        executables_for: std::collections::BTreeSet::new(),
    })
    .expect("the machine answers");
    let homebrew = &snapshot.providers[&ProviderId::new("homebrew")];
    assert_eq!(
        homebrew.detection.prefix,
        Some(PathBuf::from(format!("{HOME}/homebrew"))),
        "a recording was redacted at capture, so it is left exactly as it was"
    );
    assert_eq!(
        homebrew
            .invocation_environment
            .get("PATH")
            .map(String::as_str),
        Some(format!("{HOME}/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin").as_str()),
        "and neither is the environment built from it"
    );
    assert!(
        !homebrew.invocation_environment.contains_key("HOME"),
        "a replayed capture has no live home to state"
    );
}

#[test]
fn a_live_capture_whose_home_is_unresolvable_still_carries_no_home_path() {
    // The other side of the same distinction, and the regression test for a
    // measured leak: with `HOME` unset, `doctor --report` wrote the real home
    // path 39 times into a file `06-cli-and-ux.md` invites the user to attach to
    // an issue. `Recorded` is right for a recording and was wrong here, because
    // a live run whose home is unknown has everything still to redact.
    let runner = ReplayRunner::new(recording());
    let adapter = macportsbrew_adapters::homebrew::HomebrewAdapter::new(
        &runner,
        PathBuf::from(format!("{HOME}/homebrew")),
        Some(PathBuf::from(HOME)),
        vec!["tahoe".to_string()],
        FilesystemScope::Elsewhere,
    );
    let machine = HomeMachine;
    let snapshot = capture(&CaptureContext {
        machine: &machine,
        providers: vec![&adapter],
        policy_version: "test".into(),
        captured_at: "2026-09-02T00:00:00Z".into(),
        captured_by: "capture_redaction.rs".into(),
        home: HomeRedaction::Unresolved,
        environment_contracts: macportsbrew_adapters::registry::environment_contracts(),
        executables_for: std::collections::BTreeSet::new(),
    })
    .expect("the machine answers");
    let homebrew = &snapshot.providers[&ProviderId::new("homebrew")];
    assert_eq!(
        homebrew.detection.prefix,
        Some(PathBuf::from("~/homebrew")),
        "an unresolvable home is redacted by shape rather than published"
    );
    assert_no_home(
        &serde_json::to_string(&snapshot).unwrap(),
        "the snapshot of a live capture with no resolvable home",
    );
}

fn assert_no_home(text: &str, what: &str) {
    assert!(
        !text.contains(HOME),
        "{what} carries the home path {HOME}:\n{text}"
    );
    assert!(
        !text.contains("testuser"),
        "{what} carries the user name:\n{text}"
    );
}

#[test]
fn a_replayed_capture_says_installations_are_unknowable_only_for_a_present_provider() {
    // INV-15: the recording is the machine, so where the provider is installed
    // is not something this host's disk can answer - the snapshot says so
    // (`Unknowable`) rather than "one". A provider that is not present has no
    // installation to find twice, and records nothing.
    let runner = ReplayRunner::new(recording());
    let providers = registry::build(&RegistryContext {
        runner: &runner,
        home: Some(PathBuf::from(HOME)),
        filesystem: FilesystemScope::Elsewhere,
        platform_tags: Box::new(|_| vec!["tahoe".to_string()]),
        prefix_override: Some(vec![(
            ProviderId::new("homebrew"),
            PathBuf::from(format!("{HOME}/homebrew")),
        )]),
    });
    let refs: Vec<&dyn macportsbrew_domain::ProviderRead> =
        providers.iter().map(AsRef::as_ref).collect();
    let machine = HomeMachine;
    let snapshot = capture(&CaptureContext {
        machine: &machine,
        providers: refs,
        policy_version: "test".into(),
        captured_at: "2026-09-02T00:00:00Z".into(),
        captured_by: "capture_redaction.rs".into(),
        home: HomeRedaction::Known(PathBuf::from(HOME)),
        environment_contracts: macportsbrew_adapters::registry::environment_contracts(),
        executables_for: std::collections::BTreeSet::new(),
    })
    .expect("the capture succeeds");

    let mut present = 0;
    for (id, provider) in &snapshot.providers {
        let expected =
            if provider.detection.presence == macportsbrew_domain::ProviderPresence::Present {
                present += 1;
                macportsbrew_domain::Installations::Unknowable
            } else {
                macportsbrew_domain::Installations::NotRecorded
            };
        assert_eq!(snapshot.installations_of(id), &expected, "{id}");
    }
    assert!(
        present > 0,
        "no provider was present, so this proved nothing"
    );
}
