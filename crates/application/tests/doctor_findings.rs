// SPDX-License-Identifier: Apache-2.0

//! `doctor` findings that no test reached.
//!
//! `doctor_path.rs` covers the `PATH` findings thoroughly. Four other branches
//! had no test at all, and the first coverage run this repository has ever
//! done found them (DP-015): a provider built for a different architecture
//! than the machine, the `PATH` order *between* two providers, Command Line
//! Tools missing or unknown, and policy data old enough to warn about.
//!
//! Each is something a user is shown and acts on, which is the argument for
//! testing them rather than the line count. Every assertion here is on the
//! finding's identifier and severity — never on English, which lives in the
//! catalogue and is free to change.

use macportsbrew_application::{doctor, Severity};
use macportsbrew_domain::{
    Architecture, Capability, CapabilityState, CommandLineTools, EnvironmentSnapshot,
    Installations, Machine, MacosVersion, Provenance, ProviderCapabilities, ProviderDetection,
    ProviderId, ProviderInventory, ProviderPresence, ProviderSnapshot, Snapshot, Translation,
    Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;
use std::path::PathBuf;

const HB: &str = "homebrew";
const MP: &str = "macports";

/// The same, but recorded by a capture that redacted the prefix (F-49).
fn provider_redacted(id: &str, prefix: &str, arch: Architecture) -> ProviderSnapshot {
    let mut p = provider(id, prefix, arch);
    p.detection.prefix_redacted = true;
    p
}

/// A present provider at `prefix`, reporting `arch`.
fn provider(id: &str, prefix: &str, arch: Architecture) -> ProviderSnapshot {
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: ProviderId::new(id),
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(arch),
            prefix_redacted: false,
        },
        capabilities: ProviderCapabilities::default(),
        inventory: ProviderInventory::default(),
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        configuration: macportsbrew_domain::ProviderConfiguration::NotRecorded,
    }
}

fn snapshot(
    machine_arch: Architecture,
    tools: CommandLineTools,
    path: &[&str],
    providers: Vec<(&str, ProviderSnapshot)>,
) -> Snapshot {
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-05T00:00:00Z".into(),
            captured_by: "doctor_findings.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            macos: MacosVersion::new(26, 0, 0),
            architecture: machine_arch,
            translation: Translation::Native,
            command_line_tools: tools,
        },
        environment: EnvironmentSnapshot {
            installations: std::collections::BTreeMap::default(),
            path: path.iter().map(|e| (*e).to_string()).collect(),
            reported: BTreeMap::new(),
            path_findings: Vec::new(),
        },
        providers: providers
            .into_iter()
            .map(|(id, p)| (ProviderId::new(id), p))
            .collect(),
    }
}

fn installed() -> CommandLineTools {
    CommandLineTools::Installed {
        path: "/Library/Developer/CommandLineTools".into(),
    }
}

/// Every finding with `id`, as `(severity, reason identifier)`.
fn findings(snapshot: &Snapshot, today: &str, id: &str) -> Vec<(Severity, String)> {
    let report = doctor(
        snapshot,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        today,
    );
    report
        .findings
        .iter()
        .filter(|f| f.id == id)
        .map(|f| (f.severity, f.reason.id.as_str().to_string()))
        .collect()
}

const TODAY: &str = "2026-09-05";

#[test]
fn a_provider_built_for_another_architecture_is_a_warning_naming_both() {
    // The case this exists for: an Intel Homebrew surviving under Rosetta on an
    // Apple Silicon Mac. It runs, so it is not an error — but every bottle it
    // pours is for the wrong architecture, and nothing else in the output says
    // so.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &[],
        vec![(HB, provider(HB, "/usr/local", Architecture::X86_64))],
    );
    let found = findings(&snap, TODAY, "provider_architecture_mismatch");
    assert_eq!(found.len(), 1, "expected exactly one mismatch finding");
    assert_eq!(found[0].0, Severity::Warning);
    assert_eq!(found[0].1, "doctor.provider_architecture_mismatch");
}

#[test]
fn a_provider_matching_the_machine_says_nothing_about_architecture() {
    // The other half. Without it the test above would pass on a `doctor` that
    // warned unconditionally.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &[],
        vec![(HB, provider(HB, "/opt/homebrew", Architecture::Arm64))],
    );
    assert!(findings(&snap, TODAY, "provider_architecture_mismatch").is_empty());
}

#[test]
fn the_path_order_between_two_providers_is_reported_when_both_are_on_it() {
    // Reported, never changed: which provider's `bin` comes first decides
    // which copy of a shared executable runs, and that is the user's to
    // arrange (INV-12 — this is not a global priority the tool sets).
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/opt/local/bin", "/opt/homebrew/bin", "/usr/bin"],
        vec![
            (HB, provider(HB, "/opt/homebrew", Architecture::Arm64)),
            (MP, provider(MP, "/opt/local", Architecture::Arm64)),
        ],
    );
    let found = findings(&snap, TODAY, "path_order");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0, Severity::Ok);
}

#[test]
fn one_provider_on_the_path_is_not_an_order_to_report() {
    // An order needs two. This is the boundary the `>= 2` guard draws, and
    // without it the test above proves only that the branch runs.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/opt/homebrew/bin", "/usr/bin"],
        vec![
            (HB, provider(HB, "/opt/homebrew", Architecture::Arm64)),
            (MP, provider(MP, "/opt/local", Architecture::Arm64)),
        ],
    );
    assert!(findings(&snap, TODAY, "path_order").is_empty());
}

#[test]
fn command_line_tools_missing_or_unknown_are_both_warnings_and_are_distinguishable() {
    // Two different facts, and the difference matters: *missing* means a
    // source build will fail, *unknown* means the tool could not tell. Folding
    // them together would report a certainty nobody has.
    for (tools, expected) in [
        (
            CommandLineTools::Missing,
            "doctor.command_line_tools_missing",
        ),
        (
            CommandLineTools::Unknown,
            "doctor.command_line_tools_unknown",
        ),
    ] {
        let snap = snapshot(Architecture::Arm64, tools, &[], vec![]);
        let found = findings(&snap, TODAY, "command_line_tools");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, Severity::Warning);
        assert_eq!(found[0].1, expected, "the two must stay distinguishable");
    }

    let snap = snapshot(Architecture::Arm64, installed(), &[], vec![]);
    let found = findings(&snap, TODAY, "command_line_tools");
    assert_eq!(found[0].0, Severity::Ok);
    assert_eq!(found[0].1, "doctor.command_line_tools_present");
}

#[test]
fn policy_data_older_than_the_staleness_window_warns_and_says_when_it_was_read() {
    // `stale_after_days` is 180. The notice does not refuse to answer — the
    // recommendations still stand — it says the facts behind them are old,
    // which is the whole point of every entry carrying a read date (INV-20).
    let far_future = "2030-01-01";
    let snap = snapshot(Architecture::Arm64, installed(), &[], vec![]);

    let stale = findings(&snap, far_future, "policy_stale");
    assert_eq!(stale.len(), 1, "the data must be stale in 2030");
    assert_eq!(stale[0].0, Severity::Warning);
    assert_eq!(stale[0].1, "policy.stale_notice");

    assert!(
        findings(&snap, TODAY, "policy_stale").is_empty(),
        "the data must not be stale on the day it was read, or this asserts nothing"
    );
}

#[test]
fn an_absent_path_is_reported_as_unknown_rather_than_as_a_missing_prefix() {
    // F-34. A `Recording` carries no environment at all, so a replayed
    // `doctor` saw an empty `PATH` and concluded that every present provider's
    // prefix was missing from it. On the machine that produced the recording
    // both prefixes were on `PATH`; live `doctor` said so and emitted the real
    // findings instead. The warning was invented from absent input, and a user
    // would act on it by editing a `PATH` that is already correct.
    //
    // The mirror of F-29 rather than a copy of it: that one turns an unknown
    // into a silent "no", this one turned it into a confident warning.
    let empty = snapshot(
        Architecture::X86_64,
        installed(),
        &[],
        vec![
            (
                "homebrew",
                provider("homebrew", "/usr/local", Architecture::X86_64),
            ),
            (
                "macports",
                provider("macports", "/opt/local", Architecture::X86_64),
            ),
        ],
    );
    assert_eq!(
        findings(&empty, TODAY, "prefix_not_on_path"),
        vec![],
        "with no PATH recorded, nothing may be claimed about a prefix's absence from it"
    );
    let unknown = findings(&empty, TODAY, "path_unknown");
    assert_eq!(
        unknown.len(),
        1,
        "the absence is reported once, not once per provider: {unknown:?}"
    );
    assert_eq!(unknown[0].0, Severity::Warning);

    // The control. With a PATH that genuinely omits one prefix, the original
    // finding must still fire — otherwise this fix would have replaced a false
    // warning with no warning at all.
    let partial = snapshot(
        Architecture::X86_64,
        installed(),
        &["/usr/local/bin", "/usr/bin"],
        vec![
            (
                "homebrew",
                provider("homebrew", "/usr/local", Architecture::X86_64),
            ),
            (
                "macports",
                provider("macports", "/opt/local", Architecture::X86_64),
            ),
        ],
    );
    let missing = findings(&partial, TODAY, "prefix_not_on_path");
    assert_eq!(
        missing.len(),
        1,
        "macports really is absent from this PATH and must still be named: {missing:?}"
    );
    assert!(
        findings(&partial, TODAY, "path_unknown").is_empty(),
        "a PATH that exists is not an unknown one"
    );
}

/// F-48: a provider whose recorded prefix is not absolute analyses fine and can
/// run no step, and until this finding existed nothing said so until the refusal.
///
/// The prefix is redacted to `~` at capture — the privacy machinery exists so a
/// home-installed provider can be captured at all — and
/// `snapshot::prefix_shape_error` passes a `~`-rooted prefix on purpose, naming
/// `planner::command` as what refuses it. So `analyze` is right, every `plan` for
/// that provider exits 8, and the refusal tells the user their input is at fault
/// when nothing is.
///
/// This does not fix that. It moves the moment the user learns it from the
/// failure to the diagnosis, which is the reversible half of the design question
/// F-48 leaves open.
///
/// The predicate is the planner's own — `!is_absolute()`, not `starts_with('~')`
/// — so the two cannot drift apart. It covers only the shape that fails *closed*;
/// the `USER`-component shape redaction also produces is absolute, is accepted,
/// and is F-49.
#[test]
fn a_prefix_that_is_not_absolute_is_a_warning_because_no_step_can_run_it() {
    let redacted = snapshot(
        Architecture::Arm64,
        installed(),
        &["/usr/bin"],
        vec![
            (
                "homebrew",
                provider("homebrew", "~/homebrew", Architecture::Arm64),
            ),
            (
                "macports",
                provider("macports", "/opt/local", Architecture::Arm64),
            ),
        ],
    );
    let found = findings(&redacted, TODAY, "provider_prefix_is_not_absolute");
    assert_eq!(
        found,
        vec![(
            Severity::Warning,
            "doctor.provider_prefix_is_not_absolute".to_string()
        )],
        "exactly the home-rooted provider is reported, and as a warning: the \
         analysis is correct, one operation is unavailable"
    );

    // The other provider in the same snapshot is not swept up.
    let report = doctor(
        &redacted,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        TODAY,
    );
    let named: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.id == "provider_prefix_is_not_absolute")
        .filter_map(|f| f.provider.as_ref())
        .map(ToString::to_string)
        .collect();
    assert_eq!(named, vec!["homebrew".to_string()]);

    // And absent when neither prefix is under the home, or this asserts nothing.
    let ordinary = snapshot(
        Architecture::Arm64,
        installed(),
        &["/usr/bin"],
        vec![
            (
                "homebrew",
                provider("homebrew", "/opt/homebrew", Architecture::Arm64),
            ),
            (
                "macports",
                provider("macports", "/opt/local", Architecture::Arm64),
            ),
        ],
    );
    assert!(
        findings(&ordinary, TODAY, "provider_prefix_is_not_absolute").is_empty(),
        "an absolute prefix that nothing redacted is not reported"
    );
}

/// F-49: the shape that fails open, now caught by what capture recorded.
///
/// Redaction replaces the bare login name wherever it is a whole path component,
/// so `/opt/ada/homebrew` is written `/opt/USER/homebrew`. That is absolute, so
/// `planner::command` accepts it and emits a plan naming a program that exists on
/// no machine — where the `~` shape is refused and the user at least gets an
/// error. Nothing in the string betrays it, which is why the answer is a fact
/// capture records rather than a cleverer predicate.
///
/// The last case is the honest limit and is asserted rather than hidden: a
/// snapshot written before the field carries `false`, so a `USER`-shaped prefix
/// in one of those is still missed. That is an absence in the file, and guessing
/// at it is what made the first version of this check wrong.
#[test]
fn what_capture_recorded_catches_the_redaction_shape_the_string_hides() {
    let case = |p: ProviderSnapshot| {
        let snap = snapshot(
            Architecture::Arm64,
            installed(),
            &["/usr/bin"],
            vec![
                ("homebrew", p),
                (
                    "macports",
                    provider("macports", "/opt/local", Architecture::Arm64),
                ),
            ],
        );
        !findings(&snap, TODAY, "provider_prefix_is_not_absolute").is_empty()
    };

    assert!(
        case(provider_redacted(
            "homebrew",
            "/opt/USER/homebrew",
            Architecture::Arm64
        )),
        "absolute, but capture says it redacted: this is the shape that fails open"
    );
    assert!(
        case(provider("homebrew", "~/homebrew", Architecture::Arm64)),
        "a document written before the field still reports the `~` shape"
    );
    assert!(
        !case(provider(
            "homebrew",
            "/opt/USER/homebrew",
            Architecture::Arm64
        )),
        "and the limit: without the field, a USER-shaped prefix is indistinguishable \
         from a directory somebody named USER"
    );
}

// --- DP-027 (c): a provider's own configuration ------------------------------

fn configured(
    id: &str,
    configuration: macportsbrew_domain::ProviderConfiguration,
) -> ProviderSnapshot {
    let mut p = provider(id, "/opt/homebrew", Architecture::Arm64);
    p.configuration = configuration;
    p
}

fn file(path: &str, readable: bool, keys: &[&str]) -> macportsbrew_domain::ConfigurationFile {
    macportsbrew_domain::ConfigurationFile {
        path: path.into(),
        scope: macportsbrew_domain::ConfigurationScope::User,
        readable,
        keys: keys.iter().map(|k| (*k).to_string()).collect(),
        analytics_keys: std::collections::BTreeSet::new(),
    }
}

fn configuration_findings(
    configuration: macportsbrew_domain::ProviderConfiguration,
) -> Vec<(String, Severity, String)> {
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/usr/bin"],
        vec![("homebrew", configured("homebrew", configuration))],
    );
    let report = doctor(
        &snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        TODAY,
    );
    report
        .findings
        .iter()
        .filter(|f| f.id.starts_with("provider_configuration"))
        .map(|f| {
            let detail = format!("{:?}", f.reason.params);
            (f.id.clone(), f.severity, detail)
        })
        .collect()
}

#[test]
fn a_configuration_file_setting_a_reviewed_key_is_a_warning_naming_file_and_keys() {
    let found = configuration_findings(macportsbrew_domain::ProviderConfiguration::Read {
        files: vec![file(
            "~/.homebrew/brew.env",
            true,
            &["HOMEBREW_NO_ANALYTICS", "HOMEBREW_CACHE"],
        )],
    });
    assert_eq!(found.len(), 1, "{found:?}");
    let (id, severity, detail) = &found[0];
    assert_eq!(id, "provider_configuration_outranks");
    assert_eq!(*severity, Severity::Warning);
    assert!(
        detail.contains("~/.homebrew/brew.env")
            && detail.contains("HOMEBREW_CACHE, HOMEBREW_NO_ANALYTICS"),
        "{detail}"
    );
}

#[test]
fn an_unreadable_configuration_file_is_a_warning_and_never_a_clean_bill() {
    let found = configuration_findings(macportsbrew_domain::ProviderConfiguration::Read {
        files: vec![file("/etc/homebrew/brew.env", false, &[])],
    });
    let ids: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(ids, ["provider_configuration_unreadable"]);
    assert_eq!(found[0].1, Severity::Warning);
}

#[test]
fn nothing_found_is_said_and_is_the_only_ok() {
    for configuration in [
        macportsbrew_domain::ProviderConfiguration::Read { files: Vec::new() },
        macportsbrew_domain::ProviderConfiguration::Read {
            files: vec![file("~/.homebrew/brew.env", true, &[])],
        },
    ] {
        let found = configuration_findings(configuration);
        let ids: Vec<(&str, Severity)> = found.iter().map(|f| (f.0.as_str(), f.1)).collect();
        assert_eq!(ids, [("provider_configuration_none", Severity::Ok)]);
    }
}

#[test]
fn an_input_that_cannot_say_is_a_warning_that_it_is_unknown() {
    // An unknown is never a "none" (DP-027; the `path_unknown` precedent).
    for configuration in [
        macportsbrew_domain::ProviderConfiguration::Unknowable,
        macportsbrew_domain::ProviderConfiguration::NotRecorded,
    ] {
        let found = configuration_findings(configuration);
        let ids: Vec<(&str, Severity)> = found.iter().map(|f| (f.0.as_str(), f.1)).collect();
        assert_eq!(ids, [("provider_configuration_unknown", Severity::Warning)]);
    }
}

// --- A provider installed in more than one place (pass 12, item B) ----------

// --- DP-028: what the charter's non-goals promise is disclosed ---------------

#[test]
fn a_file_setting_an_analytics_key_discloses_it_and_one_that_does_not_is_silent() {
    let mut analytics = file(
        "~/.homebrew/brew.env",
        true,
        &["HOMEBREW_CACHE", "HOMEBREW_NO_ANALYTICS"],
    );
    analytics.analytics_keys = ["HOMEBREW_NO_ANALYTICS".to_string()].into_iter().collect();
    let found = configuration_findings(macportsbrew_domain::ProviderConfiguration::Read {
        files: vec![analytics],
    });
    let mut ids: Vec<&str> = found.iter().map(|f| f.0.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "provider_configuration_analytics",
            "provider_configuration_outranks"
        ]
    );
    let (_, severity, detail) = found
        .iter()
        .find(|f| f.0 == "provider_configuration_analytics")
        .expect("the analytics finding");
    assert_eq!(*severity, Severity::Warning);
    assert!(
        detail.contains("HOMEBREW_NO_ANALYTICS") && !detail.contains("HOMEBREW_CACHE"),
        "only the analytics keys are named: {detail}"
    );

    let found = configuration_findings(macportsbrew_domain::ProviderConfiguration::Read {
        files: vec![file("~/.homebrew/brew.env", true, &["HOMEBREW_CACHE"])],
    });
    assert!(
        found
            .iter()
            .all(|f| f.0 != "provider_configuration_analytics"),
        "{found:?}"
    );
}

fn network_finding(state: Option<CapabilityState>) -> Vec<(String, Severity)> {
    let mut p = provider("homebrew", "/opt/homebrew", Architecture::Arm64);
    p.capabilities
        .capabilities
        .remove(&Capability::OfflineMetadata);
    if let Some(state) = state {
        p.capabilities
            .capabilities
            .insert(Capability::OfflineMetadata, state);
    }
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/usr/bin"],
        vec![("homebrew", p)],
    );
    let report = doctor(
        &snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        TODAY,
    );
    report
        .findings
        .iter()
        .filter(|f| f.id.contains("network") || f.id.contains("offline"))
        .map(|f| (f.id.clone(), f.severity))
        .collect()
}

#[test]
fn whether_a_read_may_reach_the_network_is_said_and_unknown_is_never_offline() {
    assert_eq!(
        network_finding(Some(CapabilityState::Available)),
        [("provider_reads_offline".to_string(), Severity::Ok)]
    );
    assert_eq!(
        network_finding(Some(CapabilityState::Unavailable {
            reason: "no cached index".into()
        })),
        [(
            "provider_read_may_reach_network".to_string(),
            Severity::Warning
        )]
    );
    for state in [Some(CapabilityState::Unknown), None] {
        assert_eq!(
            network_finding(state),
            [(
                "provider_network_use_unknown".to_string(),
                Severity::Warning
            )]
        );
    }
}

#[test]
fn invariant_17_the_analytics_disclosure_follows_the_data_not_the_provider_id() {
    // The finding is driven by what the file record carries, which the
    // provider-configuration data decides; the same record under either
    // provider gives the same finding.
    for provider_id in ["homebrew", "macports", "some-third-provider"] {
        let mut analytics = file("~/.config-file", true, &["A_KEY"]);
        analytics.analytics_keys = ["A_KEY".to_string()].into_iter().collect();
        let mut p = configured(
            provider_id,
            macportsbrew_domain::ProviderConfiguration::Read {
                files: vec![analytics],
            },
        );
        p.detection.provider = ProviderId::new(provider_id);
        let snap = snapshot(
            Architecture::Arm64,
            installed(),
            &["/usr/bin"],
            vec![(provider_id, p)],
        );
        let report = doctor(
            &snap,
            &PolicyData::load_embedded().expect("the embedded policy data loads"),
            TODAY,
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.id == "provider_configuration_analytics"
                    && f.provider == Some(ProviderId::new(provider_id))),
            "{provider_id}: {:?}",
            report.findings
        );
    }
}

/// The snapshot, saying where `id` was found.
fn found_at(mut snap: Snapshot, id: &str, installations: Installations) -> Snapshot {
    snap.environment
        .installations
        .insert(ProviderId::new(id), installations);
    snap
}

fn read(prefixes: &[&str]) -> Installations {
    Installations::Read {
        prefixes: prefixes.iter().map(|p| (*p).to_string()).collect(),
    }
}

/// Every installation finding of a provider, as `(id, severity, reason id)`.
fn installation_findings(snap: &Snapshot) -> Vec<(String, Severity, String)> {
    let report = doctor(
        snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        TODAY,
    );
    report
        .findings
        .iter()
        .filter(|f| f.id.starts_with("provider_install"))
        .map(|f| (f.id.clone(), f.severity, f.reason.id.as_str().to_string()))
        .collect()
}

fn one_provider(id: &str, installations: Installations) -> Snapshot {
    found_at(
        snapshot(
            Architecture::Arm64,
            installed(),
            &[],
            vec![(id, provider(id, "/some/prefix", Architecture::Arm64))],
        ),
        id,
        installations,
    )
}

#[test]
fn a_provider_installed_in_two_places_is_a_warning_naming_both_for_either_provider() {
    // Provider-neutral on purpose (INV-17): the same finding for either id,
    // and neither is a special case.
    for id in [HB, MP] {
        let snap = one_provider(id, read(&["/first/prefix", "/second/prefix"]));
        let found = installation_findings(&snap);
        assert_eq!(
            found,
            vec![(
                "provider_installed_more_than_once".to_string(),
                Severity::Warning,
                "doctor.provider_installed_more_than_once".to_string()
            )],
            "{id}"
        );
        let report = doctor(&snap, &PolicyData::load_embedded().expect("policy"), TODAY);
        let finding = report
            .findings
            .iter()
            .find(|f| f.id == "provider_installed_more_than_once")
            .expect("the finding");
        assert_eq!(finding.provider, Some(ProviderId::new(id)));
        let rendered = format!("{:?}", finding.reason);
        assert!(rendered.contains("/first/prefix"), "{rendered}");
        assert!(rendered.contains("/second/prefix"), "{rendered}");
        assert!(rendered.contains('2'), "{rendered}");
        // And which copy this tool runs, so the advice cannot be to remove it.
        assert!(rendered.contains("/some/prefix"), "{rendered}");
    }
}

#[test]
fn a_provider_installed_in_one_place_says_one_and_does_not_warn() {
    let found = installation_findings(&one_provider(HB, read(&["/only/prefix"])));
    assert_eq!(
        found,
        vec![(
            "provider_installed_once".to_string(),
            Severity::Ok,
            "doctor.provider_installed_once".to_string()
        )]
    );
}

#[test]
fn an_unknown_answer_is_never_reported_as_one_place() {
    // A recording (`Unknowable`), a document from before the field (nothing
    // recorded) and a present provider found at a prefix no candidate names
    // (`Read` with nothing in it) all answer "not known".
    let snap_without = snapshot(
        Architecture::Arm64,
        installed(),
        &[],
        vec![(HB, provider(HB, "/some/prefix", Architecture::Arm64))],
    );
    for snap in [
        one_provider(HB, Installations::Unknowable),
        snap_without,
        one_provider(HB, read(&[])),
    ] {
        let found = installation_findings(&snap);
        assert_eq!(
            found,
            vec![(
                "provider_installations_unknown".to_string(),
                Severity::Ok,
                "doctor.provider_installations_unknown".to_string()
            )]
        );
    }
}

#[test]
fn an_absent_provider_has_no_installation_finding() {
    let mut absent = provider(HB, "/some/prefix", Architecture::Arm64);
    absent.detection.presence = ProviderPresence::Absent;
    let snap = found_at(
        snapshot(Architecture::Arm64, installed(), &[], vec![(HB, absent)]),
        HB,
        read(&["/a", "/b"]),
    );
    assert!(installation_findings(&snap).is_empty());
}

// --- INV-10: a build variable that names a provider's prefix ---------------

#[test]
fn a_build_variable_naming_a_providers_prefix_is_a_warning_and_one_that_does_not_is_silent() {
    // 04-migration-and-safety.md, "Cross-provider contamination": environment
    // variables leaking one prefix into builds are a MUST for `doctor`. No test
    // constructed a snapshot with a reported variable until pass 12.
    let mut snap = snapshot(
        Architecture::Arm64,
        installed(),
        &[],
        vec![(HB, provider(HB, "/opt/some-prefix", Architecture::Arm64))],
    );
    snap.environment
        .reported
        .insert("CPPFLAGS".into(), "-I/opt/some-prefix/include".into());
    snap.environment
        .reported
        .insert("LDFLAGS".into(), "-L/usr/lib".into());
    let found = findings(&snap, TODAY, "build_variable_names_prefix");
    assert_eq!(
        found,
        vec![(
            Severity::Warning,
            "doctor.build_variable_names_prefix".to_string()
        )],
        "exactly the variable that names the prefix, and not the other"
    );
}

// --- cross-provider exposure (DP-041) ---------------------------------------

/// Every exposure finding: `(affected, intruder-or-"-", finding id, severity,
/// defence-or-"-")`, in the order `doctor` emitted them.
fn exposures(
    snap: &Snapshot,
    policy: &PolicyData,
) -> Vec<(String, String, String, Severity, String)> {
    doctor(snap, policy, TODAY)
        .findings
        .iter()
        .filter(|f| f.id.starts_with("cross_provider_exposure"))
        .map(|f| {
            let param = |k: &str| {
                f.reason
                    .params
                    .get(k)
                    .cloned()
                    .unwrap_or_else(|| "-".into())
            };
            (
                f.provider
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                param("intruder"),
                f.id.clone(),
                f.severity,
                param("defence"),
            )
        })
        .collect()
}

fn embedded() -> PolicyData {
    PolicyData::load_embedded().expect("the embedded policy data loads")
}

/// The embedded data with `advice` in place of its cross-provider advice file.
fn with_advice(advice: &str) -> PolicyData {
    let mut sources = macportsbrew_policy::data::embedded_sources();
    sources.cross_provider_advice = advice;
    PolicyData::load_from(sources).expect("the synthetic advice loads")
}

fn two_providers(path: &[&str]) -> Snapshot {
    snapshot(
        Architecture::Arm64,
        installed(),
        path,
        vec![
            (HB, provider(HB, "/opt/homebrew", Architecture::Arm64)),
            (MP, provider(MP, "/opt/local", Architecture::Arm64)),
        ],
    )
}

fn row(provider: &str, defence: &str) -> String {
    format!(
        "[[entry]]\nprovider = \"{provider}\"\nbuild_defence = \"{defence}\"\nsource = \"s\"\n\
         read_on = \"2026-09-02\"\n"
    )
}

#[test]
fn the_two_directions_are_not_the_same_advice() {
    // The property the MUST names: with the real data, the pair gets a finding in
    // each direction and the two carry different defences.
    let snap = two_providers(&["/opt/local/bin", "/opt/homebrew/bin", "/usr/bin"]);
    let found = exposures(&snap, &embedded());
    assert_eq!(found.len(), 2, "{found:?}");
    let defence_of = |affected: &str| {
        found
            .iter()
            .find(|f| f.0 == affected)
            .unwrap_or_else(|| panic!("no finding for {affected}"))
            .4
            .clone()
    };
    assert_ne!(defence_of(HB), defence_of(MP));
    assert!(found.iter().all(|f| f.2 == "cross_provider_exposure"));
}

#[test]
fn invariant_17_doctor_exposure_advice_follows_the_data_not_the_provider_id() {
    // Neutrality proof: two ids no code has heard of. Whatever each is told is
    // what the data says about it; exchange the rows and the advice exchanges.
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/p/alpha/bin", "/p/beta/bin", "/usr/bin"],
        vec![
            ("alpha", provider("alpha", "/p/alpha", Architecture::Arm64)),
            ("beta", provider("beta", "/p/beta", Architecture::Arm64)),
        ],
    );
    let one = with_advice(&format!(
        "{}{}",
        row("alpha", "opt_in_trace"),
        row("beta", "default_clean_path")
    ));
    let other = with_advice(&format!(
        "{}{}",
        row("alpha", "default_clean_path"),
        row("beta", "opt_in_trace")
    ));
    let defence = |found: &[(String, String, String, Severity, String)], who: &str| {
        found.iter().find(|f| f.0 == who).unwrap().4.clone()
    };
    let (a, b) = (exposures(&snap, &one), exposures(&snap, &other));
    assert_eq!(defence(&a, "alpha"), "opt_in_trace");
    assert_eq!(defence(&a, "beta"), "default_clean_path");
    assert_eq!(defence(&b, "alpha"), "default_clean_path");
    assert_eq!(defence(&b, "beta"), "opt_in_trace");
}

#[test]
fn a_provider_the_data_does_not_describe_is_unadvised_never_safe() {
    let snap = snapshot(
        Architecture::Arm64,
        installed(),
        &["/p/alpha/bin", "/p/gamma/bin", "/usr/bin"],
        vec![
            ("alpha", provider("alpha", "/p/alpha", Architecture::Arm64)),
            ("gamma", provider("gamma", "/p/gamma", Architecture::Arm64)),
        ],
    );
    let found = exposures(&snap, &with_advice(&row("alpha", "opt_in_trace")));
    let gamma = found
        .iter()
        .find(|f| f.0 == "gamma")
        .expect("a gamma finding");
    assert_eq!(gamma.2, "cross_provider_exposure_unadvised");
    assert_eq!(gamma.3, Severity::Ok);
    assert_eq!(gamma.4, "-", "no defence is claimed for it");
    let alpha = found
        .iter()
        .find(|f| f.0 == "alpha")
        .expect("an alpha finding");
    assert_eq!(alpha.2, "cross_provider_exposure");
}

#[test]
fn it_warns_only_when_the_intruder_is_searched_first() {
    let severity_for = |path: &[&str], affected: &str| {
        exposures(&two_providers(path), &embedded())
            .into_iter()
            .find(|f| f.0 == affected)
            .map(|f| f.3)
    };
    // MacPorts first: Homebrew is the one exposed to it, not the reverse.
    let macports_first = ["/opt/local/bin", "/opt/homebrew/bin", "/usr/bin"];
    assert_eq!(severity_for(&macports_first, HB), Some(Severity::Warning));
    assert_eq!(severity_for(&macports_first, MP), Some(Severity::Ok));
    let homebrew_first = ["/opt/homebrew/bin", "/opt/local/bin", "/usr/bin"];
    assert_eq!(severity_for(&homebrew_first, HB), Some(Severity::Ok));
    assert_eq!(severity_for(&homebrew_first, MP), Some(Severity::Warning));
    // The `sbin` directory counts as the provider's position too.
    let sbin_first = ["/opt/local/sbin", "/opt/homebrew/bin", "/usr/bin"];
    assert_eq!(severity_for(&sbin_first, HB), Some(Severity::Warning));
}

#[test]
fn an_intruder_on_the_path_beside_an_affected_provider_that_is_not_on_it_warns() {
    // The affected provider's own directory is not searched at all, so whatever
    // the intruder ships wins every name they share.
    let found = exposures(&two_providers(&["/opt/local/bin", "/usr/bin"]), &embedded());
    let hb = found.iter().find(|f| f.0 == HB).expect("a finding for HB");
    assert_eq!(hb.3, Severity::Warning);
    let mp = found.iter().find(|f| f.0 == MP).expect("a finding for MP");
    assert_eq!(mp.3, Severity::Ok, "its intruder is not on PATH at all");
}

#[test]
fn an_unknown_path_is_ok_and_never_a_warning() {
    // A recording carries no environment (F-34): nothing can be said about order.
    let found = exposures(&two_providers(&[]), &embedded());
    assert_eq!(found.len(), 2);
    assert_eq!(
        findings(&two_providers(&[]), TODAY, "path_unknown").len(),
        1,
        "the unknown PATH is said once"
    );
    assert!(found.iter().all(|f| f.3 == Severity::Ok), "{found:?}");
}

#[test]
fn fewer_than_two_present_providers_have_no_pair_to_report() {
    let one = snapshot(
        Architecture::Arm64,
        installed(),
        &["/opt/homebrew/bin", "/usr/bin"],
        vec![(HB, provider(HB, "/opt/homebrew", Architecture::Arm64))],
    );
    assert!(exposures(&one, &embedded()).is_empty());

    let mut absent = two_providers(&["/opt/local/bin", "/opt/homebrew/bin", "/usr/bin"]);
    absent
        .providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .detection
        .presence = ProviderPresence::Absent;
    assert!(
        exposures(&absent, &embedded()).is_empty(),
        "an absent provider intrudes on nothing"
    );

    let mut broken = two_providers(&["/opt/local/bin", "/opt/homebrew/bin", "/usr/bin"]);
    broken
        .providers
        .get_mut(&ProviderId::new(MP))
        .unwrap()
        .detection
        .presence = ProviderPresence::Broken {
        reason: "test".into(),
    };
    assert!(
        exposures(&broken, &embedded()).is_empty(),
        "a broken provider is reported as broken, not as an intruder"
    );
}

mod interrupted_run {
    //! The `interrupted_run` finding (pass 08 slice 5, D12, D14): one reason per
    //! state, and a stranded run is a warning, never an error.

    use macportsbrew_application::{interrupted_run_finding, RunFacts, Severity, StrandedRecord};

    fn stranded(record: StrandedRecord) -> RunFacts {
        RunFacts::Stranded {
            run_id: "20261003T120000Z-00000000000000aa".into(),
            snapshot_file: Some("snapshot-20261003T120000Z.json".into()),
            record,
        }
    }

    fn id_and_severity(facts: &RunFacts) -> (String, Severity) {
        let finding = interrupted_run_finding(facts);
        assert_eq!(finding.id, "interrupted_run");
        (finding.reason.id.as_str().to_owned(), finding.severity)
    }

    #[test]
    fn every_state_has_its_own_reason_and_only_the_quiet_ones_are_ok() {
        for (facts, reason, severity) in [
            (
                RunFacts::NotChecked,
                "doctor.interrupted_run_not_checked",
                Severity::Ok,
            ),
            (
                RunFacts::NoMarker,
                "doctor.interrupted_run_none",
                Severity::Ok,
            ),
            (
                RunFacts::InProgress,
                "doctor.interrupted_run_in_progress",
                Severity::Warning,
            ),
            (
                RunFacts::PossiblyInProgress,
                "doctor.interrupted_run_possibly_in_progress",
                Severity::Warning,
            ),
            (
                RunFacts::MarkerUnreadable,
                "doctor.interrupted_run_marker_unreadable",
                Severity::Warning,
            ),
            (
                stranded(StrandedRecord::None),
                "doctor.interrupted_run_started_not_recorded",
                Severity::Warning,
            ),
            (
                stranded(StrandedRecord::Unconfirmed {
                    changed: vec!["ADD_DESTINATION".into()],
                }),
                "doctor.interrupted_run_recorded_not_confirmed",
                Severity::Warning,
            ),
            (
                stranded(StrandedRecord::Unreadable),
                "doctor.interrupted_run_recorded_unreadable",
                Severity::Warning,
            ),
        ] {
            assert_eq!(id_and_severity(&facts), (reason.to_owned(), severity));
        }
    }

    #[test]
    fn a_stranded_run_names_itself_its_snapshot_and_what_it_changed() {
        let finding = interrupted_run_finding(&stranded(StrandedRecord::Unconfirmed {
            changed: vec!["ADD_DESTINATION".into(), "REMOVE_SOURCE".into()],
        }));
        let params = &finding.reason.params;
        assert_eq!(params["run_id"], "20261003T120000Z-00000000000000aa");
        assert_eq!(params["snapshot"], "snapshot-20261003T120000Z.json");
        assert_eq!(params["changed"], "ADD_DESTINATION, REMOVE_SOURCE");
        // A marker that names no snapshot says so rather than inventing one.
        let none = interrupted_run_finding(&RunFacts::Stranded {
            run_id: "20261003T120000Z-00000000000000aa".into(),
            snapshot_file: None,
            record: StrandedRecord::None,
        });
        assert_eq!(none.reason.params["snapshot"], "none");
    }

    #[test]
    fn the_finding_is_placed_by_severity_and_changes_nothing_else() {
        use macportsbrew_application::doctor;
        let policy = macportsbrew_policy::PolicyData::load_embedded().unwrap();
        let snapshot = super::snapshot(super::Architecture::Arm64, super::installed(), &[], vec![]);
        let plain = doctor(&snapshot, &policy, "2026-10-04");
        let with = plain
            .clone()
            .with_run_finding(&stranded(StrandedRecord::None));
        assert_eq!(with.findings.len(), plain.findings.len() + 1);
        assert_eq!(with.findings[0].id, "interrupted_run");
        let rest: Vec<_> = with
            .findings
            .iter()
            .filter(|f| f.id != "interrupted_run")
            .cloned()
            .collect();
        let mut original = plain.findings.clone();
        original.retain(|f| f.id != "interrupted_run");
        assert_eq!(rest, original);
    }
}
