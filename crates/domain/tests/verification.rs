// SPDX-License-Identifier: Apache-2.0

//! How a verification read is judged (`crates/domain/src/verification.rs`).
//!
//! Every rule here was proven by mutation when it was written: break the rule,
//! watch the named test fail, restore it. The shapes are the observed ones
//! (`data/policy/provider-commands.toml`, 2026-09-19): an inventory line is a
//! name followed by versions, and one provider indents it by two spaces. The
//! names are real ones: `python@3.13` against `python313` for the two
//! spellings of one package, `gh` against `ghidra` for the measured substring
//! collision (F-51).

use macportsbrew_domain::plan::PLAN_SCHEMA_VERSION;
use macportsbrew_domain::{
    evaluate, expectation_for, stdout_names, CommandTarget, Elevation, Expectation,
    ExpectationError, FailureCause, MigrationPlan, MigrationStep, Outcome, PackageIdentity,
    PlanCommand, ProcessEnd, ProviderId, ReadEvidence, ReadOutput, Reason, StepKind,
    SuccessSemantic, Verdict, VerificationCheck,
};
use std::collections::BTreeMap;

const SOURCE: &str = "src-provider";
const DESTINATION: &str = "dst-provider";

/// A plan moving `source_name` from the source provider to `destination_name`
/// under the destination, with the two verification steps and nothing else.
/// Step 0 carries a name-in-stdout query, step 1 an inventory query.
fn plan(source_name: &str, destination_name: Option<&str>) -> MigrationPlan {
    MigrationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        package: PackageIdentity::new(ProviderId::new(SOURCE), source_name),
        source: ProviderId::new(SOURCE),
        destination: ProviderId::new(DESTINATION),
        destination_package: destination_name.map(Into::into),
        retain_source: false,
        entered_by: Some(macportsbrew_domain::EntryMode::DefaultView),
        primary_after: ProviderId::new(DESTINATION),
        recommendation: Outcome::MigrateTo {
            provider: ProviderId::new(DESTINATION),
        },
        overrode: None,
        steps: vec![
            MigrationStep {
                commands: vec![cmd(SuccessSemantic::StdoutNamesPackage, &["query"])],
                ..step(1, StepKind::VerifyDestination, DESTINATION)
            },
            MigrationStep {
                commands: vec![cmd(SuccessSemantic::StdoutOmitsPackage, &["query"])],
                ..step(2, StepKind::VerifyRemoval, SOURCE)
            },
        ],
        risks: vec![],
        elevated_steps: vec![],
        policy_version: "0".into(),
        dry_run_only: true,
    }
}

fn step(index: u32, kind: StepKind, provider: &str) -> MigrationStep {
    MigrationStep {
        index,
        kind,
        provider: Some(ProviderId::new(provider)),
        commands: vec![],
        verification: vec![],
        reason: Reason::new("test"),
    }
}

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

fn exited(code: i32, stdout: &str) -> ReadOutput<'_> {
    ReadOutput {
        end: ProcessEnd::Exited { code },
        stdout,
        truncated: false,
    }
}

/// A copy of `p` whose step `i` carries `c` as its one command, so that the
/// command is judged from inside the plan — the only way one reaches a
/// judgement (`expectation_for` takes positions, not references).
fn armed(p: &MigrationPlan, i: usize, c: &PlanCommand) -> MigrationPlan {
    let mut armed = p.clone();
    armed.steps[i].commands = vec![c.clone()];
    armed
}

/// The destination check of `p`: its own step 0, over the name-in-stdout
/// semantic.
fn names(p: &MigrationPlan) -> Expectation<'_> {
    expectation_for(p, 0, 0).expect("a destination check")
}

/// The removal check of `p`: its own step 1, over the inventory semantic.
fn omits(p: &MigrationPlan) -> Expectation<'_> {
    expectation_for(p, 1, 0).expect("a removal check")
}

/// An inventory that does not hold `python313`, 3 lines, well-formed.
const INVENTORY_WITHOUT: &str = "autoconf 2.73\nnode 26.7.0\nzlib 1.3.1\n";

// --- VERIFY_REMOVAL: the four conditions, one mutation each ----------------

#[test]
fn invariant_01_a_removal_needs_a_clean_exit() {
    // A provider that failed may still have printed an inventory that happens
    // not to name the package. Condition 1.
    let p = plan("python313", Some("python@3.13"));
    let e = omits(&p);
    assert_eq!(evaluate(&e, &exited(0, INVENTORY_WITHOUT)), Verdict::Passed);
    assert_eq!(
        evaluate(&e, &exited(1, INVENTORY_WITHOUT)),
        Verdict::Failed(FailureCause::NonZeroExit)
    );
}

#[test]
fn invariant_01_a_removal_needs_an_untruncated_inventory() {
    // An inventory cut at line 40 of 201 is non-empty, omits the name, and
    // would certify a removal it never saw. Condition 2.
    let p = plan("python313", Some("python@3.13"));
    let e = omits(&p);
    let cut = ReadOutput {
        truncated: true,
        ..exited(0, INVENTORY_WITHOUT)
    };
    assert_eq!(evaluate(&e, &cut), Verdict::Failed(FailureCause::Truncated));
}

#[test]
fn invariant_01_a_removal_needs_a_non_empty_inventory() {
    // Removed, never-a-package and could-not-run all answer with nothing.
    // Condition 3; whitespace is nothing too.
    let p = plan("python313", Some("python@3.13"));
    let e = omits(&p);
    for empty in ["", "\n", "  \n\t\n"] {
        assert_eq!(
            evaluate(&e, &exited(0, empty)),
            Verdict::Failed(FailureCause::EmptyOutput),
            "{empty:?}"
        );
    }
}

#[test]
fn a_removal_fails_while_the_name_is_a_first_token() {
    let p = plan("python313", Some("python@3.13"));
    let e = omits(&p);
    let still_there = "autoconf 2.73\npython313 3.13.7\nzlib 1.3.1\n";
    assert_eq!(
        evaluate(&e, &exited(0, still_there)),
        Verdict::Failed(FailureCause::NamePresent)
    );
}

#[test]
fn a_removal_is_not_blocked_by_a_substring_collision() {
    // `gh` removed while `ghidra` remains: the removal happened.
    let p = plan("gh", Some("gh"));
    let e = omits(&p);
    assert_eq!(
        evaluate(&e, &exited(0, "ghidra 11.4\nhighway 1.2.0\n")),
        Verdict::Passed
    );
}

// --- VERIFY_DESTINATION ------------------------------------------------------

#[test]
fn invariant_01_a_destination_is_not_certified_by_a_substring_collision() {
    // F-51: a failed install of `gh` must not pass on the line for `ghidra`,
    // and it is this step INV-01 puts in front of REMOVE_SOURCE.
    let p = plan("gh", Some("gh"));
    let e = names(&p);
    assert_eq!(
        evaluate(&e, &exited(0, "ghidra 11.4\nlibnghttp2 1.64.0\n")),
        Verdict::Failed(FailureCause::NameAbsent)
    );
    assert_eq!(
        evaluate(&e, &exited(0, "gh 2.80.0\nghidra 11.4\n")),
        Verdict::Passed
    );
}

#[test]
fn a_destination_check_fails_on_empty_stdout_whatever_the_exit() {
    // The semantic exists because the exit status says nothing here.
    let p = plan("python313", Some("python@3.13"));
    let e = names(&p);
    for code in [0, 1] {
        assert_eq!(
            evaluate(&e, &exited(code, "")),
            Verdict::Failed(FailureCause::EmptyOutput),
            "exit {code}"
        );
    }
    // And, the same rule read the other way: a nonzero exit that names it is
    // not a failure of this semantic.
    assert_eq!(
        evaluate(&e, &exited(1, "python@3.13 3.13.7\n")),
        Verdict::Passed
    );
}

#[test]
fn a_truncated_line_cannot_manufacture_the_name() {
    // `ghidra 11.4` cut after two bytes reads `gh`.
    let p = plan("gh", Some("gh"));
    let e = names(&p);
    let cut = ReadOutput {
        end: ProcessEnd::Exited { code: 0 },
        stdout: "gh",
        truncated: true,
    };
    assert_eq!(evaluate(&e, &cut), Verdict::Failed(FailureCause::Truncated));
}

#[test]
fn a_process_that_did_not_finish_certifies_nothing() {
    let p = plan("python313", Some("python@3.13"));
    let by_exit = armed(
        &p,
        0,
        &cmd(SuccessSemantic::ExitStatus, &["query", "python@3.13"]),
    );
    let exit_zero = expectation_for(&by_exit, 0, 0).unwrap();
    for (e, stdout) in [
        (names(&p), "python@3.13 3.13.7\n"),
        (omits(&p), INVENTORY_WITHOUT),
        (exit_zero, ""),
    ] {
        let killed = ReadOutput {
            end: ProcessEnd::DidNotFinish,
            stdout,
            truncated: false,
        };
        assert_eq!(
            evaluate(&e, &killed),
            Verdict::Failed(FailureCause::DidNotFinish),
            "{e:?}"
        );
    }
}

#[test]
fn a_leading_indent_is_read_as_whitespace() {
    // Observed 2026-09-19: `  autoconf @2.73_0 (active)`.
    let indented = "  autoconf @2.73_0 (active)\n  python313 @3.13.7_0 (active)\n";
    assert!(stdout_names(indented, "python313"));
    assert!(stdout_names(indented, "autoconf"));
    assert!(!stdout_names(indented, "@2.73_0"));
    assert!(!stdout_names(indented, "(active)"));
}

#[test]
fn names_with_dots_and_pluses_are_compared_literally() {
    // Compiled as a pattern, `.` matches any character and `+` repeats.
    assert!(stdout_names("lua@5.4 5.4.7\n", "lua@5.4"));
    assert!(!stdout_names("lua@5x4 5.4.7\n", "lua@5.4"));
    assert!(stdout_names("gtk+3 3.24.49\n", "gtk+3"));
    assert!(!stdout_names("gtkk3 3.24.49\n", "gtk+3"));
    assert!(!stdout_names("gtk3 3.24.49\n", "gtk+3"));
    // And no line is named by the empty name.
    assert!(!stdout_names("lua@5.4 5.4.7\n", ""));
}

// --- Which name, which provider ----------------------------------------------

/// INV-07's defence at verification: a check against one provider's inventory
/// reads that provider's spelling of the package and never the other's, so a
/// destination is never certified by the source's name (nor a removal by the
/// destination's) — which is how a cross-provider coupling would enter through
/// the step INV-01 puts in front of `REMOVE_SOURCE`.
#[test]
fn invariant_07_a_check_reads_only_its_own_providers_name() {
    a_destination_check_reads_the_destination_name_never_the_source();
    a_removal_check_reads_the_source_name_never_the_destination();
}

#[test]
fn a_destination_check_reads_the_destination_name_never_the_source() {
    // One package, two spellings. A destination inventory lists the
    // destination's spelling; reading the source's would find it only in the
    // source provider's own listing.
    let p = plan("python313", Some("python@3.13"));
    let e = names(&p);
    assert_eq!(e.semantic(), SuccessSemantic::StdoutNamesPackage);
    assert_eq!(e.destination(), p.destination_name());
    assert_eq!(e.destination().unwrap().as_str(), "python@3.13");
    assert_eq!(e.source(), None);
    assert_eq!(
        evaluate(&e, &exited(0, "python313 3.13.7\n")),
        Verdict::Failed(FailureCause::NameAbsent)
    );
    assert_eq!(
        evaluate(&e, &exited(0, "python@3.13 3.13.7\n")),
        Verdict::Passed
    );
}

#[test]
fn a_removal_check_reads_the_source_name_never_the_destination() {
    let p = plan("python313", Some("python@3.13"));
    let e = omits(&p);
    assert_eq!(e.semantic(), SuccessSemantic::StdoutOmitsPackage);
    assert_eq!(e.source(), Some(p.source_name()));
    assert_eq!(e.source().unwrap().as_str(), "python313");
    assert_eq!(e.destination(), None);
    // The source still lists its own spelling: not removed.
    assert_eq!(
        evaluate(&e, &exited(0, "  python313 @3.13.7_0 (active)\n")),
        Verdict::Failed(FailureCause::NamePresent)
    );
    // The destination's spelling in a source inventory proves nothing either
    // way, and it does not block a genuine removal.
    assert_eq!(
        evaluate(&e, &exited(0, "  python@3.13 @3.13.7_0 (active)\n")),
        Verdict::Passed
    );
}

#[test]
fn a_destination_check_without_a_destination_name_is_refused_not_defaulted() {
    for missing in [None, Some("")] {
        let p = plan("python313", missing);
        for success in [
            SuccessSemantic::StdoutNamesPackage,
            SuccessSemantic::ExitStatus,
        ] {
            assert_eq!(
                expectation_for(&armed(&p, 0, &cmd(success, &["query", "python313"])), 0, 0),
                Err(ExpectationError::NoDestinationName),
                "{missing:?} {success:?}"
            );
        }
    }
}

#[test]
fn an_exit_status_check_names_the_destination_in_argv() {
    // F7: exit 0 answers whatever the argv asked, so the argv must have asked
    // about the destination package — as its last element, which is where the
    // planner appends it. Anywhere else it may be the value of a flag.
    let p = plan("python313", Some("python@3.13"));
    let about = |argv: &[&str]| armed(&p, 0, &cmd(SuccessSemantic::ExitStatus, argv));
    let last = about(&["query", "python@3.13"]);
    let e = expectation_for(&last, 0, 0).expect("names it last");
    assert_eq!(e.semantic(), SuccessSemantic::ExitStatus);
    assert_eq!(e.destination(), p.destination_name());
    for wrong in [
        &["query"][..],
        &["query", "python313"][..],
        &["query", "python@3.1"][..],
        &["python@3.13", "query"][..],
        &["query", "python@3.13", "python313"][..],
    ] {
        assert_eq!(
            expectation_for(&about(wrong), 0, 0),
            Err(ExpectationError::ArgvDoesNotNameDestination),
            "{wrong:?}"
        );
    }
    assert_eq!(evaluate(&e, &exited(0, "")), Verdict::Passed);
    assert_eq!(
        evaluate(&e, &exited(1, "")),
        Verdict::Failed(FailureCause::NonZeroExit)
    );
    // It reads no stdout, but a read that was cut still vouches for nothing:
    // the process may have been stopped at the cap before it exited on its own.
    let cut = ReadOutput {
        truncated: true,
        ..exited(0, "python@3.13 3.13.7")
    };
    assert_eq!(evaluate(&e, &cut), Verdict::Failed(FailureCause::Truncated));
}

#[test]
fn a_removal_command_that_names_a_package_is_refused() {
    // Asked about one name, a provider gives the same empty answer for
    // removed, never-a-package and could-not-run (obligation 3).
    let p = plan("python313", Some("python@3.13"));
    for argv in [&["query", "python313"][..], &["query", "python@3.13"][..]] {
        assert_eq!(
            expectation_for(
                &armed(&p, 1, &cmd(SuccessSemantic::StdoutOmitsPackage, argv)),
                1,
                0
            ),
            Err(ExpectationError::ArgvNamesAPackage),
            "{argv:?}"
        );
    }
}

#[test]
fn invariant_08_a_destination_check_is_never_run_against_the_source_provider() {
    // The package is held by both providers, so the source provider answers
    // "yes, installed" to a destination check whatever the install did. A
    // destination check addressed to it would pass a failed install and clear
    // the gate in front of REMOVE_SOURCE.
    let mut p = plan("python313", Some("python313"));
    let check = cmd(SuccessSemantic::StdoutNamesPackage, &["query"]);
    let dual_held_answer = exited(0, "  python313 @3.13.7_0 (active)\n");

    p.steps[0].provider = Some(ProviderId::new(SOURCE));
    assert_eq!(
        expectation_for(&armed(&p, 0, &check), 0, 0),
        Err(ExpectationError::WrongProvider)
    );
    // Nor to no provider at all.
    p.steps[0].provider = None;
    assert_eq!(
        expectation_for(&armed(&p, 0, &check), 0, 0),
        Err(ExpectationError::WrongProvider)
    );
    // And the mirror: a removal is read from the source only.
    p.steps[1].provider = Some(ProviderId::new(DESTINATION));
    assert_eq!(
        expectation_for(
            &armed(&p, 1, &cmd(SuccessSemantic::StdoutOmitsPackage, &["query"])),
            1,
            0
        ),
        Err(ExpectationError::WrongProvider)
    );

    // Addressed correctly it is judged as usual.
    p.steps[0].provider = Some(ProviderId::new(DESTINATION));
    let right = armed(&p, 0, &check);
    let e = expectation_for(&right, 0, 0).unwrap();
    assert_eq!(evaluate(&e, &dual_held_answer), Verdict::Passed);
}

#[test]
fn a_step_or_command_the_plan_does_not_hold_is_refused() {
    // The step and the command are addressed by position in this plan, so
    // neither can be written by hand. A step with public fields, of the right
    // kind and provider and no `verification`, used to clear the
    // unrecognised-check refusal and to make "the command is the step's own" a
    // check against the caller's own step (spec-guardian).
    let p = plan("python313", Some("python@3.13"));
    for beyond in [p.steps.len(), usize::MAX] {
        assert_eq!(
            expectation_for(&p, beyond, 0),
            Err(ExpectationError::StepNotInPlan),
            "{beyond}"
        );
    }
    // A step with no command at the position asked for: past the end of a
    // step that carries one, and at the start of a step that carries none.
    assert!(expectation_for(&p, 0, 0).is_ok());
    assert_eq!(
        expectation_for(&p, 0, 1),
        Err(ExpectationError::CommandNotInStep)
    );
    let mut bare = p.clone();
    bare.steps[0].commands.clear();
    assert_eq!(
        expectation_for(&bare, 0, 0),
        Err(ExpectationError::CommandNotInStep)
    );
    assert_eq!(
        expectation_for(&bare, 0, 1),
        Err(ExpectationError::CommandNotInStep)
    );
}

#[test]
fn only_a_verification_step_with_a_matching_semantic_is_judged() {
    let p = plan("python313", Some("python@3.13"));
    let exit = cmd(SuccessSemantic::ExitStatus, &["query", "python@3.13"]);
    let mut with_add = p.clone();
    with_add.steps[0] = step(1, StepKind::AddDestination, DESTINATION);
    assert_eq!(
        expectation_for(&armed(&with_add, 0, &exit), 0, 0),
        Err(ExpectationError::NotAVerificationStep)
    );
    assert_eq!(
        expectation_for(
            &armed(&p, 0, &cmd(SuccessSemantic::Unknown, &["query"])),
            0,
            0
        ),
        Err(ExpectationError::SuccessUnknown)
    );
    // A removal certified by exit status is the sign-flipped check F-06 and
    // F-22 warned about; a destination certified by absence is nonsense.
    for (s, success) in [
        (1, SuccessSemantic::ExitStatus),
        (1, SuccessSemantic::StdoutNamesPackage),
        (0, SuccessSemantic::StdoutOmitsPackage),
    ] {
        assert_eq!(
            expectation_for(&armed(&p, s, &cmd(success, &["query"])), s, 0),
            Err(ExpectationError::SemanticNotValidForStep),
            "{:?} {success:?}",
            p.steps[s].kind
        );
    }
}

// --- What may be recorded ----------------------------------------------------

#[test]
fn evidence_carries_no_stdout() {
    const SENTINEL: &str = "internal-tool-sentinel-7f3a";
    let p = plan("python313", Some("python@3.13"));
    let e = omits(&p);
    let stdout = format!("autoconf 2.73\n{SENTINEL} 1.0\nzlib 1.3.1\n");
    let output = exited(0, &stdout);
    let evidence = ReadEvidence::of(&output, evaluate(&e, &output));
    assert!(evidence.passed());
    assert_eq!(evidence.line_count(), 3);
    assert!(!evidence.truncated());
    assert_eq!(evidence.cause(), None);
    let json = serde_json::to_string(&evidence).unwrap();
    let debug = format!("{evidence:?}");
    for text in [&json, &debug] {
        assert!(!text.contains(SENTINEL), "{text}");
        assert!(!text.contains("autoconf"), "{text}");
    }

    let failed = ReadEvidence::of(&output, Verdict::Failed(FailureCause::NamePresent));
    assert!(!failed.passed());
    assert_eq!(failed.cause(), Some(FailureCause::NamePresent));
    assert!(serde_json::to_string(&failed)
        .unwrap()
        .contains("NAME_PRESENT"));
}

// --- What can never be judged ------------------------------------------------

#[test]
fn invariant_01_a_removal_never_reads_an_empty_or_unsafe_source_name() {
    // An empty or whitespace name is no line's first token, so a removal check
    // reading it passes on any inventory at all: the check certifies a removal
    // it never looked for. The first four are that hole; the rest are names no
    // adapter would ever have accepted from a provider.
    for bad in ["", " ", "\t", "python 313", "-rf", "..", "python313\n"] {
        let p = plan(bad, Some("python@3.13"));
        assert_eq!(
            expectation_for(
                &armed(&p, 1, &cmd(SuccessSemantic::StdoutOmitsPackage, &["query"])),
                1,
                0
            ),
            Err(ExpectationError::UnsafePackageName),
            "{bad:?}"
        );
    }
}

#[test]
fn invariant_01_a_destination_never_reads_an_unsafe_name() {
    for bad in [" ", "python 3.13", "-x", ".."] {
        let p = plan("python313", Some(bad));
        for (success, argv) in [
            (SuccessSemantic::StdoutNamesPackage, vec!["query"]),
            (SuccessSemantic::ExitStatus, vec!["query", bad]),
        ] {
            assert_eq!(
                expectation_for(&armed(&p, 0, &cmd(success, &argv)), 0, 0),
                Err(ExpectationError::UnsafePackageName),
                "{bad:?} {success:?}"
            );
        }
    }
}

#[test]
fn a_placeholder_command_is_refused() {
    let p = plan("python313", Some("python@3.13"));
    for (s, c) in [
        (0, cmd(SuccessSemantic::StdoutNamesPackage, &["query"])),
        (1, cmd(SuccessSemantic::StdoutOmitsPackage, &["query"])),
    ] {
        let illustration = PlanCommand {
            placeholder: true,
            ..c
        };
        assert_eq!(
            expectation_for(&armed(&p, s, &illustration), s, 0),
            Err(ExpectationError::PlaceholderCommand),
            "{:?}",
            p.steps[s].kind
        );
    }
}

#[test]
fn a_command_that_is_not_a_providers_is_refused() {
    let p = plan("python313", Some("python@3.13"));
    let own = PlanCommand {
        target: CommandTarget::Tool,
        ..cmd(SuccessSemantic::StdoutNamesPackage, &["query"])
    };
    assert_eq!(
        expectation_for(&armed(&p, 0, &own), 0, 0),
        Err(ExpectationError::NotAProviderCommand)
    );
}

#[test]
fn a_step_carrying_an_unrecognised_check_is_refused() {
    // An unknown is never a yes: the check this build cannot read may be
    // exactly the one a newer build added because the rest were insufficient.
    let mut p = plan("python313", Some("python@3.13"));
    p.steps[0].verification = vec![
        VerificationCheck::ProviderReportsActive,
        VerificationCheck::Unknown,
    ];
    p.steps[1].verification = vec![VerificationCheck::Unknown];
    for (s, c) in [
        (0, cmd(SuccessSemantic::StdoutNamesPackage, &["query"])),
        (1, cmd(SuccessSemantic::StdoutOmitsPackage, &["query"])),
    ] {
        assert_eq!(
            expectation_for(&armed(&p, s, &c), s, 0),
            Err(ExpectationError::UnrecognisedCheck),
            "{:?}",
            p.steps[s].kind
        );
    }
    // The recognised ones do not stand in the way.
    p.steps[0].verification = vec![VerificationCheck::ProviderReportsActive];
    assert!(expectation_for(
        &armed(&p, 0, &cmd(SuccessSemantic::StdoutNamesPackage, &["query"])),
        0,
        0
    )
    .is_ok());
}
