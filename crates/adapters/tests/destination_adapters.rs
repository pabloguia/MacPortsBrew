// SPDX-License-Identifier: Apache-2.0

//! Each adapter's destination reads, over a fake runner and a scratch prefix (work
//! order 16 part H-2). Synthesised answers in the shapes R-25 and R-26 quote, because
//! no recording holds these reads (an owner item), and links planted in a scratch
//! tree, which is why this is an integration test: the stop gate's token scan covers
//! `src/`, and no release carries this binary.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::homebrew::destination::comparable_version as keg_comparable;
use macportsbrew_adapters::homebrew::HomebrewAdapter;
use macportsbrew_adapters::macports::destination::comparable_version as port_comparable;
use macportsbrew_adapters::macports::MacPortsAdapter;
use macportsbrew_adapters::process::{
    CommandSpec, ExitStatus, Invocation, ProcessRunner, RecordedInvocation, Recording,
    RecordingProvenance, ReplayRunner,
};
use macportsbrew_domain::{
    DependencyRecord, DestinationFacts, DestinationQuery, DomainError, ExecutableName,
    InstallationRecord, InstalledPackage, IntegrityBasis, IntegrityFacts, OwnershipFacts,
    PackageIdentity, PackageKind, PrefixClass, PrefixTable, ProviderId, ProviderRead,
    UncheckedCause, Version,
};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "mpb-destination-adapters-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(std::fs::canonicalize(&dir).unwrap())
    }

    fn put(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"plain").unwrap();
        path
    }

    fn link(&self, relative: &str, target: &Path) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(target, &path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Answers from `entries`, and says it is the live machine unless told otherwise,
/// which is what lets the adapters' scope guards be tested on their own.
struct Fake {
    inner: ReplayRunner,
    replay: bool,
}

impl ProcessRunner for Fake {
    fn run(&self, spec: &CommandSpec) -> Result<Invocation, DomainError> {
        self.inner.run(spec)
    }

    fn is_replay(&self) -> bool {
        self.replay
    }
}

fn fake(program: &str, entries: &[(Vec<String>, String, i32)], replay: bool) -> Fake {
    let recording = Recording {
        schema_version: 1,
        provenance: RecordingProvenance {
            macos: "26.6.2".into(),
            architecture: "x86_64".into(),
            provider_versions: vec![],
            captured_at: "2026-11-04T00:00:00Z".into(),
            captured_by: "crates/adapters/tests/destination_adapters.rs".into(),
            origin: "synthesised".into(),
            note: Some("built in-test for one case; not a machine capture".into()),
        },
        invocations: entries
            .iter()
            .map(|(argv, stdout, code)| RecordedInvocation {
                invocation: Invocation {
                    program: program.into(),
                    argv: argv.clone(),
                    environment: BTreeMap::new(),
                    status: ExitStatus::Exited { code: *code },
                    stdout: stdout.clone(),
                    stderr: String::new(),
                    truncated: false,
                    duration_ms: 0,
                },
            })
            .collect(),
    };
    Fake {
        inner: ReplayRunner::new(recording),
        replay,
    }
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

fn leaf(name: &str) -> ExecutableName {
    ExecutableName::parse(name).unwrap()
}

// --- Homebrew: a rule on the path, a list made at call time ------------------

/// One formula of `brew info --json=v2 --installed`, in the shape R-1 quotes.
fn formula(name: &str, version: &str, revision: u32, dependencies: Option<&[&str]>) -> String {
    let runtime = dependencies.map_or(String::new(), |names| {
        let items: Vec<String> = names
            .iter()
            .map(|n| format!(r#"{{"full_name": "{n}"}}"#))
            .collect();
        format!(r#", "runtime_dependencies": [{}]"#, items.join(", "))
    });
    format!(
        r#"{{"name": "{name}", "versions": {{"stable": "{version}"}}, "revision": {revision},
            "bottle": {{"stable": {{"files": {{"sonoma": {{"url": "x"}}}}}}}},
            "linked_keg": "{version}", "service": null, "aliases": ["{name}-alias"],
            "oldnames": ["{name}-old"],
            "installed": [{{"version": "{version}", "poured_from_bottle": true{runtime}}}]}}"#
    )
}

fn brew(
    scratch: &Scratch,
    formulae: &[String],
    lists: &[(&str, Vec<PathBuf>)],
    replay: bool,
) -> Fake {
    let document = format!(r#"{{"formulae": [{}], "casks": []}}"#, formulae.join(","));
    let mut entries = vec![
        (
            argv(&["--cellar"]),
            format!("{}\n", scratch.0.join("Cellar").display()),
            0,
        ),
        (argv(&["info", "--json=v2", "--installed"]), document, 0),
        (argv(&["--version"]), "Homebrew 7.0.11\n".to_owned(), 0),
        (argv(&["list", "--pinned"]), String::new(), 0),
        (
            argv(&["--caskroom"]),
            format!("{}\n", scratch.0.join("Caskroom").display()),
            0,
        ),
    ];
    for (name, paths) in lists {
        let lines: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        entries.push((
            argv(&["list", "--formula", name]),
            format!("{}\n", lines.join("\n")),
            0,
        ));
    }
    fake("brew", &entries, replay)
}

fn hb<'r>(runner: &'r Fake, scratch: &Scratch, scope: FilesystemScope) -> HomebrewAdapter<'r> {
    HomebrewAdapter::new(runner, scratch.0.clone(), None, Vec::new(), scope)
}

fn hb_id(name: &str) -> PackageIdentity {
    PackageIdentity::new(ProviderId::new("homebrew"), name)
}

fn mp_id(name: &str) -> PackageIdentity {
    PackageIdentity::new(ProviderId::new("macports"), name)
}

fn owner_of(adapter: &dyn ProviderRead, pkg: &PackageIdentity, name: &str) -> OwnershipFacts {
    let name = leaf(name);
    let query = DestinationQuery::Ownership {
        leaf: &name,
        held: None,
    };
    match adapter.destination_facts(pkg, &query) {
        DestinationFacts::Ownership(facts) => facts,
        other => panic!("{other:?}"),
    }
}

fn integrity_of(adapter: &dyn ProviderRead, pkg: &PackageIdentity) -> IntegrityFacts {
    match adapter.destination_facts(pkg, &DestinationQuery::Integrity) {
        DestinationFacts::Integrity(facts) => facts,
        other => panic!("{other:?}"),
    }
}

/// A linked keg: a real file, its link in `bin`, and a receipt.
fn linked_keg(scratch: &Scratch, name: &str, version: &str) -> Vec<PathBuf> {
    let keg = format!("Cellar/{name}/{version}");
    let binary = scratch.put(&format!("{keg}/bin/{name}"));
    let receipt = scratch.put(&format!("{keg}/INSTALL_RECEIPT.json"));
    scratch.link(&format!("bin/{name}"), &binary);
    vec![binary, receipt]
}

#[test]
fn homebrew_the_placed_entry_names_its_owner_and_active_version() {
    let scratch = Scratch::new();
    linked_keg(&scratch, "xz", "5.8.3");
    let runner = brew(
        &scratch,
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        &[],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &hb_id("xz"), "xz"),
        OwnershipFacts::Owner {
            name: "xz".to_owned(),
            version: "5.8.3".to_owned(),
            active: Some("5.8.3".to_owned()),
            versions_present: 1,
            held: None,
        }
    );
}

/// R-26 E-6: the list path would pass the wrong package.
#[test]
fn homebrew_the_placed_entry_answers_and_never_the_list_path() {
    let scratch = Scratch::new();
    linked_keg(&scratch, "openssl@3", "3.6.4");
    std::fs::rename(
        scratch.0.join("bin/openssl@3"),
        scratch.0.join("bin/openssl"),
    )
    .unwrap();
    // `openssl@4` ships `bin/openssl` in its keg, and is not what is placed.
    scratch.put("Cellar/openssl@4/4.0.0/bin/openssl");
    let runner = brew(
        &scratch,
        &[
            formula("openssl@3", "3.6.4", 0, Some(&[])),
            formula("openssl@4", "4.0.0", 0, Some(&[])),
        ],
        &[],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    match owner_of(&adapter, &hb_id("openssl@4"), "openssl") {
        OwnershipFacts::Owner { name, .. } => assert_eq!(name, "openssl@3"),
        other => panic!("{other:?}"),
    }
}

/// R-26 finding 14: `npm`.
#[test]
fn homebrew_a_link_leaving_its_keg_names_the_owner_of_its_first_hop() {
    let scratch = Scratch::new();
    let last = scratch.put("lib/node_modules/npm/bin/npm");
    let hop = scratch.link("Cellar/node/26.7.0/bin/npm", &last);
    scratch.link("bin/npm", &hop);
    let runner = brew(
        &scratch,
        &[formula("node", "26.7.0", 0, Some(&[]))],
        &[],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &hb_id("node"), "npm"),
        OwnershipFacts::OwnerLeavesItsDirectory {
            owner: "node".to_owned()
        }
    );
    // The owner is the hop's, not whichever package is being asked about.
    assert_eq!(
        owner_of(&adapter, &hb_id("other"), "npm"),
        OwnershipFacts::OwnerLeavesItsDirectory {
            owner: "node".to_owned()
        }
    );
}

#[test]
fn homebrew_no_entry_a_dangling_entry_and_a_file_of_no_keg_are_each_their_own() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.0.join("Cellar")).unwrap();
    scratch.link("bin/gone", &scratch.0.join("nowhere"));
    scratch.put("bin/plain");
    let runner = brew(
        &scratch,
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        &[],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    let xz = hb_id("xz");
    assert_eq!(owner_of(&adapter, &xz, "absent"), OwnershipFacts::NoEntry);
    assert_eq!(owner_of(&adapter, &xz, "gone"), OwnershipFacts::NoEntry);
    assert_eq!(owner_of(&adapter, &xz, "plain"), OwnershipFacts::NotOwned);
}

/// A resolution that failed for a reason that says nothing about the entry.
#[test]
fn a_canonicalize_error_that_is_not_absence_is_never_no_entry_on_either_provider() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.0.join("Cellar")).unwrap();
    let locked = scratch.0.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    scratch.link("bin/tool", &locked.join("tool"));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let expected = OwnershipFacts::CouldNotTell(UncheckedCause::StatFailed);
    let brew_runner = brew(&scratch, &[], &[], false);
    let brew_adapter = hb(&brew_runner, &scratch, FilesystemScope::ThisMachine);
    let brew_answer = owner_of(&brew_adapter, &hb_id("x"), "tool");
    let port_runner = port(&[], false);
    let port_adapter = mp(&port_runner, &scratch, FilesystemScope::ThisMachine);
    let port_answer = owner_of(&port_adapter, &mp_id("x"), "tool");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(brew_answer, expected);
    assert_eq!(port_answer, expected);
}

/// `gts` at R-25's shape: the keg's directory holds revision 3 and the formula is at
/// 4. Measured here: 19 of 194 formulae have a suffix a naive append would double.
#[test]
fn homebrew_the_held_version_is_the_rows_own_and_is_not_appended_to() {
    let scratch = Scratch::new();
    linked_keg(&scratch, "gts", "0.7.6_3");
    let runner = brew(
        &scratch,
        &[formula("gts", "0.7.6_3", 4, Some(&[]))],
        &[],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    let inventory = adapter.inventory().unwrap();
    let row = inventory.installed.first().expect("a row");
    assert_eq!(row.revision.as_deref(), Some("4"));
    assert_eq!(keg_comparable(row), "0.7.6_3");
    let name = leaf("gts");
    let query = DestinationQuery::Ownership {
        leaf: &name,
        held: Some(row),
    };
    match adapter.destination_facts(&hb_id("gts"), &query) {
        DestinationFacts::Ownership(OwnershipFacts::Owner { version, held, .. }) => {
            assert_eq!(held.as_deref(), Some(version.as_str()));
        }
        other => panic!("{other:?}"),
    }
}

fn homebrew_integrity(
    scratch: &Scratch,
    name: &str,
    formulae: &[String],
    list: Vec<PathBuf>,
) -> IntegrityFacts {
    let runner = brew(scratch, formulae, &[(name, list)], false);
    let adapter = hb(&runner, scratch, FilesystemScope::ThisMachine);
    integrity_of(&adapter, &hb_id(name))
}

#[test]
fn homebrew_integrity_records_what_its_list_says_and_never_claims_a_stored_record() {
    let scratch = Scratch::new();
    let list = linked_keg(&scratch, "xz", "5.8.3");
    let facts = homebrew_integrity(
        &scratch,
        "xz",
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        list.clone(),
    );
    assert_eq!(facts.basis, IntegrityBasis::GeneratedFileList);
    assert_eq!(facts.record, InstallationRecord::Present);
    assert_eq!(facts.dependencies, DependencyRecord::AllPresent);
    assert_eq!((facts.missing, facts.dangling), (0, 0));
    // Without the receipt, the record is absent.
    let without: Vec<PathBuf> = list
        .iter()
        .filter(|p| !p.ends_with("INSTALL_RECEIPT.json"))
        .cloned()
        .collect();
    let facts = homebrew_integrity(
        &scratch,
        "xz",
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        without,
    );
    assert_eq!(facts.record, InstallationRecord::Absent);
}

/// `[]` is an honest none, and no list at all says nothing (R-25 finding 11).
#[test]
fn homebrew_runtime_dependencies_told_empty_absent_and_missing_are_four_answers() {
    let scratch = Scratch::new();
    let list = linked_keg(&scratch, "wget", "1.25.0");
    let of = |dependencies: Option<&[&str]>, also: &[String]| {
        let mut formulae = vec![formula("wget", "1.25.0", 0, dependencies)];
        formulae.extend_from_slice(also);
        homebrew_integrity(&scratch, "wget", &formulae, list.clone()).dependencies
    };
    assert_eq!(of(Some(&[]), &[]), DependencyRecord::AllPresent);
    assert_eq!(of(None, &[]), DependencyRecord::NotTold);
    assert_eq!(of(Some(&["libidn2"]), &[]), DependencyRecord::Missing);
    let libidn2 = [formula("libidn2", "2.3.8", 0, Some(&[]))];
    assert_eq!(
        of(Some(&["libidn2"]), &libidn2),
        DependencyRecord::AllPresent
    );
    // R-25 finding 12: a renamed dependency answers by the names it has had.
    assert_eq!(
        of(Some(&["libidn2-alias"]), &libidn2),
        DependencyRecord::AllPresent
    );
    assert_eq!(
        of(Some(&["libidn2-old"]), &libidn2),
        DependencyRecord::AllPresent
    );
    // A tap-qualified name holds `/`, which the document's parser refuses outright,
    // so the whole inventory fails closed and the read says it could not tell.
    assert_eq!(
        of(Some(&["acme/tap/libidn2"]), &libidn2),
        DependencyRecord::NotKept
    );
}

/// INV-08: the calculation reads this provider's inventory and nothing else. The
/// other provider holding the dependency, even on this disk, satisfies nothing.
#[test]
fn invariant_08_a_dependency_is_answered_by_this_providers_inventory_alone() {
    let scratch = Scratch::new();
    let list = linked_keg(&scratch, "wget", "1.25.0");
    // The other provider's copy of `libidn2`, as a file on this disk.
    scratch.put("other-prefix/lib/libidn2.dylib");
    scratch.put("other-prefix/bin/libidn2");
    let facts = homebrew_integrity(
        &scratch,
        "wget",
        &[formula("wget", "1.25.0", 0, Some(&["libidn2"]))],
        list,
    );
    assert_eq!(facts.dependencies, DependencyRecord::Missing);
}

#[test]
fn homebrew_a_missing_or_dangling_listed_path_is_counted_apart() {
    let scratch = Scratch::new();
    let mut list = linked_keg(&scratch, "xz", "5.8.3");
    list.push(scratch.0.join("Cellar/xz/5.8.3/share/gone"));
    list.push(scratch.link("Cellar/xz/5.8.3/share/link", &scratch.0.join("nowhere")));
    list.push(
        scratch
            .0
            .join("Cellar/xz/5.8.3/share")
            .join("a".repeat(300)),
    );
    let facts = homebrew_integrity(
        &scratch,
        "xz",
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        list,
    );
    assert_eq!(
        (facts.missing, facts.dangling, facts.stat_failed),
        (1, 1, 1)
    );
}

#[test]
fn homebrew_a_list_of_another_keg_or_a_failed_read_is_unusable() {
    let scratch = Scratch::new();
    linked_keg(&scratch, "xz", "5.8.3");
    let foreign = vec![scratch.put("Cellar/zlib/1/lib/z")];
    let facts = homebrew_integrity(
        &scratch,
        "xz",
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        foreign,
    );
    assert_eq!(facts.list_unusable, Some(UncheckedCause::ReadFailed));
    // No recorded answer for the list read at all.
    let runner = brew(
        &scratch,
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        &[],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        integrity_of(&adapter, &hb_id("xz")).list_unusable,
        Some(UncheckedCause::ReadFailed)
    );
    // A package this document does not hold has no list to read.
    assert_eq!(
        integrity_of(&adapter, &hb_id("nothing")).list_unusable,
        Some(UncheckedCause::FileListNotKnown)
    );
}

fn table_for(scratch: &Scratch) -> PrefixTable {
    let mut table = PrefixTable::default();
    table.add(scratch.0.as_os_str().as_bytes(), PrefixClass::Destination);
    table
}

#[test]
fn homebrew_linkage_walks_the_keg_it_was_listed() {
    let scratch = Scratch::new();
    let list = linked_keg(&scratch, "xz", "5.8.3");
    let runner = brew(
        &scratch,
        &[formula("xz", "5.8.3", 0, Some(&[]))],
        &[("xz", list)],
        false,
    );
    let adapter = hb(&runner, &scratch, FilesystemScope::ThisMachine);
    let table = table_for(&scratch);
    let query = DestinationQuery::Linkage { prefixes: &table };
    match adapter.destination_facts(&hb_id("xz"), &query) {
        DestinationFacts::Linkage(facts) => {
            assert_eq!(facts.list_unusable, None);
            assert_eq!(facts.loadable_files, 0, "two plain files");
        }
        other => panic!("{other:?}"),
    }
}

// --- MacPorts: a query per path, a stored list ---------------------------------

fn port(entries: &[(Vec<String>, String, i32)], replay: bool) -> Fake {
    fake("port", entries, replay)
}

fn mp<'r>(
    runner: &'r Fake,
    scratch: &Scratch,
    scope: FilesystemScope,
) -> MacPortsAdapter<&'r Fake> {
    MacPortsAdapter::new(runner, scratch.0.clone(), None, scope)
}

fn provides(path: &Path, answer: &str) -> (Vec<String>, String, i32) {
    (
        argv(&["-q", "provides", path.to_str().unwrap()]),
        answer.to_owned(),
        0,
    )
}

fn installed(name: &str, rows: &str) -> (Vec<String>, String, i32) {
    (argv(&["-q", "installed", name]), rows.to_owned(), 0)
}

#[test]
fn macports_the_owner_is_a_query_and_its_version_the_active_row() {
    let scratch = Scratch::new();
    let file = scratch.put("bin/xz");
    let runner = port(
        &[
            provides(&file, "xz\n"),
            installed("xz", "  xz @5.8.3_0 (active)\n"),
        ],
        false,
    );
    let adapter = mp(&runner, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &mp_id("xz"), "xz"),
        OwnershipFacts::Owner {
            name: "xz".to_owned(),
            version: "5.8.3_0".to_owned(),
            active: Some("5.8.3_0".to_owned()),
            versions_present: 1,
            held: None,
        }
    );
}

/// An empty answer means several things (R-26 E-3); only a name is another's.
#[test]
fn macports_an_empty_answer_says_nothing_and_a_named_other_says_whose() {
    let scratch = Scratch::new();
    let file = scratch.put("bin/xz");
    let silent = port(&[provides(&file, "")], false);
    let adapter = mp(&silent, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &mp_id("xz"), "xz"),
        OwnershipFacts::CouldNotTell(UncheckedCause::OwnerNotNamed)
    );
    let named = port(&[provides(&file, "lzma\n")], false);
    let adapter = mp(&named, &scratch, FilesystemScope::ThisMachine);
    match owner_of(&adapter, &mp_id("xz"), "xz") {
        OwnershipFacts::Owner { name, .. } => assert_eq!(name, "lzma"),
        other => panic!("{other:?}"),
    }
}

/// A failed read is never "not owned", the old reader's conflation (R-26 E-8).
#[test]
fn macports_a_failed_read_or_an_odd_answer_is_not_a_negative() {
    let scratch = Scratch::new();
    let file = scratch.put("bin/xz");
    let failed = port(
        &[(
            argv(&["-q", "provides", file.to_str().unwrap()]),
            String::new(),
            1,
        )],
        false,
    );
    let adapter = mp(&failed, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &mp_id("xz"), "xz"),
        OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed)
    );
    let odd = port(&[provides(&file, "xz\nzlib\n")], false);
    let adapter = mp(&odd, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &mp_id("xz"), "xz"),
        OwnershipFacts::CouldNotTell(UncheckedCause::ReadFailed)
    );
}

/// INV-17: a link leaving the prefix answers as one leaving a keg does on the peer.
#[test]
fn macports_a_link_leaving_the_prefix_answers_as_the_peer_does() {
    let scratch = Scratch::new();
    let prefix = Scratch::new();
    let far = scratch.put("elsewhere/tool");
    let entry = prefix.link("bin/tool", &far);
    let runner = port(&[provides(&entry, "tool\n")], false);
    let adapter = mp(&runner, &prefix, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &mp_id("tool"), "tool"),
        OwnershipFacts::OwnerLeavesItsDirectory {
            owner: "tool".to_owned()
        }
    );
}

#[test]
fn macports_an_entry_and_its_final_file_must_both_answer() {
    let scratch = Scratch::new();
    let real = scratch.put("bin/xz");
    let entry = scratch.link("bin/lzcat", &real);
    let other = port(
        &[provides(&entry, "xz\n"), provides(&real, "lzma\n")],
        false,
    );
    let adapter = mp(&other, &scratch, FilesystemScope::ThisMachine);
    match owner_of(&adapter, &mp_id("xz"), "lzcat") {
        OwnershipFacts::Owner { name, .. } => assert_eq!(name, "lzma"),
        other => panic!("{other:?}"),
    }
    let silent = port(&[provides(&entry, "xz\n"), provides(&real, "")], false);
    let adapter = mp(&silent, &scratch, FilesystemScope::ThisMachine);
    assert_eq!(
        owner_of(&adapter, &mp_id("xz"), "lzcat"),
        OwnershipFacts::CouldNotTell(UncheckedCause::OwnerNotNamed)
    );
}

#[test]
fn macports_no_active_row_has_no_version_to_answer_with() {
    let scratch = Scratch::new();
    let file = scratch.put("bin/xz");
    let runner = port(
        &[provides(&file, "xz\n"), installed("xz", "  xz @5.8.3_0\n")],
        false,
    );
    let adapter = mp(&runner, &scratch, FilesystemScope::ThisMachine);
    match owner_of(&adapter, &mp_id("xz"), "xz") {
        OwnershipFacts::Owner { active, .. } => assert_eq!(active, None),
        other => panic!("{other:?}"),
    }
}

fn row(version: &str, revision: Option<&str>) -> InstalledPackage {
    InstalledPackage {
        identity: mp_id("xz"),
        kind: PackageKind::Package,
        version: Version::new(version),
        revision: revision.map(str::to_owned),
        variants: BTreeSet::new(),
        active: true,
        requested: None,
        installed_from_prebuilt: None,
        install_path: None,
    }
}

#[test]
fn macports_the_held_version_joins_what_its_row_keeps_apart() {
    assert_eq!(port_comparable(&row("5.8.3", Some("0"))), "5.8.3_0");
    assert_eq!(port_comparable(&row("5.8.3", None)), "5.8.3");
    let scratch = Scratch::new();
    let file = scratch.put("bin/xz");
    let runner = port(
        &[
            provides(&file, "xz\n"),
            installed("xz", "  xz @5.8.3_0 (active)\n"),
        ],
        false,
    );
    let adapter = mp(&runner, &scratch, FilesystemScope::ThisMachine);
    let name = leaf("xz");
    let held = row("5.8.3", Some("0"));
    let query = DestinationQuery::Ownership {
        leaf: &name,
        held: Some(&held),
    };
    match adapter.destination_facts(&mp_id("xz"), &query) {
        DestinationFacts::Ownership(OwnershipFacts::Owner { version, held, .. }) => {
            assert_eq!(held.as_deref(), Some(version.as_str()));
        }
        other => panic!("{other:?}"),
    }
}

fn contents(name: &str, paths: &[PathBuf]) -> (Vec<String>, String, i32) {
    let lines: Vec<String> = paths.iter().map(|p| format!("  {}", p.display())).collect();
    (
        argv(&["-q", "contents", name]),
        format!("{}\n", lines.join("\n")),
        0,
    )
}

#[test]
fn macports_integrity_finds_what_its_record_lists_and_is_gone() {
    let scratch = Scratch::new();
    let file = scratch.put("bin/xz");
    let gone = scratch.0.join("bin/lzcat");
    let dangling = scratch.link("bin/unxz", &scratch.0.join("nowhere"));
    let outside = PathBuf::from("/Applications/MacPorts/X.app/x");
    let long = scratch.0.join("bin").join("a".repeat(300));
    let runner = port(
        &[
            installed("xz", "  xz @5.8.3_0 (active)\n"),
            contents("xz", &[file, gone, dangling, outside, long]),
        ],
        false,
    );
    let adapter = mp(&runner, &scratch, FilesystemScope::ThisMachine);
    let facts = integrity_of(&adapter, &mp_id("xz"));
    assert_eq!(facts.basis, IntegrityBasis::RecordedFileList);
    assert_eq!(facts.list_unusable, None);
    assert_eq!(
        (facts.missing, facts.dangling, facts.stat_failed),
        (1, 1, 1)
    );
    assert_eq!(facts.outside_prefix, 1);
    assert_eq!(facts.record, InstallationRecord::NotKept);
    assert_eq!(facts.dependencies, DependencyRecord::NotKept);
}

#[test]
fn macports_an_inactive_or_doubled_port_has_no_list_to_trust() {
    let scratch = Scratch::new();
    for rows in ["  xz @5.8.3_0\n", "  xz @5.8.3_0 (active)\n  xz @5.8.2_0\n"] {
        let runner = port(&[installed("xz", rows)], false);
        let adapter = mp(&runner, &scratch, FilesystemScope::ThisMachine);
        assert_eq!(
            integrity_of(&adapter, &mp_id("xz")).list_unusable,
            Some(UncheckedCause::FileListNotKnown),
            "{rows}"
        );
    }
}

#[test]
fn macports_linkage_says_what_lies_outside_was_not_examined_and_keeps_what_it_found() {
    let scratch = Scratch::new();
    let file = scratch.put("share/plain");
    let outside = PathBuf::from("/Applications/MacPorts/X.app/x");
    let runner = port(
        &[
            installed("xz", "  xz @5.8.3_0 (active)\n"),
            contents("xz", &[file, outside]),
        ],
        false,
    );
    let adapter = mp(&runner, &scratch, FilesystemScope::ThisMachine);
    let table = table_for(&scratch);
    let query = DestinationQuery::Linkage { prefixes: &table };
    match adapter.destination_facts(&mp_id("xz"), &query) {
        DestinationFacts::Linkage(facts) => {
            assert_eq!(
                facts.list_unusable,
                Some(UncheckedCause::ListsOutsidePrefix)
            );
        }
        other => panic!("{other:?}"),
    }
}

// --- INV-15: each guard on its own ---------------------------------------------

/// Everything a read needs is answered and on disk, so a removed guard answers
/// facts rather than `NOT_OBSERVED`: a filesystem read shows, not only a program.
#[test]
fn invariant_15_the_adapters_own_guard_stops_a_read_of_this_disk() {
    for (replay, scope) in [
        (true, FilesystemScope::ThisMachine),
        (true, FilesystemScope::Elsewhere),
        (false, FilesystemScope::Elsewhere),
    ] {
        let scratch = Scratch::new();
        let list = linked_keg(&scratch, "xz", "5.8.3");
        let runner = brew(
            &scratch,
            &[formula("xz", "5.8.3", 0, Some(&[]))],
            &[("xz", list.clone())],
            replay,
        );
        let adapter = hb(&runner, &scratch, scope);
        let port_runner = port(
            &[
                provides(&scratch.0.join("bin/xz"), "xz\n"),
                installed("xz", "  xz @5.8.3_0 (active)\n"),
                contents("xz", &list),
            ],
            replay,
        );
        let port_adapter = mp(&port_runner, &scratch, scope);
        let name = leaf("xz");
        let table = table_for(&scratch);
        for query in [
            DestinationQuery::Ownership {
                leaf: &name,
                held: None,
            },
            DestinationQuery::Integrity,
            DestinationQuery::Linkage { prefixes: &table },
        ] {
            assert_eq!(
                adapter.destination_facts(&hb_id("xz"), &query),
                DestinationFacts::NotObserved,
                "{replay} {scope:?}"
            );
            assert_eq!(
                port_adapter.destination_facts(&mp_id("xz"), &query),
                DestinationFacts::NotObserved,
                "{replay} {scope:?}"
            );
        }
    }
}
