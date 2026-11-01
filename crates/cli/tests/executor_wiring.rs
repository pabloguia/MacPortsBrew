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
    run_with_mutations, Boundary, MutatingRun, MutationRefusal, MutationSite, Observer, ReadBack,
    ReadSite, RunReport,
};
use macportsbrew_adapters::persistence::history_store::{self, new_run_id};
use macportsbrew_adapters::persistence::run_lock::{self, RunLock, LOCK_FILE};
use macportsbrew_adapters::persistence::run_marker::MARKER_FILE;
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_adapters::persistence::{snapshot_file_name, snapshot_store};
use macportsbrew_adapters::process::{ExitStatus, Invocation};
use macportsbrew_adapters::registry::{self, PlanReadError};
use macportsbrew_application::{
    clear, confirm_shown, evaluable_in_this_build, plan_run, tie, AnalysisOptions,
    ConfigurationRechecked, ConsentCode, ConsentDecision, ConsentGiven, Disposition,
    ElevationRequest, GateClear, HaltReason, MutationAllowance, PlanDigest, PlanRequest,
    PlannedRun, Precondition, RunOutcome, ShownPlan,
};
use macportsbrew_domain::{
    Architecture, ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Capability,
    CapabilityState, CommandEnd, CommandLineTools, ConfigurationFile, ConfigurationScope,
    DomainError, EnvironmentSnapshot, ErrorId, HomeRedaction, InstalledPackage, Machine,
    MacosVersion, MigrationPlan, PackageIdentity, PackageKind, PackageMetadata, PlanCommand,
    Provenance, ProviderCapabilities, ProviderConfiguration, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderSnapshot, RequestedSet, RunStart, Snapshot,
    StepKind, SuccessSemantic, Translation, Version, SNAPSHOT_SCHEMA_VERSION,
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
/// What the capture under the lock is stamped with in these tests: not the first
/// capture's stamp, so a snapshot of the first capture cannot pass for the
/// second (slice 5, D1).
const RECAPTURED_AT: &str = "2026-10-17T03:00:09Z";
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
        inventory.metadata.insert(
            (*name).to_string(),
            PackageMetadata::default().with_service(macportsbrew_domain::ServiceDefinition::both(
                macportsbrew_domain::Answer::No,
            )),
        );
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
        // What the adapters build for this prefix: the run writes this snapshot
        // and reads it back through the checks any snapshot passes, so it has to
        // be one the tool could have captured.
        invocation_environment: if id == HB {
            macportsbrew_adapters::homebrew::quiet_environment(std::path::Path::new(prefix), None)
        } else {
            macportsbrew_adapters::macports::invocation_environment(
                std::path::Path::new(prefix),
                None,
            )
        },
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
    /// Narrowed further so the run can end `COMPLETED`: `SET_PRIMARY` removed
    /// whole (layer 4 of SE-036 does not exist, and a passed-over step is not a
    /// carried-out one). A real planned run cannot complete until that layer and
    /// `VERIFY_RESOLUTION` exist; the slice 5 report carries it.
    complete: bool,
}

impl Direction {
    /// Toward Homebrew: a retain-source install, Homebrew's unelevated add.
    fn toward_homebrew() -> Self {
        Self {
            snapshot: snapshot(&["other"], &["gsed"]),
            package: "gsed",
            to: HB,
            retain: true,
            complete: false,
        }
    }

    /// Toward Homebrew, narrowed so that every step is carried out.
    fn toward_homebrew_completable() -> Self {
        Self {
            complete: true,
            ..Self::toward_homebrew()
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
            complete: false,
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
        .map_plan(|plan| {
            mutable(plan);
            if self.complete {
                plan.steps.retain(|s| s.kind != StepKind::SetPrimary);
                for (i, step) in plan.steps.iter_mut().enumerate() {
                    step.index = u32::try_from(i).expect("small") + 1;
                }
            }
        })
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

    /// The marker names a snapshot file, and that file is there (slice 5, D1).
    fn marker_names_a_snapshot(&self) -> bool {
        let Ok(text) = std::fs::read_to_string(self.marker()) else {
            return false;
        };
        let Some(rest) = text.split("\"snapshotFile\":\"").nth(1) else {
            return false;
        };
        let name = rest.split('"').next().unwrap_or("");
        macportsbrew_domain::history::is_snapshot_file_name(name)
            && self.state.snapshots().join(name).is_file()
    }

    fn snapshots(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.state.snapshots())
            .map(|d| {
                d.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
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
        consent: Option<ConsentGiven>,
        _lock: &RunLock,
    ) -> Result<CommandEnd, MutationRefusal> {
        assert!(
            consent.is_none(),
            "no consent reaches a site while the guard halts every elevated command"
        );
        assert_eq!(checked.provider(), provider, "the token is this step's");
        assert_eq!(
            checked.program(),
            command.program,
            "the re-check was made for this command's program (SR 9)"
        );
        assert!(
            self.scratch.marker_names_this_run(),
            "the marker names this run before any command"
        );
        assert!(
            self.scratch.marker_names_a_snapshot(),
            "and the snapshot taken first, which is in the state directory"
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

/// What the destination's read `command` prints when the package is there and
/// active: a presence read names it, an activation read marks it in its
/// provider's own shape (R-20) — work order 16 part S, S-2.
fn destination_present(plan: &MigrationPlan, command: &PlanCommand) -> String {
    let name = plan.destination_name().expect("a destination").as_str();
    match command.success {
        SuccessSemantic::StdoutMarksPackageActive => format!("  {name} @1.0_0 (active)\n"),
        SuccessSemantic::JsonReportsPackageLinked => format!(
            r#"{{"formulae": [{{"name": "{name}", "linked_keg": "1.0", "installed": [{{"version": "1.0"}}]}}], "casks": []}}"#
        ),
        _ => format!("  {name} 1.0\n"),
    }
}

/// Answers every read as the provider would after the plan happened: the
/// destination named and active, the source missing from the inventory.
struct Reads<'p>(&'p MigrationPlan);

impl ReadSite for Reads<'_> {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        let stdout = if *provider == self.0.destination {
            destination_present(self.0, command)
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

/// A configuration re-read that finds nothing that could outrank the plan.
#[allow(clippy::unnecessary_wraps)]
fn clean_reread(_: &ProviderId, _: &str) -> Result<ProviderConfiguration, DomainError> {
    Ok(ProviderConfiguration::Read { files: Vec::new() })
}

/// How the test configures one run.
struct Setup<'a> {
    live: bool,
    recapture: Box<dyn Fn() -> Result<(Snapshot, MigrationPlan), DomainError> + 'a>,
    reread: &'a dyn Fn(&ProviderId, &str) -> Result<ProviderConfiguration, DomainError>,
    read_back: &'a dyn Fn(&StateDir, &str, u32) -> Result<String, DomainError>,
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
            let mut snap = direction.snapshot.clone();
            let plan = direction.plan_of(&snap).plan().clone();
            // The capture under the lock is stamped later than the first one,
            // so the snapshot a run writes says which of the two it is.
            snap.provenance.captured_at = RECAPTURED_AT.into();
            Ok((snap, plan))
        }),
        reread: Box::leak(Box::new(move |_: &ProviderId, _: &str| {
            assert!(
                scratch.locked_by_someone(),
                "the re-read runs under the lock"
            );
            Ok(ProviderConfiguration::Read { files: Vec::new() })
        })),
        read_back: &history_store::read_back,
    }
}

/// The consent seam of a run that has nothing to ask about: every run the gate
/// clears in this build, since it refuses every elevated step. Asked at all,
/// it fails the test.
fn never_asked(requests: &[ElevationRequest]) -> ConsentDecision {
    panic!("a cleared run asked for consent: {requests:?}")
}

/// The witness that `cleared`'s plan was shown and confirmed, minted the only
/// way there is: the exact phrase with its code.
fn shown(cleared: &GateClear) -> ShownPlan {
    confirm_shown(
        cleared,
        &ConsentCode::from_bytes([0, 0, 0]),
        "run plan 000000",
    )
    .expect("the exact phrase confirms the plan shown")
}

fn run<M: MutationSite>(
    cleared: &GateClear,
    scratch: &Scratch,
    site: &M,
    setup: &Setup<'_>,
    seen: &Seen,
) -> Result<RunReport, DomainError> {
    let plan = cleared.run().plan().clone();
    run_with_mutations(
        cleared,
        || Reads(Box::leak(Box::new(plan))),
        MutatingRun {
            shown: shown(cleared),
            live: setup.live,
            state: &scratch.state,
            euid: scratch.euid,
            start: scratch.start(),
            site,
            recapture: &*setup.recapture,
            consent: &never_asked,
            reread: setup.reread,
            read_back: setup.read_back,
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
    let mut removed_somewhere = false;
    for (name, direction) in directions() {
        let scratch = Scratch::new("order");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let report = run(
            &direction.cleared(),
            &scratch,
            &site,
            &unchanged(&direction, &scratch),
            &seen,
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        // A removal is preceded by the destination's configuration re-check
        // and its re-read, before the lock check and the source's re-check
        // (work order 16 part S, M1); an add is not.
        let removes = report.records.iter().any(|r| {
            r.kind == StepKind::RemoveSource
                && matches!(r.disposition, Disposition::Performed { .. })
        });
        let mut expected = vec![
            Probed,
            Locked,
            Replanned,
            MarkerRead,
            SnapshotWritten,
            MarkerWritten,
        ];
        if removes {
            removed_somewhere = true;
            expected.extend([ConfigurationRechecked, DestinationReverified]);
        }
        expected.extend([
            StillCurrent,
            ConfigurationRechecked,
            Dispatched,
            RecordWritten,
            RecordReadBack,
            MarkerCleared,
        ]);
        assert_eq!(seen.all(), expected, "{name}: {report:?}");
        assert_eq!(site.handed.borrow().len(), 1, "{name}");
        // `SET_PRIMARY` is passed over (SE-036 layer 4 does not exist), and a
        // passed-over step is not a carried-out one: never a completion here.
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
    assert!(
        removed_somewhere,
        "a removal ran in one direction, so the re-read order was checked"
    );
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
    let error = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect_err("refused");
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
        &unchanged(&direction, &scratch),
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
            &unchanged(&direction, &scratch),
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
            shown: shown(&cleared),
            live: true,
            state: &scratch.state,
            euid: scratch.euid,
            start: scratch.start(),
            site: &site,
            recapture: &*unchanged(&direction, &scratch).recapture,
            consent: &never_asked,
            reread: unchanged(&direction, &scratch).reread,
            read_back: &history_store::read_back,
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
        &unchanged(&direction, &scratch),
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
        format!("{{\"schemaVersion\":2,\"runId\":\"{earlier}\",\"snapshotFile\":null}}\n"),
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
    let error = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect_err("refused");
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
        let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
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
                        analytics_keys: std::collections::BTreeSet::new(),
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
        let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
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
        let plan = direction.cleared().run().plan().clone();
        let reread_providers: RefCell<Vec<ProviderId>> = RefCell::new(Vec::new());
        let mut setup = unchanged(&direction, &scratch);
        setup.reread = &*Box::leak(Box::new(|provider: &ProviderId, _: &str| {
            reread_providers.borrow_mut().push(provider.clone());
            Ok(ProviderConfiguration::Read { files: Vec::new() })
        }));
        let report = run_with_mutations(
            &direction.cleared(),
            || Reads(Box::leak(Box::new(plan.clone()))),
            MutatingRun {
                shown: shown(&direction.cleared()),
                live: setup.live,
                state: &scratch.state,
                euid: scratch.euid,
                start: scratch.start(),
                site: &site,
                recapture: &*setup.recapture,
                consent: &never_asked,
                reread: setup.reread,
                read_back: setup.read_back,
                observer: &observer,
            },
            &HomeRedaction::Recorded,
        );
        let halted = report.as_ref().ok().and_then(|report| {
            report.records.iter().find_map(|r| match &r.disposition {
                Disposition::Halted { reason, .. } => Some((r.index, reason.clone())),
                _ => None,
            })
        });
        let (index, halt) = halted.unwrap_or_else(|| panic!("{name}: no halt: {report:?}"));
        assert_eq!(
            halt,
            HaltReason::PreconditionFailed {
                command: 0,
                why: Precondition::LockLost,
            },
            "{name}: {report:?}"
        );
        assert!(site.handed.borrow().is_empty(), "{name}");
        // The halted step's own provider is never re-read: its re-read comes
        // after the lock check. The only re-read allowed before it is a
        // removal's destination re-read (M1), and only when it certified.
        let seen = observer.seen.all();
        let halted_provider = plan
            .steps
            .iter()
            .find(|s| s.index == index)
            .and_then(|s| s.provider.clone())
            .expect("the halted step has a provider");
        let reread = reread_providers.borrow().clone();
        assert!(
            !reread.contains(&halted_provider),
            "{name}: the step's own re-read comes after the check: {reread:?}"
        );
        if seen.contains(&Boundary::DestinationReverified) {
            assert!(
                reread.iter().all(|p| *p == plan.destination),
                "{name}: {reread:?}"
            );
        } else {
            assert!(reread.is_empty(), "{name}: no re-read at all: {reread:?}");
        }
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
                &unchanged(&direction, &scratch),
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
        &unchanged(&direction, &scratch),
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
                    "{\"schemaVersion\":2,\"runId\":\"20261016T120000Z-00000000000000bb\",\"snapshotFile\":null}\n",
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
        // The add deletes the lock file in one case; the certifying read's
        // configuration re-read that follows is not a mutation and is not
        // under the lock check, so this re-read does not assert it.
        let mut setup = unchanged(&direction, &scratch);
        setup.reread = &clean_reread;
        let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
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
    // Unreachable through a run whose steps are all carried out only on a
    // narrowed plan, so the mutation proof found the `(0, Some(_))` branch
    // untested; this builds the report by hand.
    let report = |history_error| RunReport {
        outcome: RunOutcome::Completed,
        records: Vec::new(),
        refusal: None,
        gate_refusals: Vec::new(),
        history_error,
        marker_left: None,
        record_written: false,
        receipt: macportsbrew::executor::RunReceipt::default(),
    };
    assert_eq!(report(None).exit_code(), 0);
    for error in [ErrorId::Io, ErrorId::HistoryUnusable] {
        assert_eq!(report(Some(error)).exit_code(), error.exit_code());
        assert_ne!(error.exit_code(), 0, "{error:?}");
    }
}

// --- Pass 08 slice 5: the brackets performed ------------------------------------

/// The run's record, read back from the scratch store.
fn stored(scratch: &Scratch) -> macportsbrew_domain::RunRecord {
    history_store::read_one(&scratch.state, &scratch.id, scratch.euid)
        .expect("the record reads")
        .expect("the record is there")
}

#[test]
fn each_re_check_is_made_for_the_program_it_is_spent_on() {
    // SR 9: the shell re-reads for the program of the command (or of the
    // verify step it re-reads) and records that program in the token; the
    // fake site asserts the token's program equals the command's.
    // Both directions: toward MacPorts the source's program and the verify
    // step's differ, so a shell re-checking the source's removal with the
    // verify program is refused at dispatch (mutant 6).
    for (name, direction) in [
        ("toward homebrew", Direction::toward_homebrew_completable()),
        ("toward macports", Direction::toward_macports()),
    ] {
        let scratch = Scratch::new("program-token");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let plan = direction.cleared().run().plan().clone();
        let asked: &'static RefCell<Vec<(ProviderId, String)>> =
            Box::leak(Box::new(RefCell::new(Vec::new())));
        let mut setup = unchanged(&direction, &scratch);
        setup.reread = &*Box::leak(Box::new(move |provider: &ProviderId, program: &str| {
            asked
                .borrow_mut()
                .push((provider.clone(), program.to_string()));
            Ok(ProviderConfiguration::Read { files: Vec::new() })
        }));
        let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
        assert!(
            !site.handed.borrow().is_empty(),
            "{name}: a command was dispatched: {report:?}"
        );
        let asked = asked.borrow().clone();
        assert!(!asked.is_empty());
        for (provider, program) in &asked {
            assert!(!program.is_empty(), "never re-read for no program");
            assert!(
                plan.steps
                    .iter()
                    .any(|s| s.provider.as_ref() == Some(provider)
                        && s.commands.iter().any(|c| &c.program == program)),
                "{provider:?} re-read for a program no step of it names: {program}"
            );
        }
        for (provider, argv) in site.handed.borrow().iter() {
            let command = plan
                .steps
                .iter()
                .filter(|s| s.provider.as_ref() == Some(provider))
                .flat_map(|s| &s.commands)
                .find(|c| &c.argv == argv)
                .expect("a handed command is the plan's");
            assert!(
                asked.contains(&(provider.clone(), command.program.clone())),
                "{name}: the command's own program was re-read: {asked:?}"
            );
        }
    }
}

#[test]
fn a_forced_run_toward_homebrew_reaches_success_with_its_snapshot_written_and_named() {
    use macportsbrew_domain::{RunStatus, StepResult};
    use Boundary::*;
    let direction = Direction::toward_homebrew_completable();
    let scratch = Scratch::new("success");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let report = run(
        &direction.cleared(),
        &scratch,
        &site,
        &unchanged(&direction, &scratch),
        &seen,
    )
    .expect("a report");
    assert_eq!(report.outcome, RunOutcome::Completed, "{report:?}");
    assert_eq!(report.exit_code(), 0);
    assert!(report.history_error.is_none() && report.marker_left.is_none());
    assert!(!scratch.marker().exists(), "the marker is cleared");

    let record = stored(&scratch);
    assert_eq!(record.status(), RunStatus::Success);
    let file = record
        .snapshot_path()
        .expect("the record names its snapshot");
    assert_eq!(report.receipt.snapshot_file.as_deref(), Some(file));
    assert_eq!(report.receipt.run_id.as_deref(), Some(scratch.id.as_str()));
    assert!(scratch.state.snapshots().join(file).is_file());
    assert_eq!(
        scratch.snapshots(),
        [file.to_string()],
        "one snapshot, no leftovers"
    );
    // The record never certifies its own write: pending on disk.
    let results: Vec<_> = record
        .steps()
        .iter()
        .map(|s| (s.kind(), s.result()))
        .collect();
    assert!(results
        .iter()
        .any(|(k, r)| *k == StepKind::RecordHistory
            && matches!(r, StepResult::RecordPendingReadBack)));
    assert!(results
        .iter()
        .any(|(k, r)| *k == StepKind::SnapshotBefore && matches!(r, StepResult::SnapshotTaken)));
    // The order of a run that completes.
    assert_eq!(
        seen.all(),
        [
            Probed,
            Locked,
            Replanned,
            MarkerRead,
            SnapshotWritten,
            MarkerWritten,
            StillCurrent,
            ConfigurationRechecked,
            Dispatched,
            RecordWritten,
            RecordReadBack,
            MarkerCleared,
        ]
    );
}

#[test]
fn the_snapshot_is_the_capture_taken_under_the_lock_not_the_first_one() {
    let direction = Direction::toward_homebrew_completable();
    let scratch = Scratch::new("which-capture");
    let site = Site::exits(&scratch, 0);
    run(
        &direction.cleared(),
        &scratch,
        &site,
        &unchanged(&direction, &scratch),
        &Seen::default(),
    )
    .expect("a report");
    // The two captures differ in what they are stamped with, so the file says
    // which one it is (slice 5 review, security L1).
    let first = snapshot_file_name(STARTED_AT).unwrap();
    let second = snapshot_file_name(RECAPTURED_AT).unwrap();
    assert_eq!(scratch.snapshots(), std::slice::from_ref(&second));
    assert_ne!(first, second);
    match snapshot_store::state_snapshot(&scratch.state, &second, scratch.euid) {
        snapshot_store::StateSnapshot::Present(taken) => {
            assert_eq!(taken.provenance.captured_at, RECAPTURED_AT);
        }
        other => panic!("expected the snapshot, got {other:?}"),
    }
    assert_eq!(stored(&scratch).snapshot_path(), Some(second.as_str()));
}

#[test]
fn a_snapshot_that_cannot_be_written_halts_before_anything_changes() {
    for (name, direction) in directions() {
        let scratch = Scratch::new("snapshot-write");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let mut setup = unchanged(&direction, &scratch);
        // Something already stands at the snapshot's name: it is never replaced.
        let inner = setup.recapture;
        let squat = scratch
            .state
            .snapshots()
            .join(snapshot_file_name(RECAPTURED_AT).unwrap());
        setup.recapture = Box::new(move || {
            let result = inner();
            std::fs::write(&squat, "mine").expect("squatter");
            result
        });
        let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
        assert!(
            matches!(
                report.outcome,
                RunOutcome::Halted {
                    reason: HaltReason::SnapshotNotTaken,
                    ..
                }
            ),
            "{name}: {:?}",
            report.outcome
        );
        assert_eq!(
            report.exit_code(),
            ErrorId::SafetyBlocked.exit_code(),
            "{name}"
        );
        assert!(
            site.handed.borrow().is_empty(),
            "{name}: nothing was dispatched"
        );
        assert!(!scratch.marker().exists(), "{name}: no marker was written");
        assert_eq!(scratch.records(), 0, "{name}: nothing was recorded");
        assert!(report.receipt.run_id.is_none() && report.receipt.snapshot_file.is_none());
        let seen = seen.all();
        assert!(
            !seen.contains(&Boundary::SnapshotWritten),
            "{name}: {seen:?}"
        );
        assert!(!seen.contains(&Boundary::MarkerWritten), "{name}: {seen:?}");
        assert_eq!(
            std::fs::read_to_string(
                scratch
                    .state
                    .snapshots()
                    .join(snapshot_file_name(RECAPTURED_AT).unwrap())
            )
            .unwrap(),
            "mine",
            "{name}: the squatter was not replaced"
        );
    }
}

#[test]
fn a_snapshot_the_reader_refuses_halts_before_the_marker_is_written() {
    // The file is published, and the run's own reader will not take it back: the
    // run does not change the machine behind a snapshot it could not restore from.
    let direction = Direction::toward_homebrew_completable();
    let scratch = Scratch::new("snapshot-read");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let mut setup = unchanged(&direction, &scratch);
    let inner = setup.recapture;
    setup.recapture = Box::new(move || {
        let (mut snap, plan) = inner()?;
        for provider in snap.providers.values_mut() {
            provider
                .invocation_environment
                .insert("SUDO_ASKPASS".into(), "/tmp/x".into());
        }
        Ok((snap, plan))
    });
    let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
    assert!(
        matches!(
            report.outcome,
            RunOutcome::Halted {
                reason: HaltReason::SnapshotNotTaken,
                ..
            }
        ),
        "{:?}",
        report.outcome
    );
    assert!(site.handed.borrow().is_empty());
    assert!(!scratch.marker().exists());
    assert_eq!(scratch.records(), 0);
    assert!(!seen.all().contains(&Boundary::MarkerWritten));
}

#[test]
fn a_record_that_cannot_be_read_back_or_differs_keeps_the_marker_and_never_exits_zero() {
    use macportsbrew::executor::ReadBackFailure;
    let failed = |_: &StateDir, _: &str, _: u32| -> Result<String, DomainError> {
        Err(DomainError::new(ErrorId::HistoryUnusable, "read-back"))
    };
    let differs = |_: &StateDir, _: &str, _: u32| -> Result<String, DomainError> {
        Ok("another record\n".to_string())
    };
    let empty =
        |_: &StateDir, _: &str, _: u32| -> Result<String, DomainError> { Ok(String::new()) };
    let readers: [(&str, ReadBackFailure, ReadBack<'_>); 3] = [
        ("cannot be read", ReadBackFailure::Failed, &failed),
        ("differs", ReadBackFailure::Disagreed, &differs),
        ("is empty", ReadBackFailure::Disagreed, &empty),
    ];
    for (what, expected, reader) in readers {
        let direction = Direction::toward_homebrew_completable();
        let scratch = Scratch::new("read-back");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let mut setup = unchanged(&direction, &scratch);
        setup.read_back = reader;
        let report = run(&direction.cleared(), &scratch, &site, &setup, &seen).expect("a report");
        // The run did all its steps; its record is what could not be confirmed.
        assert_eq!(report.outcome, RunOutcome::Completed, "{what}");
        assert_eq!(report.receipt.record_read_back, Some(expected), "{what}");
        assert_eq!(report.history_error, Some(ErrorId::Io), "{what}");
        assert_ne!(
            report.exit_code(),
            0,
            "{what}: a run is not a success on an unconfirmed record"
        );
        assert_eq!(
            report.marker_left.as_deref(),
            Some(scratch.id.as_str()),
            "{what}"
        );
        assert!(scratch.marker().exists(), "{what}: the marker stays");
        assert_eq!(
            scratch.records(),
            1,
            "{what}: the record is kept, never deleted"
        );
        assert!(report.record_written, "{what}");
        let seen = seen.all();
        assert!(seen.contains(&Boundary::RecordWritten), "{what}: {seen:?}");
        assert!(
            !seen.contains(&Boundary::RecordReadBack),
            "{what}: {seen:?}"
        );
        assert!(!seen.contains(&Boundary::MarkerCleared), "{what}: {seen:?}");
        // The run is named so the next command can say what it left.
        assert_eq!(
            report.receipt.run_id.as_deref(),
            Some(scratch.id.as_str()),
            "{what}"
        );
    }
}

#[test]
fn nothing_is_read_back_after_a_write_that_was_not_known_to_be_durable() {
    // Whatever stands at the record's name is not this run's record, so reading it
    // back could only confirm somebody else's file (slice 5 review, security M5).
    let direction = Direction::toward_homebrew_completable();
    let scratch = Scratch::new("no-read-back");
    let called = std::cell::Cell::new(false);
    let counting = |_: &StateDir, _: &str, _: u32| -> Result<String, DomainError> {
        called.set(true);
        Ok(String::new())
    };
    let site = Site::new(
        &scratch,
        Box::new(|s| {
            std::fs::create_dir_all(s.state.history()).expect("history");
            std::fs::write(s.state.history().join(format!("run-{}.json", s.id)), "{}")
                .expect("squatter");
            Ok(CommandEnd::Exited { code: 0 })
        }),
    );
    let mut setup = unchanged(&direction, &scratch);
    setup.read_back = &counting;
    let report = run(
        &direction.cleared(),
        &scratch,
        &site,
        &setup,
        &Seen::default(),
    )
    .expect("a report");
    assert_eq!(report.history_error, Some(ErrorId::Io));
    assert_eq!(
        report.receipt.record_read_back, None,
        "no read-back was attempted"
    );
    assert!(!called.get(), "the read-back was not asked for");
    assert!(scratch.marker().exists());
}
// --- work order 16 part S: the destination re-read before removal -----------------

/// Answers the destination's reads with its package for the first
/// `present_for` of them, then as if it had vanished; every read is logged.
struct Vanishing<'p> {
    plan: &'p MigrationPlan,
    present_for: usize,
    log: RefCell<Vec<(ProviderId, Vec<String>)>>,
}

impl<'p> Vanishing<'p> {
    fn new(plan: &'p MigrationPlan, present_for: usize) -> Self {
        Self {
            plan,
            present_for,
            log: RefCell::new(Vec::new()),
        }
    }

    fn destination_reads(&self) -> Vec<Vec<String>> {
        self.log
            .borrow()
            .iter()
            .filter(|(p, _)| *p == self.plan.destination)
            .map(|(_, argv)| argv.clone())
            .collect()
    }
}

impl ReadSite for &Vanishing<'_> {
    fn read(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        self.log
            .borrow_mut()
            .push((provider.clone(), command.argv.clone()));
        let earlier = self
            .log
            .borrow()
            .iter()
            .filter(|(p, _)| *p == self.plan.destination)
            .count();
        let (code, stdout) = if *provider == self.plan.destination {
            if earlier <= self.present_for {
                (0, destination_present(self.plan, command))
            } else {
                (1, String::new())
            }
        } else {
            (0, "  unrelated 1.0\n".to_string())
        };
        Ok(Invocation {
            program: "x".into(),
            argv: command.argv.clone(),
            environment: BTreeMap::new(),
            status: ExitStatus::Exited { code },
            stdout,
            stderr: String::new(),
            truncated: false,
            duration_ms: 1,
        })
    }
}

fn run_reading<'s, S: ReadSite + 's>(
    cleared: &GateClear,
    scratch: &Scratch,
    site: &Site<'_>,
    reads: S,
    seen: &Seen,
    direction: &Direction,
) -> RunReport {
    run_with_mutations(
        cleared,
        move || reads,
        MutatingRun {
            shown: shown(cleared),
            live: true,
            state: &scratch.state,
            euid: scratch.euid,
            start: scratch.start(),
            site,
            recapture: &*unchanged(direction, scratch).recapture,
            consent: &never_asked,
            reread: unchanged(direction, scratch).reread,
            read_back: &history_store::read_back,
            observer: seen,
        },
        &HomeRedaction::Recorded,
    )
    .expect("a report")
}

#[test]
fn invariant_01_the_reread_uses_the_verify_steps_own_commands() {
    // Toward MacPorts the run reaches a removal. The destination is read
    // twice: once to certify, once immediately before the removal — and both
    // times with exactly the VERIFY_DESTINATION's own commands.
    let direction = Direction::toward_macports();
    let scratch = Scratch::new("reread-commands");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let cleared = direction.cleared();
    let plan = cleared.run().plan().clone();
    let reads = Vanishing::new(&plan, usize::MAX);
    let report = run_reading(&cleared, &scratch, &site, &reads, &seen, &direction);
    let verify = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyDestination)
        .expect("a destination check");
    let own: Vec<Vec<String>> = verify.commands.iter().map(|c| c.argv.clone()).collect();
    let mut twice = own.clone();
    twice.extend(own);
    assert_eq!(reads.destination_reads(), twice, "{report:?}");
    assert_eq!(site.handed.borrow().len(), 1, "the removal went ahead");
}

#[test]
fn each_removal_command_is_preceded_by_its_own_reread() {
    // One re-read and one destination re-check per removal command handed
    // over: the re-check's token is consumed by `destination_still_verified`,
    // so it cannot certify a second command (a planner plan has one removal
    // command; the gate refuses any other, so the count is the proof here).
    let direction = Direction::toward_macports();
    let scratch = Scratch::new("reread-each");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let cleared = direction.cleared();
    let plan = cleared.run().plan().clone();
    let reads = Vanishing::new(&plan, usize::MAX);
    let _ = run_reading(&cleared, &scratch, &site, &reads, &seen, &direction);
    let removals = site.handed.borrow().len();
    let reverified = seen
        .all()
        .iter()
        .filter(|b| **b == Boundary::DestinationReverified)
        .count();
    assert_eq!(removals, 1);
    assert_eq!(reverified, removals);
}

#[test]
fn invariant_03_a_removal_whose_destination_was_not_reverified_ends_partial() {
    // The destination certifies, then has vanished when it is read again just
    // before the removal: the source is not handed over, and the run ends
    // PARTIAL_SUCCESS (exit 3) naming DESTINATION_NOT_REVERIFIED.
    let direction = Direction::toward_macports();
    let scratch = Scratch::new("reread-vanished");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let cleared = direction.cleared();
    let plan = cleared.run().plan().clone();
    let verify = plan
        .steps
        .iter()
        .find(|s| s.kind == StepKind::VerifyDestination)
        .expect("a destination check");
    let reads = Vanishing::new(&plan, verify.commands.len());
    let report = run_reading(&cleared, &scratch, &site, &reads, &seen, &direction);
    assert!(site.handed.borrow().is_empty(), "nothing was removed");
    assert!(
        !seen.all().contains(&Boundary::Dispatched),
        "{:?}",
        seen.all()
    );
    assert!(!seen.all().contains(&Boundary::DestinationReverified));
    let halt = report.records.iter().find_map(|r| match &r.disposition {
        Disposition::Halted { reason, .. } => Some((r.kind, reason.clone())),
        _ => None,
    });
    assert_eq!(
        halt,
        Some((
            StepKind::RemoveSource,
            HaltReason::PreconditionFailed {
                command: 0,
                why: Precondition::DestinationNotReverified,
            }
        )),
        "{report:?}"
    );
    assert!(
        matches!(report.outcome, RunOutcome::PartialSuccess { .. }),
        "{:?}",
        report.outcome
    );
    assert_eq!(report.exit_code(), 3);
    let json = serde_json::to_string(&report).expect("serialises");
    assert!(json.contains("DESTINATION_NOT_REVERIFIED"), "{json}");
}
#[test]
fn a_destination_configuration_that_drifted_stops_the_removal_before_the_reread() {
    // The destination's configuration is re-checked before each re-read: clean
    // when it certifies, unknowable (or unreadable) just before the removal.
    // The removal halts with the configuration's own identifier, and the
    // destination is not read again.
    for (what, then, why) in [
        (
            "drifted",
            Ok(ProviderConfiguration::Unknowable),
            Precondition::ConfigurationDrift,
        ),
        (
            "unreadable",
            Err(DomainError::new(ErrorId::SafetyBlocked, "not detected")),
            Precondition::ConfigurationUnreadable,
        ),
    ] {
        let direction = Direction::toward_macports();
        let scratch = Scratch::new("reread-drift");
        let site = Site::exits(&scratch, 0);
        let seen = Seen::default();
        let cleared = direction.cleared();
        let plan = cleared.run().plan().clone();
        let destination = plan.destination.clone();
        let calls = std::cell::Cell::new(0usize);
        let reread = move |provider: &ProviderId, _: &str| {
            if *provider == destination {
                calls.set(calls.get() + 1);
                if calls.get() > 1 {
                    return then.clone();
                }
            }
            Ok(ProviderConfiguration::Read { files: Vec::new() })
        };
        let reads = Vanishing::new(&plan, usize::MAX);
        let report = run_with_mutations(
            &cleared,
            || &reads,
            MutatingRun {
                shown: shown(&cleared),
                live: true,
                state: &scratch.state,
                euid: scratch.euid,
                start: scratch.start(),
                site: &site,
                recapture: &*unchanged(&direction, &scratch).recapture,
                consent: &never_asked,
                reread: &reread,
                read_back: &history_store::read_back,
                observer: &seen,
            },
            &HomeRedaction::Recorded,
        )
        .expect("a report");
        let halt = report.records.iter().find_map(|r| match &r.disposition {
            Disposition::Halted { reason, .. } => Some((r.kind, reason.clone())),
            _ => None,
        });
        assert_eq!(
            halt,
            Some((
                StepKind::RemoveSource,
                HaltReason::PreconditionFailed { command: 0, why }
            )),
            "{what}: {report:?}"
        );
        assert!(site.handed.borrow().is_empty(), "{what}");
        let verify = plan
            .steps
            .iter()
            .find(|s| s.kind == StepKind::VerifyDestination)
            .expect("a destination check");
        assert_eq!(
            reads.destination_reads().len(),
            verify.commands.len(),
            "{what}: the destination was read again after its configuration failed"
        );
    }
}

// --- work order 16 part E-2: the executed plan tied to the one shown ---------

#[test]
fn a_shown_plan_covers_only_the_plan_shown() {
    let cleared = Direction::toward_homebrew().cleared();
    let witness = shown(&cleared);
    assert!(witness.covers(cleared.run().plan()));
    let mut other = cleared.run().plan().clone();
    other.steps[0].index += 100;
    assert!(!witness.covers(&other), "one field changed");
    let elsewhere = Direction::toward_macports().cleared();
    assert!(!witness.covers(elsewhere.run().plan()), "another plan");
}

#[test]
fn confirm_shown_mints_only_on_the_exact_phrase_with_its_code() {
    let cleared = Direction::toward_homebrew().cleared();
    let code = ConsentCode::from_bytes([0x12, 0x34, 0x56]);
    for typed in [
        "run plan 123456",
        "run plan 123456\n",
        "run plan 123456\r\n",
    ] {
        assert!(confirm_shown(&cleared, &code, typed).is_some(), "{typed:?}");
    }
    for typed in [
        "",
        "y\n",
        "run plan\n",
        "run plan 654321\n",
        "RUN PLAN 123456\n",
        "run plan 123456 \n",
        " run plan 123456\n",
        "run plan 123456\n\n",
        "allow step 1 123456\n",
    ] {
        assert!(confirm_shown(&cleared, &code, typed).is_none(), "{typed:?}");
    }
}

#[test]
fn invariant_02_a_plan_other_than_the_one_shown_dispatches_nothing_and_takes_no_lock() {
    // A witness for one plan, a cleared run of another: refused before the
    // history probe, so nothing is locked, written or dispatched.
    let shown_for = Direction::toward_homebrew();
    let direction = Direction::toward_macports();
    let scratch = Scratch::new("not-shown");
    let site = Site::exits(&scratch, 0);
    let seen = Seen::default();
    let setup = unchanged(&direction, &scratch);
    let cleared = direction.cleared();
    let plan = cleared.run().plan().clone();
    let result = run_with_mutations(
        &cleared,
        || Reads(Box::leak(Box::new(plan))),
        MutatingRun {
            live: setup.live,
            state: &scratch.state,
            euid: scratch.euid,
            start: scratch.start(),
            site: &site,
            recapture: &*setup.recapture,
            consent: &never_asked,
            reread: setup.reread,
            read_back: setup.read_back,
            observer: &seen,
            shown: shown(&shown_for.cleared()),
        },
        &HomeRedaction::Recorded,
    );
    assert_eq!(
        result.as_ref().map_err(|e| e.id),
        Err(macportsbrew_domain::ErrorId::PlanNotShown),
        "{result:?}"
    );
    assert!(
        seen.all().is_empty(),
        "no boundary was reached: {:?}",
        seen.all()
    );
    assert!(site.handed.borrow().is_empty());
    assert!(!scratch.locked_by_someone(), "no lock is held");
    assert!(!scratch.marker().exists());
    assert_eq!(scratch.records(), 0);
}

fn confirm_with(
    cleared: &GateClear,
    terminals: (bool, bool),
    typed: &str,
    code: Option<[u8; 3]>,
) -> (
    Result<ShownPlan, macportsbrew_application::ConsentRefusal>,
    String,
    u64,
) {
    let mut input = std::io::Cursor::new(typed.as_bytes().to_vec());
    let mut prompt = Vec::new();
    let mut draw = || code;
    let render = |p: &MigrationPlan| format!("THE PLAN ({} steps)\n", p.steps.len());
    let result = macportsbrew::consent::confirm_plan(
        cleared,
        &render,
        macportsbrew::consent::ConsentStreams {
            input_is_terminal: terminals.0,
            prompt_is_terminal: terminals.1,
            input: &mut input,
            prompt: &mut prompt,
            code: &mut draw,
        },
        &macportsbrew::i18n::Messages::english(),
    );
    (
        result,
        String::from_utf8(prompt).expect("utf-8"),
        input.position(),
    )
}

#[test]
fn invariant_02_a_declined_plan_question_mints_nothing_and_the_right_answer_does() {
    use macportsbrew_application::ConsentRefusal;
    let cleared = Direction::toward_homebrew().cleared();
    let code = Some([0xab, 0xcd, 0xef]);
    let (result, prompt, _) = confirm_with(&cleared, (true, true), "run plan abcdef\n", code);
    assert!(result.expect("confirmed").covers(cleared.run().plan()));
    assert!(prompt.contains("run plan abcdef"), "{prompt}");
    for typed in ["y\n", "run plan\n", "run plan 000000\n", ""] {
        let (result, prompt, _) = confirm_with(&cleared, (true, true), typed, code);
        assert_eq!(
            result.map(|_| ()),
            Err(ConsentRefusal::Declined),
            "{typed:?}"
        );
        assert!(prompt.contains("Not allowed"), "{prompt}");
    }
    let (result, prompt, read) = confirm_with(&cleared, (true, true), "run plan abcdef\n", None);
    assert_eq!(result.map(|_| ()), Err(ConsentRefusal::CodeUnavailable));
    assert_eq!(read, 0);
    assert!(!prompt.contains("THE PLAN"), "nothing was asked: {prompt}");
}

#[test]
fn the_plan_question_is_asked_only_at_two_terminals() {
    use macportsbrew_application::ConsentRefusal;
    let cleared = Direction::toward_homebrew().cleared();
    for terminals in [(false, true), (true, false), (false, false)] {
        let (result, prompt, read) = confirm_with(
            &cleared,
            terminals,
            "run plan abcdef\n",
            Some([0xab, 0xcd, 0xef]),
        );
        assert_eq!(result.map(|_| ()), Err(ConsentRefusal::NotATerminal));
        assert!(prompt.is_empty(), "nothing written");
        assert_eq!(read, 0, "nothing read");
    }
}

#[test]
fn invariant_02_the_plan_is_shown_on_standard_error_before_it_is_confirmed() {
    let cleared = Direction::toward_homebrew().cleared();
    let (result, prompt, _) = confirm_with(
        &cleared,
        (true, true),
        "run plan abcdef\n",
        Some([0xab, 0xcd, 0xef]),
    );
    assert!(result.is_ok());
    let plan_at = prompt.find("THE PLAN").expect("the plan is shown");
    let question_at = prompt.find("run plan abcdef").expect("then the question");
    assert!(plan_at < question_at, "{prompt}");
    assert!(
        prompt.contains(&format!(
            "THE PLAN ({} steps)",
            cleared.run().plan().steps.len()
        )),
        "the plan shown is the plan cleared"
    );
}

#[test]
fn invariant_17_the_tie_behaves_alike_in_both_directions() {
    // The witness of one direction's plan never covers the other's, either
    // way round, and each direction's own witness lets its run past the tie.
    // The program-token half is `each_re_check_is_made_for_the_program_it_is_spent_on`;
    // the tie over every recording's plan, by digest across two processes, is
    // `invariant_15_the_same_recording_gives_the_same_plan_digest` and
    // `invariant_17_a_digest_tie_behaves_alike_in_both_directions`.
    let orientations = [
        (Direction::toward_homebrew(), Direction::toward_macports()),
        (Direction::toward_macports(), Direction::toward_homebrew()),
    ];
    for (witnessed, direction) in orientations {
        for (witness, refused) in [(&witnessed, true), (&direction, false)] {
            let scratch = Scratch::new("tie-both");
            let site = Site::exits(&scratch, 0);
            let seen = Seen::default();
            let setup = unchanged(&direction, &scratch);
            let cleared = direction.cleared();
            let plan = cleared.run().plan().clone();
            let result = run_with_mutations(
                &cleared,
                || Reads(Box::leak(Box::new(plan))),
                MutatingRun {
                    live: setup.live,
                    state: &scratch.state,
                    euid: scratch.euid,
                    start: scratch.start(),
                    site: &site,
                    recapture: &*setup.recapture,
                    consent: &never_asked,
                    reread: setup.reread,
                    read_back: setup.read_back,
                    observer: &seen,
                    shown: shown(&witness.cleared()),
                },
                &HomeRedaction::Recorded,
            );
            let not_shown = matches!(
                &result,
                Err(e) if e.id == macportsbrew_domain::ErrorId::PlanNotShown
            );
            assert_eq!(not_shown, refused, "{result:?}");
            assert_eq!(seen.all().contains(&Boundary::Probed), !refused);
        }
    }
}

// --- work order 16 part E-4: the plan digest (DP-049 (a)) --------------------

/// One edit per field of the plan, of one of its steps and of one of that
/// step's commands, each named. The structs are destructured without `..`, so
/// a field added to any of them fails to compile here until it is given an
/// edit — and with it a check that the digest covers it.
fn every_field_edited(plan: &MigrationPlan) -> Vec<(&'static str, MigrationPlan)> {
    use macportsbrew_domain::{
        CommandTarget, Elevation, EntryMode, MigrationStep, Outcome, PlanRisk, Reason,
        VerificationCheck,
    };
    let MigrationPlan {
        schema_version: _,
        package: _,
        source: _,
        destination: _,
        destination_package: _,
        retain_source: _,
        entered_by: _,
        primary_after: _,
        recommendation: _,
        overrode: _,
        steps: _,
        risks: _,
        elevated_steps: _,
        policy_version: _,
        dry_run_only: _,
    } = plan;
    let step = plan
        .steps
        .iter()
        .position(|s| !s.commands.is_empty())
        .expect("a step with a command");
    let MigrationStep {
        index: _,
        kind: _,
        provider: _,
        commands: _,
        verification: _,
        reason: _,
    } = &plan.steps[step];
    let PlanCommand {
        target: _,
        program: _,
        argv: _,
        environment: _,
        elevation: _,
        success: _,
        placeholder: _,
    } = &plan.steps[step].commands[0];
    let edit = |f: &dyn Fn(&mut MigrationPlan)| {
        let mut p = plan.clone();
        f(&mut p);
        p
    };
    let other = |id: &ProviderId| ProviderId::new(if id.as_str() == HB { MP } else { HB });
    let a_reason = || Reason {
        rule: Some("an-edit".into()),
        ..plan.steps[step].reason.clone()
    };
    let cmd = |f: &dyn Fn(&mut PlanCommand)| edit(&|p| f(&mut p.steps[step].commands[0]));
    vec![
        ("schemaVersion", edit(&|p| p.schema_version += 1)),
        ("package", edit(&|p| p.package.name.push('x'))),
        ("source", edit(&|p| p.source = other(&p.source))),
        (
            "destination",
            edit(&|p| p.destination = other(&p.destination)),
        ),
        (
            "destinationPackage",
            edit(&|p| p.destination_package = Some("an-edit".into())),
        ),
        (
            "retainSource",
            edit(&|p| p.retain_source = !p.retain_source),
        ),
        (
            "enteredBy",
            edit(&|p| {
                p.entered_by = Some(if p.entered_by == Some(EntryMode::Named) {
                    EntryMode::Advanced
                } else {
                    EntryMode::Named
                });
            }),
        ),
        (
            "primaryAfter",
            edit(&|p| p.primary_after = other(&p.primary_after)),
        ),
        (
            "recommendation",
            edit(&|p| {
                p.recommendation = Outcome::Keep {
                    provider: ProviderId::new("an-edit"),
                };
            }),
        ),
        (
            "overrode",
            edit(&|p| {
                p.overrode = Some(Outcome::Keep {
                    provider: ProviderId::new("an-edit"),
                });
            }),
        ),
        ("steps", edit(&|p| drop(p.steps.pop()))),
        (
            "risks",
            edit(&|p| {
                p.risks.push(PlanRisk {
                    id: "an-edit".into(),
                    reason: a_reason(),
                    blocking: false,
                });
            }),
        ),
        ("elevatedSteps", edit(&|p| p.elevated_steps.push(99))),
        ("policyVersion", edit(&|p| p.policy_version.push('x'))),
        ("dryRunOnly", edit(&|p| p.dry_run_only = !p.dry_run_only)),
        ("steps[].index", edit(&|p| p.steps[step].index += 100)),
        (
            "steps[].kind",
            edit(&|p| {
                p.steps[step].kind = if p.steps[step].kind == StepKind::RecordHistory {
                    StepKind::SnapshotBefore
                } else {
                    StepKind::RecordHistory
                };
            }),
        ),
        (
            "steps[].provider",
            edit(&|p| p.steps[step].provider = Some(ProviderId::new("an-edit"))),
        ),
        (
            "steps[].commands",
            edit(&|p| p.steps[step].commands.clear()),
        ),
        (
            "steps[].verification",
            edit(&|p| {
                p.steps[step]
                    .verification
                    .push(VerificationCheck::NoCrossProviderLinkage);
            }),
        ),
        (
            "steps[].reason",
            edit(&|p| p.steps[step].reason = a_reason()),
        ),
        (
            "steps[].commands[].target",
            cmd(&|c| {
                c.target = if c.target == CommandTarget::Tool {
                    CommandTarget::Provider
                } else {
                    CommandTarget::Tool
                };
            }),
        ),
        ("steps[].commands[].program", cmd(&|c| c.program.push('x'))),
        (
            "steps[].commands[].argv",
            cmd(&|c| c.argv.push("--an-edit".into())),
        ),
        (
            "steps[].commands[].environment",
            cmd(&|c| {
                c.environment.insert("AN_EDIT".into(), "1".into());
            }),
        ),
        (
            "steps[].commands[].elevation",
            cmd(&|c| {
                c.elevation = Elevation::Required {
                    reason: "an-edit".into(),
                };
            }),
        ),
        (
            "steps[].commands[].success",
            cmd(&|c| {
                c.success = if c.success == SuccessSemantic::ExitStatus {
                    SuccessSemantic::StdoutNamesPackage
                } else {
                    SuccessSemantic::ExitStatus
                };
            }),
        ),
        (
            "steps[].commands[].placeholder",
            cmd(&|c| c.placeholder = !c.placeholder),
        ),
    ]
}

#[test]
fn plan_digest_is_stable_and_covers_every_field() {
    for direction in [Direction::toward_homebrew(), Direction::toward_macports()] {
        let plan = direction.plan_of(&direction.snapshot).plan().clone();
        let digest = PlanDigest::of(&plan);
        let again = direction.plan_of(&direction.snapshot).plan().clone();
        assert_eq!(PlanDigest::of(&again), digest, "the same plan built twice");
        for (field, edited) in every_field_edited(&plan) {
            assert_ne!(edited, plan, "{field}: the edit changed nothing");
            assert_ne!(PlanDigest::of(&edited), digest, "{field} is not digested");
        }
    }
}

#[test]
fn plan_digest_tells_apart_plans_that_differ_only_in_a_home_directory() {
    // The digest is of the plan as it will run, not of a redacted rendering:
    // two plans that name different users' files are two plans (E-4 mutant 14).
    let direction = Direction::toward_homebrew();
    let plan = direction.plan_of(&direction.snapshot).plan().clone();
    let step = plan
        .steps
        .iter()
        .position(|s| !s.commands.is_empty())
        .expect("a step with a command");
    let with_path = |user: &str| {
        let mut p = plan.clone();
        p.steps[step].commands[0]
            .argv
            .push(format!("/Users/{user}/Library/an-edit"));
        p
    };
    assert_ne!(
        PlanDigest::of(&with_path("alice")),
        PlanDigest::of(&with_path("bob")),
        "a home directory in a command is not digested as written"
    );
}

#[test]
fn plan_digest_is_pinned_for_a_fixed_plan() {
    // The bytes of a fixed plan's digest: a `serde_json` or `sha2` bump, a
    // dropped domain prefix or version, or a change to what is serialised
    // fails here rather than in a script that saved a digest.
    let direction = Direction::toward_homebrew();
    let plan = direction.plan_of(&direction.snapshot).plan().clone();
    assert_eq!(
        PlanDigest::of(&plan).to_string(),
        PINNED_DIGEST,
        "the digest of a fixed plan moved (version {})",
        env!("CARGO_PKG_VERSION")
    );
}

/// Taken from this build at version 0.1.0 and policy data as embedded on
/// 2026-11-01. A version bump or a policy-data change moves it, deliberately:
/// both are inside what is digested.
const PINNED_DIGEST: &str =
    "sha256:835d0640af49d8fb2b5eedddd11553aa5e6db5e0d47f3abd2cf1e100cd58e872";

#[test]
fn a_plan_digest_reads_back_as_written_and_nothing_else() {
    let direction = Direction::toward_homebrew();
    let digest = PlanDigest::of(direction.plan_of(&direction.snapshot).plan());
    let written = digest.to_string();
    assert_eq!(PlanDigest::parse(&written), Some(digest));
    let hex = &written["sha256:".len()..];
    for bad in [
        String::new(),
        hex.to_string(),
        format!("SHA256:{hex}"),
        format!("sha256:{}", hex.to_uppercase()),
        format!("sha256:{}", &hex[1..]),
        format!("sha256:{hex}0"),
        format!(" {written}"),
        format!("{written}\n"),
        format!("sha256: {}", &hex[1..]),
        format!("sha256:{}g", &hex[1..]),
        format!("sha512:{hex}"),
    ] {
        assert_eq!(PlanDigest::parse(&bad), None, "{bad:?}");
    }
}

#[test]
fn a_shown_plan_cannot_be_minted_from_another_digest() {
    for (direction, elsewhere) in [
        (Direction::toward_homebrew(), Direction::toward_macports()),
        (Direction::toward_macports(), Direction::toward_homebrew()),
    ] {
        let run = direction.plan_of(&direction.snapshot);
        let own = PlanDigest::of(run.plan());
        assert!(tie(&run, &own).expect("its own digest").covers(run.plan()));
        let other = PlanDigest::of(elsewhere.plan_of(&elsewhere.snapshot).plan());
        let refused = tie(&run, &other).expect_err("another plan's digest");
        assert_eq!(refused.id, ErrorId::PlanNotShown);
        assert_eq!(refused.exit_code(), 4);
        for (field, edited) in every_field_edited(run.plan()) {
            assert!(
                tie(&run, &PlanDigest::of(&edited)).is_err(),
                "a digest of the plan with {field} edited"
            );
        }
    }
}

#[test]
fn invariant_17_a_digest_tie_behaves_alike_in_both_directions() {
    // The digest route's witness lets its own run past the run loop's tie and
    // no other, either way round — the same check the interactive witness
    // passes in `invariant_17_the_tie_behaves_alike_in_both_directions`.
    let orientations = [
        (Direction::toward_homebrew(), Direction::toward_macports()),
        (Direction::toward_macports(), Direction::toward_homebrew()),
    ];
    for (witnessed, direction) in orientations {
        for (witness, refused) in [(&witnessed, true), (&direction, false)] {
            let scratch = Scratch::new("digest-both");
            let site = Site::exits(&scratch, 0);
            let seen = Seen::default();
            let setup = unchanged(&direction, &scratch);
            let cleared = direction.cleared();
            let witness_run = witness.cleared();
            let digest = PlanDigest::of(witness_run.run().plan());
            let tied = tie(witness_run.run(), &digest).expect("its own digest");
            let plan = cleared.run().plan().clone();
            let result = run_with_mutations(
                &cleared,
                || Reads(Box::leak(Box::new(plan))),
                MutatingRun {
                    live: setup.live,
                    state: &scratch.state,
                    euid: scratch.euid,
                    start: scratch.start(),
                    site: &site,
                    recapture: &*setup.recapture,
                    consent: &never_asked,
                    reread: setup.reread,
                    read_back: setup.read_back,
                    observer: &seen,
                    shown: tied,
                },
                &HomeRedaction::Recorded,
            );
            let not_shown = matches!(
                &result,
                Err(e) if e.id == macportsbrew_domain::ErrorId::PlanNotShown
            );
            assert_eq!(not_shown, refused, "{result:?}");
            assert_eq!(seen.all().contains(&Boundary::Probed), !refused);
        }
    }
}
