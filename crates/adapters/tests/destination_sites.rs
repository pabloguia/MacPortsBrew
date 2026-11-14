// SPDX-License-Identifier: Apache-2.0

//! The registry's destination sites under a scope that is not this machine
//! (INV-15), and the adapters' own answer there. No recording holds these reads
//! (`capture-fixture.sh` has none, an owner item), so the substance is that nothing
//! is asked at all: this runner refuses by panicking.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::homebrew::destination::keg_of;
use macportsbrew_adapters::persistence::run_lock::acquire;
use macportsbrew_adapters::persistence::StateDir;
use macportsbrew_adapters::process::{CommandSpec, Invocation, ProcessRunner};
use macportsbrew_adapters::registry::{destination_sites, RegistryContext};
use macportsbrew_domain::{
    CheckAnswer, DestinationFacts, DomainError, ExecutableName, ProviderId, UncheckedCause,
};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Answers nothing: reaching it fails the test.
struct NeverRun {
    replay: bool,
}

impl ProcessRunner for NeverRun {
    fn run(&self, _spec: &CommandSpec) -> Result<Invocation, DomainError> {
        panic!("a site that is not on this machine started a program");
    }

    fn is_replay(&self) -> bool {
        self.replay
    }
}

fn state(tag: &str) -> (std::path::PathBuf, StateDir) {
    let base = std::env::temp_dir().join(format!("mpb-sites-{}-{tag}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).unwrap();
    let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
    state.ensure().unwrap();
    (base, state)
}

#[test]
fn invariant_15_a_replayed_or_elsewhere_site_observes_nothing_and_asks_nothing() {
    let (base, state) = state("scope");
    let lock = acquire(&state, std::fs::metadata(&base).unwrap().uid()).unwrap();
    let leaf = ExecutableName::parse("xz").unwrap();
    // The third is the registry's own guard: `build` collapses a live runner's
    // `Elsewhere` to this machine, so without the guard the adapter would ask, and
    // this runner panics. The halves on their own are held by the next test, the
    // adapters' guard by
    // `invariant_15_the_adapters_own_guard_stops_a_read_of_this_disk`.
    let cases = [
        (true, FilesystemScope::Elsewhere),
        (true, FilesystemScope::ThisMachine),
        (false, FilesystemScope::Elsewhere),
    ];
    for (replay, filesystem) in cases {
        let runner = NeverRun { replay };
        let ctx = RegistryContext {
            runner: &runner,
            home: None,
            platform_tags: Box::new(|_| Vec::new()),
            filesystem,
            prefix_override: Some(vec![(ProviderId::new("homebrew"), "/usr/local".into())]),
        };
        let sites = destination_sites(&ctx, &state, &lock);
        for id in ["homebrew", "macports"] {
            let provider = ProviderId::new(id);
            assert_eq!(
                sites.ownership(&provider, "xz", &leaf, None),
                DestinationFacts::NotObserved
            );
            assert_eq!(
                sites.integrity(&provider, "xz"),
                DestinationFacts::NotObserved
            );
            assert_eq!(
                sites.linkage(&provider, "xz"),
                DestinationFacts::NotObserved
            );
        }
    }
    drop(lock);
    std::fs::remove_dir_all(&base).ok();
}

/// Each half of the guard, asked directly, so it holds on any host. `override_for`
/// honours a prefix under a replay only, so a live runner with `ThisMachine` is the
/// host's business, and only its liveness is asserted.
#[test]
fn invariant_15_each_half_of_the_sites_guard_stops_it_looking() {
    let (base, state) = state("halves");
    let lock = acquire(&state, std::fs::metadata(&base).unwrap().uid()).unwrap();
    let homebrew = ProviderId::new("homebrew");
    for (replay, filesystem, live) in [
        (true, FilesystemScope::ThisMachine, false),
        (true, FilesystemScope::Elsewhere, false),
        (false, FilesystemScope::Elsewhere, false),
        (false, FilesystemScope::ThisMachine, true),
    ] {
        let runner = NeverRun { replay };
        let ctx = RegistryContext {
            runner: &runner,
            home: None,
            platform_tags: Box::new(|_| Vec::new()),
            filesystem,
            prefix_override: Some(vec![(homebrew.clone(), "/usr/local".into())]),
        };
        let sites = destination_sites(&ctx, &state, &lock);
        assert_eq!(sites.is_live(), live, "{replay} {filesystem:?}");
        // No table is built, so no documented prefix is canonicalised on this disk.
        assert_eq!(
            sites.linkage_prefixes(&homebrew).is_some(),
            live,
            "{replay} {filesystem:?}"
        );
        if !live {
            assert!(!sites.observes(&homebrew), "{replay} {filesystem:?}");
        }
    }
    drop(lock);
    std::fs::remove_dir_all(&base).ok();
}

/// Neither provider: a replay with no prefix for either, and a live site asked about
/// a provider this build does not know, which needs nothing the host has installed.
#[test]
fn invariant_15_a_provider_not_detected_observes_nothing() {
    let (base, state) = state("neither");
    let lock = acquire(&state, std::fs::metadata(&base).unwrap().uid()).unwrap();
    let leaf = ExecutableName::parse("xz").unwrap();
    let replayed = NeverRun { replay: true };
    let ctx = RegistryContext {
        runner: &replayed,
        home: None,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::ThisMachine,
        prefix_override: None,
    };
    let sites = destination_sites(&ctx, &state, &lock);
    for id in ["homebrew", "macports"] {
        assert!(!sites.observes(&ProviderId::new(id)), "{id}");
    }
    let live = NeverRun { replay: false };
    let ctx = RegistryContext {
        runner: &live,
        home: None,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::ThisMachine,
        prefix_override: None,
    };
    let sites = destination_sites(&ctx, &state, &lock);
    assert!(sites.is_live());
    let unknown = ProviderId::new("no-such-provider");
    assert!(!sites.observes(&unknown));
    assert_eq!(
        sites.ownership(&unknown, "xz", &leaf, None),
        DestinationFacts::NotObserved
    );
    assert_eq!(
        sites.integrity(&unknown, "xz"),
        DestinationFacts::NotObserved
    );
    assert_eq!(sites.linkage(&unknown, "xz"), DestinationFacts::NotObserved);
    drop(lock);
    std::fs::remove_dir_all(&base).ok();
}

/// The probe's one route, holding a real lock: under a replay, or a filesystem that
/// is not this machine, it answers `NOT_OBSERVED` for either provider and asks the
/// runner nothing, which panics if asked (INV-15; "What H-3 owes", H.md).
#[test]
fn invariant_15_a_replay_never_runs_a_probe() {
    let (base, state) = state("probe");
    let lock = acquire(&state, std::fs::metadata(&base).unwrap().uid()).unwrap();
    let leaf = ExecutableName::parse("xz").unwrap();
    let argv = ["--version".to_owned()];
    for (replay, filesystem) in [
        (true, FilesystemScope::Elsewhere),
        (true, FilesystemScope::ThisMachine),
        (false, FilesystemScope::Elsewhere),
    ] {
        let runner = NeverRun { replay };
        let ctx = RegistryContext {
            runner: &runner,
            home: None,
            platform_tags: Box::new(|_| Vec::new()),
            filesystem,
            prefix_override: Some(vec![(ProviderId::new("homebrew"), "/usr/local".into())]),
        };
        let sites = destination_sites(&ctx, &state, &lock);
        for id in ["homebrew", "macports"] {
            assert_eq!(
                sites.liveness_probe(&ProviderId::new(id), "xz", &leaf, &argv, &lock),
                CheckAnswer::Unchecked(UncheckedCause::NotObserved),
                "{id} {replay} {filesystem:?}"
            );
        }
    }
    drop(lock);
    std::fs::remove_dir_all(&base).ok();
}

/// Live, on this host, with a runner that has not said it may start a probe: the
/// admission's condition 1 refuses before anything is read, whichever provider is
/// installed here, and a provider this build does not know is never built. The
/// runner panics if asked to run anything.
#[test]
fn a_live_runner_that_may_not_run_probes_never_starts_one() {
    let (base, state) = state("probe-live");
    let lock = acquire(&state, std::fs::metadata(&base).unwrap().uid()).unwrap();
    let leaf = ExecutableName::parse("xz").unwrap();
    let argv = ["--version".to_owned()];
    let runner = NeverRun { replay: false };
    let ctx = RegistryContext {
        runner: &runner,
        home: None,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::ThisMachine,
        prefix_override: None,
    };
    let sites = destination_sites(&ctx, &state, &lock);
    for id in ["homebrew", "macports", "no-such-provider"] {
        assert_eq!(
            sites.liveness_probe(&ProviderId::new(id), "xz", &leaf, &argv, &lock),
            CheckAnswer::Unchecked(UncheckedCause::NotObserved),
            "{id}"
        );
    }
    drop(lock);
    std::fs::remove_dir_all(&base).ok();
}

/// The list path would pass the wrong package: `openssl@4` ships `bin/openssl`
/// while the placed entry resolves into `openssl@3` (R-26 E-6). The rule is on the
/// entry's real path, so it names the package that owns what is placed.
#[test]
fn the_placed_entry_answers_for_its_own_keg_and_not_for_the_list_path() {
    let cellar = Path::new("/usr/local/Cellar");
    let (name, version) = keg_of(
        Path::new("/usr/local/Cellar/openssl@3/3.5.0/bin/openssl"),
        cellar,
    )
    .expect("in a keg");
    assert_eq!((name.as_str(), version.as_str()), ("openssl@3", "3.5.0"));
}
