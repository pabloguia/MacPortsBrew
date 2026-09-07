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
                if let Some(prefix) = &p.detection.prefix {
                    prefixes.push((id.clone(), prefix.to_string_lossy().into_owned()));
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

    // Two prefixes for one provider id cannot appear in a snapshot keyed by
    // id; a second installation under the other default prefix on Apple
    // Silicon is detected by the adapter's detect_prefix order and reported
    // through `provider_architecture_mismatch` when it is the one found.
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
            Reason::new("policy.stale_notice")
                .with("readOn", &policy.newest_read_on())
                .with("url", &"https://github.com/macportsbrew/macportsbrew"),
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
