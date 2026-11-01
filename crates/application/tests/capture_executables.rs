// SPDX-License-Identifier: Apache-2.0

//! Part F-2b's capture: the executable lists for the packages a command
//! names, and one `PATH` map with an explicit state per name beside the
//! per-provider map (`docs/passes/16-plans/F.md`, Design §3).
//!
//! The providers and the machine here are stubs constructed in code: the
//! capture loop is what is under test, and it reaches the machine only
//! through the two domain contracts. Nothing here is a recording, and nothing
//! is written under `fixtures/` (SG2-N7). The adapters' own reads are tested
//! against the captured recording in `crates/adapters/tests/executable_list.rs`.

use macportsbrew_application::{capture, CaptureContext};
use macportsbrew_domain::{
    AvailabilityAssessment, DomainError, EnvironmentSnapshot, ErrorId, ExecutableList,
    ExecutableName, ExecutableOwner, HomeRedaction, ListReason, Machine, MachineRead,
    PackageIdentity, PathSearch, Placement, ProviderCapabilities, ProviderDetection, ProviderId,
    ProviderInventory, ProviderRead, ProviderSnapshot, Resolution, ResolutionState, Snapshot,
    PINNED_ENVIRONMENT,
};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const HOME: &str = "/Users/someone";

/// A provider at `/opt/<id>` (or under the home) holding `names`, each with a
/// metadata row, answering `executables` from `lists` and recording what it
/// was asked.
struct Stub {
    id: ProviderId,
    prefix: PathBuf,
    snap: ProviderSnapshot,
    lists: BTreeMap<String, (ExecutableList, Placement)>,
    fail: bool,
    asked: RefCell<Vec<PackageIdentity>>,
    /// Answers no resolution at all (an `Err`).
    silent: bool,
    /// Claims to own every path, wherever it is.
    claims_all: bool,
    /// Reports this directory joined with the leaf as the canonical path.
    canonical_in: Option<PathBuf>,
    /// Names this package as the owner, instead of the leaf.
    owner_package: Option<String>,
}

impl Stub {
    fn new(id: &str, prefix: &str, names: &[&str]) -> Self {
        let installed: Vec<serde_json::Value> = names
            .iter()
            .map(|n| {
                serde_json::json!({
                    "identity": {"provider": id, "name": n},
                    "kind": "PACKAGE", "version": "1.0", "active": true
                })
            })
            .collect();
        let metadata: serde_json::Map<String, serde_json::Value> = names
            .iter()
            .map(|n| ((*n).to_string(), serde_json::json!({})))
            .collect();
        let mut invocation: BTreeMap<String, String> = PINNED_ENVIRONMENT
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        invocation.insert(
            "PATH".into(),
            format!("{prefix}/bin:/usr/bin:/bin:/usr/sbin:/sbin"),
        );
        let snap: ProviderSnapshot = serde_json::from_value(serde_json::json!({
            "detection": {"provider": id, "presence": {"state": "PRESENT"}, "prefix": prefix},
            "capabilities": {"capabilities": {}},
            "inventory": {
                "installed": installed,
                "requested": {"names": [], "mechanism": "stub"},
                "dependencies": [], "metadata": metadata, "availability": {}
            },
            "invocationEnvironment": invocation,
        }))
        .expect("the stub's snapshot parses");
        Stub {
            id: ProviderId::new(id),
            prefix: PathBuf::from(prefix),
            snap,
            lists: BTreeMap::new(),
            fail: false,
            asked: RefCell::new(Vec::new()),
            silent: false,
            claims_all: false,
            canonical_in: None,
            owner_package: None,
        }
    }

    fn with_list(mut self, name: &str, list: ExecutableList, placement: Placement) -> Self {
        self.lists.insert(name.into(), (list, placement));
        self
    }
}

impl ProviderRead for Stub {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn detect(&self) -> Result<ProviderDetection, DomainError> {
        Ok(self.snap.detection.clone())
    }
    fn capabilities(&self) -> Result<ProviderCapabilities, DomainError> {
        Ok(self.snap.capabilities.clone())
    }
    fn inventory(&self) -> Result<ProviderInventory, DomainError> {
        Ok(self.snap.inventory.clone())
    }
    fn artifact_availability(
        &self,
        _pkg: &PackageIdentity,
        _machine: &Machine,
    ) -> Result<AvailabilityAssessment, DomainError> {
        Err(DomainError::new(ErrorId::Internal, "not asked by capture"))
    }
    /// Owns what is under its prefix, from its "records"; says nothing it can
    /// settle about anything else.
    fn resolution(&self, executable: &Path) -> Result<Resolution, DomainError> {
        let leaf = executable
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if self.silent {
            return Err(DomainError::new(ErrorId::Internal, "says nothing"));
        }
        let owner = if self.claims_all || executable.starts_with(&self.prefix) {
            ExecutableOwner::Provider {
                provider: self.id.clone(),
                package: Some(self.owner_package.clone().unwrap_or_else(|| leaf.clone())),
            }
        } else {
            ExecutableOwner::Unknown
        };
        let canonical_path = self.canonical_in.as_ref().map(|d| d.join(&leaf));
        Ok(Resolution {
            executable: leaf,
            path: Some(executable.to_path_buf()),
            canonical_path,
            authoritative: matches!(owner, ExecutableOwner::Provider { .. }),
            owner,
        })
    }
    fn environment_info(&self) -> Result<BTreeMap<String, String>, DomainError> {
        Ok(BTreeMap::new())
    }
    fn snapshot(&self) -> Result<ProviderSnapshot, DomainError> {
        Ok(self.snap.clone())
    }
    fn executables(
        &self,
        pkg: &PackageIdentity,
    ) -> Result<(ExecutableList, Placement), DomainError> {
        self.asked.borrow_mut().push(pkg.clone());
        if self.fail {
            return Err(DomainError::new(
                ErrorId::Internal,
                "the adapter's own defect",
            ));
        }
        Ok(self
            .lists
            .get(&pkg.name)
            .cloned()
            .unwrap_or((ExecutableList::NotRecorded, Placement::default())))
    }
}

/// A machine whose `PATH` is `path` (unredacted; redacted by `HOME` when the
/// environment is read) and whose search answers from `hits`. Live: a name
/// with no hit was searched and not found. Replayed: nothing is searched.
struct Machine2 {
    path: Vec<String>,
    hits: BTreeMap<String, String>,
    replay: bool,
    searched: RefCell<Vec<String>>,
    /// How this machine redacts, and the capture with it.
    home: HomeRedaction,
}

impl Machine2 {
    fn live(path: &[&str], hits: &[(&str, &str)]) -> Self {
        Machine2 {
            path: path.iter().map(|e| (*e).to_string()).collect(),
            hits: hits
                .iter()
                .map(|(n, p)| ((*n).to_string(), (*p).to_string()))
                .collect(),
            replay: false,
            searched: RefCell::new(Vec::new()),
            home: HomeRedaction::Known(HOME.into()),
        }
    }
}

impl MachineRead for Machine2 {
    fn machine(&self) -> Result<Machine, DomainError> {
        use macportsbrew_domain::{Architecture, CommandLineTools, MacosVersion, Translation};
        Ok(Machine {
            macos: MacosVersion::new(26, 0, 0),
            architecture: Architecture::X86_64,
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Missing,
        })
    }
    fn environment(&self) -> Result<EnvironmentSnapshot, DomainError> {
        let home = &self.home;
        Ok(EnvironmentSnapshot {
            path: self
                .path
                .iter()
                .map(|e| macportsbrew_domain::redact_home(e, home))
                .collect(),
            ..EnvironmentSnapshot::default()
        })
    }
    fn which(&self, executable: &str) -> Option<PathBuf> {
        if self.replay {
            return None;
        }
        self.hits.get(executable).map(PathBuf::from)
    }
    fn search_path(&self, executable: &ExecutableName) -> PathSearch {
        self.searched
            .borrow_mut()
            .push(executable.as_str().to_string());
        if self.replay {
            return PathSearch::NotSearched;
        }
        match self.which(executable.as_str()) {
            Some(path) => PathSearch::Found {
                path_index: macportsbrew_domain::path_index(&self.path, &path),
                path,
            },
            None => PathSearch::NotFound,
        }
    }
}

fn run(machine: &Machine2, providers: &[&Stub], named: &[&str]) -> Result<Snapshot, DomainError> {
    capture(&CaptureContext {
        machine,
        providers: providers.iter().map(|p| *p as &dyn ProviderRead).collect(),
        policy_version: "test".into(),
        captured_at: "2026-11-01T00:00:00Z".into(),
        captured_by: "capture_executables.rs".into(),
        home: machine.home.clone(),
        environment_contracts: BTreeMap::new(),
        executables_for: named.iter().map(|n| (*n).to_string()).collect(),
    })
}

fn name(text: &str) -> ExecutableName {
    ExecutableName::parse(text).expect("a valid name")
}

fn xz_list() -> (ExecutableList, Placement) {
    (
        ExecutableList::from_leaves(["xz", "lzcat"]),
        Placement::OnPathDir {
            dirs: [
                (name("xz"), "/opt/first/bin".to_string()),
                (name("lzcat"), "/opt/first/bin".to_string()),
            ]
            .into_iter()
            .collect(),
        },
    )
}

const PATH: &[&str] = &["/opt/first/bin", "/usr/bin", "/opt/second/bin"];

#[test]
fn the_provider_map_is_still_written_by_todays_rule() {
    // `xz` under both providers, found in `first`'s bin; `lzcat` only in a
    // list; `m4` found in `/usr/bin`, which neither provider owns; `tree`
    // nowhere.
    let (list, placement) = xz_list();
    let first = Stub::new("first", "/opt/first", &["xz", "m4"]).with_list("xz", list, placement);
    let second = Stub::new("second", "/opt/second", &["xz", "tree"]);
    let machine = Machine2::live(
        PATH,
        &[
            ("xz", "/opt/first/bin/xz"),
            ("lzcat", "/opt/first/bin/lzcat"),
            ("m4", "/usr/bin/m4"),
        ],
    );
    let snapshot = run(&machine, &[&first, &second], &["xz"]).expect("the capture loads");

    // The per-provider map: installed package names only, each under the
    // provider that answered best - the owner for `xz`, the first answer for
    // `m4` - and nothing for a list-only name or a name that did not resolve.
    let filed = |id: &str| -> BTreeSet<String> {
        snapshot.providers[&ProviderId::new(id)]
            .resolutions
            .keys()
            .cloned()
            .collect()
    };
    assert_eq!(filed("first"), ["m4", "xz"].map(String::from).into());
    assert!(filed("second").is_empty());

    // The PATH map: every searched name, with its state.
    let state = |n: &str| snapshot.resolution_of(&name(n));
    match state("xz") {
        ResolutionState::Found {
            resolution,
            path_index,
        } => {
            assert_eq!(path_index, Some(0));
            assert_eq!(
                resolution,
                snapshot.providers[&ProviderId::new("first")].resolutions["xz"],
                "the two maps carry one resolution"
            );
        }
        other => panic!("xz: {other:?}"),
    }
    assert!(matches!(
        state("lzcat"),
        ResolutionState::Found {
            path_index: Some(0),
            ..
        }
    ));
    assert!(matches!(
        state("m4"),
        ResolutionState::Found {
            path_index: Some(1),
            ..
        }
    ));
    assert_eq!(state("tree"), ResolutionState::NotOnPath);
    assert_eq!(state("never-installed"), ResolutionState::NotObserved);

    // The list is filed on its row, the older set written from it.
    let row = &snapshot.providers[&ProviderId::new("first")]
        .inventory
        .metadata["xz"];
    assert_eq!(
        row.executable_list,
        ExecutableList::from_leaves(["xz", "lzcat"])
    );
    assert_eq!(row.executables, ["lzcat", "xz"].map(String::from).into());
    assert!(matches!(row.placement, Placement::OnPathDir { .. }));
    // And the document reloads: the load rules accept what capture wrote.
    let text = serde_json::to_string(&snapshot).unwrap();
    let back: Snapshot = serde_json::from_str(&text).unwrap();
    back.check(&BTreeMap::new())
        .expect("a captured snapshot loads under the fail-closed rule");
    assert_eq!(back, snapshot);
}

#[test]
fn invariant_15_a_replayed_capture_records_no_resolution_rather_than_none() {
    // INV-15 and SG-2: a reader that did not search records nothing - the
    // map stays empty, so every name reads NOT_OBSERVED - and never
    // NOT_ON_PATH, which only a search that ran may say.
    let first = Stub::new("first", "/opt/first", &["xz", "tree"]);
    let mut machine = Machine2::live(PATH, &[("xz", "/opt/first/bin/xz")]);
    machine.replay = true;
    let snapshot = run(&machine, &[&first], &[]).expect("the capture completes");
    assert!(snapshot.environment.resolutions.is_empty());
    assert!(snapshot.providers[&ProviderId::new("first")]
        .resolutions
        .is_empty());
    for n in ["xz", "tree"] {
        assert_eq!(
            snapshot.resolution_of(&name(n)),
            ResolutionState::NotObserved,
            "{n}"
        );
    }
}

#[test]
fn a_name_failing_the_class_is_never_resolved_and_stays_not_observed() {
    // A list holds only names of the class (`from_leaves` refuses the whole
    // list otherwise), and every package name a capture keeps is of it too,
    // so the guard is the capture's own parse before the search: a name
    // outside the class reaches no search at all. Here a provider reports
    // one, and the search log shows it was never asked about; the capture then
    // fails on the name itself, as it always has.
    for bad in ["-x", "a/b", ".."] {
        let first = Stub::new("first", "/opt/first", &["xz", bad]);
        let machine = Machine2::live(PATH, &[("xz", "/opt/first/bin/xz")]);
        let outcome = run(&machine, &[&first], &[]);
        assert!(
            !machine.searched.borrow().iter().any(|n| n == bad),
            "{bad:?} was searched"
        );
        assert!(machine.searched.borrow().iter().any(|n| n == "xz"));
        assert!(outcome.is_err(), "{bad:?}: the name check still refuses it");
    }
}

#[test]
fn only_named_installed_rows_are_read_and_with_their_own_identity() {
    // `executables_for` selects rows by the inventory's name, in every
    // provider that holds one; a name nobody holds sends nothing (SR-13).
    let first = Stub::new("first", "/opt/first", &["xz", "m4"]);
    let second = Stub::new("second", "/opt/second", &["xz"]);
    let machine = Machine2::live(PATH, &[]);
    run(&machine, &[&first, &second], &["xz", "nosuch"]).expect("the capture completes");
    assert_eq!(
        *first.asked.borrow(),
        vec![PackageIdentity::new(ProviderId::new("first"), "xz")]
    );
    assert_eq!(
        *second.asked.borrow(),
        vec![PackageIdentity::new(ProviderId::new("second"), "xz")]
    );
    // And a capture that names nothing reads nothing: analyze, doctor and
    // every other command capture exactly as before.
    let first = Stub::new("first", "/opt/first", &["xz"]);
    run(&machine, &[&first], &[]).expect("the capture completes");
    assert!(first.asked.borrow().is_empty());
}

#[test]
fn an_adapter_defect_in_the_list_read_aborts_the_capture() {
    // F.md §2, SR-8: an `Err` is a defect or a vanished provider, never a
    // per-package unknown.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.fail = true;
    let machine = Machine2::live(PATH, &[]);
    assert!(run(&machine, &[&first], &["xz"]).is_err());
}

#[test]
fn a_placement_the_resolution_contradicts_becomes_unknown() {
    // SR L-1: the adapter says `xz` lands in `/opt/first/bin`, entry 0 of
    // PATH. The search found it only at entry 2, or not at all: the placement
    // is not trusted over what PATH did.
    for hits in [vec![("xz", "/opt/second/bin/xz")], vec![]] {
        let (list, placement) = xz_list();
        let first = Stub::new("first", "/opt/first", &["xz"]).with_list("xz", list, placement);
        let machine = Machine2::live(PATH, &hits);
        let snapshot = run(&machine, &[&first], &["xz"]).expect("the capture completes");
        assert_eq!(
            snapshot.providers[&ProviderId::new("first")]
                .inventory
                .metadata["xz"]
                .placement,
            Placement::unknown(ListReason::ContradictedByResolution),
            "{hits:?}"
        );
    }
    // Confirmed: found at the placed entry, or shadowed by an earlier one.
    let at_the_entry = [
        ("xz", "/opt/first/bin/xz"),
        ("lzcat", "/opt/first/bin/lzcat"),
    ];
    let shadowed = [("xz", "/opt/first/bin/xz"), ("lzcat", "/usr/bin/lzcat")];
    for (path, hits) in [
        (PATH.to_vec(), &at_the_entry),
        (vec!["/usr/bin", "/opt/first/bin"], &shadowed),
    ] {
        let (list, placement) = xz_list();
        let first =
            Stub::new("first", "/opt/first", &["xz"]).with_list("xz", list, placement.clone());
        let machine = Machine2::live(&path, hits);
        let snapshot = run(&machine, &[&first], &["xz"]).expect("the capture completes");
        assert_eq!(
            snapshot.providers[&ProviderId::new("first")]
                .inventory
                .metadata["xz"]
                .placement,
            placement,
            "{path:?}"
        );
    }
}

#[test]
fn the_path_map_and_a_placement_under_the_home_are_redacted() {
    // A provider installed under the home (DP-021): its placement and both
    // copies of a resolution are written without the home, and the reloaded
    // document still passes the placement rule, because the prefix and the
    // directory were redacted by one function.
    let prefix = format!("{HOME}/tool");
    let dir = format!("{prefix}/bin");
    let first = Stub::new("first", &prefix, &["xz"]).with_list(
        "xz",
        ExecutableList::from_leaves(["xz"]),
        Placement::OnPathDir {
            dirs: [(name("xz"), dir.clone())].into_iter().collect(),
        },
    );
    let machine = Machine2::live(&[dir.as_str(), "/usr/bin"], &[("xz", &format!("{dir}/xz"))]);
    // Captured here with the provider's own prefix redacted as capture does.
    let snapshot = run(&machine, &[&first], &["xz"]).expect("the capture completes");
    let text = serde_json::to_string(&snapshot).unwrap();
    assert!(!text.contains(HOME), "the home survived: {text}");
    assert!(!text.contains("someone"), "the user name survived");
    assert!(
        matches!(
            snapshot.resolution_of(&name("xz")),
            ResolutionState::Found {
                path_index: Some(0),
                ..
            }
        ),
        "the index was taken before redaction"
    );
}

#[test]
fn the_names_searched_from_lists_are_bounded() {
    // Security review of F-2b, L-3: every hit can cost a provider call, so the
    // listed names searched in one capture are capped at MAX_EXECUTABLES; the
    // rest stay NOT_OBSERVED.
    let many = |prefix: &str| -> ExecutableList {
        let names: Vec<String> = (0..3000).map(|i| format!("{prefix}{i}")).collect();
        ExecutableList::from_leaves(names.iter().map(String::as_str))
    };
    let first = Stub::new("first", "/opt/first", &["a", "b"])
        .with_list("a", many("a"), Placement::unknown(ListReason::NotRecorded))
        .with_list("b", many("b"), Placement::unknown(ListReason::NotRecorded));
    let machine = Machine2::live(PATH, &[]);
    let snapshot = run(&machine, &[&first], &["a", "b"]).expect("the capture completes");
    let listed = machine
        .searched
        .borrow()
        .iter()
        .filter(|n| n.len() > 1)
        .count();
    assert_eq!(listed, macportsbrew_domain::MAX_EXECUTABLES);
    assert_eq!(
        snapshot.resolution_of(&name("b2999")),
        ResolutionState::NotObserved,
        "past the budget: unknown, not a miss"
    );
    assert_eq!(
        snapshot.resolution_of(&name("a0")),
        ResolutionState::NotOnPath
    );
}

#[test]
fn a_row_without_metadata_is_not_read() {
    // spec-guardian L-4: the list could not be filed, so nothing is sent.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.snap.inventory.metadata.clear();
    let machine = Machine2::live(PATH, &[]);
    let _ = run(&machine, &[&first], &["xz"]);
    assert!(first.asked.borrow().is_empty());
}

#[test]
fn a_not_linked_reason_under_the_home_is_redacted() {
    // Security review of F-2b, Info-2: the provider's free text is redacted
    // like every other, the user name with the home.
    let first = Stub::new("first", "/opt/first", &["xz"]).with_list(
        "xz",
        ExecutableList::from_leaves(["xz"]),
        Placement::NotLinked {
            reason: format!("keg-only: shadows {HOME}/bin/xz"),
        },
    );
    let machine = Machine2::live(PATH, &[]);
    let snapshot = run(&machine, &[&first], &["xz"]).expect("the capture completes");
    let Placement::NotLinked { reason } = &snapshot.providers[&ProviderId::new("first")]
        .inventory
        .metadata["xz"]
        .placement
    else {
        panic!("the placement kept its kind");
    };
    assert!(!reason.contains(HOME), "{reason}");
    assert!(!reason.contains("someone"), "{reason}");
    assert!(reason.contains("~/bin/xz"), "{reason}");
}

#[test]
fn a_path_entry_through_dot_dot_never_makes_a_live_capture_refuse_itself() {
    // Security review of F-2b, M-1: `which` keeps a `..` entry as written, and
    // the load rule refuses such a FOUND. Capture records the hit in neither
    // map, so the snapshot loads and the name reads NOT_OBSERVED.
    let first = Stub::new("first", "/opt/first", &["xz", "m4"]);
    let machine = Machine2::live(
        &["/opt/first/libexec/../bin", "/usr/bin"],
        &[
            ("xz", "/opt/first/libexec/../bin/xz"),
            ("m4", "/usr/bin/m4"),
        ],
    );
    let snapshot = run(&machine, &[&first], &[]).expect("the capture completes");
    assert_eq!(
        snapshot.resolution_of(&name("xz")),
        ResolutionState::NotObserved
    );
    assert!(!snapshot.providers[&ProviderId::new("first")]
        .resolutions
        .contains_key("xz"));
    assert!(matches!(
        snapshot.resolution_of(&name("m4")),
        ResolutionState::Found {
            path_index: Some(1),
            ..
        }
    ));
}

#[test]
fn an_index_that_redaction_makes_ambiguous_is_dropped_not_fatal() {
    // Security review of F-2b, I-1: with the home unresolved, every
    // `/Users/<name>` reads alike once redacted, so a correct index into the
    // second home's `bin` would fail the load rule against the redacted
    // copy. Capture records the hit with no index instead of aborting.
    let first = Stub::new("first", "/opt/first", &["xz"]);
    let mut machine = Machine2::live(
        &["/Users/ada/bin", "/Users/bob/bin"],
        &[("xz", "/Users/bob/bin/xz")],
    );
    machine.home = HomeRedaction::Unresolved;
    let snapshot = run(&machine, &[&first], &[]).expect("the capture completes");
    let recorded = &snapshot.environment.path;
    assert_eq!(
        recorded[0], recorded[1],
        "the two entries read alike: {recorded:?}"
    );
    assert!(
        matches!(
            snapshot.resolution_of(&name("xz")),
            ResolutionState::Found {
                path_index: None,
                ..
            }
        ),
        "{:?}",
        snapshot.resolution_of(&name("xz"))
    );
}

#[test]
fn the_selection_is_present_providers_installed_rows_with_metadata() {
    // Security review of F-2b, L-2: the `snapshot` report and the reads come
    // from one selection.
    let first = Stub::new("first", "/opt/first", &["xz", "m4"]);
    let mut absent = Stub::new("second", "/opt/second", &["xz"]);
    absent.snap.detection.presence = macportsbrew_domain::ProviderPresence::Absent;
    let mut bare = Stub::new("third", "/opt/third", &["xz"]);
    bare.snap.inventory.metadata.clear();
    let providers: BTreeMap<ProviderId, ProviderSnapshot> = [&first, &absent, &bare]
        .iter()
        .map(|p| (p.id.clone(), p.snap.clone()))
        .collect();
    let names: BTreeSet<String> = ["xz", "nosuch"].map(String::from).into();
    assert_eq!(
        macportsbrew_application::executables_selection(&providers, &names),
        [(ProviderId::new("first"), "xz".to_string())].into()
    );
}

// --- the independent mutation proof at `1061877`: one test per surviving gap ---

fn placement_of(snapshot: &Snapshot, id: &str, pkg: &str) -> Placement {
    snapshot.providers[&ProviderId::new(id)].inventory.metadata[pkg]
        .placement
        .clone()
}

fn contradicted() -> Placement {
    Placement::unknown(ListReason::ContradictedByResolution)
}

#[test]
fn one_executable_found_only_further_along_contradicts_the_placement() {
    // CR2: `lzcat` at the placed entry does not save `xz`, found later.
    let (list, placement) = xz_list();
    let first = Stub::new("first", "/opt/first", &["xz"]).with_list("xz", list, placement);
    let machine = Machine2::live(
        PATH,
        &[
            ("xz", "/opt/second/bin/xz"),
            ("lzcat", "/opt/first/bin/lzcat"),
        ],
    );
    let snapshot = run(&machine, &[&first], &["xz"]).unwrap();
    assert_eq!(placement_of(&snapshot, "first", "xz"), contradicted());
}

#[test]
fn every_placed_executable_is_checked_not_only_the_first() {
    // CR7: `lzcat` (first in order) confirmed, `xz` not on PATH.
    let (list, placement) = xz_list();
    let first = Stub::new("first", "/opt/first", &["xz"]).with_list("xz", list, placement);
    let machine = Machine2::live(PATH, &[("lzcat", "/opt/first/bin/lzcat")]);
    let snapshot = run(&machine, &[&first], &["xz"]).unwrap();
    assert_eq!(placement_of(&snapshot, "first", "xz"), contradicted());
}

#[test]
fn a_path_entry_with_a_trailing_slash_is_the_placed_directory() {
    // CR6: compared by component, so `/opt/first/bin/` is the entry.
    let (list, placement) = xz_list();
    let first = Stub::new("first", "/opt/first", &["xz"]).with_list("xz", list, placement);
    let machine = Machine2::live(&["/opt/first/bin/", "/usr/bin"], &[]);
    let snapshot = run(&machine, &[&first], &["xz"]).unwrap();
    assert_eq!(placement_of(&snapshot, "first", "xz"), contradicted());
}

#[test]
fn a_placed_directory_under_the_home_is_found_on_the_redacted_path() {
    // CR5: the directory is redacted the same way as the recorded PATH before
    // they are compared, so a home prefix's `bin` is still found on it.
    let prefix = format!("{HOME}/tool");
    let dir = format!("{prefix}/bin");
    let first = Stub::new("first", &prefix, &["xz"]).with_list(
        "xz",
        ExecutableList::from_leaves(["xz"]),
        Placement::OnPathDir {
            dirs: [(name("xz"), dir.clone())].into_iter().collect(),
        },
    );
    let machine = Machine2::live(&[dir.as_str(), "/usr/bin"], &[]);
    let snapshot = run(&machine, &[&first], &["xz"]).unwrap();
    assert_eq!(placement_of(&snapshot, "first", "xz"), contradicted());
}

#[test]
fn an_owner_is_preferred_over_an_earlier_unknown_answer() {
    // C6: `first` answers UNKNOWN, `second` owns: filed under `second`.
    let first = Stub::new("first", "/opt/first", &["xz"]);
    let second = Stub::new("second", "/opt/second", &["xz"]);
    let machine = Machine2::live(PATH, &[("xz", "/opt/second/bin/xz")]);
    let snapshot = run(&machine, &[&first, &second], &[]).unwrap();
    assert!(snapshot.providers[&ProviderId::new("second")]
        .resolutions
        .contains_key("xz"));
    assert!(!snapshot.providers[&ProviderId::new("first")]
        .resolutions
        .contains_key("xz"));
}

#[test]
fn the_first_owner_in_registry_order_wins() {
    // C5: both claim the path; the first in the order the registry gives wins.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.claims_all = true;
    let mut second = Stub::new("second", "/opt/second", &["xz"]);
    second.claims_all = true;
    let machine = Machine2::live(PATH, &[("xz", "/opt/second/bin/xz")]);
    let snapshot = run(&machine, &[&first, &second], &[]).unwrap();
    assert!(snapshot.providers[&ProviderId::new("first")]
        .resolutions
        .contains_key("xz"));
    assert!(!snapshot.providers[&ProviderId::new("second")]
        .resolutions
        .contains_key("xz"));
}

#[test]
fn package_names_do_not_spend_the_listed_name_budget() {
    // C3: `xz` and `m4` are package names, searched anyway; only the 4095
    // `a…` names and `b0` spend the budget, so `b0` is still searched.
    let mut a: Vec<String> = vec!["xz".into()];
    a.extend((0..4095).map(|i| format!("a{i}")));
    let first = Stub::new("first", "/opt/first", &["xz", "m4"])
        .with_list(
            "xz",
            ExecutableList::from_leaves(a.iter().map(String::as_str)),
            Placement::default(),
        )
        .with_list(
            "m4",
            ExecutableList::from_leaves(["m4", "b0"]),
            Placement::default(),
        );
    let machine = Machine2::live(PATH, &[]);
    let snapshot = run(&machine, &[&first], &["xz", "m4"]).unwrap();
    assert_eq!(
        snapshot.resolution_of(&name("b0")),
        ResolutionState::NotOnPath
    );
}

#[test]
fn two_rows_of_one_name_in_one_provider_are_read_once() {
    // S2.
    let first = Stub::new("first", "/opt/first", &["xz", "xz"]);
    let machine = Machine2::live(PATH, &[]);
    let _ = run(&machine, &[&first], &["xz"]);
    assert_eq!(first.asked.borrow().len(), 1);
}

#[test]
fn a_name_found_that_no_provider_answers_about_is_left_out() {
    // C7 (deviation 3): found, and every provider is silent: absent from the
    // map - NOT_OBSERVED - never NOT_ON_PATH, and in neither map.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.silent = true;
    let machine = Machine2::live(PATH, &[("xz", "/opt/first/bin/xz")]);
    let snapshot = run(&machine, &[&first], &[]).unwrap();
    assert!(!snapshot.environment.resolutions.contains_key(&name("xz")));
    assert!(snapshot.providers[&ProviderId::new("first")]
        .resolutions
        .is_empty());
}

#[test]
fn a_canonical_path_under_the_home_is_redacted_in_the_path_map() {
    // X4.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.canonical_in = Some(PathBuf::from(format!("{HOME}/real")));
    let machine = Machine2::live(PATH, &[("xz", "/opt/first/bin/xz")]);
    let snapshot = run(&machine, &[&first], &[]).unwrap();
    let ResolutionState::Found { resolution, .. } = snapshot.resolution_of(&name("xz")) else {
        panic!("xz was found");
    };
    let canonical = resolution.canonical_path.expect("a canonical path");
    assert_eq!(canonical, PathBuf::from("~/real/xz"));
}

#[test]
fn an_absent_provider_gets_no_read() {
    // S4: a named installed row under a provider that is not present.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.snap.detection.presence = macportsbrew_domain::ProviderPresence::Absent;
    first.snap.invocation_environment.clear();
    let machine = Machine2::live(PATH, &[]);
    let _ = run(&machine, &[&first], &["xz"]);
    assert!(first.asked.borrow().is_empty());
}

// --- the independent mutation proof of the fix round, and security I-4 ------

/// Capture completes, `xz` is unobserved and in neither map, and the
/// snapshot passes the whole load check.
fn left_out_and_loadable(first: &Stub) {
    let machine = Machine2::live(PATH, &[("xz", "/opt/first/bin/xz")]);
    let snapshot = run(&machine, &[first], &[]).expect("the capture completes");
    assert_eq!(
        snapshot.resolution_of(&name("xz")),
        ResolutionState::NotObserved
    );
    assert!(!snapshot.providers[&ProviderId::new("first")]
        .resolutions
        .contains_key("xz"));
    snapshot
        .check(&BTreeMap::new())
        .expect("the snapshot loads");
}

#[test]
fn a_hit_with_a_canonical_path_that_is_not_printable_is_left_out() {
    // M1-canonical.
    let mut first = Stub::new("first", "/opt/first", &["xz"]);
    first.canonical_in = Some(PathBuf::from("/opt/first/x\u{1b}[2J"));
    left_out_and_loadable(&first);
}

#[test]
fn a_hit_whose_owner_package_is_not_a_name_is_left_out() {
    // M1-owner.
    for package in ["-rf", "a b"] {
        let mut first = Stub::new("first", "/opt/first", &["xz"]);
        first.owner_package = Some(package.into());
        left_out_and_loadable(&first);
    }
}

#[test]
fn a_placement_the_load_rule_would_refuse_is_recorded_as_unknown() {
    // Security I-4: a directory with a `.` component cannot be kept, so the
    // placement is unknown and `snapshot <package>` still completes.
    let first = Stub::new("first", "/opt/first", &["xz"]).with_list(
        "xz",
        ExecutableList::from_leaves(["xz"]),
        Placement::OnPathDir {
            dirs: [(name("xz"), "/opt/first/./bin".to_string())]
                .into_iter()
                .collect(),
        },
    );
    // The directory is not on this PATH, so the resolution settles nothing and
    // only the shape rule decides.
    let machine = Machine2::live(&["/usr/bin"], &[]);
    let snapshot = run(&machine, &[&first], &["xz"]).expect("the capture completes");
    assert_eq!(
        placement_of(&snapshot, "first", "xz"),
        Placement::unknown(ErrorId::ProviderOutputChanged)
    );
    snapshot
        .check(&BTreeMap::new())
        .expect("the snapshot loads");
}

#[test]
fn every_name_not_read_gets_the_notice_its_case_calls_for() {
    // L2: decided beside the selection, from the same facts.
    use macportsbrew_application::{executables_notices, NoticeKey};
    let present = Stub::new("first", "/opt/first", &["xz", "m4"]);
    let mut gone = Stub::new("second", "/opt/second", &["tree"]);
    gone.snap.detection.presence = macportsbrew_domain::ProviderPresence::Broken {
        reason: "error.provider_output_changed".into(),
    };
    let mut bare = Stub::new("third", "/opt/third", &["htop"]);
    bare.snap.inventory.metadata.clear();
    let providers: BTreeMap<ProviderId, ProviderSnapshot> = [&present, &gone, &bare]
        .iter()
        .map(|p| (p.id.clone(), p.snap.clone()))
        .collect();
    let names: BTreeSet<String> = ["xz", "tree", "htop", "nosuch"].map(String::from).into();
    assert_eq!(
        executables_notices(&providers, &names),
        vec![
            ("htop".to_string(), NoticeKey::PackageNotRead),
            ("nosuch".to_string(), NoticeKey::PackageNotInstalled),
            ("tree".to_string(), NoticeKey::PackageNotRead),
        ],
        "a selected name (xz) gets none"
    );
    assert_eq!(
        NoticeKey::PackageNotInstalled.message_id(),
        "snapshot.package_not_installed"
    );
    assert_eq!(
        NoticeKey::PackageNotRead.message_id(),
        "snapshot.package_not_read"
    );
}
