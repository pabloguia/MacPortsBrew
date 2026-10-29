// SPDX-License-Identifier: Apache-2.0

//! The MacPorts Portfile reader's path walk (`macports::portfile::declared`),
//! on scratch registries. These build symbolic links to prove the walk refuses
//! them, so they live here rather than in `src/`, which the stop gate scans and
//! no symlink call may appear in (work order 16, part D). The recogniser's
//! tests, on in-code text, stay beside it.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::macports::portfile::{
    declared, Entry, MAX_FILE_BYTES, MAX_PORTGROUP_ENTRIES, PORTFILES_DIRECTORY,
    PORTGROUPS_DIRECTORY,
};
use macportsbrew_domain::Answer;
use std::path::PathBuf;

// ---------------------------------------------------------------------
// The path walk, on scratch trees.
// ---------------------------------------------------------------------

/// A scratch `portdbpath` with a registry in the A-1 layout.
struct Registry(PathBuf);

const PLAIN: &str = "PortSystem 1.0\nname p\n";

impl Registry {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("mpb-portfile-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join(PORTFILES_DIRECTORY)).unwrap();
        std::fs::create_dir_all(root.join(PORTGROUPS_DIRECTORY)).unwrap();
        Self(root)
    }

    fn hash_name(text: &str) -> String {
        // Any 64 hex digits will do: the reader checks the shape and the
        // size, not the digest.
        format!("{:064x}-{}", text.len(), text.len())
    }

    /// Installs `text` as the Portfile of `leaf`; returns its hash dir.
    fn install(&self, leaf: &str, text: &str) -> PathBuf {
        let dir = self
            .0
            .join(PORTFILES_DIRECTORY)
            .join(leaf)
            .join(Self::hash_name(text));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Portfile"), text).unwrap();
        dir
    }

    fn group(&self, name: &str, text: &str) {
        let dir = self
            .0
            .join(PORTGROUPS_DIRECTORY)
            .join(Self::hash_name(text));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), text).unwrap();
    }

    fn answer(&self, name: &str, version: &str, revision: Option<&str>) -> Answer {
        let entries = [Entry {
            name,
            version,
            revision,
        }];
        declared(Some(&self.0), &entries, FilesystemScope::ThisMachine)
            .remove(name)
            .unwrap()
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[test]
fn an_installed_portfile_is_read_through_the_registry() {
    let reg = Registry::new("happy");
    reg.install("p-1.0_0", PLAIN);
    reg.install("s-1.0_0", "startupitem.create yes\n");
    std::fs::write(
        reg.0.join(PORTFILES_DIRECTORY).join("p-1.0_0/.DS_Store"),
        "",
    )
    .unwrap();
    assert_eq!(reg.answer("p", "1.0", Some("0")), Answer::No);
    assert_eq!(reg.answer("s", "1.0", Some("0")), Answer::Yes);
}

#[test]
fn every_shape_the_walk_does_not_expect_is_unknown() {
    let reg = Registry::new("shapes");
    // Absent version directory.
    assert_eq!(reg.answer("absent", "1.0", Some("0")), Answer::Unknown);
    // A missing revision is unknown, never assumed `0` (L1).
    reg.install("p-1.0_0", PLAIN);
    assert_eq!(reg.answer("p", "1.0", None), Answer::Unknown);
    // A hostile version.
    assert_eq!(reg.answer("p", "../x", Some("0")), Answer::Unknown);
    assert_eq!(reg.answer("p", "1.0/../../x", Some("0")), Answer::Unknown);
    assert_eq!(reg.answer("p", "-1.0", Some("0")), Answer::Unknown);
    // Two hash directories (the variants case of A-1).
    reg.install("two-1.0_0", PLAIN);
    reg.install("two-1.0_0", "PortSystem 1.0\nname two\n");
    assert_eq!(reg.answer("two", "1.0", Some("0")), Answer::Unknown);
    // A non-matching sibling.
    let dir = reg.install("sib-1.0_0", PLAIN);
    std::fs::write(dir.parent().unwrap().join("stray"), "").unwrap();
    assert_eq!(reg.answer("sib", "1.0", Some("0")), Answer::Unknown);
    // A symlinked hash directory.
    let real = reg.install("real-1.0_0", PLAIN);
    let linked = reg.0.join(PORTFILES_DIRECTORY).join("link-1.0_0");
    std::fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(&real, linked.join(Registry::hash_name(PLAIN))).unwrap();
    assert_eq!(reg.answer("link", "1.0", Some("0")), Answer::Unknown);
    // A size mismatch.
    let dir = reg.install("size-1.0_0", PLAIN);
    std::fs::write(dir.join("Portfile"), format!("{PLAIN}more\n")).unwrap();
    assert_eq!(reg.answer("size", "1.0", Some("0")), Answer::Unknown);
    // A symlinked Portfile.
    let dir = reg.install("slink-1.0_0", PLAIN);
    std::fs::remove_file(dir.join("Portfile")).unwrap();
    std::os::unix::fs::symlink(real.join("Portfile"), dir.join("Portfile")).unwrap();
    assert_eq!(reg.answer("slink", "1.0", Some("0")), Answer::Unknown);
    // A hard-linked Portfile.
    let dir = reg.install("hard-1.0_0", PLAIN);
    std::fs::hard_link(dir.join("Portfile"), reg.0.join("second-name")).unwrap();
    assert_eq!(reg.answer("hard", "1.0", Some("0")), Answer::Unknown);
}

#[test]
fn an_oversized_portfile_is_unknown() {
    let reg = Registry::new("oversized");
    let big = "#".repeat(usize::try_from(MAX_FILE_BYTES).unwrap() + 1);
    reg.install("big-1.0_0", &big);
    assert_eq!(reg.answer("big", "1.0", Some("0")), Answer::Unknown);
}

#[test]
fn a_stored_portgroup_mentioning_the_token_or_unreadable_blocks_no() {
    let reg = Registry::new("groups");
    reg.install("g-1.0_0", "PortSystem 1.0\nPortGroup github 1.0\n");
    reg.install("p-1.0_0", PLAIN);
    reg.group("github-1.0.tcl", "proc github.setup {} {}\n");
    assert_eq!(reg.answer("g", "1.0", Some("0")), Answer::No);
    reg.group("x-1.0.tcl", "startupitem.create yes\n");
    assert_eq!(reg.answer("g", "1.0", Some("0")), Answer::Unknown);
    assert_eq!(
        reg.answer("p", "1.0", Some("0")),
        Answer::No,
        "no PortGroup used"
    );

    let unreadable = Registry::new("groups-unreadable");
    unreadable.install("g-1.0_0", "PortSystem 1.0\nPortGroup github 1.0\n");
    let dir = unreadable
        .0
        .join(PORTGROUPS_DIRECTORY)
        .join(Registry::hash_name("x"));
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", dir.join("x-1.0.tcl")).unwrap();
    assert_eq!(unreadable.answer("g", "1.0", Some("0")), Answer::Unknown);

    let absent = Registry::new("groups-absent");
    absent.install("g-1.0_0", "PortSystem 1.0\nPortGroup github 1.0\n");
    std::fs::remove_dir_all(absent.0.join(PORTGROUPS_DIRECTORY)).unwrap();
    assert_eq!(absent.answer("g", "1.0", Some("0")), Answer::Unknown);
}

#[test]
fn more_stored_groups_than_the_bound_is_unknown_not_truncated() {
    let reg = Registry::new("groups-many");
    reg.install("g-1.0_0", "PortGroup github 1.0\n");
    for n in 0..=MAX_PORTGROUP_ENTRIES {
        std::fs::create_dir_all(reg.0.join(PORTGROUPS_DIRECTORY).join(format!("{n:064x}-1")))
            .unwrap();
    }
    assert_eq!(reg.answer("g", "1.0", Some("0")), Answer::Unknown);
}

#[test]
fn a_non_default_portdbpath_or_another_machine_is_unknown_for_every_port() {
    let reg = Registry::new("elsewhere");
    reg.install("s-1.0_0", "startupitem.create yes\n");
    let entries = [Entry {
        name: "s",
        version: "1.0",
        revision: Some("0"),
    }];
    assert_eq!(
        declared(None, &entries, FilesystemScope::ThisMachine)["s"],
        Answer::Unknown
    );
    assert_eq!(
        declared(Some(&reg.0), &entries, FilesystemScope::Elsewhere)["s"],
        Answer::Unknown,
        "INV-15"
    );
}

#[test]
fn installed_entries_of_one_port_combine_yes_then_unknown() {
    let reg = Registry::new("combine");
    reg.install("p-1.0_0", PLAIN);
    reg.install("p-2.0_0", "startupitem.create yes\n");
    let entry = |version, revision| Entry {
        name: "p",
        version,
        revision,
    };
    let root = Some(reg.0.as_path());
    let scope = FilesystemScope::ThisMachine;
    assert_eq!(
        declared(root, &[entry("1.0", Some("0"))], scope)["p"],
        Answer::No
    );
    assert_eq!(
        declared(
            root,
            &[entry("1.0", Some("0")), entry("3.0", Some("0"))],
            scope
        )["p"],
        Answer::Unknown
    );
    assert_eq!(
        declared(
            root,
            &[
                entry("3.0", Some("0")),
                entry("2.0", Some("0")),
                entry("1.0", Some("0"))
            ],
            scope
        )["p"],
        Answer::Yes
    );
}
