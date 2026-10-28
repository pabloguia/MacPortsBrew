// SPDX-License-Identifier: Apache-2.0

//! `doctor`: the cheap structural checks (`04-migration-and-safety.md`,
//! "Cross-provider contamination"; `05-environment-and-primary-provider.md`).
//! Report only; never repair.

use macportsbrew_domain::{
    Architecture, CommandLineTools, PathFindingKind, ProviderId, ProviderPresence, Reason,
    Snapshot, Translation,
};
use macportsbrew_policy::PolicyData;
use serde::{Deserialize, Serialize};

/// How bad a finding is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    /// Nothing wrong; recorded so the user sees what was checked.
    Ok,
    /// Worth knowing; nothing broken.
    Warning,
    /// Something is wrong or dangerous.
    Error,
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorFinding {
    /// Stable identifier.
    pub id: String,
    /// Severity.
    pub severity: Severity,
    /// Renderable reason.
    pub reason: Reason,
    /// Provider concerned, when one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    /// Output schema version.
    pub schema_version: u32,
    /// Tool version.
    pub tool_version: String,
    /// Policy version.
    pub policy_version: String,
    /// Findings, sorted by severity descending then id.
    pub findings: Vec<DoctorFinding>,
}

impl DoctorReport {
    /// Highest severity present.
    pub fn worst(&self) -> Severity {
        self.findings
            .iter()
            .map(|f| f.severity)
            .max()
            .unwrap_or(Severity::Ok)
    }
}

/// What the shell found when it looked for an interrupted run (pass 08
/// slice 5, D12, D14). `doctor` itself stays a pure function of a snapshot and
/// policy data; the state directory is read by the shell and handed in, and the
/// check is added with [`DoctorReport::with_run_finding`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunFacts {
    /// Not looked at: a recording, a snapshot file, or a user that could not be
    /// established. Said, never read as "none".
    NotChecked,
    /// No marker is there.
    NoMarker,
    /// A marker is there and the run lock is held: a run is in progress.
    InProgress,
    /// A marker is there and the lock could not be tested.
    PossiblyInProgress,
    /// The marker could not be read.
    MarkerUnreadable,
    /// A marker is there and nobody holds the lock: a run was interrupted.
    Stranded {
        /// The run the marker names.
        run_id: String,
        /// The snapshot file the marker names, when it names one.
        snapshot_file: Option<String>,
        /// What is known of the run's record.
        record: StrandedRecord,
    },
}

/// What is known of the record of a stranded run (three states, one text for
/// each in `doctor`, `history`, `history show` and `history acknowledge`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StrandedRecord {
    /// There is none: the run began and left no record.
    None,
    /// It exists and reads, and the marker still names it.
    Unconfirmed {
        /// The mutating steps the run dispatched, by identifier.
        changed: Vec<String>,
    },
    /// Something is at that id that cannot be read as a record.
    Unreadable,
}

/// The `interrupted_run` finding. A stranded run is a **warning**, not an error:
/// it blocks the next command that changes packages (exit 4), but nothing about
/// the machine's providers is unhealthy, and `doctor`'s error exit is for those.
#[must_use]
pub fn interrupted_run_finding(facts: &RunFacts) -> DoctorFinding {
    let (severity, reason) = match facts {
        RunFacts::NotChecked => (
            Severity::Ok,
            Reason::new("doctor.interrupted_run_not_checked"),
        ),
        RunFacts::NoMarker => (Severity::Ok, Reason::new("doctor.interrupted_run_none")),
        RunFacts::InProgress => (
            Severity::Warning,
            Reason::new("doctor.interrupted_run_in_progress"),
        ),
        RunFacts::PossiblyInProgress => (
            Severity::Warning,
            Reason::new("doctor.interrupted_run_possibly_in_progress"),
        ),
        RunFacts::MarkerUnreadable => (
            Severity::Warning,
            Reason::new("doctor.interrupted_run_marker_unreadable"),
        ),
        RunFacts::Stranded {
            run_id,
            snapshot_file,
            record,
        } => {
            let id = match record {
                StrandedRecord::None => "doctor.interrupted_run_started_not_recorded",
                StrandedRecord::Unconfirmed { .. } => {
                    "doctor.interrupted_run_recorded_not_confirmed"
                }
                StrandedRecord::Unreadable => "doctor.interrupted_run_recorded_unreadable",
            };
            let mut reason = Reason::new(id)
                .with("run_id", run_id)
                .with("snapshot", &snapshot_file.as_deref().unwrap_or("none"));
            if let StrandedRecord::Unconfirmed { changed } = record {
                reason = reason.with("changed", &changed.join(", "));
            }
            (Severity::Warning, reason)
        }
    };
    DoctorFinding {
        id: "interrupted_run".into(),
        severity,
        reason,
        provider: None,
    }
}

impl DoctorReport {
    /// This report with the interrupted-run finding added, in its place in the
    /// order (severity descending, then id).
    #[must_use]
    pub fn with_run_finding(mut self, facts: &RunFacts) -> Self {
        self.findings.push(interrupted_run_finding(facts));
        self.findings
            .sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
        self
    }
}

/// Runs the checks.
pub fn doctor(snapshot: &Snapshot, policy: &PolicyData, today: &str) -> DoctorReport {
    let mut findings = Vec::new();
    let mut add = |id: &str, severity, reason, provider: Option<ProviderId>| {
        findings.push(DoctorFinding {
            id: id.into(),
            severity,
            reason,
            provider,
        });
    };

    // Providers: presence, health, architecture, support.
    let mut prefixes: Vec<(ProviderId, String)> = Vec::new();
    for (id, p) in &snapshot.providers {
        match &p.detection.presence {
            ProviderPresence::Present => {
                add(
                    "provider_present",
                    Severity::Ok,
                    Reason::new("doctor.provider_present")
                        .with("provider", id)
                        .with(
                            "version",
                            &p.detection
                                .version
                                .as_ref()
                                .map_or("?".into(), ToString::to_string),
                        ),
                    Some(id.clone()),
                );
                for (finding, severity, reason) in configuration_findings(id, &p.configuration) {
                    add(finding, severity, reason, Some(id.clone()));
                }
                let (finding, severity, reason) = network_finding(id, &p.capabilities);
                add(finding, severity, reason, Some(id.clone()));
                let running = p.detection.prefix.as_ref().map_or_else(
                    || "?".to_string(),
                    |prefix| prefix.to_string_lossy().into_owned(),
                );
                let (finding, severity, reason) =
                    installation_finding(id, snapshot.installations_of(id), &running);
                add(finding, severity, reason, Some(id.clone()));
                if let Some(prefix) = &p.detection.prefix {
                    let text = prefix.to_string_lossy().into_owned();
                    // F-48. `planner::command` refuses to build a step whose
                    // program is not absolute, because spawning a program resolves a
                    // string containing a slash against the working directory.
                    // A prefix under the user's home arrives here as `~/...`,
                    // redacted at capture so a home-installed provider can be
                    // captured at all, and `prefix_shape_error` passes it on
                    // purpose naming `planner::command` as what refuses it. So
                    // `analyze` is right and any step that would run this
                    // provider cannot be built - and until this finding existed,
                    // the user met that as a refusal telling them their input was
                    // at fault when nothing was.
                    //
                    // The *same* predicate as the planner, deliberately. An
                    // earlier version tested `starts_with('~')`, which agrees
                    // only because `is_safe_captured_path` admits nothing else
                    // non-absolute; widen that and doctor would have gone quiet
                    // about a prefix the planner still refuses, returning the
                    // user to the surprise this removes.
                    //
                    // Two questions, not one, and the union of them is the
                    // answer (F-49). `prefix_redacted` is what capture recorded
                    // about itself, and it covers the other shape redaction
                    // produces: a login name replaced as a whole path component
                    // leaves `/opt/USER/brew`, which *is* absolute, so nothing
                    // in the string betrays it. The shape test stays because the
                    // field is `#[serde(default)]` - every snapshot and recording
                    // written before it reads `false`, meaning "not known to have
                    // been redacted", and for those the `~` shape is still all
                    // there is to go on.
                    //
                    // A `USER`-shaped prefix in one of those older documents is
                    // therefore still missed. That is not a gap in the predicate
                    // but an absence in the file, and guessing harder is what
                    // made the first version of this check wrong.
                    //
                    // A structural fact about the machine, which is what
                    // `doctor` reports: not "move your install", which would be
                    // a recommendation and belongs in the policy engine.
                    // `Warning` rather than `Error`: no answer is wrong here,
                    // one operation is unavailable, and an `Error` would make
                    // `doctor` exit 5 on a machine that answers correctly.
                    if p.detection.prefix_redacted || !std::path::Path::new(&text).is_absolute() {
                        add(
                            "provider_prefix_is_not_absolute",
                            Severity::Warning,
                            Reason::new("doctor.provider_prefix_is_not_absolute")
                                .with("provider", id)
                                .with("prefix", &text),
                            Some(id.clone()),
                        );
                    }
                    prefixes.push((id.clone(), text));
                }
                if let Some(arch) = p.detection.architecture {
                    if arch != snapshot.machine.architecture {
                        add(
                            "provider_architecture_mismatch",
                            Severity::Warning,
                            Reason::new("doctor.provider_architecture_mismatch")
                                .with("provider", id)
                                .with("provider_arch", &arch)
                                .with("machine_arch", &snapshot.machine.architecture),
                            Some(id.clone()),
                        );
                    }
                }
                let support = policy.support_state(id, &snapshot.machine);
                if let Some(end) = &support.end_date {
                    add(
                        "provider_support_ending",
                        Severity::Warning,
                        Reason::new("doctor.provider_support_ending")
                            .with("provider", id)
                            .with("endDate", end)
                            .with("source", &support.fact.source)
                            .with("readOn", &support.fact.read_on),
                        Some(id.clone()),
                    );
                }
                if support.tier != macportsbrew_domain::SupportTier::Supported {
                    add(
                        "provider_support_degraded",
                        Severity::Warning,
                        Reason::new("doctor.provider_support")
                            .with("provider", id)
                            // The stable identifier, not a `Debug` rendering. `Messages::render`
                            // turns it into the catalogue's wording; building the
                            // display text here would be a display string in the
                            // application layer (F-20).
                            .with("tier", &macportsbrew_policy::labels::tier(support.tier))
                            .with("source", &support.fact.source)
                            .with("readOn", &support.fact.read_on),
                        Some(id.clone()),
                    );
                }
            }
            ProviderPresence::Absent => add(
                "provider_absent",
                Severity::Ok,
                Reason::new("doctor.provider_absent").with("provider", id),
                Some(id.clone()),
            ),
            ProviderPresence::Broken { reason } => add(
                "provider_broken",
                Severity::Error,
                Reason::new("doctor.provider_broken")
                    .with("provider", id)
                    .with("reason", reason),
                Some(id.clone()),
            ),
        }
    }

    // PATH order between provider prefixes: reported, never changed.
    let path = &snapshot.environment.path;
    let position = |prefix: &str| {
        path.iter()
            .position(|e| e == &format!("{prefix}/bin") || e == &format!("{prefix}/sbin"))
    };
    // F-34: an empty `PATH` here means the capture could not record one, not
    // that the user's `PATH` is empty. A `Recording` carries no environment at
    // all, so a replayed `doctor` used to emit "<provider>'s prefix is not on
    // PATH" for every present provider - a confident warning invented from
    // absent input, which a user would act on by editing a `PATH` that is
    // already correct. Both checks below read the same missing thing, so both
    // are skipped and the absence is reported once.
    if path.is_empty() {
        add(
            "path_unknown",
            Severity::Warning,
            Reason::new("doctor.path_unknown"),
            None,
        );
    } else {
        let mut ordered: Vec<(usize, &ProviderId)> = prefixes
            .iter()
            .filter_map(|(id, prefix)| position(prefix).map(|pos| (pos, id)))
            .collect();
        ordered.sort();
        if ordered.len() >= 2 {
            add(
                "path_order",
                Severity::Ok,
                Reason::new("doctor.path_order").with(
                    "order",
                    &ordered
                        .iter()
                        .map(|(_, id)| id.to_string())
                        .collect::<Vec<_>>()
                        .join(" > "),
                ),
                None,
            );
        }
        for (id, prefix) in &prefixes {
            if position(prefix).is_none() {
                add(
                    "prefix_not_on_path",
                    Severity::Warning,
                    Reason::new("doctor.prefix_not_on_path")
                        .with("provider", id)
                        .with("prefix", prefix),
                    Some(id.clone()),
                );
            }
        }
    }
    // Cross-provider exposure (DP-041): for every ordered pair of present
    // providers, whether the one earlier on `PATH` can reach into the other's
    // builds and tools, and what the *affected* provider's own tooling does about
    // it. Which direction is the riskier one is the data's answer
    // (`cross-provider-advice.toml`), never this function's.
    for (affected, _) in &prefixes {
        for (intruder, intruder_prefix) in &prefixes {
            if affected == intruder {
                continue;
            }
            let (finding, severity, reason) = cross_provider_exposure(
                affected,
                intruder,
                intruder_prefix,
                policy,
                path_position(&prefixes, affected, &position),
                path_position(&prefixes, intruder, &position),
            );
            add(finding, severity, reason, Some(affected.clone()));
        }
    }
    // What was wrong with the `PATH` entries themselves, audited at capture
    // and carried in the snapshot (`adapters::system::path_audit`). The audit
    // is provider-neutral and deliberately stops at "this entry is writable";
    // the half of the threat-model item that needs an opinion — does a
    // writable directory come *before* the system ones, so that it decides
    // what `git` means? — is a comparison of positions, and it is answered
    // here (`04-migration-and-safety.md`, "Threat model": executable
    // shadowing).
    // Compared with trailing slashes trimmed. `/usr/bin/` is an ordinary entry
    // that works, and matching it by string equality meant one character
    // bypassed every escalation below *and* fired the no-system-directory
    // error while all four were present and searched.
    let system = |e: &str| {
        let e = e.trim_end_matches('/');
        macportsbrew_domain::SYSTEM_PATH
            .iter()
            .any(|s| s.trim_end_matches('/') == e)
    };
    let first_system = path.iter().position(|e| system(e));
    // A `PATH` with no system directory on it at all is a finding in its own
    // right, and it used to be the quietest kind of gap: `precedes_system` is
    // `false` for every entry when there is nothing to precede, so every
    // escalation below silently degraded to a warning on exactly the machine
    // whose search path is most unusual.
    if first_system.is_none() && !path.is_empty() {
        add(
            "path_has_no_system_directory",
            Severity::Error,
            Reason::new("doctor.path_has_no_system_directory"),
            None,
        );
    }
    // One entry yields one writable finding. `/usr/local/bin` is both
    // group-writable and owner-writable on a stock Intel Homebrew, and
    // reporting it twice for one directory with one remedy is how a count
    // stops meaning anything. The strongest writer decides which is reported.
    let strongest = |index: usize| {
        [
            PathFindingKind::WorldWritable,
            PathFindingKind::GroupWritable,
            PathFindingKind::OwnerWritable,
        ]
        .into_iter()
        .find(|kind| {
            snapshot
                .environment
                .path_findings
                .iter()
                .any(|f| f.index == index && f.kind == *kind)
        })
    };
    for finding in &snapshot.environment.path_findings {
        let precedes_system = first_system.is_some_and(|system| finding.index < system);
        let writable = matches!(
            finding.kind,
            PathFindingKind::WorldWritable
                | PathFindingKind::GroupWritable
                | PathFindingKind::OwnerWritable
        );
        if writable && strongest(finding.index) != Some(finding.kind) {
            continue;
        }
        // The message identifiers are written out as literals so that
        // `scripts/check-i18n-parity.sh`, which greps for `Reason::new("…")`,
        // sees every one of them.
        let (id, severity, reason) = match finding.kind {
            // Anyone on the machine may drop a binary here, and the shell
            // finds it before the real one. That is the shadowing attack, not
            // a tidiness problem.
            // `ERROR` is reserved for a search path somebody *other than this
            // user* can decide the contents of, and for one whose meaning
            // changes with the working directory. Both are broken in the sense
            // this project reserves the word for: they produce wrong answers.
            //
            // A directory this user owns, ahead of the system ones, is not.
            // It is how every developer machine is arranged and how Homebrew
            // installs itself - `/usr/local/bin` is `admin`-writable by the
            // installer's design, and MacPortsBrew's own analysis needs that
            // prefix on the `PATH` it warns you about not having it on. Making
            // it an error meant `doctor` exited 5 on a stock install and on
            // every machine with a `~/bin`, and an error everybody has is an
            // error nobody reads. It is reported - which is the finding, since
            // it produced nothing at all before - as a warning.
            PathFindingKind::WorldWritable if precedes_system => (
                "path_world_writable_precedes_system",
                Severity::Error,
                Reason::new("doctor.path_world_writable_precedes_system"),
            ),
            // An empty entry is the working directory, so what runs depends on
            // where the user is standing and any repository they clone can
            // carry a `git`. Nobody arranges this deliberately.
            PathFindingKind::Relative if precedes_system => (
                "path_relative_precedes_system",
                Severity::Error,
                Reason::new("doctor.path_relative_precedes_system"),
            ),
            PathFindingKind::WorldWritable => (
                "path_world_writable",
                Severity::Warning,
                Reason::new("doctor.path_world_writable"),
            ),
            PathFindingKind::GroupWritable => (
                "path_group_writable",
                Severity::Warning,
                Reason::new("doctor.path_group_writable"),
            ),
            PathFindingKind::OwnerWritable => (
                "path_owner_writable",
                Severity::Warning,
                Reason::new("doctor.path_owner_writable"),
            ),
            PathFindingKind::Missing => (
                "path_missing",
                Severity::Warning,
                Reason::new("doctor.path_missing"),
            ),
            PathFindingKind::Relative => (
                "path_relative",
                Severity::Warning,
                Reason::new("doctor.path_relative"),
            ),
            // A snapshot from a newer build naming a kind this one does not
            // know. Reported rather than dropped: the reader can see that
            // something was found and that this build cannot name it.
            PathFindingKind::Unknown => (
                "path_finding_unknown",
                Severity::Warning,
                Reason::new("doctor.path_finding_unknown"),
            ),
            PathFindingKind::Duplicate => (
                "path_duplicate",
                Severity::Warning,
                Reason::new("doctor.path_duplicate"),
            ),
        };
        add(id, severity, reason.with("entry", &finding.entry), None);
    }

    // Build-flag variables leaking one prefix into the other (INV-10).
    for (var, value) in &snapshot.environment.reported {
        for (id, prefix) in &prefixes {
            if value.contains(prefix.as_str()) {
                add(
                    "build_variable_names_prefix",
                    Severity::Warning,
                    Reason::new("doctor.build_variable_names_prefix")
                        .with("variable", var)
                        .with("provider", id),
                    Some(id.clone()),
                );
            }
        }
    }

    // A provider installed in two places - on Apple Silicon, a second copy
    // under the old Intel prefix - is reported per provider above, from what
    // capture recorded (`installation_finding`); a snapshot keyed by provider
    // id carries one detected prefix, so the second one is the record's, not
    // the detection's.
    if snapshot.machine.architecture == Architecture::Arm64
        && snapshot.machine.translation == Translation::Translated
    {
        add(
            "running_translated",
            Severity::Warning,
            Reason::new("doctor.running_translated"),
            None,
        );
    }
    match &snapshot.machine.command_line_tools {
        CommandLineTools::Installed { .. } => add(
            "command_line_tools",
            Severity::Ok,
            Reason::new("doctor.command_line_tools_present"),
            None,
        ),
        CommandLineTools::Missing => add(
            "command_line_tools",
            Severity::Warning,
            Reason::new("doctor.command_line_tools_missing"),
            None,
        ),
        CommandLineTools::Unknown => add(
            "command_line_tools",
            Severity::Warning,
            Reason::new("doctor.command_line_tools_unknown"),
            None,
        ),
    }
    if policy.is_stale(today) {
        add(
            "policy_stale",
            Severity::Warning,
            Reason::new("policy.stale_notice").with("readOn", &policy.newest_read_on()),
            None,
        );
    }

    findings.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.id.cmp(&b.id)));
    DoctorReport {
        schema_version: crate::ANALYSIS_SCHEMA_VERSION,
        tool_version: crate::TOOL_VERSION.to_string(),
        policy_version: policy.version().to_string(),
        findings,
    }
}

/// What capture established about how many places a provider is installed in.
///
/// Generic on purpose: the provider is a name carried through, the places are
/// whatever the snapshot recorded, and no path or provider is known here
/// (INV-17, INV-20). Two or more is the finding `04-migration-and-safety.md`
/// asks for ("Cross-provider contamination"): a second working installation
/// is the one a detection that stops at the first hit never mentions. An
/// unknown is never "one" (`Installations`): a capture made from a recording, a
/// document from before the field, and a provider found at a prefix none of
/// the candidates names all say so, and say it at `Ok` because nothing is
/// known to be wrong.
fn installation_finding(
    id: &ProviderId,
    installations: &macportsbrew_domain::Installations,
    running: &str,
) -> (&'static str, Severity, Reason) {
    match installations.count() {
        Some(count) if count > 1 => {
            let places = match installations {
                macportsbrew_domain::Installations::Read { prefixes } => prefixes.join(", "),
                _ => String::new(),
            };
            (
                "provider_installed_more_than_once",
                Severity::Warning,
                Reason::new("doctor.provider_installed_more_than_once")
                    .with("provider", id)
                    .with("count", &count.to_string())
                    .with("prefixes", &places)
                    .with("running", &running),
            )
        }
        Some(1) => (
            "provider_installed_once",
            Severity::Ok,
            Reason::new("doctor.provider_installed_once").with("provider", id),
        ),
        _ => (
            "provider_installations_unknown",
            Severity::Ok,
            Reason::new("doctor.provider_installations_unknown").with("provider", id),
        ),
    }
}

/// A present provider's position on `PATH`, `None` when `PATH` is unknown or the
/// provider's directories are not on it.
fn path_position(
    prefixes: &[(ProviderId, String)],
    id: &ProviderId,
    position: &dyn Fn(&str) -> Option<usize>,
) -> Option<usize> {
    prefixes
        .iter()
        .find(|(p, _)| p == id)
        .and_then(|(_, prefix)| position(prefix))
}

/// One ordered pair of present providers: `intruder`'s executables and
/// libraries against `affected`'s builds and tools (DP-041).
///
/// A pure function of two ids, their positions on a `PATH` and the policy
/// data; it names no provider. The finding is `Warning` only when `PATH` is
/// known and the intruder's directory is searched before the affected
/// provider's, or the affected provider's is not on it at all. Everything else
/// is `Ok`, an unknown `PATH` included: an unknown is never a warning, and never
/// a clean bill either. The sentence is chosen by the affected provider's
/// recorded build defence; a provider the data says nothing about gets the
/// `_unadvised` finding, which makes no direction-specific claim.
fn cross_provider_exposure(
    affected: &ProviderId,
    intruder: &ProviderId,
    intruder_prefix: &str,
    policy: &PolicyData,
    affected_at: Option<usize>,
    intruder_at: Option<usize>,
) -> (&'static str, Severity, Reason) {
    let intruder_first = match (intruder_at, affected_at) {
        (Some(i), Some(a)) => i < a,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let severity = if intruder_first {
        Severity::Warning
    } else {
        Severity::Ok
    };
    match policy.build_defence_of(affected) {
        Some(defence) => (
            "cross_provider_exposure",
            severity,
            Reason::new("doctor.cross_provider_exposure")
                .with("affected", affected)
                .with("intruder", intruder)
                .with("intruder_prefix", &intruder_prefix)
                .with("defence", &defence.identifier()),
        ),
        None => (
            "cross_provider_exposure_unadvised",
            Severity::Ok,
            Reason::new("doctor.cross_provider_exposure_unadvised")
                .with("affected", affected)
                .with("intruder", intruder),
        ),
    }
}

/// What `doctor` says about a present provider's own configuration files
/// (DP-027 (c): detect and report on reads).
///
/// A file that sets a key, or that exists and could not be read, is a
/// warning naming the file and the keys - never their values, which capture
/// does not keep. Nothing found is said, so a clean machine reads as checked
/// rather than as silence. An input that carries no record of the files - a
/// replayed recording, a snapshot from before the field - is a warning that it
/// is unknown, never the "nothing found" a reader would take it for.
fn configuration_findings(
    id: &ProviderId,
    configuration: &macportsbrew_domain::ProviderConfiguration,
) -> Vec<(&'static str, Severity, Reason)> {
    use macportsbrew_domain::ProviderConfiguration;
    let ProviderConfiguration::Read { files } = configuration else {
        return vec![(
            "provider_configuration_unknown",
            Severity::Warning,
            Reason::new("doctor.provider_configuration_unknown").with("provider", id),
        )];
    };
    let mut out = Vec::new();
    for file in files {
        if !file.readable {
            out.push((
                "provider_configuration_unreadable",
                Severity::Warning,
                Reason::new("doctor.provider_configuration_unreadable")
                    .with("provider", id)
                    .with("file", &file.path),
            ));
        } else if !file.keys.is_empty() {
            let keys: Vec<&str> = file.keys.iter().map(String::as_str).collect();
            out.push((
                "provider_configuration_outranks",
                Severity::Warning,
                Reason::new("doctor.provider_configuration_outranks")
                    .with("provider", id)
                    .with("file", &file.path)
                    .with("keys", &keys.join(", ")),
            ));
            // DP-028: the charter promises that a provider configuration that
            // may enable the provider's own analytics is disclosed. The value
            // is never kept, so this says the key is set, not which way.
            if !file.analytics_keys.is_empty() {
                let analytics: Vec<&str> = file.analytics_keys.iter().map(String::as_str).collect();
                out.push((
                    "provider_configuration_analytics",
                    Severity::Warning,
                    Reason::new("doctor.provider_configuration_analytics")
                        .with("provider", id)
                        .with("file", &file.path)
                        .with("keys", &analytics.join(", ")),
                ));
            }
        }
    }
    if out.is_empty() {
        out.push((
            "provider_configuration_none",
            Severity::Ok,
            Reason::new("doctor.provider_configuration_none").with("provider", id),
        ));
    }
    out
}

/// Whether a read this tool asks `id` for may reach the network on the
/// provider's own account (DP-028: "MacPortsBrew says when it can").
///
/// MacPortsBrew makes no network call of its own; the question is the
/// provider's. Its local metadata index answers it: present, a read is served
/// from it; absent, the provider may download it first; not known from this
/// input - a recording, a snapshot without it - is said as unknown, never as
/// "offline". A configuration that turns fetching back on is the separate
/// `provider_configuration_outranks` finding.
fn network_finding(
    id: &ProviderId,
    capabilities: &macportsbrew_domain::ProviderCapabilities,
) -> (&'static str, Severity, Reason) {
    use macportsbrew_domain::{Capability, CapabilityState};
    match capabilities.capabilities.get(&Capability::OfflineMetadata) {
        Some(CapabilityState::Available) => (
            "provider_reads_offline",
            Severity::Ok,
            Reason::new("doctor.provider_reads_offline").with("provider", id),
        ),
        Some(CapabilityState::Unavailable { .. }) => (
            "provider_read_may_reach_network",
            Severity::Warning,
            Reason::new("doctor.provider_read_may_reach_network").with("provider", id),
        ),
        Some(CapabilityState::Unknown) | None => (
            "provider_network_use_unknown",
            Severity::Warning,
            Reason::new("doctor.provider_network_use_unknown").with("provider", id),
        ),
    }
}
