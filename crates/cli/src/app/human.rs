// SPDX-License-Identifier: Apache-2.0

//! Human renderings. Plain text, stable when not on a terminal, every
//! label through the catalogue, every column measured in display width.

use super::context::Context;
use crate::render::table::Table;
use macportsbrew_application::{Analysis, DoctorReport, Explanation, PackageReport, Severity};
use macportsbrew_domain::{
    ArtifactAvailability, MigrationPlan, Outcome, Ownership, PackageClass, ProviderPresence,
    SourceBuildReason, StepKind, SupportTier, Translation, VerificationCheck,
};
use std::collections::BTreeMap;
use std::fmt::Write;

fn label(ctx: &Context, id: &str) -> String {
    ctx.messages.render_id(id)
}

fn params(pairs: &[(&str, String)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), v.clone()))
        .collect()
}

fn outcome_text(ctx: &Context, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Keep { provider } => ctx.messages.render(
            "outcome.keep",
            &params(&[("provider", provider.to_string())]),
        ),
        Outcome::MigrateTo { provider } => ctx.messages.render(
            "outcome.migrate_to",
            &params(&[("provider", provider.to_string())]),
        ),
        Outcome::KeepBoth { primary } => ctx.messages.render(
            "outcome.keep_both",
            &params(&[("primary", primary.to_string())]),
        ),
        Outcome::UseRuntimeManager => label(ctx, "outcome.use_runtime_manager"),
        Outcome::ManualReview => label(ctx, "outcome.manual_review"),
        Outcome::NoAction => label(ctx, "outcome.no_action"),
    }
}

fn availability_text(ctx: &Context, a: &ArtifactAvailability) -> String {
    match a {
        ArtifactAvailability::BinaryAvailable => label(ctx, "availability.binary_available"),
        ArtifactAvailability::SourceBuildRequired { reason } => {
            let why = match reason {
                SourceBuildReason::NoArtifactForThisPlatform => {
                    "source_build.no_artifact_for_this_platform"
                }
                SourceBuildReason::NonDefaultOptions => "source_build.non_default_options",
                SourceBuildReason::LicenceRestricted => "source_build.licence_restricted",
                SourceBuildReason::ForcedByConfiguration => "source_build.forced_by_configuration",
            };
            ctx.messages.render(
                "availability.source_build_required",
                &params(&[("reason", label(ctx, why))]),
            )
        }
        ArtifactAvailability::Unavailable => label(ctx, "availability.unavailable"),
        ArtifactAvailability::UnsupportedPlatform => {
            label(ctx, "availability.unsupported_platform")
        }
        ArtifactAvailability::Deprecated => label(ctx, "availability.deprecated"),
        ArtifactAvailability::Disabled => label(ctx, "availability.disabled"),
        ArtifactAvailability::Unknown => label(ctx, "availability.unknown"),
    }
}

fn ownership_text(ctx: &Context, o: Ownership) -> String {
    match o {
        Ownership::TopLevel => label(ctx, "ownership.top_level"),
        Ownership::Dependency => label(ctx, "ownership.dependency"),
        Ownership::Orphan => label(ctx, "ownership.orphan"),
        Ownership::Unknown => label(ctx, "ownership.unknown"),
    }
}

fn class_text(ctx: &Context, c: PackageClass) -> String {
    match c {
        PackageClass::Simple => label(ctx, "class.simple"),
        PackageClass::Stateful => label(ctx, "class.stateful"),
        PackageClass::Service => label(ctx, "class.service"),
        PackageClass::Runtime => label(ctx, "class.runtime"),
        PackageClass::GuiApp => label(ctx, "class.gui_app"),
        PackageClass::ThirdPartySourced => label(ctx, "class.third_party_sourced"),
        PackageClass::ProviderInternal => label(ctx, "class.provider_internal"),
    }
}

/// `status`.
pub fn status(ctx: &Context, analysis: &Analysis) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", label(ctx, "status.machine_heading"));
    let m = &analysis.machine;
    let _ = writeln!(
        out,
        "  {}",
        ctx.messages.render(
            "status.machine_line",
            &params(&[
                ("macos", m.macos.to_string()),
                ("arch", m.architecture.to_string()),
                ("translation", label(ctx, translation_key(m.translation)),),
            ])
        )
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "status.providers_heading"));
    let mut table = Table::new([
        label(ctx, "table.provider"),
        label(ctx, "table.presence"),
        label(ctx, "table.version"),
        label(ctx, "table.prefix"),
        label(ctx, "table.support"),
        label(ctx, "table.packages"),
    ]);
    for (id, p) in &analysis.providers {
        let presence = match &p.detection.presence {
            ProviderPresence::Present => label(ctx, "presence.present"),
            ProviderPresence::Absent => label(ctx, "presence.absent"),
            // F-38: `presence.broken` used to read "Broken ({reason})" while
            // this site called `label`, which renders without parameters - so
            // the placeholder reached the user verbatim. Filling it is not the
            // fix: `reason` holds a message *identifier* whose catalogue text
            // is a full remediation sentence carrying its own `{detail}`, so
            // rendering it here put a paragraph in a table cell and leaked
            // `{detail}` instead. The cell now says "Broken", like its
            // `Present`/`Absent` siblings, and `doctor` carries the diagnosis.
            ProviderPresence::Broken { .. } => label(ctx, "presence.broken"),
        };
        table.push([
            id.to_string(),
            presence,
            p.detection
                .version
                .as_ref()
                .map_or(String::from("-"), ToString::to_string),
            p.detection
                .prefix
                .as_ref()
                .map_or(String::from("-"), |x| x.display().to_string()),
            // Not `{:?}`. `locales/en.toml` has said "Not supported" for
            // `Unsupported` since it was written, and the column printed
            // `unsupported` - a Rust identifier, lower-cased, that no catalogue
            // sees and no pseudolocale marks (F-20).
            label(
                ctx,
                match p.support.tier {
                    SupportTier::Supported => "support.supported",
                    SupportTier::Degraded => "support.degraded",
                    SupportTier::Unsupported => "support.unsupported",
                    SupportTier::Unknown => "support.unknown",
                },
            ),
            // And not a sentence built here. `06-cli-and-ux.md` makes composing
            // user-facing text in code a **MUST NOT**: the plural rule for
            // "package" is the catalogue's business, and "(n top-level)" is not
            // a phrase every language puts in parentheses after the count.
            ctx.messages.render(
                "count.packages_with_top_level",
                &params(&[
                    ("count", p.package_count.to_string()),
                    ("topLevel", p.top_level_count.to_string()),
                ]),
            ),
        ]);
    }
    out.push_str(&table.render(None));
    let _ = writeln!(out);
    let s = &analysis.summary;
    let _ = writeln!(
        out,
        "{}",
        ctx.messages.render(
            "status.summary_line",
            &params(&[
                ("topLevel", s.top_level_units.to_string()),
                ("manualReview", s.manual_review.to_string()),
                ("policyVersion", analysis.policy_version.clone()),
            ])
        )
    );
    if s.ownership_unknown > 0 {
        let _ = writeln!(
            out,
            "{}",
            ctx.messages.render(
                "status.ownership_unknown",
                &params(&[("count", s.ownership_unknown.to_string())])
            )
        );
    }
    if analysis.policy_stale {
        let _ = writeln!(
            out,
            "{}",
            ctx.messages.render(
                "policy.stale_notice",
                &params(&[
                    ("readOn", analysis.policy_read_on.clone()),
                    ("url", "https://github.com/macportsbrew/macportsbrew".into())
                ])
            )
        );
    }
    out
}

/// `analyze` and `list`: one unit per package across providers.
pub fn analysis(ctx: &Context, analysis: &Analysis, with_reasons: bool) -> String {
    let mut out = String::new();
    if analysis.packages.is_empty() {
        let _ = writeln!(out, "{}", label(ctx, "analysis.nothing_to_show"));
        return out;
    }
    let mut table = Table::new([
        label(ctx, "table.package"),
        label(ctx, "table.provider"),
        label(ctx, "table.version"),
        label(ctx, "table.ownership"),
        label(ctx, "table.availability"),
        label(ctx, "table.class"),
        label(ctx, "table.recommendation"),
    ]);
    for unit in &analysis.packages {
        let mut first = true;
        for (id, row) in &unit.rows {
            let version = row
                .installed
                .iter()
                .find(|r| r.active)
                .or_else(|| row.installed.first())
                .map_or_else(
                    || label(ctx, "row.not_installed"),
                    |r| r.version.to_string(),
                );
            table.push([
                if first {
                    unit.name.clone()
                } else {
                    String::new()
                },
                format!(
                    "{id}{}",
                    if row.presence == macportsbrew_application::RowPresence::Mapped {
                        " *"
                    } else {
                        ""
                    }
                ),
                version,
                if row.presence == macportsbrew_application::RowPresence::Mapped {
                    String::from("-")
                } else {
                    ownership_text(ctx, row.ownership)
                },
                availability_text(ctx, &row.availability.availability),
                if first {
                    class_text(ctx, unit.class.dominant())
                } else {
                    String::new()
                },
                if first {
                    outcome_text(ctx, &unit.recommendation.outcome)
                } else {
                    String::new()
                },
            ]);
            first = false;
        }
    }
    out.push_str(&table.render(None));
    if with_reasons {
        let _ = writeln!(out);
        for unit in &analysis.packages {
            let _ = writeln!(out, "{}:", unit.name);
            for reason in &unit.recommendation.reasons {
                let _ = writeln!(out, "  - {}", ctx.messages.render_reason(reason));
            }
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "analysis.mapped_row_note"));
    if analysis.summary.dependency_units == 0 && !ctx_advanced(analysis) {
        let _ = writeln!(out, "{}", label(ctx, "analysis.advanced_hint"));
    }
    out
}

fn ctx_advanced(analysis: &Analysis) -> bool {
    analysis.packages.iter().any(|p| p.advanced_only)
}

/// `explain`.
pub fn explanation(ctx: &Context, e: &Explanation) -> String {
    let mut out = String::new();
    let unit: &PackageReport = &e.package;
    let _ = writeln!(
        out,
        "{}",
        ctx.messages.render(
            "explain.heading",
            &params(&[
                ("package", unit.name.clone()),
                ("provider", unit.source.to_string())
            ])
        )
    );
    let _ = writeln!(
        out,
        "  {}: {}",
        label(ctx, "explain.recommendation"),
        outcome_text(ctx, &unit.recommendation.outcome)
    );
    let _ = writeln!(
        out,
        "  {}: {}",
        label(ctx, "explain.policy_version"),
        e.derivation.policy_version
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "explain.facts_heading"));
    for (k, v) in &e.derivation.facts_used {
        let _ = writeln!(out, "  {k}: {v}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "explain.fired_heading"));
    for f in &e.derivation.fired {
        let _ = writeln!(out, "  {} -> {}", f.rule, outcome_text(ctx, &f.outcome));
        let _ = writeln!(out, "      {}", ctx.messages.render_reason(&f.reason));
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "explain.not_fired_heading"));
    for r in &e.derivation.not_fired {
        let _ = writeln!(out, "  {r}");
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "explain.mappings_heading"));
    for m in &unit.mappings {
        let _ = writeln!(
            out,
            "  -> {}: {}",
            m.destination,
            label(ctx, confidence_key(m.confidence))
        );
        for c in &m.candidates {
            let _ = writeln!(
                out,
                "     {} ({})",
                c.package.name,
                label(ctx, confidence_key(c.confidence))
            );
            for ev in &c.evidence {
                let _ = writeln!(out, "       {}: {} (+{})", ev.kind, ev.detail, ev.weight);
            }
        }
    }
    out
}

/// `plan`: the dry-run rendering of a plan.
pub fn plan(ctx: &Context, plan: &MigrationPlan) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{}",
        ctx.messages.render(
            "plan.heading",
            &params(&[
                ("package", plan.package.name.clone()),
                ("source", plan.source.to_string()),
                ("destination", plan.destination.to_string()),
            ])
        )
    );
    let _ = writeln!(out, "  {}", label(ctx, "plan.dry_run_only"));
    if let Some(o) = &plan.overrode {
        let _ = writeln!(
            out,
            "  {}",
            ctx.messages.render(
                "plan.overrides",
                &params(&[("recommendation", outcome_text(ctx, o))])
            )
        );
    }
    if !plan.risks.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", label(ctx, "plan.risks_heading"));
        for r in &plan.risks {
            let marker = if r.blocking { "!" } else { "-" };
            let _ = writeln!(out, "  {marker} {}", ctx.messages.render_reason(&r.reason));
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "plan.steps_heading"));
    for step in &plan.steps {
        let kind = label(ctx, step_key(step.kind));
        let _ = writeln!(
            out,
            "  {}. {} - {}",
            step.index,
            kind,
            ctx.messages.render_reason(&step.reason)
        );
        for c in &step.commands {
            // An illustration is marked where it is read. A plan built against
            // a provider that was not detected carries commands no executor
            // could run, and they read exactly like the real ones (F-20).
            let illustrative = if c.placeholder {
                format!("  [{}]", label(ctx, "plan.placeholder"))
            } else {
                String::new()
            };
            let elevation = match &c.elevation {
                macportsbrew_domain::Elevation::None => String::new(),
                macportsbrew_domain::Elevation::Required { reason } => {
                    format!("  [{}: {reason}]", label(ctx, "plan.elevation_required"))
                }
                macportsbrew_domain::Elevation::Unknown => {
                    format!("  [{}]", label(ctx, "plan.elevation_unknown"))
                }
            };
            let _ = writeln!(
                out,
                "       $ {} {}{elevation}{illustrative}",
                c.program,
                c.argv.join(" ")
            );
        }
        for v in &step.verification {
            let _ = writeln!(out, "       ? {}", verification_line(ctx, v));
        }
        if step.kind == StepKind::ManualReview {
            let _ = writeln!(out, "       {}", label(ctx, "plan.stops_here"));
        }
    }
    // `plan.elevated_steps` is every step whose elevation is not `None`, which
    // is the conservative set and the right one for a machine-readable field and
    // for gating consent. Saying "these need administrator rights" about all of
    // it is a different claim: a step is in there because a provider fact is
    // *unknown* as often as because it is known to elevate, and telling someone
    // a read will ask for a password is how they learn to approve prompts
    // without reading them (INV-06, finding F-10).
    //
    // The split lives on the plan rather than here. A first version computed it
    // inline, which made two copies of the definition of "elevating" - and a
    // second copy of a definition is how the first one drifts.
    let required = plan.steps_requiring_elevation();
    let unknown = plan.steps_with_unknown_elevation();
    let joined = |v: &[u32]| {
        v.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    if !required.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "{}",
            ctx.messages.render(
                "plan.elevated_summary",
                &params(&[("steps", joined(&required))])
            )
        );
    }
    if !unknown.is_empty() {
        if required.is_empty() {
            let _ = writeln!(out);
        }
        let _ = writeln!(
            out,
            "{}",
            ctx.messages.render(
                "plan.unknown_elevation_summary",
                &params(&[("steps", joined(&unknown))])
            )
        );
    }
    out
}

/// The catalogue key for a machine's translation state.
///
/// An exhaustive `match` rather than a `Debug` rendering, so a new variant is a
/// compile error here instead of a Rust identifier in front of a person (F-20).
fn translation_key(value: Translation) -> &'static str {
    match value {
        Translation::Native => "translation.native",
        Translation::Translated => "translation.translated",
        Translation::Unknown => "translation.unknown",
    }
}

/// The catalogue key for a plan step's kind. Same reasoning as
/// [`translation_key`].
fn step_key(value: StepKind) -> &'static str {
    match value {
        StepKind::SnapshotBefore => "step.snapshot_before",
        StepKind::AddDestination => "step.add_destination",
        StepKind::VerifyDestination => "step.verify_destination",
        StepKind::SetPrimary => "step.set_primary",
        StepKind::VerifyResolution => "step.verify_resolution",
        StepKind::RemoveSource => "step.remove_source",
        StepKind::VerifyRemoval => "step.verify_removal",
        StepKind::CleanupReview => "step.cleanup_review",
        StepKind::RecordHistory => "step.record_history",
        StepKind::ManualReview => "step.manual_review",
    }
}

/// The catalogue key for a mapping confidence. Same reasoning as
/// [`translation_key`]; F-20 listed three `{:?}` sites and missed the two that
/// render this one.
fn confidence_key(value: macportsbrew_domain::Confidence) -> &'static str {
    use macportsbrew_domain::Confidence;
    match value {
        Confidence::None => "confidence.none",
        Confidence::Ambiguous => "confidence.ambiguous",
        Confidence::Probable => "confidence.probable",
        Confidence::Strong => "confidence.strong",
        Confidence::Exact => "confidence.exact",
    }
}

/// One verification check, rendered.
///
/// The two that carry data render it through the catalogue too, so a
/// translation decides where the executable name sits in the sentence rather
/// than this function deciding it in English.
fn verification_line(ctx: &Context, value: &VerificationCheck) -> String {
    match value {
        VerificationCheck::ProviderReportsActive => {
            label(ctx, "verification.provider_reports_active")
        }
        VerificationCheck::ProviderIntegrityCheck => {
            label(ctx, "verification.provider_integrity_check")
        }
        VerificationCheck::ExecutablesOwnedByDestination { executables } => ctx.messages.render(
            "verification.executables_owned_by_destination",
            &params(&[("executables", executables.join(", "))]),
        ),
        VerificationCheck::LivenessProbe { executable, flag } => ctx.messages.render(
            "verification.liveness_probe",
            &params(&[("executable", executable.clone()), ("flag", flag.clone())]),
        ),
        VerificationCheck::NoCrossProviderLinkage => {
            label(ctx, "verification.no_cross_provider_linkage")
        }
        VerificationCheck::PackageAbsent => label(ctx, "verification.package_absent"),
        VerificationCheck::PackageNewlyPresent => label(ctx, "verification.package_newly_present"),
        VerificationCheck::Unknown => label(ctx, "verification.unknown"),
    }
}

/// `doctor`.
pub fn doctor(ctx: &Context, doc: &DoctorReport) -> String {
    let mut out = String::new();
    for f in &doc.findings {
        let sev = match f.severity {
            Severity::Ok => label(ctx, "doctor.ok"),
            Severity::Warning => label(ctx, "doctor.warning"),
            Severity::Error => label(ctx, "doctor.error"),
        };
        let _ = writeln!(out, "[{sev}] {}", ctx.messages.render_reason(&f.reason));
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "doctor.deep_not_in_this_build"));
    out
}
