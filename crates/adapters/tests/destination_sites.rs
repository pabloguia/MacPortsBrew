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
    DestinationFacts, DestinationProbe, DomainError, ExecutableName, ProbeObservation, ProviderId,
    UncheckedCause,
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
                sites.ownership(&provider, "xz", &leaf),
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

#[test]
fn invariant_15_a_replay_never_runs_a_probe() {
    let (base, state) = state("probe");
    let lock = acquire(&state, std::fs::metadata(&base).unwrap().uid()).unwrap();
    let runner = NeverRun { replay: true };
    let ctx = RegistryContext {
        runner: &runner,
        home: None,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::Elsewhere,
        prefix_override: None,
    };
    let sites = destination_sites(&ctx, &state, &lock);
    assert_eq!(
        sites.liveness_probe(
            "xz",
            &ExecutableName::parse("xz").unwrap(),
            &["--version".to_owned()]
        ),
        ProbeObservation::NotRun(UncheckedCause::NotObserved)
    );
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
