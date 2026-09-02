// SPDX-License-Identifier: Apache-2.0

//! `doctor`: the cheap structural checks (`04-migration-and-safety.md`,
//! "Cross-provider contamination"; `05-environment-and-primary-provider.md`).
//! Report only; never repair.

use macportsbrew_domain::{
    Architecture, CommandLineTools, ProviderId, ProviderPresence, Reason, Snapshot, Translation,
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
                            .with("tier", &format!("{:?}", support.tier))
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
    let mut seen = std::collections::BTreeSet::new();
    for entry in path {
        if !seen.insert(entry.clone()) {
            add(
                "path_duplicate",
                Severity::Warning,
                Reason::new("doctor.path_duplicate").with("entry", entry),
                None,
            );
        }
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
