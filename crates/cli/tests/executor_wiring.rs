// SPDX-License-Identifier: Apache-2.0

//! The mutating run loop, driven end to end behind a **forced** mutation
//! allowance (pass 08 slice 4, the work order's first definition-of-done box).
//!
//! `THIS_BUILD_DISPATCHES_MUTATIONS` stays `false`: the allowance here comes
//! from `MutationAllowance::forced_in_tests`, which exists only in test builds,
//! and every mutating command goes to a fake mutation site. Nothing touches a
//! live installation (INV-05); the state directory is a scratch one.
//!
//! **The order is backed by real state, not by the observer alone** (slice 4
//! review, M4): when the fake site is handed a command, the marker file exists
//! and names this run, and the run lock cannot be taken by anyone else; the
//! recapture runs under the lock and before the marker; the configuration
//! re-read runs under the lock.
//!
//! **What these runs do and do not reach** (slice 4, D12). MacPorts' own writes
//! elevate, and consent is not wired. Toward Homebrew the run reaches a
//! retain-source install, Homebrew's unelevated add. Toward MacPorts the add
//! elevates, so the plan's add is **narrowed away** — as if the package had
//! been installed by hand — which leaves `VERIFY_DESTINATION` and the removal
//! from Homebrew, unelevated. (A consolidation cannot stand in: since D11 its
//! `VERIFY_DESTINATION` carries no check this build can evaluate, and the gate
//! refuses it as asserting nothing.) Each plan is narrowed as `mutable` lists,
//! and every run ends `PARTIAL_SUCCESS` (exit 3), because `SNAPSHOT_BEFORE` and
//! `RECORD_HISTORY` are not performed in this build (D9).

use macportsbrew::executor::{
    run_with_mutations, Boundary, MutatingRun, MutationRefusal, MutationSite, Observer, ReadSite,
    RunReport,
};
use macportsbrew_adapters::persistence::history_store::new_run_id;
use macportsbrew_adapters::persistence::run_lock::{self, RunLock, LOCK_FILE};
use macportsbrew_adapters::persistence::run_marker::MARKER_FILE;
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_adapters::process::{ExitStatus, Invocation};
use macportsbrew_adapters::registry::{self, PlanReadError};
use macportsbrew_application::{
    clear, evaluable_in_this_build, plan_run, AnalysisOptions, ConfigurationRechecked, Disposition,
    GateClear, HaltReason, MutationAllowance, PlanRequest, PlannedRun, Precondition, RunOutcome,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandEnd, CommandLineTools, ConfigurationFile, ConfigurationScope,
    DomainError, EnvironmentSnapshot, ErrorId, HomeRedaction, InstalledPackage, Machine,
    MacosVersion, MigrationPlan, PackageIdentity, PackageKind, PackageMetadata, PlanCommand,
    Provenance, ProviderCapabilities, ProviderConfiguration, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderSnapshot, RequestedSet, RunStart, Snapshot,
    StepKind, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
};
use macportsbrew_policy::PolicyData;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const HB: &str = "homebrew";
const MP: &str = "macports";
const STAMP: &str = "20261017T030000Z";
const STARTED_AT: &str = "2026-10-17T03:00:00Z";

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

// --- the machine --------------------------------------------------------------

fn provider(id: &str, prefix: &str, packages: &[&str]) -> ProviderSnapshot {
    let pid = ProviderId::new(id);
    let mut inventory = ProviderInventory::default();
    for name in packages {
        inventory.installed.push(InstalledPackage {
            identity: PackageIdentity::new(pid.clone(), *name),
            kind: PackageKind::Package,
            version: Version::new("1.0"),
            revision: None,
            variants: BTreeSet::new(),
            active: true,
            requested: Some(true),
            installed_from_prebuilt: Some(true),
            install_path: None,
        });
        inventory
            .metadata
            .insert((*name).to_string(), PackageMetadata::default());
        inventory.availability.insert(
            (*name).to_string(),
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
        names: packages.iter().map(|n| (*n).to_string()).collect(),
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
            prefix: Some(PathBuf::from(prefix)),
            executable: None,
            architecture: Some(Architecture::Arm64),
            prefix_redacted: false,
        },
        capabilities,
        inventory,
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
        // Read and empty: no configuration risk, so DP-027 (b) does not stand
        // between the test and the mutating path.
        configuration: ProviderConfiguration::Read { files: Vec::new() },
    }
}

fn snapshot(hb: &[&str], mp: &[&str]) -> Snapshot {
    let mut providers = BTreeMap::new();
    providers.insert(ProviderId::new(HB), provider(HB, "/opt/first", hb));
    providers.insert(ProviderId::new(MP), provider(MP, "/opt/second", mp));
    Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: "test".into(),
        policy_version: "test".into(),
        provenance: Provenance {
            captured_at: STARTED_AT.into(),
            captured_by: "executor_wiring.rs".into(),
            origin: "synthesised".into(),
            note: None,
        },
        machine: Machine {
            architecture: Architecture::Arm64,
            macos: MacosVersion::new(15, 6, 1),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Installed {
                path: PathBuf::from("/Library/Developer/CommandLineTools"),
            },
        },
        environment: EnvironmentSnapshot::default(),
        providers,
    }
}

/// A plan's narrowing, each edit a statement about this build: `dryRunOnly`
/// false; `VERIFY_RESOLUTION` removed (it asserts nothing, and carries no
/// command); every check this build cannot evaluate removed; `SET_PRIMARY`'s
/// command removed (SE-036 layer 4 does not exist); toward MacPorts, the add's
/// command removed (MacPorts' writes elevate and consent is not wired);
/// renumbered.
fn mutable(plan: &mut MigrationPlan) {
    let toward_macports = plan.destination.as_str() == MP;
    plan.dry_run_only = false;
    plan.steps.retain(|s| s.kind != StepKind::VerifyResolution);
    for (i, step) in plan.steps.iter_mut().enumerate() {
        step.index = u32::try_from(i).expect("small") + 1;
        let kind = step.kind;
        step.verification
            .retain(|c| evaluable_in_this_build(kind, c));
        if kind == StepKind::SetPrimary || (toward_macports && kind == StepKind::AddDestination) {
            step.commands.clear();
        }
    }
    plan.elevated_steps.clear();
}

/// One direction of the test: the machine, the request, and the plan built
/// and narrowed from it.
struct Direction {
    snapshot: Snapshot,
    package: &'static str,
    to: &'static str,
    retain: bool,
}

impl Direction {
    /// Toward Homebrew: a retain-source install, Homebrew's unelevated add.
    fn toward_homebrew() -> Self {
        Self {
            snapshot: snapshot(&["other"], &["gsed"]),
            package: "gsed",
            to: HB,
            retain: true,
        }
    }

    /// Toward MacPorts: the add narrowed away, so the removal is Homebrew's
    /// unelevated uninstall.
    fn toward_macports() -> Self {
        Self {
            snapshot: snapshot(&["gnu-sed"], &["other"]),
            package: "gnu-sed",
            to: MP,
            retain: false,
        }
    }

    fn plan_of(&self, snap: &Snapshot) -> PlannedRun {
        plan_run(
            snap,
            &PolicyData::load_embedded().expect("the embedded policy data loads"),
            &AnalysisOptions {
                today: "2026-10-17".into(),
                advanced: false,
                only: BTreeSet::new(),
            },
            &PlanRequest {
                package: self.package.into(),
                to: ProviderId::new(self.to),
                retain_source: self.retain,
            },
            &registry::known_provider_ids(),
        )
        .expect("the planner builds this plan")
        .map_plan(mutable)
    }

    fn cleared(&self) -> GateClear {
        clear(
            self.plan_of(&self.snapshot),
            Some(MutationAllowance::forced_in_tests()),
        )
        .unwrap_or_else(|(_, report)| panic!("the gate refused: {:?}", report.refusals()))
    }
}

// --- the fakes ------------------------------------------------------------------

struct Scratch {
    base: PathBuf,
    state: StateDir,
    euid: u32,
    id: String,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "mpb-wiring-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).expect("scratch");
        let euid = std::fs::metadata(&base).expect("scratch").uid();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().expect("utf-8")))
            .expect("a state directory");
        Self {
            base,
            state,
            euid,
            id: new_run_id(STAMP),
        }
    }

    fn marker(&self) -> PathBuf {
        self.state.root().join(MARKER_FILE)
    }

    fn records(&self) -> usize {
        std::fs::read_dir(self.state.history()).map_or(0, |d| {
            d.filter(|e| {
                e.as_ref()
                    .is_ok_and(|e| !e.file_name().to_string_lossy().starts_with('.'))
            })
            .count()
        })
    }

    fn locked_by_someone(&self) -> bool {
        run_lock::acquire(&self.state, self.euid).is_err()
    }

    fn marker_names_this_run(&self) -> bool {
        std::fs::read_to_string(self.marker()).is_ok_and(|t| t.contains(&self.id))
    }

    fn start(&self) -> RunStart {
        RunStart {
            id: self.id.clone(),
            started_at: STARTED_AT.into(),
            tool_version: "test".into(),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.base).ok();
    }
}

/// What the fake site does with the command it is handed, after it has
/// checked the run's state.
type Action<'a> = Box<dyn Fn(&Scratch) -> Result<CommandEnd, MutationRefusal> + 'a>;

/// A mutation site that checks, at the moment it is handed a command, that the
/// marker is there and names this run and that the lock is held — then does
/// what the test asks.
struct Site<'a> {
    scratch: &'a Scratch,
    handed: RefCell<Vec<(ProviderId, Vec<String>)>>,
    action: Action<'a>,
}

impl<'a> Site<'a> {
    fn new(scratch: &'a Scratch, action: Action<'a>) -> Self {
        Self {
            scratch,
            handed: RefCell::new(Vec::new()),
            action,
        }
    }

    fn exits(scratch: &'a Scratch, code: i32) -> Self {
        Self::new(scratch, Box::new(move |_| Ok(CommandEnd::Exited { code })))
    }
}

impl MutationSite for Site<'_> {
    fn perform(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
        checked: ConfigurationRechecked,
        _lock: &RunLock,
    ) -> Result<CommandEnd, MutationRefusal> {
        assert_eq!(checked.provider(), provider, "the token is this step's");
        assert!(
            self.scratch.marker_names_this_run(),
            "the marker names this run before any command"
        );
        assert!(self.scratch.locked_by_someone(), "the lock is held");
        self.handed
            .borrow_mut()
            .push((provider.clone(), command.argv.clone()));
        (self.action)(self.scratch)
    }
}

#[derive(Default)]
struct Seen(RefCell<Vec<Boundary>>);

impl Observer for Seen {
    fn at(&self, boundary: Boundary, _step: Option<u32>) {
        self.0.borrow_mut().push(boundary);
    }
}

impl Seen {
    fn all(&self) -> Vec<Boundary> {
        self.0.borrow().clone()
    }
}

/// Answers every read as the provider would after the plan happened: the
/// destination named, the source missing from the inventory.
struct Reads<'p>(&'p MigrationPlan);

impl ReadSite for Reads<'_> {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        let stdout = if *provider == self.0.destination {
            format!(
                "  {} 1.0\n",
                self.0.destination_name().expect("a destination").as_str()
            )
        } else {
            "  unrelated 1.0\n".to_string()
        };
        Ok(Invocation {
            program: "x".into(),
            argv: command.argv.clone(),
            environment: BTreeMap::new(),
            status: ExitStatus::Exited { code: 0 },
            stdout,
            stderr: String::new(),
            truncated: false,
            duration_ms: 1,
        })
    }
}

/// How the test configures one run.
struct Setup<'a> {
    live: bool,
    recapture: Box<dyn FnOnce() -> Result<(Snapshot, MigrationPlan), DomainError> + 'a>,
    reread: &'a dyn Fn(&ProviderId, &str) -> Result<ProviderConfiguration, DomainError>,
}

/// A recapture that finds the machine as it was, and checks it runs under the
/// lock and before the marker.
fn unchanged<'a>(direction: &'a Direction, scratch: &'a Scratch) -> Setup<'a> {
    Setup {
        live: true,
        recapture: Box::new(move || {
            assert!(
                scratch.locked_by_someone(),
                "the recapture runs under the lock"
            );
            assert!(
                !scratch.marker().exists(),
                "and before the marker is written"
            );
            let snap = direction.snapshot.clone();
            let plan = direction.plan_of(&snap).plan().clone();
            Ok((snap, plan))
        }),
        reread: Box::leak(Box::new(move |_: &ProviderId, _: &str| {
            assert!(
                scratch.locked_by_someone(),
                "the re-read runs under the lock"
            );
            Ok(ProviderConfiguration::Read { files: Vec::new() })
        })),
    }
}

fn run<M: MutationSite>(
    cleared: &GateClear,
    scratch: &Scratch,
    site: &M,
    setup: Setup<'_>,
    seen: &Seen,
) -> Result<RunReport, DomainError> {
    let plan = cleared.run().plan().clone();
    run_with_mutations(
        cleared,
        || Reads(Box::leak(Box::new(plan))),
        MutatingRun {
            live: setup.live,
            state: &scratch.state,
            euid: scratch.euid,
            start: scratch.start(),
            site,
            recapture: setup.recapture,
            reread: setup.reread,
            observer: seen,
        },
        &HomeRedaction::Recorded,
    )
}

fn directions() -> [(&'static str, Direction); 2] {
    [
        ("toward Homebrew", Direction::toward_homebrew()),
        ("toward MacPorts", Direction::toward_macports()),
    ]
}

// --- the order ------------------------------------------------------------------

#[test]
fn a_mutating_run_goes_probe_lock_replan_marker_check_dispatch_record_clear() {
    use Boundary::*;
    for (name, direction) in directions() {
        let scratch = Scratch::new("order");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let report = run(
            &direction.cleared(),
            &scratch,
            &site,
            unchanged(&direction, &scratch),
            &seen,
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            seen.all(),
            [
                Probed,
                Locked,
                Replanned,
                MarkerRead,
                MarkerWritten,
                StillCurrent,
                ConfigurationRechecked,
                Dispatched,
                RecordWritten,
                MarkerCleared,
            ],
            "{name}: {report:?}"
        );
        assert_eq!(site.handed.borrow().len(), 1, "{name}");
        // The brackets are not performed in this build: never a completion.
        assert!(
            matches!(report.outcome, RunOutcome::PartialSuccess { .. }),
            "{name}: {:?}",
            report.outcome
        );
        assert_eq!(report.exit_code(), 3, "{name}");
        assert_eq!(scratch.records(), 1, "{name}: one durable record");
        assert!(!scratch.marker().exists(), "{name}: the marker is cleared");
        assert!(report.marker_left.is_none() && report.history_error.is_none());
        assert!(
            report
                .records
                .iter()
                .any(|r| matches!(r.disposition, Disposition::Performed { .. })),
            "{name}"
        );
    }
}

// --- each boundary made to fail -------------------------------------------------

#[test]
fn a_run_that_is_not_live_is_refused_before_the_state_directory_is_touched() {
    // INV-15: under `--replay` or `--from-snapshot`, with a forced allowance.
    let direction = Direction::toward_homebrew();
    let scratch = Scratch::new("not-live");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let mut setup = unchanged(&direction, &scratch);
    setup.live = false;
    let error = run(&direction.cleared(), &scratch, &site, setup, &seen).expect_err("refused");
    assert_eq!(error.id, ErrorId::SafetyBlocked);
    assert!(seen.all().is_empty());
    assert!(!scratch.state.root().exists(), "nothing was created");
    assert!(site.handed.borrow().is_empty());
}

#[test]
fn without_an_allowance_the_mutating_run_is_not_entered() {
    // A witness cleared without an allowance — a plan with no mutating step,
    // which the gate passes for the read-only loop — is refused by the
    // mutating run before anything is touched (`INTERNAL`).
    let direction = Direction::toward_homebrew();
    let read_only = direction.plan_of(&direction.snapshot).map_plan(|p| {
        p.dry_run_only = true;
        p.steps.retain(|s| s.kind == StepKind::VerifyDestination);
        for (i, step) in p.steps.iter_mut().enumerate() {
            step.index = u32::try_from(i).expect("small") + 1;
        }
    });
    let cleared = clear(read_only, None).unwrap_or_else(|(_, r)| panic!("{:?}", r.refusals()));
    assert!(cleared.allowance().is_none());
    let scratch = Scratch::new("no-allowance");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let error = run(
        &cleared,
        &scratch,
        &site,
        unchanged(&direction, &scratch),
        &seen,
    )
    .expect_err("refused");
    assert_eq!(error.id, ErrorId::Internal);
    assert!(seen.all().is_empty());
    assert!(site.handed.borrow().is_empty());
    assert!(!scratch.state.root().exists());
    // And the narrowed mutating plan is refused by the gate without one.
    assert!(clear(direction.plan_of(&direction.snapshot), None).is_err());
}

#[test]
fn invariant_02_the_mutation_site_is_handed_exactly_the_plans_command() {
    for (name, direction) in directions() {
        let scratch = Scratch::new("inv02");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let cleared = direction.cleared();
        run(
            &cleared,
            &scratch,
            &site,
            unchanged(&direction, &scratch),
            &seen,
        )
        .expect("a report");
        let planned: Vec<(ProviderId, Vec<String>)> = cleared
            .run()
            .plan()
            .steps
            .iter()
            .filter(|s| !s.commands.is_empty() && s.kind.is_mutating())
            .flat_map(|s| {
                s.commands
                    .iter()
                    .map(|c| (s.provider.clone().expect("a provider"), c.argv.clone()))
            })
            .collect();
        assert_eq!(*site.handed.borrow(), planned, "{name}");
    }
}

#[test]
fn invariant_01_a_destination_that_is_not_there_stops_the_removal() {
    // Toward MacPorts the removal follows VERIFY_DESTINATION; a read that does
    // not name the destination halts it, and the removal is never handed over.
    let direction = Direction::toward_macports();
    let scratch = Scratch::new("inv01");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let cleared = direction.cleared();
    let report = run_with_mutations(
        &cleared,
        || Absent,
        MutatingRun {
            live: true,
            state: &scratch.state,
            euid: scratch.euid,
            start: scratch.start(),
            site: &site,
            recapture: unchanged(&direction, &scratch).recapture,
            reread: unchanged(&direction, &scratch).reread,
            observer: &seen,
        },
        &HomeRedaction::Recorded,
    )
    .expect("a report");
    assert!(site.handed.borrow().is_empty(), "nothing was removed");
    assert!(report
        .records
        .iter()
        .any(|r| r.kind == StepKind::VerifyDestination
            && matches!(r.disposition, Disposition::Halted { .. })));
    assert!(!scratch.marker().exists() && scratch.records() == 0);
}

/// Answers every read with an inventory that names nothing the plan moves.
struct Absent;

impl ReadSite for Absent {
    fn read(
        &self,
        _provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        Ok(Invocation {
            program: "x".into(),
            argv: command.argv.clone(),
            environment: BTreeMap::new(),
            status: ExitStatus::Exited { code: 0 },
            stdout: "  unrelated 1.0\n".to_string(),
            stderr: String::new(),
            truncated: false,
            duration_ms: 1,
        })
    }
}

#[test]
fn a_held_lock_refuses_and_nothing_is_dispatched() {
    let direction = Direction::toward_homebrew();
    let scratch = Scratch::new("held");
    scratch.state.ensure().expect("state");
    let _other = run_lock::acquire(&scratch.state, scratch.euid).expect("another run's lock");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let error = run(
        &direction.cleared(),
        &scratch,
        &site,
        unchanged(&direction, &scratch),
        &seen,
    )
    .expect_err("refused");
    assert_eq!(error.id, ErrorId::RunLockHeld);
    assert_eq!(seen.all(), [Boundary::Probed]);
    assert!(site.handed.borrow().is_empty());
}

#[test]
fn an_earlier_runs_marker_refuses_and_nothing_is_dispatched() {
    let direction = Direction::toward_homebrew();
    let scratch = Scratch::new("interrupted");
    scratch.state.ensure().expect("state");
    let earlier = "20261016T120000Z-00000000000000aa";
    std::fs::write(
        scratch.marker(),
        format!("{{\"schemaVersion\":1,\"runId\":\"{earlier}\"}}\n"),
    )
    .expect("marker");
    std::fs::set_permissions(
        scratch.marker(),
        std::os::unix::fs::PermissionsExt::from_mode(0o600),
    )
    .expect("mode");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let mut setup = unchanged(&direction, &scratch);
    let d = &direction;
    // The earlier run's marker is there before the recapture, by design.
    setup.recapture = Box::new(move || {
        let snap = d.snapshot.clone();
        let plan = d.plan_of(&snap).plan().clone();
        Ok((snap, plan))
    });
    let error = run(&direction.cleared(), &scratch, &site, setup, &seen).expect_err("refused");
    assert_eq!(error.id, ErrorId::RunInterrupted);
    assert_eq!(error.detail, earlier);
    assert!(site.handed.borrow().is_empty());
    assert_eq!(scratch.records(), 0);
}

#[test]
fn a_machine_that_changed_under_the_lock_refuses_and_nothing_is_dispatched() {
    // D6: re-planned under the lock; a different plan refuses, the fact named.
    for (name, direction) in directions() {
        let scratch = Scratch::new("changed");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let mut setup = unchanged(&direction, &scratch);
        let d = &direction;
        setup.recapture = Box::new(move || {
            let mut snap = d.snapshot.clone();
            for provider in snap.providers.values_mut() {
                provider.detection.presence = ProviderPresence::Absent;
            }
            let plan = d.plan_of(&d.snapshot).plan().clone();
            let mut fresh = plan;
            fresh.policy_version.push('x');
            Ok((snap, fresh))
        });
        let report = run(&direction.cleared(), &scratch, &site, setup, &seen).expect("a report");
        assert!(
            matches!(
                report.outcome,
                RunOutcome::Halted {
                    step: 0,
                    reason: HaltReason::StateChanged { .. }
                }
            ),
            "{name}: {:?}",
            report.outcome
        );
        assert_eq!(report.exit_code(), 4, "{name}");
        assert!(site.handed.borrow().is_empty(), "{name}");
        assert!(
            !scratch.marker().exists() && scratch.records() == 0,
            "{name}"
        );
    }
}

#[test]
fn a_configuration_that_drifted_or_could_not_be_read_stops_the_step_before_its_command() {
    for (what, reread, why) in [
        (
            "unreadable",
            Box::new(
                |_: &ProviderId, _: &str| -> Result<ProviderConfiguration, DomainError> {
                    Err(DomainError::new(ErrorId::SafetyBlocked, "not detected"))
                },
            )
                as Box<dyn Fn(&ProviderId, &str) -> Result<ProviderConfiguration, DomainError>>,
            Precondition::ConfigurationUnreadable,
        ),
        (
            "drifted",
            Box::new(|_: &ProviderId, _: &str| {
                Ok(ProviderConfiguration::Read {
                    files: vec![ConfigurationFile {
                        path: "/opt/first/etc/brew.env".into(),
                        scope: ConfigurationScope::Prefix,
                        readable: true,
                        keys: ["HOMEBREW_NO_AUTO_UPDATE".to_string()]
                            .into_iter()
                            .collect(),
                    }],
                })
            }),
            Precondition::ConfigurationDrift,
        ),
        (
            "unknowable",
            Box::new(|_: &ProviderId, _: &str| Ok(ProviderConfiguration::Unknowable)),
            Precondition::ConfigurationDrift,
        ),
    ] {
        let direction = Direction::toward_homebrew();
        let scratch = Scratch::new("drift");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let mut setup = unchanged(&direction, &scratch);
        setup.reread = &*Box::leak(reread);
        let report = run(&direction.cleared(), &scratch, &site, setup, &seen).expect("a report");
        let halt = report.records.iter().find_map(|r| match &r.disposition {
            Disposition::Halted { reason, .. } => Some(reason.clone()),
            _ => None,
        });
        assert_eq!(
            halt,
            Some(HaltReason::PreconditionFailed { command: 0, why }),
            "{what}"
        );
        assert!(site.handed.borrow().is_empty(), "{what}");
        // Nothing reached the site, so nothing is recorded, and the marker the
        // run wrote is taken back at once (D8).
        assert_eq!(scratch.records(), 0, "{what}");
        assert!(!scratch.marker().exists(), "{what}");
        assert!(!seen.all().contains(&Boundary::Dispatched), "{what}");
    }
}

/// Records the boundaries and, once the marker is written, replaces
/// `run.lock` with another file: the lock this run holds is no longer the one
/// at the path, which is what `verify_still_current` notices.
struct LosesTheLock<'a> {
    scratch: &'a Scratch,
    seen: Seen,
}

impl Observer for LosesTheLock<'_> {
    fn at(&self, boundary: Boundary, step: Option<u32>) {
        self.seen.at(boundary, step);
        if boundary == Boundary::MarkerWritten {
            let path = self.scratch.state.root().join(LOCK_FILE);
            std::fs::rename(&path, self.scratch.base.join("old.lock")).expect("moved aside");
            std::fs::write(&path, b"").expect("another lock file");
        }
    }
}

#[test]
fn a_lock_lost_before_the_command_stops_the_step_before_the_re_read() {
    // The mutation proof's two lock survivors: no test ever lost the lock, so
    // neither dropping the still-current check nor moving it after the
    // configuration re-read failed anything (plan §4, item 2).
    for (name, direction) in directions() {
        let scratch = Scratch::new("lock-lost");
        let site = Site::exits(&scratch, 0);
        let observer = LosesTheLock {
            scratch: &scratch,
            seen: Seen::default(),
        };
        let reread_called = std::cell::Cell::new(false);
        let mut setup = unchanged(&direction, &scratch);
        setup.reread = &*Box::leak(Box::new(|_: &ProviderId, _: &str| {
            reread_called.set(true);
            Ok(ProviderConfiguration::Read { files: Vec::new() })
        }));
        let plan = direction.cleared().run().plan().clone();
        let report = run_with_mutations(
            &direction.cleared(),
            || Reads(Box::leak(Box::new(plan))),
            MutatingRun {
                live: setup.live,
                state: &scratch.state,
                euid: scratch.euid,
                start: scratch.start(),
                site: &site,
                recapture: setup.recapture,
                reread: setup.reread,
                observer: &observer,
            },
            &HomeRedaction::Recorded,
        );
        let halt = report.as_ref().ok().and_then(|report| {
            report.records.iter().find_map(|r| match &r.disposition {
                Disposition::Halted { reason, .. } => Some(reason.clone()),
                _ => None,
            })
        });
        assert_eq!(
            halt,
            Some(HaltReason::PreconditionFailed {
                command: 0,
                why: Precondition::LockLost,
            }),
            "{name}: {report:?}"
        );
        assert!(site.handed.borrow().is_empty(), "{name}");
        assert!(
            !reread_called.get(),
            "{name}: the re-read comes after the check"
        );
        let seen = observer.seen.all();
        assert!(!seen.contains(&Boundary::StillCurrent), "{name}: {seen:?}");
        assert!(!seen.contains(&Boundary::Dispatched), "{name}: {seen:?}");
        assert_ne!(report.map(|r| r.exit_code()).ok(), Some(0), "{name}");
    }
}

#[test]
fn a_refused_or_failed_command_is_recorded_as_dispatched() {
    for (what, action) in [
        (
            "refused",
            Box::new(|_: &Scratch| Err(MutationRefusal::Refused)) as Action<'static>,
        ),
        (
            "exit 1",
            Box::new(|_: &Scratch| Ok(CommandEnd::Exited { code: 1 })),
        ),
        (
            "did not finish",
            Box::new(|_: &Scratch| Ok(CommandEnd::DidNotFinish)),
        ),
        (
            "transport",
            Box::new(|_: &Scratch| Err(MutationRefusal::Transport(ErrorId::Io))),
        ),
    ] {
        for (name, direction) in directions() {
            let scratch = Scratch::new("failed");
            let site = Site::new(&scratch, Box::new(|s| action(s)));
            let seen = Seen::default();
            let report = run(
                &direction.cleared(),
                &scratch,
                &site,
                unchanged(&direction, &scratch),
                &seen,
            )
            .expect("a report");
            assert_ne!(report.exit_code(), 0, "{what} {name}");
            assert_eq!(scratch.records(), 1, "{what} {name}: recorded");
            assert!(
                !scratch.marker().exists(),
                "{what} {name}: cleared after it"
            );
        }
    }
}

// --- the endings against the marker (D8) ----------------------------------------

#[test]
fn a_record_that_cannot_be_written_keeps_the_marker_and_never_exits_zero() {
    let direction = Direction::toward_homebrew();
    let scratch = Scratch::new("no-record");
    // Something is already at the record's name: the store never replaces.
    let site = Site::new(
        &scratch,
        Box::new(|s| {
            std::fs::create_dir_all(s.state.history()).expect("history");
            std::fs::write(s.state.history().join(format!("run-{}.json", s.id)), "{}")
                .expect("squatter");
            Ok(CommandEnd::Exited { code: 0 })
        }),
    );
    let seen = Seen::default();
    let report = run(
        &direction.cleared(),
        &scratch,
        &site,
        unchanged(&direction, &scratch),
        &seen,
    )
    .expect("a report");
    assert_eq!(report.history_error, Some(ErrorId::Io));
    assert_eq!(report.marker_left.as_deref(), Some(scratch.id.as_str()));
    assert_ne!(report.exit_code(), 0);
    assert!(scratch.marker().exists(), "the marker stays");
    assert!(!seen.all().contains(&Boundary::RecordWritten));
}

#[test]
fn a_marker_gone_or_another_runs_after_the_record_is_an_error_and_stays() {
    for (what, action) in [
        (
            "another run's",
            Box::new(|s: &Scratch| {
                std::fs::write(
                    s.marker(),
                    "{\"schemaVersion\":1,\"runId\":\"20261016T120000Z-00000000000000bb\"}\n",
                )
                .expect("swap");
                Ok(CommandEnd::Exited { code: 0 })
            }) as Action<'static>,
        ),
        (
            "gone",
            Box::new(|s: &Scratch| {
                std::fs::remove_file(s.marker()).expect("removed");
                Ok(CommandEnd::Exited { code: 0 })
            }),
        ),
        (
            "lock file deleted, so clear cannot confirm the lock",
            Box::new(|s: &Scratch| {
                std::fs::remove_file(s.state.root().join(LOCK_FILE)).expect("removed");
                Ok(CommandEnd::Exited { code: 0 })
            }),
        ),
    ] {
        let direction = Direction::toward_homebrew();
        let scratch = Scratch::new("anomaly");
        let site = Site::new(&scratch, Box::new(|s| action(s)));
        let seen = Seen::default();
        let report = run(
            &direction.cleared(),
            &scratch,
            &site,
            unchanged(&direction, &scratch),
            &seen,
        )
        .expect("a report");
        assert_eq!(
            report.history_error,
            Some(ErrorId::HistoryUnusable),
            "{what}"
        );
        assert_eq!(
            report.marker_left.as_deref(),
            Some(scratch.id.as_str()),
            "{what}"
        );
        assert_ne!(report.exit_code(), 0, "{what}");
        assert_eq!(scratch.records(), 1, "{what}: the record was written first");
        assert!(!seen.all().contains(&Boundary::MarkerCleared), "{what}");
    }
}

#[test]
fn a_completed_run_whose_history_went_wrong_never_exits_zero() {
    // Unreachable through a run in this build (every mutating run is partial,
    // D9), so the mutation proof found the `(0, Some(_))` branch untested.
    let report = |history_error| RunReport {
        outcome: RunOutcome::Completed,
        records: Vec::new(),
        refusal: None,
        gate_refusals: Vec::new(),
        history_error,
        marker_left: None,
        record_written: false,
    };
    assert_eq!(report(None).exit_code(), 0);
    for error in [ErrorId::Io, ErrorId::HistoryUnusable] {
        assert_eq!(report(Some(error)).exit_code(), error.exit_code());
        assert_ne!(error.exit_code(), 0, "{error:?}");
    }
}
