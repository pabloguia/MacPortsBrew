// SPDX-License-Identifier: Apache-2.0

//! Human renderings. Plain text, stable when not on a terminal, every
//! label through the catalogue, every column measured in display width.

use super::context::Context;
use crate::executor::{RenderedRefusal, RunReport, SiteRefusalKind};
use crate::render::table::Table;
use macportsbrew_application::NotRecognised;
use macportsbrew_application::{
    execution_refusals, Analysis, CheckKind, Disposition, DoctorReport, ExecutionRefusal,
    Explanation, HaltReason, PackageReport, RunOutcome, Severity, Unevaluable,
};
use macportsbrew_domain::{
    ArtifactAvailability, ConcurrentProviderActivity, ExpectationError, FailureCause,
    MigrationPlan, Outcome, Ownership, PackageClass, ProviderId, ProviderPresence,
    SourceBuildReason, StepKind, SuccessSemantic, SupportTier, Translation, VerificationCheck,
};
use std::collections::BTreeMap;
use std::fmt::Write;

fn label(ctx: &Context, id: &str) -> String {
    ctx.messages.render_id(id)
}

/// Snapshot-derived text printed outside a catalogue parameter, escaped the
/// way `Messages::render` escapes every parameter: one classification
/// (`macportsbrew_domain::text`), one escape, so a cell, an argv or an evidence
/// line shows a format character the way a sentence does.
fn shown(value: &str) -> String {
    macportsbrew_domain::escape_hostile(value)
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
            shown(id.as_str()),
            presence,
            p.detection
                .version
                .as_ref()
                .map_or(String::from("-"), |v| shown(v.as_str())),
            p.detection
                .prefix
                .as_ref()
                .map_or(String::from("-"), |x| shown(&x.display().to_string())),
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
    // A stale policy is said once, on stderr, for every command
    // (`app::run`); repeating it here printed it twice on `status`.
    out
}

/// `word` as one word for a POSIX shell: unchanged when it holds only
/// characters no shell treats specially, otherwise single-quoted with any
/// single quote inside closed, escaped and reopened. A leading `=` is quoted
/// too: zsh, the default shell on macOS, expands `=name` to a command's path.
pub(crate) fn shell_word(word: &str) -> String {
    let plain = !word.is_empty()
        && !word.starts_with('=')
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./@%+=:,-".contains(c));
    if plain {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// `analyze` and `list`: one unit per package across providers.
pub fn analysis(ctx: &Context, analysis: &Analysis, with_reasons: bool) -> String {
    let mut out = String::new();
    // A broken provider is named whatever else is shown: its packages are
    // missing from the view, and the view alone would not say why.
    for (id, p) in &analysis.providers {
        if matches!(p.detection.presence, ProviderPresence::Broken { .. }) {
            let _ = writeln!(
                out,
                "{}",
                ctx.messages.render(
                    "analysis.provider_broken",
                    &params(&[("provider", id.to_string())])
                )
            );
        }
    }
    if analysis.packages.is_empty() {
        // `--advanced` cannot help when neither provider answered.
        let none_present = analysis
            .providers
            .values()
            .all(|p| !matches!(p.detection.presence, ProviderPresence::Present));
        let key = if none_present {
            "analysis.no_provider"
        } else {
            "analysis.nothing_to_show"
        };
        let _ = writeln!(out, "{}", label(ctx, key));
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
                    |r| shown(&r.version.to_string()),
                );
            table.push([
                if first {
                    shown(&unit.name)
                } else {
                    String::new()
                },
                format!(
                    "{}{}",
                    shown(id.as_str()),
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
            let _ = writeln!(out, "{}:", shown(&unit.name));
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
        shown(&e.derivation.policy_version)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "explain.facts_heading"));
    for (k, v) in &e.derivation.facts_used {
        // An empty fact (no class at all, say) reads as a value, not a blank.
        let value = if v.is_empty() {
            label(ctx, "explain.fact_empty")
        } else {
            shown(v)
        };
        let _ = writeln!(out, "  {}: {value}", shown(k));
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
            shown(&m.destination.to_string()),
            label(ctx, confidence_key(m.confidence))
        );
        for c in &m.candidates {
            let _ = writeln!(
                out,
                "     {} ({})",
                shown(&c.package.name),
                label(ctx, confidence_key(c.confidence))
            );
            for ev in &c.evidence {
                let _ = writeln!(
                    out,
                    "       {}: {} (+{})",
                    shown(&ev.kind),
                    shown(&ev.detail),
                    ev.weight
                );
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
            // This tool's own command is not one this version has: a person
            // following the plan by hand is told what to do instead of being
            // shown a `$` line that fails (pass 10, truth sweep H1). The plan
            // and its `--json` form are unchanged.
            if c.target == macportsbrew_domain::CommandTarget::Tool {
                let provider = step
                    .provider
                    .as_ref()
                    .map_or_else(String::new, ToString::to_string);
                let _ = writeln!(
                    out,
                    "       {}",
                    ctx.messages.render(
                        "plan.tool_command_absent",
                        &params(&[("provider", provider)])
                    )
                );
                continue;
            }
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
                    format!(
                        "  [{}: {}]",
                        label(ctx, "plan.elevation_required"),
                        shown(reason)
                    )
                }
                macportsbrew_domain::Elevation::Unknown => {
                    format!("  [{}]", label(ctx, "plan.elevation_unknown"))
                }
            };
            // **SE-083** recorded "the human renderer does not show the
            // semantic" as deliberately not done, that pass's work order having
            // put it out of scope. (This comment cited SE-084 and quoted a
            // sentence that appears in neither entry, until `spec-guardian`
            // checked it on 2026-09-19 — an invented rationale credited to the
            // wrong record, in the file that is the audit trail.) That
            // reasoning held while each verb had one semantic. It stopped on
            // 2026-09-19, when `VERIFY_REMOVAL` gained the *same verb* as
            // `VERIFY_DESTINATION` with the opposite meaning: this view renders
            // `$ /opt/local/bin/port -q installed` and `$ ... -q installed foo`,
            // which differ by a trailing argument and mean opposite things.
            // INV-02 makes this the surface a human consents on, so a reader
            // who cannot tell them apart is the defect (`security-reviewer`).
            //
            // Only the two that change how the line should be read are marked.
            // `EXIT_STATUS` and `STDOUT_NAMES_PACKAGE` are what a reader already
            // assumes a verification command means, and a marker on every
            // command would bury the two that matter.
            let success = match c.success {
                SuccessSemantic::ExitStatus | SuccessSemantic::StdoutNamesPackage => String::new(),
                SuccessSemantic::StdoutOmitsPackage => {
                    format!("  [{}]", label(ctx, "plan.success_omits"))
                }
                SuccessSemantic::Unknown => {
                    format!("  [{}]", label(ctx, "plan.success_unknown"))
                }
            };
            // Each word is quoted for a POSIX shell, because the plan invites
            // a person to type these lines: a program or argument taken from a
            // snapshot file could otherwise carry `$(...)` or `;` into their
            // shell (security review before 0.1.0, M1).
            let words = std::iter::once(&c.program)
                .chain(&c.argv)
                .map(|w| shell_word(&shown(w)))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(out, "       $ {words}{elevation}{success}{illustrative}");
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

/// `migrate`, when there is nothing to move (INV-16).
pub fn nothing_to_move(
    ctx: &Context,
    package: &str,
    provider: &ProviderId,
    not_detected: &[ProviderId],
) -> String {
    let mut out = ctx.messages.render(
        "migrate.nothing_to_move",
        &params(&[
            ("package", package.to_string()),
            ("provider", provider.to_string()),
        ]),
    );
    out.push('\n');
    // DP-026: the answer stands, and says how far it could see.
    for other in not_detected {
        out.push_str(&ctx.messages.render(
            "migrate.nothing_to_move_not_detected",
            &params(&[
                ("provider", other.to_string()),
                ("package", package.to_string()),
            ]),
        ));
        out.push('\n');
    }
    out
}

/// `migrate --retain-source` when history and a fresh capture show the
/// migration done (INV-16): nothing planned, nothing changed.
pub fn retained_run_done(
    ctx: &Context,
    package: &str,
    provider: &ProviderId,
    run_id: &str,
) -> String {
    let mut out = ctx.messages.render(
        "migrate.retained_run_done",
        &params(&[
            ("package", package.to_string()),
            ("provider", provider.to_string()),
            ("run", run_id.to_string()),
        ]),
    );
    out.push('\n');
    out
}

/// `migrate --retain-source`, before the plan, when history had a record to
/// say something about and it was not enough: why the migration is planned
/// again rather than reported done. Exhaustive, with literal keys, so a new
/// reason is a compile error here and every key is seen rendered.
pub fn retained_run_not_recognised(ctx: &Context, why: &NotRecognised) -> String {
    let (key, run) = match why {
        NotRecognised::LatestRunNotSuccess { run_id } => (
            "migrate.history_not_recognised.latest_run_not_success",
            Some(run_id),
        ),
        NotRecognised::LatestRunRemovedSource { run_id } => (
            "migrate.history_not_recognised.latest_run_removed_source",
            Some(run_id),
        ),
        NotRecognised::Tie => ("migrate.history_not_recognised.tie", None),
        NotRecognised::RecordFromTheFuture => (
            "migrate.history_not_recognised.record_from_the_future",
            None,
        ),
        NotRecognised::DestinationNotActive { run_id } => (
            "migrate.history_not_recognised.destination_not_active",
            Some(run_id),
        ),
        NotRecognised::PrimaryIsShim { run_id } => (
            "migrate.history_not_recognised.primary_is_shim",
            Some(run_id),
        ),
        NotRecognised::PrimaryNotConfirmed { run_id } => (
            "migrate.history_not_recognised.primary_not_confirmed",
            Some(run_id),
        ),
    };
    let mut out = ctx
        .messages
        .render(key, &params(&[("run", run.cloned().unwrap_or_default())]));
    out.push_str("\n\n");
    out
}

/// `migrate` in render mode, after the plan: what would put it to the gate.
pub fn render_only(ctx: &Context) -> String {
    format!("\n{}\n", label(ctx, "migrate.render_only"))
}

/// `migrate`, `plan` and `analyze`, last: whether a provider may have been
/// changing the machine while it was read. Every answer this build gives says
/// "possibly inconsistent" and says why, and none claims a check that was not
/// made (`plan.v1.md`, obligation 5). The `match` is exhaustive, so a new
/// answer is a compile error here rather than an identifier in front of a
/// person. `status` and `list` do not end with it: their `--json` carries the
/// flag, but their human views are an overview and an inventory, not an
/// answer to act on (SE-015).
pub fn concurrency(ctx: &Context, activity: &ConcurrentProviderActivity) -> String {
    let line = match activity {
        ConcurrentProviderActivity::NotChecked => label(ctx, "concurrency.not_checked"),
        ConcurrentProviderActivity::Unknowable => label(ctx, "concurrency.unknowable"),
        ConcurrentProviderActivity::NoneObserved => label(ctx, "concurrency.none_observed"),
        ConcurrentProviderActivity::Observed { providers } => ctx.messages.render(
            "concurrency.observed",
            &params(&[
                ("count", providers.len().to_string()),
                (
                    "providers",
                    providers
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ]),
        ),
    };
    format!("\n{line}\n")
}

/// `migrate` in execute mode, after the plan: how the run ended, the gate's
/// refusals, what became of each step, and a read site's refusal.
///
/// Every sentence is the catalogue's, keyed by an exhaustive `match` over the
/// typed outcome, so a new variant is a compile error here rather than an
/// identifier in front of a person (F-20). The gate's refusals are rendered
/// from the gate's own typed list for this plan — the report keeps them only
/// as cleaned documents — and only by step number and kind, so no plan text
/// reaches this view; `--json` carries their fields, cleaned.
pub fn run_report(ctx: &Context, plan: &MigrationPlan, report: &RunReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out);
    let _ = writeln!(out, "{}", label(ctx, "migrate.execution_heading"));
    let _ = writeln!(out, "  {}", outcome_line(ctx, &report.outcome));
    if matches!(report.outcome, RunOutcome::Refused { .. }) {
        for refusal in execution_refusals(plan).refusals() {
            let _ = writeln!(out, "    - {}", gate_refusal_line(ctx, refusal));
        }
        // What a person can do now, after the reasons: the plan above is the
        // product of this version (pass 10, truth sweep H5).
        let _ = writeln!(out);
        let _ = writeln!(out, "  {}", label(ctx, "migrate.by_hand"));
    }
    if !report.records.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "{}", label(ctx, "migrate.steps_heading"));
        for record in &report.records {
            let _ = writeln!(
                out,
                "  {}. {}: {}",
                record.index,
                label(ctx, step_key(record.kind)),
                disposition_text(ctx, &record.disposition)
            );
        }
    }
    if let Some(refusal) = &report.refusal {
        let _ = writeln!(out, "  {}", site_refusal_line(ctx, refusal));
    }
    out
}

fn outcome_line(ctx: &Context, outcome: &RunOutcome) -> String {
    match outcome {
        RunOutcome::Completed => label(ctx, "migrate.outcome.completed"),
        RunOutcome::Refused { refusals } => ctx.messages.render(
            "migrate.outcome.refused",
            &params(&[("count", refusals.to_string())]),
        ),
        RunOutcome::Halted { step, reason } => ctx.messages.render(
            "migrate.outcome.halted",
            &params(&[
                ("step", step.to_string()),
                ("reason", halt_text(ctx, reason)),
            ]),
        ),
        RunOutcome::PartialSuccess { step, reason } => ctx.messages.render(
            "migrate.outcome.partial_success",
            &params(&[
                ("step", step.to_string()),
                ("reason", halt_text(ctx, reason)),
            ]),
        ),
    }
}

fn disposition_text(ctx: &Context, disposition: &Disposition) -> String {
    match disposition {
        Disposition::Verified { evidence } => ctx.messages.render(
            "migrate.disposition.verified",
            &params(&[("count", evidence.len().to_string())]),
        ),
        Disposition::NotPerformedInThisBuild => {
            label(ctx, "migrate.disposition.not_performed_in_this_build")
        }
        Disposition::NotDispatchedMutating => {
            label(ctx, "migrate.disposition.not_dispatched_mutating")
        }
        Disposition::Halted { reason, .. } => ctx.messages.render(
            "migrate.disposition.halted",
            &params(&[("reason", halt_text(ctx, reason))]),
        ),
        Disposition::NotReached => label(ctx, "migrate.disposition.not_reached"),
    }
}

/// A command position as a person counts it: the halt reason counts from 0.
fn command_number(command: usize) -> String {
    (command + 1).to_string()
}

fn halt_text(ctx: &Context, reason: &HaltReason) -> String {
    let command = |id: &str, n: usize| {
        ctx.messages
            .render(id, &params(&[("command", command_number(n))]))
    };
    match reason {
        HaltReason::ManualReview { step } => ctx.messages.render(
            "migrate.halt.manual_review",
            &params(&[("step", step.to_string())]),
        ),
        HaltReason::UnexecutedMutation => label(ctx, "migrate.halt.unexecuted_mutation"),
        HaltReason::NotPerformedInThisBuild => {
            label(ctx, "migrate.halt.not_performed_in_this_build")
        }
        HaltReason::SiteUnavailable { command: n } => command("migrate.halt.site_unavailable", *n),
        HaltReason::SiteRefused { command: n } => command("migrate.halt.site_refused", *n),
        HaltReason::TransportFailed { command: n, error } => ctx.messages.render(
            "migrate.halt.transport_failed",
            &params(&[
                ("command", command_number(*n)),
                ("code", error.exit_code().to_string()),
            ]),
        ),
        HaltReason::Expectation { command: n, error } => ctx.messages.render(
            "migrate.halt.expectation",
            &params(&[
                ("command", command_number(*n)),
                ("why", label(ctx, expectation_key(*error))),
            ]),
        ),
        HaltReason::VerificationFailed { command: n, cause } => ctx.messages.render(
            "migrate.halt.verification_failed",
            &params(&[
                ("command", command_number(*n)),
                ("cause", label(ctx, failure_key(*cause))),
            ]),
        ),
        HaltReason::ElevationNotNone { command: n } => {
            command("migrate.halt.elevation_not_none", *n)
        }
        HaltReason::Unevaluable {
            check: Some(check),
            why,
        } => ctx.messages.render(
            "migrate.halt.unevaluable_check",
            &params(&[
                ("check", label(ctx, check_key(*check))),
                ("why", label(ctx, unevaluable_key(*why))),
            ]),
        ),
        HaltReason::Unevaluable { check: None, why } => ctx.messages.render(
            "migrate.halt.unevaluable",
            &params(&[("why", label(ctx, unevaluable_key(*why)))]),
        ),
        HaltReason::RecordsDoNotMatchPlan => label(ctx, "migrate.halt.records_do_not_match_plan"),
    }
}

fn expectation_key(value: ExpectationError) -> &'static str {
    match value {
        ExpectationError::StepNotInPlan => "migrate.expectation.step_not_in_plan",
        ExpectationError::CommandNotInStep => "migrate.expectation.command_not_in_step",
        ExpectationError::NotAVerificationStep => "migrate.expectation.not_a_verification_step",
        ExpectationError::PlaceholderCommand => "migrate.expectation.placeholder_command",
        ExpectationError::NotAProviderCommand => "migrate.expectation.not_a_provider_command",
        ExpectationError::UnrecognisedCheck => "migrate.expectation.unrecognised_check",
        ExpectationError::SuccessUnknown => "migrate.expectation.success_unknown",
        ExpectationError::WrongProvider => "migrate.expectation.wrong_provider",
        ExpectationError::SemanticNotValidForStep => {
            "migrate.expectation.semantic_not_valid_for_step"
        }
        ExpectationError::NoDestinationName => "migrate.expectation.no_destination_name",
        ExpectationError::UnsafePackageName => "migrate.expectation.unsafe_package_name",
        ExpectationError::ArgvDoesNotNameDestination => {
            "migrate.expectation.argv_does_not_name_destination"
        }
        ExpectationError::ArgvNamesAPackage => "migrate.expectation.argv_names_a_package",
    }
}

fn failure_key(value: FailureCause) -> &'static str {
    match value {
        FailureCause::DidNotFinish => "migrate.failure.did_not_finish",
        FailureCause::Truncated => "migrate.failure.truncated",
        FailureCause::NonZeroExit => "migrate.failure.non_zero_exit",
        FailureCause::EmptyOutput => "migrate.failure.empty_output",
        FailureCause::NameAbsent => "migrate.failure.name_absent",
        FailureCause::NamePresent => "migrate.failure.name_present",
    }
}

fn unevaluable_key(value: Unevaluable) -> &'static str {
    match value {
        Unevaluable::NoEvaluationForStepKind => "migrate.unevaluable.no_evaluation_for_step_kind",
        Unevaluable::NoCommand => "migrate.unevaluable.no_command",
        Unevaluable::NoCheck => "migrate.unevaluable.no_check",
        Unevaluable::CheckHasNoEvaluation => "migrate.unevaluable.check_has_no_evaluation",
        Unevaluable::NoPassingRead => "migrate.unevaluable.no_passing_read",
        Unevaluable::MissingRead => "migrate.unevaluable.missing_read",
        Unevaluable::PresenceBeforeUnknown => "migrate.unevaluable.presence_before_unknown",
        Unevaluable::PresentBeforeTheRun => "migrate.unevaluable.present_before_the_run",
    }
}

fn check_key(value: CheckKind) -> &'static str {
    match value {
        CheckKind::ProviderReportsActive => "migrate.check.provider_reports_active",
        CheckKind::ProviderIntegrityCheck => "migrate.check.provider_integrity_check",
        CheckKind::ExecutablesOwnedByDestination => {
            "migrate.check.executables_owned_by_destination"
        }
        CheckKind::LivenessProbe => "migrate.check.liveness_probe",
        CheckKind::NoCrossProviderLinkage => "migrate.check.no_cross_provider_linkage",
        CheckKind::PackageAbsent => "migrate.check.package_absent",
        CheckKind::PackageNewlyPresent => "migrate.check.package_newly_present",
        CheckKind::Unknown => "migrate.check.unknown",
    }
}

fn steps_text(steps: &[u32]) -> String {
    steps
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn gate_refusal_line(ctx: &Context, refusal: &ExecutionRefusal) -> String {
    let step = |id: &str, step: u32| {
        ctx.messages
            .render(id, &params(&[("step", step.to_string())]))
    };
    let steps = |id: &str, steps: &[u32]| {
        ctx.messages
            .render(id, &params(&[("steps", steps_text(steps))]))
    };
    match refusal {
        ExecutionRefusal::SchemaVersionNotThisBuilds { found, expected } => ctx.messages.render(
            "migrate.refusal.schema_version_not_this_builds",
            &params(&[
                ("found", found.to_string()),
                ("expected", expected.to_string()),
            ]),
        ),
        ExecutionRefusal::Ordering => label(ctx, "migrate.refusal.ordering"),
        ExecutionRefusal::StepIndicesNotSequential => {
            label(ctx, "migrate.refusal.step_indices_not_sequential")
        }
        ExecutionRefusal::EmptyPlan => label(ctx, "migrate.refusal.empty_plan"),
        ExecutionRefusal::NoSnapshotBeforeFirstMutation { step: s } => {
            step("migrate.refusal.no_snapshot_before_first_mutation", *s)
        }
        ExecutionRefusal::NoRecordHistoryAtEnd => {
            label(ctx, "migrate.refusal.no_record_history_at_end")
        }
        ExecutionRefusal::MoreThanOneSnapshotBefore { steps: s } => {
            steps("migrate.refusal.more_than_one_snapshot_before", s)
        }
        ExecutionRefusal::MoreThanOneRecordHistory { steps: s } => {
            steps("migrate.refusal.more_than_one_record_history", s)
        }
        ExecutionRefusal::ManualReview { step: s } => step("migrate.refusal.manual_review", *s),
        ExecutionRefusal::Placeholder { steps: s } => steps("migrate.refusal.placeholder", s),
        ExecutionRefusal::UnrecognisedCheck { steps: s } => {
            steps("migrate.refusal.unrecognised_check", s)
        }
        ExecutionRefusal::CheckNotEvaluableInThisBuild { step: s, check } => ctx.messages.render(
            "migrate.refusal.check_not_evaluable_in_this_build",
            &params(&[
                ("step", s.to_string()),
                ("check", label(ctx, check_key(CheckKind::from(check)))),
            ]),
        ),
        ExecutionRefusal::VerificationAssertsNothing { step: s } => {
            step("migrate.refusal.verification_asserts_nothing", *s)
        }
        ExecutionRefusal::UnknownSuccess { steps: s } => {
            steps("migrate.refusal.unknown_success", s)
        }
        ExecutionRefusal::CommandOnStepThePlannerWritesNoneOn { step: s } => step(
            "migrate.refusal.command_on_step_the_planner_writes_none_on",
            *s,
        ),
        ExecutionRefusal::CommandIsNotThePlannersRead { step: s } => {
            step("migrate.refusal.command_is_not_the_planners_read", *s)
        }
        ExecutionRefusal::Expectation { step: s, error } => ctx.messages.render(
            "migrate.refusal.expectation",
            &params(&[
                ("step", s.to_string()),
                ("why", label(ctx, expectation_key(*error))),
            ]),
        ),
        ExecutionRefusal::SameProviderOnBothSides => {
            label(ctx, "migrate.refusal.same_provider_on_both_sides")
        }
        ExecutionRefusal::ElevationRequired { steps: s } => {
            steps("migrate.refusal.elevation_required", s)
        }
        ExecutionRefusal::ElevationUnknown { steps: s } => {
            steps("migrate.refusal.elevation_unknown", s)
        }
        ExecutionRefusal::MutationNotInThisBuild { step: s } => {
            step("migrate.refusal.mutation_not_in_this_build", *s)
        }
        ExecutionRefusal::PlanNotDryRunOnly => label(ctx, "migrate.refusal.plan_not_dry_run_only"),
        ExecutionRefusal::ProviderConfiguration { step: s, provider } => ctx.messages.render(
            "migrate.refusal.provider_configuration",
            &params(&[("step", s.to_string()), ("provider", provider.to_string())]),
        ),
    }
}

/// A read site's refusal, by its kind and the values it quotes, which the
/// report keeps cleaned (home redacted, controls escaped).
fn site_refusal_line(ctx: &Context, refusal: &RenderedRefusal) -> String {
    let value = |name: &str| refusal.values.get(name).cloned().unwrap_or_default();
    let step = refusal.step.to_string();
    match refusal.refusal {
        SiteRefusalKind::NotAProviderCommand => ctx.messages.render(
            "migrate.site.not_a_provider_command",
            &params(&[("step", step)]),
        ),
        SiteRefusalKind::UnsafePrefix => ctx.messages.render(
            "migrate.site.unsafe_prefix",
            &params(&[("step", step), ("prefix", value("prefix"))]),
        ),
        SiteRefusalKind::ProgramDisagrees => ctx.messages.render(
            "migrate.site.program_disagrees",
            &params(&[
                ("step", step),
                ("planned", value("planned")),
                ("derived", value("derived")),
            ]),
        ),
        SiteRefusalKind::NotAPermittedRead => ctx.messages.render(
            "migrate.site.not_a_permitted_read",
            &params(&[("step", step), ("argv", value("argv"))]),
        ),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_plan_word_is_quoted_unless_no_shell_would_read_it_specially() {
        use super::shell_word;
        assert_eq!(shell_word("/opt/local/bin/port"), "/opt/local/bin/port");
        assert_eq!(shell_word("--versions"), "--versions");
        assert_eq!(shell_word("python@3.12"), "python@3.12");
        assert_eq!(
            shell_word("/opt/x $(touch y)/bin/port"),
            "'/opt/x $(touch y)/bin/port'"
        );
        assert_eq!(shell_word("a;b"), "'a;b'");
        assert_eq!(shell_word("it's"), "'it'\\''s'");
        assert_eq!(shell_word(""), "''");
        assert_eq!(shell_word("=ls"), "'=ls'");
        assert_eq!(shell_word("a=b"), "a=b");
    }
    use super::*;
    use crate::executor::{RenderedRefusal, RunReport, SiteRefusalKind};
    use clap::Parser;
    use macportsbrew_application::StepRecord;
    use macportsbrew_domain::ErrorId;

    /// A context over a committed recording; nothing here reads this machine.
    fn context() -> Context {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/recordings/both-providers-arm64-macos15.json"
        );
        let cli = super::super::Cli::parse_from([
            "macportsbrew",
            "--replay",
            fixture,
            "--today",
            "2026-09-02",
            "status",
        ]);
        Context::build(&cli, &mut Vec::new()).expect("the recording builds a context")
    }

    /// Every halt reason the run loop can record.
    fn every_halt() -> Vec<HaltReason> {
        let mut out = vec![
            HaltReason::ManualReview { step: 9 },
            HaltReason::UnexecutedMutation,
            HaltReason::NotPerformedInThisBuild,
            HaltReason::SiteUnavailable { command: 0 },
            HaltReason::SiteRefused { command: 0 },
            HaltReason::TransportFailed {
                command: 0,
                error: ErrorId::ProviderTimeout,
            },
            HaltReason::ElevationNotNone { command: 1 },
            HaltReason::RecordsDoNotMatchPlan,
        ];
        for error in [
            ExpectationError::StepNotInPlan,
            ExpectationError::CommandNotInStep,
            ExpectationError::NotAVerificationStep,
            ExpectationError::PlaceholderCommand,
            ExpectationError::NotAProviderCommand,
            ExpectationError::UnrecognisedCheck,
            ExpectationError::SuccessUnknown,
            ExpectationError::WrongProvider,
            ExpectationError::SemanticNotValidForStep,
            ExpectationError::NoDestinationName,
            ExpectationError::UnsafePackageName,
            ExpectationError::ArgvDoesNotNameDestination,
            ExpectationError::ArgvNamesAPackage,
        ] {
            out.push(HaltReason::Expectation { command: 0, error });
        }
        for cause in [
            FailureCause::DidNotFinish,
            FailureCause::Truncated,
            FailureCause::NonZeroExit,
            FailureCause::EmptyOutput,
            FailureCause::NameAbsent,
            FailureCause::NamePresent,
        ] {
            out.push(HaltReason::VerificationFailed { command: 0, cause });
        }
        let whys = [
            Unevaluable::NoEvaluationForStepKind,
            Unevaluable::NoCommand,
            Unevaluable::NoCheck,
            Unevaluable::CheckHasNoEvaluation,
            Unevaluable::NoPassingRead,
            Unevaluable::MissingRead,
            Unevaluable::PresenceBeforeUnknown,
            Unevaluable::PresentBeforeTheRun,
        ];
        let checks = [
            CheckKind::ProviderReportsActive,
            CheckKind::ProviderIntegrityCheck,
            CheckKind::ExecutablesOwnedByDestination,
            CheckKind::LivenessProbe,
            CheckKind::NoCrossProviderLinkage,
            CheckKind::PackageAbsent,
            CheckKind::PackageNewlyPresent,
            CheckKind::Unknown,
        ];
        for (why, check) in whys.iter().zip(checks.iter()) {
            out.push(HaltReason::Unevaluable {
                check: Some(*check),
                why: *why,
            });
            out.push(HaltReason::Unevaluable {
                check: None,
                why: *why,
            });
        }
        out
    }

    #[test]
    fn every_sentence_of_a_run_report_comes_from_the_catalogue() {
        // S5 owed this: a run's report rendered through the catalogue, with
        // no identifier and no placeholder reaching a person. A run past the
        // gate is unreachable from the binary, so this renders one directly.
        let ctx = context();
        let plan = macportsbrew_application::build_plan(
            &ctx.snapshot,
            &ctx.policy,
            &macportsbrew_application::AnalysisOptions {
                today: ctx.today.clone(),
                ..Default::default()
            },
            &macportsbrew_application::PlanRequest {
                package: "htop".into(),
                to: ProviderId::new("macports"),
                retain_source: false,
            },
            &macportsbrew_adapters::registry::known_provider_ids(),
        )
        .expect("a plan");
        let mut records = Vec::new();
        for (i, reason) in every_halt().into_iter().enumerate() {
            let index = u32::try_from(i).expect("small");
            records.push(StepRecord {
                index,
                kind: StepKind::VerifyDestination,
                disposition: Disposition::Halted {
                    reason,
                    evidence: Vec::new(),
                },
            });
        }
        for disposition in [
            Disposition::Verified {
                evidence: Vec::new(),
            },
            Disposition::NotPerformedInThisBuild,
            Disposition::NotDispatchedMutating,
            Disposition::NotReached,
        ] {
            records.push(StepRecord {
                index: 0,
                kind: StepKind::SnapshotBefore,
                disposition,
            });
        }
        let mut outcomes = vec![RunOutcome::Completed, RunOutcome::Refused { refusals: 1 }];
        for reason in every_halt() {
            outcomes.push(RunOutcome::Halted {
                step: 3,
                reason: reason.clone(),
            });
            outcomes.push(RunOutcome::PartialSuccess { step: 3, reason });
        }
        let mut text = String::new();
        for (kind, values) in [
            (SiteRefusalKind::NotAProviderCommand, vec![]),
            (SiteRefusalKind::UnsafePrefix, vec![("prefix", "/x")]),
            (
                SiteRefusalKind::ProgramDisagrees,
                vec![("planned", "/a/p"), ("derived", "/b/p")],
            ),
            (SiteRefusalKind::NotAPermittedRead, vec![("argv", "a b")]),
        ] {
            for outcome in &outcomes {
                let report = RunReport {
                    outcome: outcome.clone(),
                    records: records.clone(),
                    refusal: Some(RenderedRefusal {
                        step: 3,
                        refusal: kind,
                        values: values.iter().map(|(k, v)| (*k, (*v).to_string())).collect(),
                        text: String::new(),
                    }),
                    gate_refusals: Vec::new(),
                };
                text.push_str(&run_report(&ctx, &plan, &report));
            }
        }
        assert!(
            ctx.messages.missing().is_empty(),
            "{:?}",
            ctx.messages.missing()
        );
        for raw in ["migrate.", "{", "}", "_"] {
            assert!(!text.contains(raw), "{raw:?} reached the user:\n{text}");
        }
    }
}
