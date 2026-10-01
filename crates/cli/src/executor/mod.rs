// SPDX-License-Identifier: Apache-2.0

//! The run loop's I/O shell (DP-024, SE-092): it runs a plan's verification
//! reads and nothing else, and decides as little as sequencing allows.
//!
//! Every decision is a pure function in `macportsbrew_application::executor`;
//! this module obtains reads and hands them over. Its one caller is
//! `app/migrate.rs` (`migrate --yes`), and the execution gate refuses every
//! plan this build's planner produces, so past the gate the loop is
//! exercised by tests against recordings and synthesised reads only — it is
//! unreachable from the binary.
//!
//! **Rules for this module.** They belong in a path-scoped rule file under
//! `.claude/rules/`, which is the owner's to add; until then they live here.
//!
//! 1. **The gate first, structurally.** [`run`] takes the registry context,
//!    not read sites: it consults `execution_refusals` and builds the sites
//!    only when the gate passed the plan, so no caller holds a read site for
//!    a plan the gate refused (SE-050, "Not closed" 1). [`run_with`] takes a
//!    factory for the same reason — it is called after the gate or not at
//!    all. A word scan holds the read-site constructors to this module.
//! 2. **The guard before everything else about a step.** A step is dispatched
//!    only when `next_action` answers `Dispatch`, which it never does for a
//!    mutating step, for a read whose elevation is not `NONE`, or for any step
//!    of a plan carrying `MANUAL_REVIEW`. This module never reads the build's
//!    mutation constant or the plan's own claim about it (A11); a textual
//!    test holds it to that.
//! 3. **No spec is built here.** A read goes through [`ReadSite::read`], whose
//!    production implementation is the adapters' read sites: the program and
//!    the environment are re-derived from the provider's detected prefix, the
//!    argv is checked against the shipped read allowlist, and the read runs
//!    through the very runner the prefix was detected through, so a prefix a
//!    recording answered never reaches a live spawn (DP-023; SE-050, "Not
//!    closed" 2 and 7). Nothing in this crate constructs a command spec or
//!    calls the plan-read constructor; a textual test holds that too.
//! 4. **Step success is judged, never inferred from an exit status alone**
//!    (`04-migration-and-safety.md`): `judge_step` evaluates every read
//!    against the snapshot the plan was built from, which arrives paired with
//!    the plan (`PlannedRun`) and cannot be supplied beside it.
//! 5. **Nothing a read printed is kept, and no plan text is kept raw.** A
//!    record carries `ReadEvidence` — result, line count, truncation flag — a
//!    halt reason carries a check's kind and never its fields, and the texts
//!    this module keeps (a read site's refusal, the gate's refusals) are
//!    rendered with the home redacted and control and format characters
//!    escaped (SE-050, "Not closed" 3).
//! 6. **No provider is named** here, in code or in prose (INV-17): the loop
//!    treats both directions identically because it cannot tell them apart.
//! 7. No verb is written as a literal, and no elevation, link or launch token
//!    appears in code (the Stop gate's rules).
//!
//! **What this shell still decides** (SE-092): that a step's reads stop at the
//! first command whose read could not be obtained, and how a read-site error
//! maps onto `StepRead`. Both are sequencing; `crates/cli/tests/executor.rs`
//! tests them in place of a coverage floor.
//!
//! **Not offline.** A destination read that names a package can make one
//! provider fetch its catalogue data despite the quieting environment
//! (SE-050, "Not closed" 6). A verification read only reads, so this is a
//! network side effect and not a change to the machine; the mutating pass
//! inherits it as a recorded fact, and this build cannot reach it.

use macportsbrew_adapters::process::{Invocation, PlanReadRefusal};
use macportsbrew_adapters::registry::{
    plan_read_sites, PlanReadError, PlanReadSites, RegistryContext,
};
use macportsbrew_application::{
    execution_refusals, judge_step, next_action, outcome_of, refused, Action, Disposition,
    PlannedRun, RunOutcome, StepRead, StepRecord,
};
use macportsbrew_domain::{redact_home, HomeRedaction, PlanCommand, ProviderId};
use serde::Serialize;
use std::collections::BTreeMap;

/// Where a plan's command is read.
///
/// The production implementation is the adapters' read sites; a test double
/// answers from synthesised invocations. Either way this module never sees a
/// runner or a command spec.
pub trait ReadSite {
    /// Reads `command` from `provider`.
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError>;
}

impl ReadSite for PlanReadSites<'_> {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        self.run(provider, command)
    }
}

impl<T: ReadSite + ?Sized> ReadSite for &T {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        (**self).read(provider, command)
    }
}

/// A read site's refusal, as it may be shown or recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedRefusal {
    /// The step it stopped.
    pub step: u32,
    /// Which refusal it is: what a renderer keys its sentence on.
    pub refusal: SiteRefusalKind,
    /// The values the refusal quotes, by name, each cleaned like `text`.
    pub values: BTreeMap<&'static str, String>,
    /// The refusal, home redacted and control characters escaped.
    pub text: String,
}

/// A read site's refusal, by kind and without the plan text it quotes:
/// `PlanReadRefusal`'s variants, one for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SiteRefusalKind {
    /// The command does not name a provider.
    NotAProviderCommand,
    /// The detected prefix is not a safe path.
    UnsafePrefix,
    /// The plan's program is not the one derived from the detected prefix.
    ProgramDisagrees,
    /// The argv is not on the shipped read allowlist.
    NotAPermittedRead,
}

/// What a run produced. Holds no read output and no raw plan text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    /// How the run ended.
    pub outcome: RunOutcome,
    /// One per plan step when the gate passed the plan; empty when it refused.
    pub records: Vec<StepRecord>,
    /// The read site's refusal, when one stopped the run.
    pub refusal: Option<RenderedRefusal>,
    /// The gate's refusals in the gate's own order, each the refusal's
    /// document (its `refusal` tag and its fields) with every string in it
    /// cleaned like `refusal`'s text: a check's fields are plan text.
    pub gate_refusals: Vec<serde_json::Value>,
}

impl RunReport {
    /// The exit code the run reports.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        self.outcome.exit_code()
    }
}

/// Runs `planned`'s verification reads through the read sites `ctx` detects —
/// built only if the execution gate passes the plan. `home` is how any text
/// the report keeps is redacted.
pub fn run(planned: &PlannedRun, ctx: &RegistryContext<'_>, home: &HomeRedaction) -> RunReport {
    run_with(planned, || plan_read_sites(ctx), home)
}

/// [`run`], with the read sites from `sites`, which is called only after the
/// gate passed the plan and never otherwise.
pub fn run_with<S: ReadSite>(
    planned: &PlannedRun,
    sites: impl FnOnce() -> S,
    home: &HomeRedaction,
) -> RunReport {
    let plan = planned.plan();
    let gate = execution_refusals(plan);
    if let Some(outcome) = refused(&gate) {
        return RunReport {
            outcome,
            records: Vec::new(),
            refusal: None,
            gate_refusals: gate
                .refusals()
                .iter()
                .map(|r| clean_value(refusal_document(r), home))
                .collect(),
        };
    }
    let site = sites();
    let mut records: Vec<StepRecord> = Vec::new();
    let mut refusal = None;
    for (position, step) in plan.steps.iter().enumerate() {
        let disposition = match next_action(plan, position, &records) {
            Action::PassOver(disposition) => disposition,
            Action::Halt(reason) => Disposition::Halted {
                reason,
                evidence: Vec::new(),
            },
            Action::Dispatch => {
                let results = dispatch(step.provider.as_ref(), &step.commands, &site);
                if let Some(Err(PlanReadError::Refused(r))) = results.last() {
                    refusal = Some(rendered_refusal(step.index, r, home));
                }
                let reads: Vec<StepRead<'_>> = results.iter().map(step_read).collect();
                judge_step(planned, position, &reads)
            }
        };
        let halted = matches!(disposition, Disposition::Halted { .. });
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition,
        });
        if halted {
            break;
        }
    }
    for step in &plan.steps[records.len()..] {
        records.push(StepRecord {
            index: step.index,
            kind: step.kind,
            disposition: Disposition::NotReached,
        });
    }
    RunReport {
        outcome: outcome_of(plan, &records),
        records,
        refusal,
        gate_refusals: Vec::new(),
    }
}

/// Reads each command in order, stopping at the first that could not be read.
fn dispatch(
    provider: Option<&ProviderId>,
    commands: &[PlanCommand],
    site: &dyn ReadSite,
) -> Vec<Result<Invocation, PlanReadError>> {
    let mut results = Vec::new();
    for command in commands {
        let result = match provider {
            Some(provider) => site.read(provider, command),
            None => Err(PlanReadError::NotDetected),
        };
        let stop = result.is_err();
        results.push(result);
        if stop {
            break;
        }
    }
    results
}

fn step_read(result: &Result<Invocation, PlanReadError>) -> StepRead<'_> {
    match result {
        Ok(invocation) => StepRead::Output(invocation.read_output()),
        Err(PlanReadError::NotDetected) => StepRead::SiteUnavailable,
        Err(PlanReadError::Refused(_)) => StepRead::SiteRefused,
        Err(PlanReadError::Transport(error)) => StepRead::TransportFailed(error.id),
    }
}

/// A refusal's text with the home redacted from every value it quotes and
/// every control and format character escaped. The values are plan text,
/// verbatim.
#[must_use]
pub fn render_refusal(refusal: &PlanReadRefusal, home: &HomeRedaction) -> String {
    let clean = |value: &str| clean(value, home);
    match refusal {
        PlanReadRefusal::NotAProviderCommand => "NOT_A_PROVIDER_COMMAND".to_string(),
        PlanReadRefusal::UnsafePrefix { prefix } => {
            format!("UNSAFE_PREFIX prefix={}", clean(prefix))
        }
        PlanReadRefusal::ProgramDisagrees { planned, derived } => format!(
            "PROGRAM_DISAGREES planned={} derived={}",
            clean(planned),
            clean(derived)
        ),
        PlanReadRefusal::NotAPermittedRead { argv } => format!(
            "NOT_A_PERMITTED_READ argv=[{}]",
            argv.iter().map(|a| clean(a)).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// A refusal as the report keeps it: its stable identifier, the values it
/// quotes by name, and [`render_refusal`]'s text, every value cleaned.
fn rendered_refusal(step: u32, refusal: &PlanReadRefusal, home: &HomeRedaction) -> RenderedRefusal {
    let clean = |value: &str| clean(value, home);
    let (id, values): (SiteRefusalKind, Vec<(&'static str, String)>) = match refusal {
        PlanReadRefusal::NotAProviderCommand => (SiteRefusalKind::NotAProviderCommand, Vec::new()),
        PlanReadRefusal::UnsafePrefix { prefix } => (
            SiteRefusalKind::UnsafePrefix,
            vec![("prefix", clean(prefix))],
        ),
        PlanReadRefusal::ProgramDisagrees { planned, derived } => (
            SiteRefusalKind::ProgramDisagrees,
            vec![("planned", clean(planned)), ("derived", clean(derived))],
        ),
        PlanReadRefusal::NotAPermittedRead { argv } => (
            SiteRefusalKind::NotAPermittedRead,
            vec![(
                "argv",
                argv.iter().map(|a| clean(a)).collect::<Vec<_>>().join(" "),
            )],
        ),
    };
    RenderedRefusal {
        step,
        refusal: id,
        values: values.into_iter().collect(),
        text: render_refusal(refusal, home),
    }
}

/// Plan text as it may be kept: the home redacted, then every control and
/// format character escaped.
fn clean(value: &str, home: &HomeRedaction) -> String {
    escape_controls(&redact_home(value, home))
}

/// A gate refusal as a document. Serialising a derived enum of numbers,
/// strings and lists cannot fail; if it ever did, the refusal is kept as
/// `UNSERIALISABLE` rather than dropped or turned into `null`, so the list
/// still has one entry per refusal and says which one it could not show.
fn refusal_document(refusal: &macportsbrew_application::ExecutionRefusal) -> serde_json::Value {
    match serde_json::to_value(refusal) {
        Ok(value @ serde_json::Value::Object(_)) => value,
        _ => serde_json::json!({ "refusal": "UNSERIALISABLE" }),
    }
}

/// [`clean`], applied to every string a document holds, at any depth.
fn clean_value(value: serde_json::Value, home: &HomeRedaction) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::String(s) => Value::String(clean(&s, home)),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(|v| clean_value(v, home)).collect())
        }
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(k, v)| (k, clean_value(v, home)))
                .collect(),
        ),
        other => other,
    }
}

/// Escapes every control character, every Unicode format character
/// (category Cf) and the two Unicode line and paragraph separators (U+2028,
/// U+2029, categories Zl and Zp), so a value cannot move a cursor, clear or
/// break a line, start a terminal sequence, or reorder or hide what is shown
/// around it (U+202E and the other bidirectional overrides, zero-width
/// characters).
pub(crate) fn escape_controls(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() || is_format(c) || is_line_separator(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// Whether `c` is U+2028 LINE SEPARATOR or U+2029 PARAGRAPH SEPARATOR: not
/// controls and not Cf, but a renderer that honours them breaks the line, so a
/// value could start a line of its own.
fn is_line_separator(c: char) -> bool {
    // Decimal, deliberately: the hexadecimal spelling of these code points
    // reads as a year to INV-20's deadline scan, and an exemption there would
    // be worse than an unfamiliar radix here. 8232 is U+2028, 8233 is U+2029.
    matches!(u32::from(c), 8232 | 8233)
}

/// Whether `c` is in Unicode general category Cf (Format), Unicode 16.0.
fn is_format(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x0600..=0x0605
            | 0x061C
            | 0x06DD
            | 0x070F
            | 0x0890..=0x0891
            | 0x08E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_refusal_is_rendered_with_the_home_redacted_and_controls_escaped() {
        let home = HomeRedaction::Known(PathBuf::from("/Users/ada"));
        let text = render_refusal(
            &PlanReadRefusal::ProgramDisagrees {
                planned: "/Users/ada/evil/bin/x\u{1b}[2J\n".into(),
                derived: "/opt/somewhere/bin/x".into(),
            },
            &home,
        );
        assert!(!text.contains("/Users/ada"), "{text}");
        assert!(!text.contains("ada"), "{text}");
        assert!(!text.chars().any(char::is_control), "{text:?}");
        assert!(
            text.contains("\\u{1b}") && text.contains("\\u{a}"),
            "{text}"
        );
        let argv = render_refusal(
            &PlanReadRefusal::NotAPermittedRead {
                argv: vec!["--x=/Users/ada/f\r".into()],
            },
            &home,
        );
        assert!(
            !argv.contains("ada") && !argv.chars().any(char::is_control),
            "{argv:?}"
        );
        let prefix = render_refusal(
            &PlanReadRefusal::UnsafePrefix {
                prefix: "/Users/ada\t/p".into(),
            },
            &home,
        );
        assert!(
            !prefix.contains("ada") && !prefix.chars().any(char::is_control),
            "{prefix:?}"
        );
    }

    #[test]
    fn a_format_character_cannot_reorder_or_hide_what_is_shown() {
        // U+202E would render the rest of the line right to left, so a path
        // could display as another; the others hide or isolate text.
        let home = HomeRedaction::Recorded;
        for c in [
            '\u{202E}',
            '\u{2066}',
            '\u{2069}',
            '\u{200B}',
            '\u{FEFF}',
            '\u{E0041}',
            '\u{2028}',
            '\u{2029}',
        ] {
            let text = render_refusal(
                &PlanReadRefusal::UnsafePrefix {
                    prefix: format!("/opt/a{c}b"),
                },
                &home,
            );
            assert!(!text.contains(c), "{c:?} survived: {text:?}");
            assert!(text.contains(&format!("{}", c.escape_unicode())), "{text}");
        }
        // Ordinary text is left alone.
        assert_eq!(escape_controls("/opt/local/é"), "/opt/local/é");
    }
}
