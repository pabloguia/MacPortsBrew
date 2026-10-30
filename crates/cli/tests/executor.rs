// SPDX-License-Identifier: Apache-2.0

//! The run loop's I/O shell (`crates/cli/src/executor/`), driven end to end:
//! the execution gate first, the dispatch guard, read sites, judgement and the
//! run outcome. Its pure decisions are tested on their own in
//! `crates/application/tests/executor.rs`.
//!
//! Nothing here touches a live installation (INV-05). Reads come from a spy
//! site answering synthesised invocations, or from the adapters' real read
//! sites over the captured `both-providers-intel-macos26` recording, with the
//! destination reads it does not hold synthesised in the shapes the provider
//! command table's provenance records (F13): a destination read names the
//! package as a line's first token, and the bare inventory is the recording's
//! own where it has one. Verbs are read from `provider-commands.toml` through
//! `provider_commands::load`, never written here.

use macportsbrew::executor::{run, run_with, ReadSite, RunReport, SiteRefusalKind};
use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::process::{
    ExitStatus, Invocation, PlanReadRefusal, RecordedInvocation, Recording, ReplayRunner,
};
use macportsbrew_adapters::registry::{self, PlanReadError, RegistryContext};
use macportsbrew_application::{
    evaluable_in_this_build, execution_refusals, plan_run, provider_commands, AnalysisOptions,
    Disposition, HaltReason, PlanRequest, PlannedRun, RunOutcome,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandLineTools, DomainError, EnvironmentSnapshot, FailureCause,
    HomeRedaction, InstalledPackage, Machine, MacosVersion, MigrationPlan, PackageIdentity,
    PackageKind, PackageMetadata, PlanCommand, Provenance, ProviderCapabilities,
    ProviderConfiguration, ProviderDetection, ProviderId, ProviderInventory, ProviderPresence,
    ProviderSnapshot, RequestedSet, Snapshot, StepKind, SuccessSemantic, Translation, Version,
    SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const HB: &str = "homebrew";
const MP: &str = "macports";
const FIXTURE: &str = "both-providers-intel-macos26";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

/// The prefix the recording answers for each provider: Homebrew states its
/// own; MacPorts has no such read and the recording is of its documented
/// default.
fn recorded_prefix(id: &str) -> PathBuf {
    let rec = recording_as_captured();
    rec.invocations
        .iter()
        .find(|r| r.invocation.program == leaf(id) && r.invocation.argv == ["--prefix"])
        .map_or_else(
            || registry::candidate_prefixes(&ProviderId::new(id))[0].clone(),
            |r| PathBuf::from(r.invocation.stdout.trim()),
        )
}

fn verb(id: &str, operation: &str) -> Vec<String> {
    provider_commands::load().expect("the provider command table loads")[id].verbs[operation]
        .clone()
}

fn leaf(id: &str) -> String {
    let table = provider_commands::load().expect("the provider command table loads");
    Path::new(&table[id].executable)
        .file_name()
        .expect("an executable has a file name")
        .to_string_lossy()
        .into_owned()
}

fn invocation(program: &str, argv: &[String], status: ExitStatus, stdout: &str) -> Invocation {
    Invocation {
        program: program.to_string(),
        argv: argv.to_vec(),
        environment: BTreeMap::new(),
        status,
        stdout: stdout.to_string(),
        stderr: String::new(),
        truncated: false,
        duration_ms: 1,
    }
}

fn recording_as_captured() -> Recording {
    let path = root().join(format!("fixtures/recordings/{FIXTURE}.json"));
    Recording::from_json(&std::fs::read_to_string(&path).expect("the fixture reads"))
        .expect("the fixture parses")
}

/// The captured recording plus what it does not hold: each provider's
/// destination read of the package a plan here moves, naming it, and the
/// bare inventory where the recording has none, not naming it. Then each named
/// provider's activation read answers with the package active (S-2): that
/// invocation is also the capture's inventory read, and for MacPorts its
/// removal read, so a provider named here is one whose inventory now holds the
/// package.
fn recording(
    names: &[(&str, &str)],
    destination_output: impl Fn(&str, &str) -> String,
) -> Recording {
    let mut rec = recording_as_captured();
    let mut add = |program: String, argv: Vec<String>, stdout: String| {
        if !rec
            .invocations
            .iter()
            .any(|r| r.invocation.program == program && r.invocation.argv == argv)
        {
            rec.invocations.push(RecordedInvocation {
                invocation: invocation(&program, &argv, ExitStatus::Exited { code: 0 }, &stdout),
            });
        }
    };
    for (id, name) in names {
        let mut argv = verb(id, "verify");
        argv.push((*name).to_string());
        add(leaf(id), argv, destination_output(id, name));
    }
    for id in [HB, MP] {
        add(
            leaf(id),
            verb(id, "verify_absent"),
            "  unrelated 1.0\n".to_string(),
        );
    }
    // The destination's activation read as it would answer after the plan
    // happened: the captured answer with the package added, active (work order
    // 16 part S, S-2). It replaces the captured one, which predates the plan.
    for (id, name) in names {
        let argv = verb(id, "verify_active");
        let program = leaf(id);
        let at = rec
            .invocations
            .iter()
            .position(|r| r.invocation.program == program && r.invocation.argv == argv);
        let captured = at.map_or(String::new(), |i| {
            rec.invocations[i].invocation.stdout.clone()
        });
        let answered = RecordedInvocation {
            invocation: invocation(
                &program,
                &argv,
                ExitStatus::Exited { code: 0 },
                &with_active(id, name, &captured),
            ),
        };
        match at {
            Some(i) => rec.invocations[i] = answered,
            None => rec.invocations.push(answered),
        }
    }
    rec
}

/// The destination read's output when the package is there: the name as a
/// line's first token, indented as one provider's inventory is.
fn naming(_id: &str, name: &str) -> String {
    format!("  {name} 1.0\n")
}

/// `captured`, the activation read of provider `id`, with `name` added as the
/// active record, in the shape that provider's semantic reads (R-20).
fn with_active(id: &str, name: &str, captured: &str) -> String {
    use std::fmt::Write as _;
    let table = provider_commands::load().expect("the provider command table loads");
    match table[id].success["verify_active"].as_str() {
        "stdout_marks_package_active" => {
            let mut text = captured.to_string();
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            let _ = writeln!(text, "  {name} @1.0_0 (active)");
            text
        }
        "json_reports_package_linked" => {
            let mut doc: serde_json::Value = if captured.is_empty() {
                serde_json::json!({ "formulae": [], "casks": [] })
            } else {
                serde_json::from_str(captured).expect("the captured activation read parses")
            };
            doc["formulae"]
                .as_array_mut()
                .expect("a formulae list")
                .push(serde_json::json!({
                    "name": name,
                    "linked_keg": "1.0",
                    "installed": [{ "version": "1.0" }]
                }));
            doc.to_string()
        }
        other => panic!("{id}: {other} is not an activation semantic"),
    }
}

/// What the destination's read `command` prints when the package is there and
/// active: a presence read names it, an activation read marks it (S-2).
fn destination_present(plan: &MigrationPlan, command: &PlanCommand) -> String {
    let name = plan.destination_name().expect("a destination").as_str();
    match command.success {
        SuccessSemantic::StdoutMarksPackageActive | SuccessSemantic::JsonReportsPackageLinked => {
            with_active(plan.destination.as_str(), name, "")
        }
        _ => naming("", name),
    }
}

fn context(runner: &ReplayRunner) -> RegistryContext<'_> {
    RegistryContext {
        runner,
        home: None,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::Elsewhere,
        prefix_override: Some(
            [HB, MP]
                .iter()
                .map(|id| (ProviderId::new(*id), recorded_prefix(id)))
                .collect(),
        ),
    }
}

struct Pkg {
    name: &'static str,
    homepage: Option<&'static str>,
    service: bool,
}

fn pkg(name: &'static str) -> Pkg {
    Pkg {
        name,
        homepage: None,
        service: false,
    }
}

fn provider(id: &str, packages: &[Pkg]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for p in packages {
        inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(pid.clone(), p.name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        inventory.requested.names.insert(p.name.to_string());
        inventory.metadata.insert(
            p.name.to_string(),
            PackageMetadata {
                homepage: p.homepage.map(str::to_string),
                defines_service: p.service,
                service: Some(macportsbrew_domain::ServiceDefinition::both(
                    macportsbrew_domain::Answer::from(p.service),
                )),
                ..PackageMetadata::default()
            },
        );
        inventory.availability.insert(
            p.name.to_string(),
            AvailabilityAssessment {
                availability: ArtifactAvailability::BinaryAvailable,
                evidence: vec![AvailabilityEvidence {
                    source: "test".into(),
                    detail: "prebuilt".into(),
                    offline: true,
                }],
                cached: false,
            },
        );
    }
    inventory.requested = RequestedSet {
        names: inventory.requested.names.clone(),
        mechanism: "test".into(),
    };
    let mut capabilities = ProviderCapabilities::default();
    capabilities.capabilities.insert(
        Capability::MachineReadableInventory,
        CapabilityState::Available,
    );
    ProviderSnapshot {
        detection: ProviderDetection {
            provider: pid,
            presence: ProviderPresence::Present,
            version: Some(Version::new("9.9")),
            prefix: Some(recorded_prefix(id)),
            executable: None,
            architecture: Some(Architecture::X86_64),
            prefix_redacted: false,
        },
        capabilities,
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        // Read, holding nothing that could outrank the plan: a certifying read
        // follows a configuration re-check (SE-107 C1), which a configuration
        // the capture never recorded fails closed.
        configuration: ProviderConfiguration::Read { files: Vec::new() },
    }
}

fn snapshot(hb: &[Pkg], mp: &[Pkg]) -> Snapshot {
    let mut providers = BTreeMap::new();
    providers.insert(ProviderId::new(HB), provider(HB, hb));
    providers.insert(ProviderId::new(MP), provider(MP, mp));
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: "2026-09-30T00:00:00Z".into(),
            captured_by: "executor.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            architecture: Architecture::X86_64,
            macos: MacosVersion::new(26, 0, 0),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        },
        environment: EnvironmentSnapshot::default(),
        providers,
    }
}

/// `gnu-sed` and `gsed` are paired by the curated table, so a package held by
/// one side plans a real add on the other, in either direction (INV-17).
fn install_needed(source: &str) -> (Snapshot, &'static str, &'static str) {
    if source == HB {
        (snapshot(&[pkg("gnu-sed")], &[pkg("other")]), "gnu-sed", MP)
    } else {
        (snapshot(&[pkg("other")], &[pkg("gsed")]), "gsed", HB)
    }
}

/// `tool` held by both, and the stateful `postgresql@17`.
fn both_held() -> Snapshot {
    let tool = || Pkg {
        name: "tool",
        homepage: Some("https://example.org/tool"),
        service: false,
    };
    snapshot(
        &[
            tool(),
            Pkg {
                name: "postgresql@17",
                homepage: None,
                service: true,
            },
        ],
        &[tool()],
    )
}

fn planned(snap: &Snapshot, package: &str, to: &str, retain: bool) -> PlannedRun {
    plan_run(
        snap,
        &PolicyData::load_embedded().expect("the embedded policy data loads"),
        &AnalysisOptions {
            today: "2026-09-30".into(),
            advanced: false,
            only: BTreeSet::new(),
        },
        &PlanRequest {
            package: package.into(),
            to: ProviderId::new(to),
            retain_source: retain,
        },
        &registry::known_provider_ids(),
    )
    .expect("the planner builds this plan")
}

fn planner_runs() -> Vec<PlannedRun> {
    let mut runs = Vec::new();
    for source in [HB, MP] {
        let (snap, package, to) = install_needed(source);
        for retain in [false, true] {
            runs.push(planned(&snap, package, to, retain));
        }
    }
    let both = both_held();
    for (package, to) in [("tool", MP), ("tool", HB), ("postgresql@17", MP)] {
        runs.push(planned(&both, package, to, false));
    }
    runs
}

/// The verification part of a planner plan, renumbered: the shape the gate
/// passes in this build. `VERIFY_DESTINATION` keeps only the checks this build
/// evaluates, and is dropped when none is left — a consolidation's, since
/// pass 08 slice 4 made `PROVIDER_REPORTS_ACTIVE` unevaluable (D11), which the
/// gate would refuse as asserting nothing; with `brackets` the snapshot and
/// history steps stay. The snapshot the plan was built from is kept with it.
fn read_only(run: PlannedRun, brackets: bool) -> PlannedRun {
    let run = run.map_plan(|out| {
        out.steps.retain(|s| match s.kind {
            StepKind::VerifyDestination | StepKind::VerifyRemoval => s
                .verification
                .iter()
                .any(|c| evaluable_in_this_build(s.kind, c)),
            StepKind::SnapshotBefore | StepKind::RecordHistory => brackets,
            _ => false,
        });
        for (i, step) in out.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).expect("small") + 1;
            let kind = step.kind;
            step.verification
                .retain(|c| evaluable_in_this_build(kind, c));
        }
        out.elevated_steps.clear();
    });
    let report = execution_refusals(run.plan());
    assert!(report.may_execute(), "{:?}", report.refusals());
    run
}

fn install(source: &str, brackets: bool) -> PlannedRun {
    let (snap, package, to) = install_needed(source);
    read_only(planned(&snap, package, to, false), brackets)
}

/// A read site that records what it was handed and answers from `answer`.
struct Spy<F: Fn(&ProviderId, &PlanCommand) -> Result<Invocation, PlanReadError>> {
    calls: RefCell<Vec<(ProviderId, PlanCommand)>>,
    answer: F,
}

impl<F: Fn(&ProviderId, &PlanCommand) -> Result<Invocation, PlanReadError>> Spy<F> {
    fn new(answer: F) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            answer,
        }
    }

    fn calls(&self) -> Vec<(ProviderId, PlanCommand)> {
        self.calls.borrow().clone()
    }
}

impl<F: Fn(&ProviderId, &PlanCommand) -> Result<Invocation, PlanReadError>> ReadSite for Spy<F> {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        self.calls
            .borrow_mut()
            .push((provider.clone(), command.clone()));
        (self.answer)(provider, command)
    }
}

/// Answers every read as the provider would when the plan has happened: the
/// destination named, the source missing from the inventory.
fn honest(
    plan: &MigrationPlan,
) -> impl Fn(&ProviderId, &PlanCommand) -> Result<Invocation, PlanReadError> + '_ {
    move |provider, command| {
        let stdout = if *provider == plan.destination {
            destination_present(plan, command)
        } else {
            "  unrelated 1.0\n".to_string()
        };
        Ok(invocation(
            "x",
            &command.argv,
            ExitStatus::Exited { code: 0 },
            &stdout,
        ))
    }
}

/// Runs `planned` through `spy`, which is handed to the loop only after the
/// gate passed the plan.
fn spied<S: ReadSite>(planned: &PlannedRun, spy: &S, home: &HomeRedaction) -> RunReport {
    run_with(planned, || spy, &clean, home)
}

/// A provider configuration re-read that finds nothing that could outrank the
/// plan, as on a machine whose configuration did not move (SE-107 C1).
#[allow(clippy::unnecessary_wraps)]
fn clean(_: &ProviderId, _: &str) -> Result<ProviderConfiguration, DomainError> {
    Ok(ProviderConfiguration::Read { files: Vec::new() })
}

/// What a run did, step by step, without the evidence counts: those are the
/// providers' own outputs, which differ in length, not the loop's behaviour.
fn shape(report: &RunReport) -> Vec<(StepKind, String)> {
    report
        .records
        .iter()
        .map(|r| {
            let d = match &r.disposition {
                Disposition::Halted { reason, .. } => format!("HALTED {reason:?}"),
                Disposition::Verified { evidence } => format!(
                    "VERIFIED {:?}",
                    evidence
                        .iter()
                        .map(macportsbrew_domain::ReadEvidence::passed)
                        .collect::<Vec<_>>()
                ),
                other => format!("{other:?}"),
            };
            (r.kind, d)
        })
        .collect()
}

fn gate_refused(report: &RunReport, tag: &str) -> bool {
    matches!(report.outcome, RunOutcome::Refused { .. })
        && report
            .gate_refusals
            .iter()
            .any(|refusal| refusal["refusal"] == tag)
}

#[test]
fn the_execution_gate_is_consulted_before_any_step() {
    // A plan the gate passes runs reads; the same plan with one field the
    // gate refuses runs nothing, records no step, and never builds a site.
    let passed = install(HB, false);
    let spy = Spy::new(honest(passed.plan()));
    let ran = spied(&passed, &spy, &HomeRedaction::Recorded);
    assert!(!spy.calls().is_empty(), "the gate passed, so reads ran");
    assert_eq!(ran.outcome, RunOutcome::Completed);

    let refused = passed.clone().map_plan(|p| p.dry_run_only = false);
    let built = std::cell::Cell::new(false);
    let report = run_with(
        &refused,
        || {
            built.set(true);
            Spy::new(|_: &ProviderId, _: &PlanCommand| Err(PlanReadError::NotDetected))
        },
        &clean,
        &HomeRedaction::Recorded,
    );
    assert!(!built.get(), "a read site was built for a refused plan");
    assert!(report.records.is_empty());
    assert!(gate_refused(&report, "PLAN_NOT_DRY_RUN_ONLY"), "{report:?}");
    assert_eq!(report.exit_code(), 4);
}

#[test]
fn the_dispatch_guard_hands_no_mutating_step_to_the_runner() {
    // Every plan the planner builds carries the real verbs. Nothing reaches
    // the site: the gate refuses each plan before the loop looks at a step,
    // and behind it the guard never answers `Dispatch` for a mutating step
    // (proved alone by `invariant_01_the_loop_never_dispatches_a_mutating_step`
    // in `crates/application/tests/executor.rs`).
    let mutating: BTreeSet<Vec<String>> = [HB, MP]
        .iter()
        .flat_map(|id| [verb(id, "add"), verb(id, "remove")])
        .collect();
    for run in planner_runs() {
        let spy = Spy::new(honest(run.plan()));
        let report = spied(&run, &spy, &HomeRedaction::Recorded);
        for (_, command) in spy.calls() {
            assert!(
                !mutating.iter().any(|v| command.argv.starts_with(v)),
                "a mutating verb reached the site: {:?}",
                command.argv
            );
        }
        assert!(spy.calls().is_empty(), "{:?}", spy.calls());
        assert_eq!(report.exit_code(), 4);
        if run.plan().has_mutating_steps() && !run.plan().requires_manual_review() {
            assert!(
                gate_refused(&report, "MUTATION_NOT_IN_THIS_BUILD"),
                "{report:?}"
            );
        }
    }
}

#[test]
fn the_guard_holds_behind_the_real_read_site_too() {
    // The adapters' read sites over a recording that would answer every
    // destination and inventory read: still nothing consulted.
    for run in planner_runs() {
        let name = run
            .plan()
            .destination_name()
            .map_or("x".to_string(), |n| n.as_str().to_string());
        let runner = ReplayRunner::new(recording(&[(HB, &name), (MP, &name)], naming));
        let report = run_real(&run, &runner);
        assert!(runner.consulted().is_empty(), "{:?}", runner.consulted());
        assert_eq!(report.exit_code(), 4);
    }
}

/// The product's read sites over `runner`, with a configuration re-read that
/// finds nothing (`clean`). Not `run` itself: under a replay the product's
/// re-read cannot know the configuration, so `run` halts every certifying read
/// before it is made (SE-107 C1, `plan.v1.md` obligation 6) — pinned on its own
/// by `a_replayed_certifying_read_halts_before_it_is_made`.
fn run_real(planned: &PlannedRun, runner: &ReplayRunner) -> RunReport {
    let ctx = context(runner);
    run_with(
        planned,
        || registry::plan_read_sites(&ctx),
        &clean,
        &HomeRedaction::Recorded,
    )
}

#[test]
fn a_replayed_certifying_read_halts_before_it_is_made() {
    // The product's own loop over a recording: the destination's configuration
    // cannot be re-read from a replay, so the certifying read is never made
    // and the step halts as a failed precondition, never as a verdict.
    for source in [HB, MP] {
        let run_ = install(source, true);
        let plan = run_.plan();
        let name = plan
            .destination_name()
            .expect("a destination")
            .as_str()
            .to_string();
        let runner = ReplayRunner::new(recording(&[(plan.destination.as_str(), &name)], naming));
        let report = run(&run_, &context(&runner), &HomeRedaction::Recorded);
        let verify = plan
            .steps
            .iter()
            .find(|s| s.kind == StepKind::VerifyDestination)
            .expect("a destination check")
            .index;
        assert!(
            matches!(
                report.outcome,
                RunOutcome::Halted {
                    step,
                    reason: HaltReason::PreconditionFailed { command: 0, .. }
                } if step == verify
            ),
            "{source}: {:?}",
            report.outcome
        );
    }
}

#[test]
fn invariant_19_manual_review_halts_before_any_dispatch() {
    let run = planned(&both_held(), "postgresql@17", MP, false);
    assert!(run.plan().requires_manual_review());
    let spy = Spy::new(honest(run.plan()));
    let report = spied(&run, &spy, &HomeRedaction::Recorded);
    assert!(spy.calls().is_empty());
    assert!(gate_refused(&report, "MANUAL_REVIEW"), "{report:?}");
    assert_eq!(report.exit_code(), 4);
}

#[test]
fn invariant_02_every_dispatched_invocation_is_a_plan_command() {
    for source in [HB, MP] {
        let run = install(source, true);
        let spy = Spy::new(honest(run.plan()));
        let _ = spied(&run, &spy, &HomeRedaction::Recorded);
        let planned: Vec<(ProviderId, PlanCommand)> = run
            .plan()
            .steps
            .iter()
            .flat_map(|s| {
                s.commands
                    .iter()
                    .map(|c| (s.provider.clone().expect("a provider"), c.clone()))
            })
            .collect();
        assert_eq!(
            spy.calls(),
            planned,
            "{source}: exactly the plan's commands, in order"
        );
    }
}

#[test]
fn invariant_17_the_loop_treats_both_directions_identically() {
    // A real install each way, through the adapters' read sites over the
    // recording, and a package held by both, each way.
    let mut shapes = Vec::new();
    for source in [HB, MP] {
        let run = install(source, true);
        let name = run
            .plan()
            .destination_name()
            .expect("a destination")
            .as_str()
            .to_string();
        let to = run.plan().destination.as_str().to_string();
        let runner = ReplayRunner::new(recording(&[(&to, &name)], naming));
        let report = run_real(&run, &runner);
        shapes.push((shape(&report), report.outcome.clone()));
    }
    assert_eq!(shapes[0], shapes[1]);
    assert_eq!(
        shapes[0].1,
        RunOutcome::PartialSuccess {
            step: 1,
            reason: HaltReason::NotPerformedInThisBuild
        }
    );

    let both = both_held();
    let mut dual = Vec::new();
    for to in [MP, HB] {
        let run = read_only(planned(&both, "tool", to, false), false);
        let spy = Spy::new(honest(run.plan()));
        let report = spied(&run, &spy, &HomeRedaction::Recorded);
        dual.push((shape(&report), report.outcome.clone(), spy.calls().len()));
    }
    assert_eq!(dual[0], dual[1]);
    assert_eq!(dual[0].1, RunOutcome::Completed);
}

/// Every word of every source file under `path`, lower-cased.
fn words(path: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            for entry in std::fs::read_dir(&p).expect("readable") {
                stack.push(entry.expect("entry").path());
            }
            continue;
        }
        if p.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&p).expect("utf-8 source");
        for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            out.push((p.display().to_string(), word.to_lowercase()));
        }
    }
    out
}

fn migrate_path() -> PathBuf {
    root().join("crates/cli/src/app/migrate.rs")
}

fn executor_sources() -> Vec<PathBuf> {
    let root = root();
    let mut paths = vec![
        root.join("crates/cli/src/executor"),
        root.join("crates/application/src/executor.rs"),
    ];
    // The single caller (SE-092), read unconditionally: a scan that skipped
    // it after a rename would still pass.
    assert!(migrate_path().exists(), "the run loop's caller moved");
    paths.push(migrate_path());
    paths
}

#[test]
fn invariant_17_the_executor_names_no_provider() {
    // `check-layering.sh` does not scan the command-line crate, so the
    // executor's own sources are scanned here, word by word, prose included.
    let mut names: BTreeSet<String> = registry::known_provider_ids()
        .iter()
        .map(|id| id.as_str().to_string())
        .collect();
    for id in [HB, MP] {
        names.insert(leaf(id));
    }
    assert!(names.len() >= 4, "{names:?}");
    let mut scanned = 0;
    for path in executor_sources() {
        for (file, word) in words(&path) {
            scanned += 1;
            assert!(!names.contains(&word), "{file} names a provider: {word}");
        }
    }
    assert!(scanned > 100, "the scan read the executor's sources");
}

#[test]
fn the_executor_reads_neither_the_mutation_constant_nor_the_plans_claim() {
    // A11: a guard that read either could be opened by flipping it.
    for path in executor_sources() {
        for (file, word) in words(&path) {
            assert_ne!(word, "dry_run_only", "{file}");
            assert_ne!(word, "this_build_dispatches_mutations", "{file}");
        }
    }
}

#[test]
fn the_command_line_crate_builds_no_command_spec_and_no_read_site_outside_the_loop() {
    // A provider's command reaches a spawn only through the adapters' read
    // sites (SE-050, "Not closed" 2), and a read site exists only inside the
    // loop, after the gate (security review of S5, M1).
    let executor = root().join("crates/cli/src/executor");
    for (file, word) in words(&root().join("crates/cli/src")) {
        for banned in ["commandspec", "plan_read_spec", "detected_prefix"] {
            assert_ne!(word, banned, "{file}");
        }
        // No exemption for `app/migrate.rs`: `run` builds the sites from the
        // context itself, so a caller never needs them, and holding them
        // would be a way to read on a plan the gate never saw (review of S5).
        let in_loop = Path::new(&file).starts_with(&executor);
        if !in_loop {
            for banned in ["planreadsites", "plan_read_sites"] {
                assert_ne!(word, banned, "{file} builds a read site outside the loop");
            }
            if !file.contains("/app/") {
                // The context holds the capture's runner; nothing else does.
                assert_ne!(word, "processrunner", "{file}");
            }
        }
    }
}

#[test]
fn a_provider_spec_is_built_only_by_an_adapter_reading_its_own_inventory() {
    // `CommandSpec::provider` is public for the adapters, so where it is named
    // is the rule: the two provider adapters and one spawn-site test.
    let crates = root().join("crates");
    let mut stack = vec![crates.clone()];
    let mut sites = BTreeSet::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().is_some_and(|e| e == "rs")
                && path.components().any(|c| c.as_os_str() == "src")
                && std::fs::read_to_string(&path)
                    .expect("utf-8")
                    .contains("CommandSpec::provider(")
            {
                let rel = path.strip_prefix(&crates).expect("under crates");
                sites.insert(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let allowed: BTreeSet<String> = [
        "adapters/src/homebrew/mod.rs",
        "adapters/src/macports/mod.rs",
        "adapters/src/process/system.rs",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    assert_eq!(sites, allowed);
}

#[test]
fn a_verification_failure_halts_and_names_the_check() {
    let run = install(HB, true);
    let plan = run.plan();
    let name = plan
        .destination_name()
        .expect("a destination")
        .as_str()
        .to_string();
    // The destination read answers, but not with the package.
    let runner = ReplayRunner::new(recording(&[(plan.destination.as_str(), &name)], |_, _| {
        "  unrelated 1.0\n".into()
    }));
    let report = run_real(&run, &runner);
    let verify = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyDestination)
        .expect("a destination check")
        .index;
    assert_eq!(
        report.outcome,
        RunOutcome::Halted {
            step: verify,
            reason: HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NameAbsent
            }
        }
    );
    assert_eq!(report.exit_code(), 4);
    // Nothing after the failed step was read.
    let removal = verb(plan.source.as_str(), "verify_absent").join(" ");
    assert!(
        !runner.consulted().iter().any(|c| c.ends_with(&removal)),
        "{:?}",
        runner.consulted()
    );
    assert!(report
        .records
        .iter()
        .filter(|r| r.index > verify)
        .all(|r| r.disposition == Disposition::NotReached));
}

#[test]
fn a_truncated_or_killed_read_halts() {
    let run = install(MP, false);
    let plan = run.plan();
    for (status, truncated, cause) in [
        (
            ExitStatus::Exited { code: 0 },
            true,
            FailureCause::Truncated,
        ),
        (
            ExitStatus::Signalled { signal: Some(9) },
            false,
            FailureCause::DidNotFinish,
        ),
        (ExitStatus::TimedOut, false, FailureCause::DidNotFinish),
    ] {
        let spy = Spy::new(|_: &ProviderId, command: &PlanCommand| {
            let named = destination_present(plan, command);
            let mut inv = invocation("x", &command.argv, status.clone(), &named);
            inv.truncated = truncated;
            Ok(inv)
        });
        let report = spied(&run, &spy, &HomeRedaction::Recorded);
        assert!(
            matches!(
                &report.outcome,
                RunOutcome::Halted { reason: HaltReason::VerificationFailed { cause: c, .. }, .. } if *c == cause
            ),
            "{:?}",
            report.outcome
        );
        // The step's two reads (presence, activation) are both made before it
        // is judged; nothing is read after the halt.
        assert_eq!(spy.calls().len(), 2, "nothing read after the halt");
    }
}

#[test]
fn a_recorded_read_keeps_no_stdout() {
    let run = install(HB, true);
    let plan = run.plan();
    let marker = "inventory-marker-that-must-not-be-kept";
    let spy = Spy::new(|provider: &ProviderId, command: &PlanCommand| {
        let mut stdout = format!("{marker} 1.0\n");
        if *provider == plan.destination {
            stdout.push_str(&destination_present(plan, command));
        }
        let mut inv = invocation("x", &command.argv, ExitStatus::Exited { code: 0 }, &stdout);
        inv.stderr = format!("{marker} on stderr\n");
        Ok(inv)
    });
    let report = spied(&run, &spy, &HomeRedaction::Recorded);
    assert_eq!(
        spy.calls().len(),
        3,
        "the destination's two reads and the source's ran"
    );
    let json = serde_json::to_string(&report).expect("a report serialises");
    assert!(!json.contains(marker), "{json}");
    assert!(!format!("{report:?}").contains(marker));
    assert!(
        json.contains("lineCount"),
        "the evidence is what is kept: {json}"
    );
}

#[test]
fn a_run_that_skipped_its_snapshot_is_never_completed() {
    let bracketed = install(HB, true);
    let spy = Spy::new(honest(bracketed.plan()));
    let report = spied(&bracketed, &spy, &HomeRedaction::Recorded);
    assert_eq!(
        report.records[0].disposition,
        Disposition::NotPerformedInThisBuild
    );
    assert_eq!(
        report.outcome,
        RunOutcome::PartialSuccess {
            step: 1,
            reason: HaltReason::NotPerformedInThisBuild
        }
    );
    assert_eq!(report.exit_code(), 3);
    // The same reads with no bracket to skip complete.
    let bare = install(HB, false);
    let spy = Spy::new(honest(bare.plan()));
    assert_eq!(spied(&bare, &spy, &HomeRedaction::Recorded).exit_code(), 0);
}

#[test]
fn invariant_03_partial_success_is_its_own_outcome_and_exit_code() {
    // The destination verifies; the source is still in the inventory.
    let run = install(HB, false);
    let plan = run.plan();
    let source = plan.source_name().as_str().to_string();
    let spy = Spy::new(|provider: &ProviderId, command: &PlanCommand| {
        let stdout = if *provider == plan.destination {
            destination_present(plan, command)
        } else {
            format!("{source} 1.0\n")
        };
        Ok(invocation(
            "x",
            &command.argv,
            ExitStatus::Exited { code: 0 },
            &stdout,
        ))
    });
    let report = spied(&run, &spy, &HomeRedaction::Recorded);
    let removal = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyRemoval)
        .expect("a removal check")
        .index;
    assert_eq!(
        report.outcome,
        RunOutcome::PartialSuccess {
            step: removal,
            reason: HaltReason::VerificationFailed {
                command: 0,
                cause: FailureCause::NamePresent
            }
        }
    );
    assert_eq!(report.exit_code(), 3);
}

#[test]
fn a_site_refusal_halts_and_is_rendered_with_the_home_redacted() {
    let run = install(HB, false);
    let spy = Spy::new(|_: &ProviderId, _: &PlanCommand| {
        Err(PlanReadError::Refused(PlanReadRefusal::ProgramDisagrees {
            planned: "/Users/ada/elsewhere/bin/x\u{1b}[2J\u{202E}".into(),
            derived: "/usr/local/bin/x".into(),
        }))
    });
    let home = HomeRedaction::Known(PathBuf::from("/Users/ada"));
    let report = spied(&run, &spy, &home);
    assert!(matches!(
        report.outcome,
        RunOutcome::Halted {
            reason: HaltReason::SiteRefused { command: 0 },
            ..
        }
    ));
    assert_eq!(report.exit_code(), 4);
    let refusal = report.refusal.clone().expect("the refusal is kept");
    assert_eq!(refusal.refusal, SiteRefusalKind::ProgramDisagrees);
    assert!(
        refusal
            .values
            .values()
            .all(|v| !v.contains("ada") && !v.chars().any(|c| c.is_control() || c == '\u{202E}')),
        "{:?}",
        refusal.values
    );
    assert!(!refusal.text.contains("ada"), "{}", refusal.text);
    assert!(
        !refusal
            .text
            .chars()
            .any(|c| c.is_control() || c == '\u{202E}'),
        "{:?}",
        refusal.text
    );
    let json = serde_json::to_string(&report).expect("serialises");
    assert!(!json.contains("ada"), "{json}");
}

#[test]
fn a_gate_refusal_is_rendered_with_the_home_redacted() {
    // A check's own fields are plan text: an executable under the home, with
    // a control and a format character, carried into a refusal the gate
    // names (L1).
    let run = install(HB, false).map_plan(|p| {
        let verify = p
            .steps
            .iter()
            .position(|s| s.kind == StepKind::VerifyDestination)
            .expect("a destination check");
        p.steps[verify]
            .verification
            .push(macportsbrew_domain::VerificationCheck::LivenessProbe {
                executable: "/Users/ada/bin/x\u{1b}[2J\u{202E}".into(),
                flag: "--version".into(),
            });
    });
    let home = HomeRedaction::Known(PathBuf::from("/Users/ada"));
    let spy = Spy::new(honest(run.plan()));
    let report = spied(&run, &spy, &home);
    assert!(spy.calls().is_empty());
    assert!(
        gate_refused(&report, "CHECK_NOT_EVALUABLE_IN_THIS_BUILD"),
        "{report:?}"
    );
    let json = serde_json::to_string(&report).expect("serialises");
    assert!(!json.contains("ada"), "{json}");
    assert!(
        !json.contains('\u{202E}') && !json.contains('\u{1b}'),
        "{json:?}"
    );
}

#[test]
fn a_provider_with_no_read_site_halts() {
    let run = install(HB, false);
    let runner = ReplayRunner::new(recording_as_captured());
    let mut ctx = context(&runner);
    ctx.prefix_override = Some(Vec::new());
    // The product's loop stops earlier: a provider it cannot detect has no
    // configuration to re-read, so the certifying read is never made.
    let report = macportsbrew::executor::run(&run, &ctx, &HomeRedaction::Recorded);
    assert!(
        matches!(
            report.outcome,
            RunOutcome::Halted {
                reason: HaltReason::PreconditionFailed { command: 0, .. },
                ..
            }
        ),
        "{:?}",
        report.outcome
    );
    // Past that re-read, the read site itself is missing.
    let report = run_with(
        &run,
        || registry::plan_read_sites(&ctx),
        &clean,
        &HomeRedaction::Recorded,
    );
    assert!(matches!(
        report.outcome,
        RunOutcome::Halted {
            reason: HaltReason::SiteUnavailable { command: 0 },
            ..
        }
    ));
    assert!(runner.consulted().is_empty());
}

#[test]
fn the_plan_editing_test_hook_is_out_of_product_source() {
    // `PlannedRun::map_plan` pairs the snapshot a plan was built from with a
    // narrowed plan. It exists only in test builds and under the non-default
    // `test-support` feature, and no product source names it (review of S5).
    let crates = root().join("crates");
    let home = crates.join("application/src/executor.rs");
    let tests = crates.join("application/src/executor");
    for entry in std::fs::read_dir(&crates).expect("crates/") {
        let src = entry.expect("entry").path().join("src");
        for (file, word) in words(&src) {
            let path = Path::new(&file);
            if path == home || path.starts_with(&tests) {
                continue;
            }
            assert_ne!(word, "map_plan", "{file}");
        }
    }
    let text = std::fs::read_to_string(&home).expect("executor.rs");
    let at = text
        .find("pub fn map_plan")
        .expect("the hook is defined here");
    let before: Vec<&str> = text[..at].lines().rev().take(3).collect();
    assert!(
        before
            .iter()
            .any(|l| l.trim() == "#[cfg(any(test, feature = \"test-support\"))]"),
        "map_plan is compiled only for tests: {before:?}"
    );
    let manifest =
        std::fs::read_to_string(crates.join("application/Cargo.toml")).expect("manifest");
    assert!(
        manifest.contains("test-support = []"),
        "the feature is declared"
    );
    assert!(
        !manifest
            .lines()
            .any(|l| l.trim_start().starts_with("default")),
        "test-support is never a default feature"
    );
    // And no manifest enables it for a product build: every place that turns
    // `test-support` on is a `[dev-dependencies]` table, so the binary that
    // ships never compiles the hook (review of the executor pass).
    let mut manifests = vec![root().join("Cargo.toml")];
    let mut stack = vec![crates.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    stack.push(path);
                }
            } else if path.file_name().is_some_and(|n| n == "Cargo.toml") {
                manifests.push(path);
            }
        }
    }
    let mut enablings = 0;
    for path in &manifests {
        let text = std::fs::read_to_string(path).expect("manifest");
        let mut section = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                section = trimmed.to_string();
                continue;
            }
            if trimmed.starts_with('#') || !trimmed.contains("test-support\"") {
                continue;
            }
            enablings += 1;
            assert!(
                section.contains("dev-dependencies"),
                "{} enables test-support under {section}, not [dev-dependencies]",
                path.display()
            );
        }
    }
    assert!(enablings > 0, "the scan saw the test build's enabling");
}

/// Every `.rs` file under some crate's `src/`, with its path.
fn product_sources() -> Vec<(PathBuf, String)> {
    let crates = root().join("crates");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&crates).expect("crates/") {
        let mut stack = vec![entry.expect("entry").path().join("src")];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).expect("utf-8 source");
                    out.push((path, text));
                }
            }
        }
    }
    assert!(out.len() > 50, "the scan read the workspace");
    out
}

/// The whole attribute and doc block above `marker`: every line directly
/// above it that is an attribute or a comment (slice 4 review: a scan of one
/// line let a derive above `#[must_use]` through).
fn attributes_above<'a>(text: &'a str, marker: &str) -> Vec<&'a str> {
    let at = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker} is defined"));
    text[..at]
        .lines()
        .rev()
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| {
            let t = l.trim();
            t.starts_with("#[") || t.starts_with("//") || t.is_empty()
        })
        .collect()
}

/// The lines just above `marker` in `text`.
fn lines_above<'a>(text: &'a str, marker: &str, count: usize) -> Vec<&'a str> {
    let at = text
        .find(marker)
        .unwrap_or_else(|| panic!("{marker} is defined"));
    text[..at].lines().rev().take(count).collect()
}

#[test]
fn the_mutation_allowance_and_the_gate_witness_are_made_only_by_the_gate() {
    // Pass 08 slice 4, D1 and D2: the power to mutate is a value only the gate
    // makes, and in product code only when the build constant is true. A test
    // obtains one through `forced_in_tests`, compiled only for tests and the
    // dev-only `test-support` feature; nothing else may construct either type,
    // implement a trait for it, or derive one onto it.
    let crates = root().join("crates");
    let gate = crates.join("application/src/execution_gate.rs");
    // The one product test module allowed to force an allowance, included only
    // under `#[cfg(test)]` (checked below).
    let unit_tests = crates.join("application/src/executor/tests.rs");
    for (path, text) in product_sources() {
        if path == gate {
            continue;
        }
        let file = path.display();
        if path != unit_tests {
            assert!(
                !text.contains("forced_in_tests"),
                "{file} forces an allowance"
            );
        }
        for banned in [
            "MutationAllowance(",
            "GateClear {",
            "for MutationAllowance",
            "for GateClear",
        ] {
            assert!(!text.contains(banned), "{file}: {banned}");
        }
    }
    let executor =
        std::fs::read_to_string(crates.join("application/src/executor.rs")).expect("executor.rs");
    let tests_mod = lines_above(&executor, "mod tests;", 1);
    assert_eq!(tests_mod, ["#[cfg(test)]"], "the unit tests are test-only");
    let text = std::fs::read_to_string(&gate).expect("the gate");
    assert!(
        lines_above(&text, "pub fn forced_in_tests", 2)
            .iter()
            .any(|l| l.trim() == "#[cfg(any(test, feature = \"test-support\"))]"),
        "forced_in_tests is compiled only for tests"
    );
    for definition in ["pub struct MutationAllowance(", "pub struct GateClear {"] {
        assert!(
            attributes_above(&text, definition)
                .iter()
                .filter(|l| l.trim().starts_with("#["))
                .all(|l| !l.contains("derive")),
            "{definition} derives nothing"
        );
    }
    // Inside the gate's own file, where the compiler allows construction:
    // one product producer, one test constructor, one witness construction.
    assert_eq!(
        text.matches("MutationAllowance(())").count(),
        2,
        "the definition and the one producer"
    );
    assert_eq!(text.matches("Self(())").count(), 1, "one test constructor");
    assert_eq!(
        text.lines()
            .filter(|l| l.contains("GateClear {"))
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("pub struct") && !t.starts_with("impl")
            })
            .count(),
        1,
        "one construction: clear()'s construction"
    );
    for banned in ["impl Default", "impl Clone", "impl From"] {
        assert!(!text.contains(banned), "the gate file: {banned}");
    }
    // The only product producer reads the constant.
    assert!(text.contains("THIS_BUILD_DISPATCHES_MUTATIONS.then_some(MutationAllowance(()))"));
}

#[test]
fn consent_is_minted_only_by_confirm_and_asked_only_by_the_consent_module() {
    // Slice 4, D10 (review N2): a `ConsentGiven` exists only through
    // `consent::confirm`, which checks the typed phrase; nothing constructs
    // one, implements a trait for it or derives one onto it; and `confirm` is
    // called only by the command-line crate's consent prompt.
    let crates = root().join("crates");
    let home = crates.join("application/src/consent.rs");
    let prompt = crates.join("cli/src/consent.rs");
    for (path, text) in product_sources() {
        let file = path.display();
        if path != home {
            for banned in ["ConsentGiven {", "for ConsentGiven"] {
                assert!(!text.contains(banned), "{file}: {banned}");
            }
            if path != prompt {
                assert!(!text.contains("confirm("), "{file} calls confirm");
            }
        }
    }
    let text = std::fs::read_to_string(&home).expect("consent.rs");
    assert!(
        attributes_above(&text, "pub struct ConsentGiven {")
            .iter()
            .filter(|l| l.trim().starts_with("#["))
            .all(|l| !l.contains("Clone") && !l.contains("Copy") && !l.contains("Deserialize")),
        "ConsentGiven is neither Clone, Copy nor read from a file"
    );
    assert_eq!(
        text.lines()
            .filter(|l| l.contains("ConsentGiven {"))
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("pub struct") && !t.starts_with("impl")
            })
            .count(),
        1,
        "one construction: confirm()'s construction"
    );
    assert!(
        !text.contains("for ConsentGiven"),
        "no trait implementation"
    );
}

#[test]
fn there_is_exactly_one_mutation_site_and_it_refuses() {
    // D7: the product mutation site refuses every command, so no mutating
    // command spec is built anywhere in shipped source. One implementation
    // under `crates/*/src`, naming no runner, registry or process item.
    // Any `impl`, generic or not, whose trait is `MutationSite`.
    let mut impls = Vec::new();
    for (path, text) in product_sources() {
        for (at, _) in text.match_indices("impl") {
            let head = &text[at..];
            let header = &head[..head.find(['{', ';']).unwrap_or(head.len())];
            let word_start = text[..at]
                .chars()
                .next_back()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if word_start && header.contains("MutationSite for") {
                impls.push((path.clone(), head.to_string()));
            }
        }
    }
    assert_eq!(
        impls.len(),
        1,
        "{:?}",
        impls.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
    let (path, from) = &impls[0];
    assert!(
        path.ends_with("crates/cli/src/executor/mod.rs"),
        "{}",
        path.display()
    );
    let body = &from[..from.find("\n}\n").expect("the impl ends")];
    assert!(body.contains("Err(MutationRefusal::Refused)"), "{body}");
    for banned in ["runner", "registry", "process::", "CommandSpec", "spawn"] {
        assert!(
            !body.contains(banned),
            "the product site names {banned}: {body}"
        );
    }
}
#[test]
fn a_certifying_read_follows_a_configuration_recheck() {
    // SE-107 C1: the destination's configuration is re-read and re-checked
    // before its VERIFY_DESTINATION is read; unreadable or drifted, the step
    // halts and nothing of the destination is read.
    #[allow(clippy::unnecessary_wraps)]
    fn unreadable(_: &ProviderId, _: &str) -> Result<ProviderConfiguration, DomainError> {
        Err(DomainError::new(
            macportsbrew_domain::ErrorId::SafetyBlocked,
            "not detected",
        ))
    }
    #[allow(clippy::unnecessary_wraps)]
    fn unknowable(_: &ProviderId, _: &str) -> Result<ProviderConfiguration, DomainError> {
        Ok(ProviderConfiguration::Unknowable)
    }
    type Reread = fn(&ProviderId, &str) -> Result<ProviderConfiguration, DomainError>;
    for source in [HB, MP] {
        for (reread, why) in [
            (
                unreadable as Reread,
                macportsbrew_application::Precondition::ConfigurationUnreadable,
            ),
            (
                unknowable as Reread,
                macportsbrew_application::Precondition::ConfigurationDrift,
            ),
        ] {
            let run_ = install(source, false);
            let plan = run_.plan();
            let verify = plan
                .steps
                .iter()
                .find(|s| s.kind == StepKind::VerifyDestination)
                .expect("a destination check")
                .index;
            let spy = Spy::new(honest(plan));
            let report = run_with(&run_, || &spy, &reread, &HomeRedaction::Recorded);
            assert_eq!(
                report.outcome,
                RunOutcome::Halted {
                    step: verify,
                    reason: HaltReason::PreconditionFailed { command: 0, why }
                },
                "{source}"
            );
            assert!(
                !spy.calls().iter().any(|(p, _)| *p == plan.destination),
                "{source}: the destination was read before its configuration was re-checked"
            );
        }
    }
}

/// Detection, widened by work order 16 part S (§8): every check the planner
/// writes on a `VERIFY_DESTINATION`, in each direction, must be evaluable in
/// this build before the switch goes on — a removal is otherwise guarded by
/// fewer checks than 04's contract (INV-01; slice 4 review, spec 2). The
/// failure names the checks that are not. Held whatever the switch is: the
/// check this build does evaluate there stays evaluable.
#[test]
fn the_switch_needs_the_destination_contract_evaluable() {
    use macportsbrew_domain::plan::THIS_BUILD_DISPATCHES_MUTATIONS;
    use macportsbrew_domain::VerificationCheck;
    let mut not_evaluable = BTreeSet::new();
    let mut directions = BTreeSet::new();
    for source in [HB, MP] {
        // The planner's own plan, not narrowed to what this build evaluates.
        let (snap, package, to) = install_needed(source);
        let run_ = planned(&snap, package, to, false);
        let plan = run_.plan();
        let steps: Vec<_> = plan
            .steps
            .iter()
            .filter(|s| s.kind == StepKind::VerifyDestination)
            .collect();
        assert_eq!(steps.len(), 1, "{source}");
        directions.insert((plan.source.clone(), plan.destination.clone()));
        for step in steps {
            assert!(!step.verification.is_empty(), "{source}");
            for check in &step.verification {
                if !evaluable_in_this_build(step.kind, check) {
                    not_evaluable.insert(format!(
                        "{:?}",
                        macportsbrew_application::CheckKind::from(check)
                    ));
                }
            }
        }
    }
    assert_eq!(directions.len(), 2, "a plan each way");
    assert!(
        !THIS_BUILD_DISPATCHES_MUTATIONS || not_evaluable.is_empty(),
        "mutations dispatch and a VERIFY_DESTINATION guarding a removal cannot evaluate the \
         destination contract (04, Verification contract; SE-094): {not_evaluable:?}"
    );
    assert!(
        evaluable_in_this_build(
            StepKind::VerifyDestination,
            &VerificationCheck::PackageNewlyPresent
        ),
        "PACKAGE_NEWLY_PRESENT is evaluated on a VERIFY_DESTINATION"
    );
    // What the failure names in S-1: the five checks of 04's contract this
    // build cannot evaluate yet (PROVIDER_REPORTS_ACTIVE lands in S-2, the
    // other four are DP-051's, part H).
    assert_eq!(not_evaluable.len(), 5, "{not_evaluable:?}");
}
