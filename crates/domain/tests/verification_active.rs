// SPDX-License-Identifier: Apache-2.0

//! How `PROVIDER_REPORTS_ACTIVE`'s reads are judged
//! (`crates/domain/src/verification.rs`; work order 16 part S, §2).
//!
//! The plain-text shapes are R-20's: an active row is
//! `  <name> @<version> (active)` (observed, E3), and the not-active sibling is
//! `  <name> @<version>`, two tokens — **documentation-derived** (read from the
//! source, R-20 finding 7; no inactive image exists on the machine R-20 read,
//! finding 3). The JSON half is parsed outside this crate; here only the rule
//! that consumes its answer (`LinkedReport`) is judged.

use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
use macportsbrew_domain::{
    evaluate, evaluate_with, expectation_for, CommandTarget, Elevation, ExpectationError,
    FailureCause, LinkedReport, MigrationPlan, MigrationStep, Outcome, PackageIdentity,
    PlanCommand, ProcessEnd, ProviderId, ReadOutput, Reason, StepKind, SuccessSemantic, Verdict,
};
use std::collections::BTreeMap;

const SOURCE: &str = "src-provider";
const DESTINATION: &str = "dst-provider";

fn cmd(success: SuccessSemantic, argv: &[&str]) -> PlanCommand {
    PlanCommand {
        target: CommandTarget::Provider,
        program: "/p/bin/tool".into(),
        argv: argv.iter().map(|a| (*a).to_string()).collect(),
        environment: BTreeMap::new(),
        elevation: Elevation::None,
        placeholder: false,
        success,
    }
}

fn step(index: u32, kind: StepKind, provider: &str, command: PlanCommand) -> MigrationStep {
    MigrationStep {
        index,
        kind,
        provider: Some(ProviderId::new(provider)),
        commands: vec![command],
        verification: vec![],
        reason: Reason::new("test"),
    }
}

/// A plan of `source_name` to `destination_name` whose step 0 is a
/// `VERIFY_DESTINATION` carrying `command` and step 1 a `VERIFY_REMOVAL`
/// carrying the same command.
fn plan(source_name: &str, destination_name: &str, command: &PlanCommand) -> MigrationPlan {
    MigrationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        package: PackageIdentity::new(ProviderId::new(SOURCE), source_name),
        source: ProviderId::new(SOURCE),
        destination: ProviderId::new(DESTINATION),
        destination_package: Some(destination_name.into()),
        retain_source: false,
        entered_by: Some(macportsbrew_domain::EntryMode::DefaultView),
        primary_after: ProviderId::new(DESTINATION),
        recommendation: Outcome::MigrateTo {
            provider: ProviderId::new(DESTINATION),
        },
        overrode: None,
        steps: vec![
            step(1, StepKind::VerifyDestination, DESTINATION, command.clone()),
            step(2, StepKind::VerifyRemoval, SOURCE, command.clone()),
        ],
        risks: vec![],
        elevated_steps: vec![],
        policy_version: "0".into(),
        dry_run_only: true,
    }
}

fn exited(code: i32, stdout: &str) -> ReadOutput<'_> {
    ReadOutput {
        end: ProcessEnd::Exited { code },
        stdout,
        truncated: false,
    }
}

fn active_cmd() -> PlanCommand {
    cmd(
        SuccessSemantic::StdoutMarksPackageActive,
        &["-q", "inventory"],
    )
}

fn linked_cmd() -> PlanCommand {
    cmd(
        SuccessSemantic::JsonReportsPackageLinked,
        &["inventory", "--json"],
    )
}

/// The verdict of the marked-shape read for `name` over `stdout`.
fn active(name: &str, stdout: &str) -> Verdict {
    let p = plan("src-name", name, &active_cmd());
    let e = expectation_for(&p, 0, 0).expect("an active check");
    evaluate(&e, &exited(0, stdout))
}

const FAILED: Verdict = Verdict::Failed(FailureCause::ActiveNotReported);

// --- the marked shape ------------------------------------------------------

#[test]
fn active_marker_must_be_on_the_named_line() {
    let inventory = "  xz-bootstrap @5.8.3_1 (active)\n  ghidra @11.0_0 (active)\n";
    assert_eq!(
        active("xz", inventory),
        FAILED,
        "xz is not answered by xz-bootstrap"
    );
    assert_eq!(
        active("gh", inventory),
        FAILED,
        "gh is not answered by ghidra"
    );
    assert_eq!(
        active(
            "xz",
            "  xz @5.8.3_0 (active)\n  xz-bootstrap @5.8.3_1 (active)\n"
        ),
        Verdict::Passed
    );
}

#[test]
fn the_name_is_compared_without_case_folding() {
    // The registry's own spelling decides (R-20 finding 6): a plan whose
    // destination differs in case halts rather than matching (L7).
    assert_eq!(active("XZ", "  xz @5.8.3_0 (active)\n"), FAILED);
}

#[test]
fn an_inactive_sibling_is_ignored_and_a_lone_inactive_row_is_not_active() {
    // Documentation-derived: the two-token not-active shape is read from the
    // source (R-20 finding 7), never observed (finding 3).
    assert_eq!(
        active("xz", "  xz @5.8.2_0\n  xz @5.8.3_0 (active)\n"),
        Verdict::Passed
    );
    assert_eq!(
        active("xz", "  xz @5.8.2_0\n  jq @1.8.2_0 (active)\n"),
        FAILED
    );
}

#[test]
fn a_verbose_row_is_not_active() {
    assert_eq!(
        active("xz", "  xz @5.8.3_0 (active) requested_variants=''\n"),
        FAILED
    );
}

#[test]
fn a_second_token_without_at_is_not_active() {
    assert_eq!(active("xz", "  xz 5.8.3_0 (active)\n"), FAILED);
}

#[test]
fn any_row_of_the_name_in_an_unknown_shape_fails_even_beside_an_active_row() {
    let good = "  xz @5.8.3_0 (active)\n";
    for odd in [
        "  xz\n",
        "  xz @5.8.2_0 (active) extra\n",
        "  xz @5.8.2_0 (inactive)\n",
        "  xz 5.8.2_0\n",
    ] {
        assert_eq!(active("xz", &format!("{good}{odd}")), FAILED, "{odd:?}");
        assert_eq!(active("xz", &format!("{odd}{good}")), FAILED, "{odd:?}");
    }
}

#[test]
fn two_active_rows_for_one_name_are_not_active() {
    assert_eq!(
        active("xz", "  xz @5.8.2_0 (active)\n  xz @5.8.3_0 (active)\n"),
        FAILED
    );
}

#[test]
fn active_read_fails_closed() {
    let p = plan("src-name", "xz", &active_cmd());
    let e = expectation_for(&p, 0, 0).expect("an active check");
    let good = "  xz @5.8.3_0 (active)\n";
    assert_eq!(
        evaluate(
            &e,
            &ReadOutput {
                end: ProcessEnd::DidNotFinish,
                stdout: good,
                truncated: false
            }
        ),
        Verdict::Failed(FailureCause::DidNotFinish)
    );
    assert_eq!(
        evaluate(
            &e,
            &ReadOutput {
                end: ProcessEnd::Exited { code: 0 },
                stdout: good,
                truncated: true
            }
        ),
        Verdict::Failed(FailureCause::Truncated)
    );
    assert_eq!(
        evaluate(&e, &exited(1, good)),
        Verdict::Failed(FailureCause::NonZeroExit)
    );
    assert_eq!(
        evaluate(&e, &exited(0, "  \n")),
        Verdict::Failed(FailureCause::EmptyOutput)
    );
    // The same four for the JSON semantic, with a parse that would pass.
    let p = plan("src-name", "xz", &linked_cmd());
    let e = expectation_for(&p, 0, 0).expect("a linked check");
    let yes = |_: &str, _: &str| LinkedReport::Linked;
    for (output, cause) in [
        (
            ReadOutput {
                end: ProcessEnd::DidNotFinish,
                stdout: "{}",
                truncated: false,
            },
            FailureCause::DidNotFinish,
        ),
        (
            ReadOutput {
                end: ProcessEnd::Exited { code: 0 },
                stdout: "{}",
                truncated: true,
            },
            FailureCause::Truncated,
        ),
        (exited(1, "{}"), FailureCause::NonZeroExit),
        (exited(0, ""), FailureCause::EmptyOutput),
    ] {
        assert_eq!(evaluate_with(&e, &output, &yes), Verdict::Failed(cause));
    }
}

// --- where the semantics may appear ----------------------------------------

#[test]
fn a_verify_active_read_that_names_a_package_has_no_expectation() {
    for success in [
        SuccessSemantic::StdoutMarksPackageActive,
        SuccessSemantic::JsonReportsPackageLinked,
    ] {
        for named in ["xz", "src-name"] {
            let p = plan("src-name", "xz", &cmd(success, &["-q", "inventory", named]));
            assert_eq!(
                expectation_for(&p, 0, 0),
                Err(ExpectationError::ArgvNamesAPackage),
                "{success:?} {named}"
            );
        }
    }
}

#[test]
fn the_new_semantics_are_valid_on_verify_destination_only() {
    for command in [active_cmd(), linked_cmd()] {
        let p = plan("src-name", "xz", &command);
        assert!(expectation_for(&p, 0, 0).is_ok());
        assert_eq!(
            expectation_for(&p, 1, 0),
            Err(ExpectationError::SemanticNotValidForStep),
            "{:?} on VERIFY_REMOVAL",
            command.success
        );
    }
}

#[test]
fn the_parserless_evaluate_never_passes_a_linked_read() {
    let p = plan("src-name", "xz", &linked_cmd());
    let e = expectation_for(&p, 0, 0).expect("a linked check");
    assert_eq!(evaluate(&e, &exited(0, "{\"formulae\":[]}")), FAILED);
    for report in [
        LinkedReport::NotLinked,
        LinkedReport::Ambiguous,
        LinkedReport::Unparseable,
    ] {
        assert_eq!(
            evaluate_with(&e, &exited(0, "{}"), &|_, _| report),
            FAILED,
            "{report:?}"
        );
    }
    assert_eq!(
        evaluate_with(&e, &exited(0, "{}"), &|_, name| {
            if name == "xz" {
                LinkedReport::Linked
            } else {
                LinkedReport::NotLinked
            }
        }),
        Verdict::Passed,
        "the parse is asked about the destination name"
    );
}

// --- INV-17 ----------------------------------------------------------------

/// The same four cases through each provider's semantic give the same
/// verdicts: one marked; one marked beside an unmarked sibling; none; two
/// marked. For the JSON semantic the parse's answer stands for the document
/// (the application's parser is tested on documents beside it).
#[test]
fn invariant_17_exactly_one_record_marked_active_is_the_rule_for_both_providers() {
    let cases: [(&str, LinkedReport, Verdict); 4] = [
        (
            "  xz @5.8.3_0 (active)\n",
            LinkedReport::Linked,
            Verdict::Passed,
        ),
        (
            "  xz @5.8.2_0\n  xz @5.8.3_0 (active)\n",
            LinkedReport::Linked,
            Verdict::Passed,
        ),
        ("  xz @5.8.3_0\n", LinkedReport::NotLinked, FAILED),
        (
            "  xz @5.8.2_0 (active)\n  xz @5.8.3_0 (active)\n",
            LinkedReport::Ambiguous,
            FAILED,
        ),
    ];
    let lp = plan("src-name", "xz", &linked_cmd());
    let le = expectation_for(&lp, 0, 0).expect("a linked check");
    for (text, report, expected) in cases {
        assert_eq!(active("xz", text), expected, "text: {text:?}");
        assert_eq!(
            evaluate_with(&le, &exited(0, "{}"), &|_, _| report),
            expected,
            "json: {report:?}"
        );
    }
}
