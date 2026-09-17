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
    Architecture, CommandLineTools, EnvironmentSnapshot, Machine, MacosVersion, Provenance,
    ProviderCapabilities, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, Snapshot, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
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
