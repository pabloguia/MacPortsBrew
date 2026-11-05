// SPDX-License-Identifier: Apache-2.0

//! The filesystem half, in a scratch directory. No recording of a contaminated
//! binary can exist, so detection is shown on bytes built here, and said so.
// A helper builds byte counts the format stores in 32 bits; none comes near it.
#![allow(clippy::cast_possible_truncation)]

use macportsbrew_adapters::destination_reads::{
    self, checked_list, examine_with, is_under_system_root, placed_entry, stat_counts, CheckedList,
    Fallbacks, Outside, StatCounts, WalkWork, MAX_WALK_FILES, MAX_WALK_REFERENCES,
};
use macportsbrew_adapters::macho::{HEAD_BYTES, SLICE_HEADER_BYTES};
use macportsbrew_adapters::process::MAX_PATH_LEN;
use macportsbrew_domain::{LinkageFacts, PrefixClass, PrefixTable, UncheckedCause};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "mpb-destination-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(std::fs::canonicalize(&dir).unwrap())
    }

    fn table(&self) -> (PathBuf, PathBuf, PrefixTable) {
        let own = self.0.join("own");
        let other = self.0.join("other");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let mut table = PrefixTable::default();
        table.add(own.as_os_str().as_bytes(), PrefixClass::Destination);
        table.add(other.as_os_str().as_bytes(), PrefixClass::DetectedOther);
        table.add(
            self.0.join("candidate").as_os_str().as_bytes(),
            PrefixClass::UndetectedCandidate,
        );
        (own, other, table)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A thin 64-bit little-endian Mach-O of `file_type` with these load commands.
fn thin(file_type: u32, commands: &[(u32, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    for (cmd, name) in commands {
        let fixed: u32 = if *cmd == 0xc || *cmd == 0xd || *cmd == 0x8000_0018 {
            24
        } else {
            12
        };
        let size = (fixed as usize + name.len() + 1 + 7) & !7;
        let start = body.len();
        body.extend_from_slice(&cmd.to_le_bytes());
        body.extend_from_slice(&(size as u32).to_le_bytes());
        body.extend_from_slice(&fixed.to_le_bytes());
        body.resize(body.len() + fixed as usize - 12, 0);
        body.extend_from_slice(name);
        body.resize(start + size, 0);
    }
    let mut file = Vec::new();
    file.extend_from_slice(&0xfeed_facf_u32.to_le_bytes());
    file.extend_from_slice(&0x0100_0007_u32.to_le_bytes());
    file.extend_from_slice(&3_u32.to_le_bytes());
    file.extend_from_slice(&file_type.to_le_bytes());
    file.extend_from_slice(&(commands.len() as u32).to_le_bytes());
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&[0; 8]);
    file.extend_from_slice(&body);
    file
}

fn put(path: &Path, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    path.to_path_buf()
}

/// `examine` takes only a checked list, so these go through the one way to make one.
fn examine(paths: &[PathBuf], dirs: &[PathBuf], table: &PrefixTable) -> LinkageFacts {
    destination_reads::examine(&listed(Path::new("/"), paths), dirs, table)
}

fn examine_counted(
    paths: &[PathBuf],
    dirs: &[PathBuf],
    table: &PrefixTable,
) -> (LinkageFacts, WalkWork) {
    destination_reads::examine_counted(&listed(Path::new("/"), paths), dirs, table)
}

const LOAD: u32 = 0xc;
const ID: u32 = 0xd;
const RPATH: u32 = 0x8000_001c;
const EXECUTE: u32 = 2;
const DYLIB: u32 = 6;

fn bytes_of(path: &Path) -> &[u8] {
    path.as_os_str().as_bytes()
}

#[test]
fn invariant_07_a_load_command_into_the_other_provider_is_found() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let target = other.join("lib/libz.dylib");
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&target))]),
    );
    let facts = examine(&[file], &[own.join("bin")], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
    assert_eq!(facts.loadable_files, 1);
}

#[test]
fn a_files_own_id_is_not_a_reference_to_anything() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let file = put(
        &own.join("lib/a.dylib"),
        &thin(DYLIB, &[(ID, bytes_of(&other.join("lib/a.dylib")))]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(!facts.links_detected_provider, "{facts:?}");
}

#[test]
fn every_slice_is_read_and_not_only_the_first() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let clean = thin(EXECUTE, &[(LOAD, b"/usr/lib/libSystem.B.dylib")]);
    let dirty = thin(EXECUTE, &[(LOAD, bytes_of(&other.join("lib/x.dylib")))]);
    let first = 4096_u32;
    let second = first + clean.len() as u32;
    let mut fat = Vec::new();
    fat.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    fat.extend_from_slice(&2_u32.to_be_bytes());
    for (offset, size) in [(first, clean.len()), (second, dirty.len())] {
        fat.extend_from_slice(&0x0100_0007_u32.to_be_bytes());
        fat.extend_from_slice(&3_u32.to_be_bytes());
        fat.extend_from_slice(&offset.to_be_bytes());
        fat.extend_from_slice(&(size as u32).to_be_bytes());
        fat.extend_from_slice(&0_u32.to_be_bytes());
    }
    fat.resize(first as usize, 0);
    fat.extend_from_slice(&clean);
    fat.extend_from_slice(&dirty);
    let file = put(&own.join("bin/fat"), &fat);
    let facts = examine(&[file], &[own.join("bin")], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

#[test]
fn a_reference_is_measured_after_dots_are_resolved_and_case_folded() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let dotted = format!("{}/lib/../../other/x.dylib", own.display());
    let upper = other.join("x.dylib").display().to_string().to_uppercase();
    for reference in [dotted, upper] {
        let file = put(
            &own.join("bin/a"),
            &thin(EXECUTE, &[(LOAD, reference.as_bytes())]),
        );
        let facts = examine(&[file], &[], &table);
        assert!(facts.links_detected_provider, "{reference}: {facts:?}");
    }
}

/// The directory-link case: lexically the destination's own, canonically the
/// other's. Mutant: the lexical test alone.
#[test]
fn invariant_07_a_directory_link_inside_the_destination_does_not_launder_the_other_prefix() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    put(&other.join("lib/x.dylib"), &thin(DYLIB, &[]));
    symlink(other.join("lib"), own.join("linked")).unwrap();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&own.join("linked/x.dylib")))]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

/// R-27 finding 14: A links a destination dependency that links the source.
/// Mutant: a dropped walk.
#[test]
fn invariant_07_the_walk_follows_the_destinations_own_files() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let dependency = put(
        &own.join("lib/dep.dylib"),
        &thin(DYLIB, &[(LOAD, bytes_of(&other.join("lib/x.dylib")))]),
    );
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&dependency))]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
    assert_eq!(facts.loadable_files, 2);
}

#[test]
fn a_cycle_of_load_commands_ends() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let a = own.join("lib/a.dylib");
    let b = own.join("lib/b.dylib");
    put(&a, &thin(DYLIB, &[(LOAD, bytes_of(&b))]));
    put(&b, &thin(DYLIB, &[(LOAD, bytes_of(&a))]));
    let facts = examine(&[a], &[], &table);
    assert_eq!(facts.loadable_files, 2, "{facts:?}");
    assert!(!facts.walk_over_bound);
}

#[test]
fn a_destination_file_a_load_command_names_and_nobody_can_read_is_not_fine() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&own.join("lib/gone.dylib")))]),
    );
    assert!(examine(&[file], &[], &table).walk_over_bound);
}

#[test]
fn the_file_bound_is_a_number_and_passing_it_says_so() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let list: Vec<PathBuf> = (0..=MAX_WALK_FILES)
        .map(|n| put(&own.join(format!("share/{n}")), b"plain"))
        .collect();
    assert!(examine(&list, &[], &table).walk_over_bound);
    assert!(!examine(&list[..MAX_WALK_FILES], &[], &table).walk_over_bound);
}

#[test]
fn an_rpath_name_with_no_candidate_is_not_clean() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, b"@rpath/libx.dylib")]),
    );
    assert!(examine(&[file], &[], &table).rpath_unresolved);
}

#[test]
fn invariant_07_an_rpath_candidate_under_the_other_prefix_is_found_though_unused() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let file = put(
        &own.join("bin/a"),
        &thin(
            EXECUTE,
            &[
                (RPATH, bytes_of(&other.join("lib"))),
                (LOAD, b"@rpath/libx.dylib"),
            ],
        ),
    );
    assert!(examine(&[file], &[], &table).links_detected_provider);
}

#[test]
fn executable_path_outside_an_executable_depends_on_who_loads_it() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let dylib = put(
        &own.join("lib/a.dylib"),
        &thin(DYLIB, &[(RPATH, b"@executable_path/../lib")]),
    );
    assert!(examine(&[dylib], &[], &table).rpath_unresolved);
    let executable = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(RPATH, b"@executable_path/../lib")]),
    );
    assert!(!examine(&[executable], &[], &table).rpath_unresolved);
}

#[test]
fn a_candidate_prefix_nobody_was_detected_at_is_only_a_doubt() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&scratch.0.join("candidate/x")))]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.under_undetected_prefix && !facts.links_detected_provider);
}

#[test]
fn the_destinations_own_prefix_counts_for_nothing() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let dep = put(&own.join("lib/d.dylib"), &thin(DYLIB, &[]));
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&dep))]),
    );
    let facts = examine(&[file], &[], &table);
    assert_eq!(
        facts,
        LinkageFacts {
            loadable_files: 2,
            ..LinkageFacts::default()
        }
    );
}

#[test]
fn a_link_is_tested_lexically_and_never_followed_to_read() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let link = own.join("bin/a");
    std::fs::create_dir_all(own.join("bin")).unwrap();
    // Dangling, on purpose: a follow would have failed.
    symlink(other.join("bin/gone"), &link).unwrap();
    let facts = examine(&[link], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
    assert!(!facts.unreadable_file);
}

#[test]
fn a_shebang_counts_as_a_reference_and_env_is_indeterminate_on_the_list_only() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let bin = own.join("bin");
    let absolute = put(
        &bin.join("a"),
        format!("#!{}/bin/python3\n", other.display()).as_bytes(),
    );
    assert!(examine(&[absolute], std::slice::from_ref(&bin), &table).links_detected_provider);
    let env = put(&bin.join("b"), b"#!/usr/bin/env python3\n");
    let facts = examine(&[env], std::slice::from_ref(&bin), &table);
    assert!(facts.env_shebang_on_list, "{facts:?}");
    let off = put(&own.join("share/c"), b"#!/usr/bin/env python3\n");
    let facts = examine(&[off], &[bin], &table);
    assert!(!facts.env_shebang_on_list);
}

/// R-27's closing ruling: a Java class is not hidden from the fat-header rule.
#[test]
fn a_cafebabe_that_does_not_fit_is_malformed_and_never_not_macho() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let mut class = 0xcafe_babe_u32.to_be_bytes().to_vec();
    class.extend_from_slice(&52_u32.to_be_bytes());
    class.extend_from_slice(&[0; 40]);
    let file = put(&own.join("share/A.class"), &class);
    assert!(examine(&[file], &[], &table).malformed_file);
}

#[test]
fn a_listed_fifo_does_not_block_and_a_file_that_cannot_be_opened_says_so() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let locked = put(&own.join("share/locked"), b"x");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let facts = examine(std::slice::from_ref(&locked), &[], &table);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(facts.unreadable_file, "{facts:?}");
}

#[test]
fn stat_names_missing_and_dangling_apart_and_lstat_alone_would_not() {
    let scratch = Scratch::new();
    let present = put(&scratch.0.join("present"), b"x");
    let gone = scratch.0.join("gone");
    let under_a_file = put(&scratch.0.join("a-file"), b"x").join("below");
    let dangling = scratch.0.join("dangling");
    symlink(scratch.0.join("nowhere"), &dangling).unwrap();
    let looped = scratch.0.join("loop");
    symlink(&looped, &looped).unwrap();
    let counts = stat_counts(&listed(
        &scratch.0,
        &[present, gone, under_a_file, dangling, looped],
    ));
    assert_eq!(
        counts,
        StatCounts {
            missing: 2,
            dangling: 2,
            failed: 0,
        }
    );
}

/// What `stat_counts` takes: only [`checked_list`] builds one.
fn listed(root: &Path, paths: &[PathBuf]) -> CheckedList {
    let lines: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    checked_list(
        &format!("{}\n", lines.join("\n")),
        "",
        "test",
        root,
        Outside::IsAnotherShape,
    )
    .expect("a list under its root")
}

/// `ENAMETOOLONG` says nothing about the file: not missing, on `lstat` or on `stat`.
#[test]
fn an_errno_that_proves_nothing_is_counted_as_failed() {
    let scratch = Scratch::new();
    let name = "a".repeat(300);
    let counts = stat_counts(&listed(&scratch.0, &[scratch.0.join(&name)]));
    assert_eq!(counts.failed, 1, "lstat: {counts:?}");
    let link = scratch.0.join("link");
    symlink(&name, &link).unwrap();
    let counts = stat_counts(&listed(&scratch.0, &[link]));
    assert_eq!(counts.failed, 1, "stat: {counts:?}");
}

/// R-25 finding 16's signals, as the read would give them.
#[test]
fn a_list_is_refused_before_anything_touches_the_disk() {
    let root = Path::new("/opt/local");
    let list = |text, outside| checked_list(text, "  ", "test", root, outside);
    assert_eq!(
        list(
            "  /opt/local/bin/xz\n  /Applications/X.app/x\n",
            Outside::IsNotTested
        )
        .map(|list| (list.paths().to_vec(), list.outside())),
        Ok((vec![PathBuf::from("/opt/local/bin/xz")], 1))
    );
    assert_eq!(
        list("  /Applications/X.app/x\n", Outside::IsNotTested),
        Err(UncheckedCause::ListsOutsidePrefix)
    );
    assert_eq!(
        list("  /opt/local2/bin/xz\n", Outside::IsAnotherShape),
        Err(UncheckedCause::ReadFailed)
    );
    assert_eq!(
        list("", Outside::IsNotTested),
        Err(UncheckedCause::ReadFailed)
    );
    assert_eq!(
        list("  /opt/local/../etc/x\n", Outside::IsNotTested),
        Err(UncheckedCause::ReadFailed)
    );
    let long = format!("  /opt/local/{}\n", "a".repeat(MAX_PATH_LEN));
    assert_eq!(
        list(&long, Outside::IsNotTested),
        Err(UncheckedCause::ReadFailed)
    );
}

#[test]
fn the_placed_entry_is_bin_then_sbin_and_lstat_only() {
    let scratch = Scratch::new();
    put(&scratch.0.join("sbin/x"), b"");
    assert_eq!(
        placed_entry(&scratch.0, "x"),
        Ok(Some(scratch.0.join("sbin/x")))
    );
    put(&scratch.0.join("bin/x"), b"");
    assert_eq!(
        placed_entry(&scratch.0, "x"),
        Ok(Some(scratch.0.join("bin/x")))
    );
    assert_eq!(placed_entry(&scratch.0, "y"), Ok(None));
}

// --- review round 1 of H-2 --------------------------------------------------

const DYLD_ENVIRONMENT: u32 = 0x27;

/// A reference that says nothing by its spelling, carried each way a file can.
fn every_carrier(own: &Path, reference: &Path) -> Vec<(&'static str, PathBuf)> {
    let text = bytes_of(reference);
    let mut environment = b"DYLD_LIBRARY_PATH=".to_vec();
    environment.extend_from_slice(text);
    let mut shebang = b"#!".to_vec();
    shebang.extend_from_slice(text);
    shebang.push(b'\n');
    let mut argument = b"#!/usr/bin/env ".to_vec();
    argument.extend_from_slice(text);
    argument.push(b'\n');
    vec![
        (
            "load",
            put(&own.join("bin/load"), &thin(EXECUTE, &[(LOAD, text)])),
        ),
        (
            "rpath",
            put(&own.join("bin/rpath"), &thin(EXECUTE, &[(RPATH, text)])),
        ),
        (
            "environment",
            put(
                &own.join("bin/env"),
                &thin(EXECUTE, &[(DYLD_ENVIRONMENT, &environment)]),
            ),
        ),
        ("shebang", put(&own.join("bin/script"), &shebang)),
        ("argument", put(&own.join("share/argument"), &argument)),
    ]
}

fn assert_found_by_each_carrier(scratch: &Scratch, reference: &Path) {
    let (own, _, table) = scratch.table();
    for (carrier, file) in every_carrier(&own, reference) {
        let facts = examine(&[file], &[], &table);
        assert!(
            facts.links_detected_provider,
            "{carrier} {}: {facts:?}",
            reference.display()
        );
    }
}

/// A link outside every prefix, as `/private/tmp` or `$HOME` would hold one.
#[test]
fn invariant_07_a_plain_link_outside_every_prefix_is_followed_to_where_it_leads() {
    let scratch = Scratch::new();
    let (_, other, _) = scratch.table();
    let outside = scratch.0.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    symlink(&other, outside.join("away")).unwrap();
    put(&other.join("lib/x"), b"x");
    assert_found_by_each_carrier(&scratch, &outside.join("away/lib/x"));
    // So is one whose file is not there: the directory link is enough.
    assert_found_by_each_carrier(&scratch, &outside.join("away/lib/absent"));
}

/// The firmlink spelling: the same inode, in text no prefix matches.
#[test]
fn invariant_07_a_firmlink_spelling_of_the_other_prefix_is_found_by_inode() {
    let scratch = Scratch::new();
    let (_, other, _) = scratch.table();
    let data = Path::new("/System/Volumes/Data");
    let spelled = data.join(other.strip_prefix("/").unwrap());
    if !spelled.exists() {
        // On a Mac the scratch is on the Data volume unless someone put it elsewhere,
        // and a silent return here is how this test would stop testing anything.
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::fs::MetadataExt;
            let scratch_device = std::fs::metadata(&other).unwrap().dev();
            let data_device = std::fs::metadata(data).map(|m| m.dev());
            assert!(
                data_device.map_or(true, |device| device != scratch_device),
                "{} is on the Data volume, and its firmlink spelling is not there",
                other.display()
            );
        }
        eprintln!("skipped: {} is not on a firmlinked volume", other.display());
        return;
    }
    put(&other.join("lib/x"), b"x");
    assert_found_by_each_carrier(&scratch, &spelled.join("lib/x"));
    assert_found_by_each_carrier(&scratch, &spelled.join("lib/absent"));
}

/// The Unicode fold: `ſ` opens `s` on a case-insensitive volume.
#[test]
fn invariant_07_a_unicode_fold_of_the_other_prefix_is_found_by_inode() {
    let scratch = Scratch::new();
    let (own, _, _) = scratch.table();
    let solo = scratch.0.join("solo");
    std::fs::create_dir_all(solo.join("lib")).unwrap();
    let folded = scratch.0.join("\u{17f}olo");
    if !folded.exists() {
        eprintln!("skipped: a case-sensitive volume, where the spelling names nothing");
        return;
    }
    let mut table = PrefixTable::default();
    table.add(bytes_of(&own), PrefixClass::Destination);
    table.add(bytes_of(&solo), PrefixClass::DetectedOther);
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&folded.join("lib/x")))]),
    );
    assert!(examine(&[file], &[], &table).links_detected_provider);
}

/// Neither spelling nor resolution places it, so nobody examined it, and that is
/// not `NO_LOAD_COMMANDS`.
#[test]
fn a_listed_or_reached_file_outside_every_prefix_is_not_examined_and_says_so() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let nowhere = put(&scratch.0.join("nowhere/file"), b"plain");
    // Listed, through a directory link in the destination.
    symlink(scratch.0.join("nowhere"), own.join("away")).unwrap();
    let facts = examine(&[own.join("away/file")], &[], &table);
    assert!(facts.file_not_examined, "{facts:?}");
    assert_eq!(facts.loadable_files, 0);
    // Reached by a load command, under the destination by its spelling.
    let reaches = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&own.join("away/file")))]),
    );
    assert!(examine(&[reaches], &[], &table).file_not_examined);
    // A real file in no known prefix, however it is spelled: another provider's
    // relocated store looks exactly like this.
    let elsewhere = put(
        &own.join("bin/b"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&nowhere))]),
    );
    assert!(examine(&[elsewhere], &[], &table).file_not_examined);
    // As an `@rpath` candidate too, and the name is answered by it.
    let dir = bytes_of(&scratch.0.join("nowhere")).to_vec();
    let candidate = put(
        &own.join("bin/c"),
        &thin(EXECUTE, &[(RPATH, dir.as_slice()), (LOAD, b"@rpath/file")]),
    );
    let facts = examine(&[candidate], &[], &table);
    assert!(
        facts.file_not_examined && !facts.rpath_unresolved,
        "{facts:?}"
    );
    // A link in the list that leads outside every prefix.
    let link = own.join("share/link");
    std::fs::create_dir_all(own.join("share")).unwrap();
    symlink(&nowhere, &link).unwrap();
    assert!(examine(&[link], &[], &table).file_not_examined);
}

#[test]
fn a_load_command_naming_something_not_a_regular_file_is_not_examined() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    std::fs::create_dir_all(own.join("lib/a-directory")).unwrap();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&own.join("lib/a-directory")))]),
    );
    assert!(examine(&[file], &[], &table).file_not_examined);
}

fn numbered(prefix: &str, count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|n| format!("{prefix}{n}").into_bytes())
        .collect()
}

/// The quadratic file: `LC_RPATH` entries times `@rpath` names. A bound in numbers,
/// since an adapter reads no clock.
#[test]
fn a_quadratic_file_passes_the_reference_bound_and_says_so() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let rpaths = numbered(&format!("{}/r", own.display()), 300);
    let names = numbered("@rpath/n", 300);
    assert!(rpaths.len() * names.len() > MAX_WALK_REFERENCES);
    let mut commands: Vec<(u32, &[u8])> = Vec::new();
    commands.extend(rpaths.iter().map(|r| (RPATH, r.as_slice())));
    commands.extend(names.iter().map(|n| (LOAD, n.as_slice())));
    let file = put(&own.join("bin/a"), &thin(EXECUTE, &commands));
    let (facts, work) = examine_counted(&[file], &[], &table);
    assert!(facts.walk_over_bound, "{facts:?}");
    // One candidate's ancestors can pass it by a little, and the next one stops.
    assert!(work.operations <= MAX_WALK_REFERENCES + 64, "{work:?}");
}

/// A repeated rpath and a repeated name are tried once; a mutant with no dedupe
/// resolves the candidate each time.
#[test]
fn a_repeated_candidate_reaches_the_filesystem_once() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let rpath = bytes_of(&own.join("lib")).to_vec();
    let mut commands: Vec<(u32, &[u8])> = vec![(RPATH, rpath.as_slice()); 40];
    commands.extend(vec![(LOAD, &b"@rpath/same"[..]); 40]);
    let file = put(&own.join("bin/a"), &thin(EXECUTE, &commands));
    let (_, work) = examine_counted(&[file], &[], &table);
    // The file, the rpath directory and the one candidate.
    assert_eq!(work.resolutions, 3, "{work:?}");
}

/// Sixteen entries naming one range: read once. A mutant with no dedupe reads it
/// sixteen times, and the count of operations shows it.
#[test]
fn overlapping_fat_slices_cost_one_read_of_the_range() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let slice = thin(EXECUTE, &[(LOAD, b"/usr/lib/libSystem.B.dylib")]);
    let offset = 4096_u32;
    let mut fat = Vec::new();
    fat.extend_from_slice(&0xcafe_babe_u32.to_be_bytes());
    fat.extend_from_slice(&16_u32.to_be_bytes());
    for _ in 0..16 {
        fat.extend_from_slice(&0x0100_0007_u32.to_be_bytes());
        fat.extend_from_slice(&3_u32.to_be_bytes());
        fat.extend_from_slice(&offset.to_be_bytes());
        fat.extend_from_slice(&(slice.len() as u32).to_be_bytes());
        fat.extend_from_slice(&0_u32.to_be_bytes());
    }
    fat.resize(offset as usize, 0);
    fat.extend_from_slice(&slice);
    let file = put(&own.join("bin/fat"), &fat);
    let (facts, work) = examine_counted(&[file], &[], &table);
    assert_eq!(facts.loadable_files, 1);
    // The listed file, and the one reference.
    assert_eq!(work.resolutions, 2, "{work:?}");
    // The head, then one header and one window of commands: with no dedupe, sixteen.
    let once = HEAD_BYTES as u64 + SLICE_HEADER_BYTES as u64 + slice.len() as u64;
    assert!(work.bytes <= once, "{} > {once}", work.bytes);
}

/// `..` after a directory link is another file than the collapsed text names.
#[test]
fn a_path_whose_parent_component_crosses_a_link_is_not_told_by_its_text() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    put(&own.join("lib/x"), b"x");
    put(&other.join("lib/x"), b"x");
    symlink(other.join("deep"), own.join("door")).unwrap();
    std::fs::create_dir_all(other.join("deep")).unwrap();
    // Collapsed this reads as the destination's own lib; written, it is the other's.
    let written = format!("{}/door/../lib/x", own.display());
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, written.as_bytes())]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.spellings_disagree, "{facts:?}");
    assert!(facts.links_detected_provider, "{facts:?}");
    // Where nothing crosses a link, the two agree.
    let plain = format!("{}/bin/../lib/x", own.display());
    let file = put(
        &own.join("bin/b"),
        &thin(EXECUTE, &[(LOAD, plain.as_bytes())]),
    );
    assert!(!examine(&[file], &[], &table).spellings_disagree);
}

/// A chain: its first hop stays in the destination and its end does not.
#[test]
fn invariant_07_a_link_is_judged_by_its_whole_resolution_and_not_its_first_hop() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    std::fs::create_dir_all(own.join("share")).unwrap();
    let last = other.join("real");
    put(&last, b"x");
    symlink(&last, own.join("share/second")).unwrap();
    symlink(own.join("share/second"), own.join("share/first")).unwrap();
    assert!(examine(&[own.join("share/first")], &[], &table).links_detected_provider);
}

#[test]
fn an_rpath_name_with_candidates_none_there_and_none_in_the_destination_is_unresolved() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let elsewhere = bytes_of(&scratch.0.join("nowhere-at-all")).to_vec();
    let file = put(
        &own.join("bin/a"),
        &thin(
            EXECUTE,
            &[(RPATH, elsewhere.as_slice()), (LOAD, b"@rpath/libx.dylib")],
        ),
    );
    assert!(examine(&[file], &[], &table).rpath_unresolved);
    // Spelled in the destination and not there: nothing says what it loads.
    let inside = bytes_of(&own.join("lib")).to_vec();
    let file = put(
        &own.join("bin/b"),
        &thin(
            EXECUTE,
            &[(RPATH, inside.as_slice()), (LOAD, b"@rpath/libx.dylib")],
        ),
    );
    assert!(examine(&[file], &[], &table).rpath_unresolved);
    // One that exists answers, whichever others do not.
    put(&own.join("lib/libx.dylib"), &thin(DYLIB, &[]));
    let file = put(
        &own.join("bin/c"),
        &thin(
            EXECUTE,
            &[
                (RPATH, elsewhere.as_slice()),
                (RPATH, inside.as_slice()),
                (LOAD, b"@rpath/libx.dylib"),
            ],
        ),
    );
    assert!(!examine(&[file], &[], &table).rpath_unresolved);
}

/// What runs under a name on the executable list is its target, by link, by
/// hardlink, and whichever name was read first.
#[test]
fn an_env_script_is_on_the_list_by_any_name_it_has_there() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let bin = own.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = put(&own.join("libexec/tool.py"), b"#!/usr/bin/env python3\n");
    let dirs = std::slice::from_ref(&bin);

    symlink(&script, bin.join("tool")).unwrap();
    let by_link = examine(&[bin.join("tool")], dirs, &table);
    assert!(by_link.env_shebang_on_list, "{by_link:?}");
    // The target read first, under its own name, then the link.
    let both = examine(&[script.clone(), bin.join("tool")], dirs, &table);
    assert!(both.env_shebang_on_list, "{both:?}");

    std::fs::hard_link(&script, bin.join("hard")).unwrap();
    let hard = examine(&[script.clone(), bin.join("hard")], dirs, &table);
    assert!(hard.env_shebang_on_list, "{hard:?}");
    // By its own name alone it is off the list.
    assert!(!examine(&[script], dirs, &table).env_shebang_on_list);
}

#[test]
fn a_shebang_interpreter_argument_is_a_reference() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let text = format!("#!/usr/bin/env {}/bin/python3\n", other.display());
    let file = put(&own.join("share/a"), text.as_bytes());
    assert!(examine(&[file], &[], &table).links_detected_provider);
}

/// A file of the system's sealed roots.
#[cfg(target_os = "macos")]
const SEALED: [&str; 2] = [
    "/usr/lib/dyld",
    "/System/Library/Frameworks/Foundation.framework/Resources/Info.plist",
];

/// The system's own roots are the one exception to "a real file nobody placed", and
/// only they: nothing here is a provider's.
#[cfg(target_os = "macos")]
#[test]
fn a_real_file_under_a_system_root_is_not_flagged_and_one_beside_it_is() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let cases = SEALED
        .map(|path| (path, false))
        .into_iter()
        .chain([("/usr/bin/true", true)]);
    for (index, (reference, flagged)) in cases.enumerate() {
        assert!(Path::new(reference).exists(), "{reference}");
        let file = put(
            &own.join(format!("bin/{index}")),
            &thin(EXECUTE, &[(LOAD, reference.as_bytes())]),
        );
        let facts = examine(&[file], &[], &table);
        assert_eq!(facts.file_not_examined, flagged, "{reference}: {facts:?}");
    }
}

/// An allow-list of what `man dyld` names, so that a firmlink a later macOS adds opens
/// nothing. `/System` would have covered `/System/Volumes/Data`, the whole writable
/// volume: `canonicalize` keeps that spelling, and dyld honours it.
#[test]
fn only_what_dyld_names_is_the_systems() {
    for path in [
        "/System/Volumes/Data/tmp/shim.dylib",
        "/System/Volumes/Data/usr/lib/shim.dylib",
        "/System/Volumes/Preboot/x",
        "/System/x",
        "/System/Library/x",
        "/System/Library/Caches/x",
        "/System/Library/AssetsV2/locks/x",
        "/System/Library/Templates/Data/x",
        "/System/Library/CoreServices/x",
        "/System/Library/FrameworksX/x",
        "/usr/libexec/x",
        "/usr/local/lib/x",
        "/usr/LIB/x",
    ] {
        assert!(!is_under_system_root(Path::new(path)), "{path}");
    }
    for path in [
        "/usr/lib/libSystem.B.dylib",
        "/usr/lib/swift/libswiftCore.dylib",
        "/System/Library/Frameworks/Foo.framework/Foo",
        "/System/Library/PrivateFrameworks/Foo.framework/Foo",
    ] {
        assert!(is_under_system_root(Path::new(path)), "{path}");
    }
}

/// What is under a root by its text and not by where it really is: `/usr/lib/cron` is
/// a link out of the sealed volume. A fold of a root is the root, because `realpath`
/// returns the stored name (observed 2026-11-04), so it asks about the same place.
#[cfg(target_os = "macos")]
#[test]
fn an_absent_candidate_answers_only_under_a_root_by_its_real_path() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let cases: [(&[u8], bool); 3] = [
        (b"/usr/lib/nothing-here", false),
        (b"/usr/lib/cron", true),
        (b"/USR/LIB/nothing-here", false),
    ];
    for (index, (rpath, unresolved)) in cases.into_iter().enumerate() {
        if rpath == b"/usr/lib/cron" && !Path::new("/usr/lib/cron").is_symlink() {
            eprintln!("skipped: /usr/lib/cron is no link on this system");
            continue;
        }
        let file = put(
            &own.join(format!("bin/{index}")),
            &thin(
                EXECUTE,
                &[(RPATH, rpath), (LOAD, b"@rpath/libnone-such.dylib")],
            ),
        );
        let facts = examine(&[file], &[], &table);
        assert_eq!(
            facts.rpath_unresolved,
            unresolved,
            "{}: {facts:?}",
            String::from_utf8_lossy(rpath)
        );
    }
}

/// Three ways to reach a real file through the firmlink spelling, none of them a
/// provider's prefix by any measure.
#[cfg(target_os = "macos")]
#[test]
fn invariant_07_the_firmlink_spelling_of_a_file_nobody_placed_is_not_the_systems() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    put(&scratch.0.join("nowhere/shim.dylib"), &thin(DYLIB, &[]));
    let data = Path::new("/System/Volumes/Data").join(scratch.0.strip_prefix("/").unwrap());
    assert!(data.exists(), "{}", data.display());
    let dir = data.join("nowhere");
    let shim = dir.join("shim.dylib");
    let load = put(
        &own.join("bin/load"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&shim))]),
    );
    assert!(examine(&[load], &[], &table).file_not_examined);
    let rpath = put(
        &own.join("bin/rpath"),
        &thin(
            EXECUTE,
            &[(RPATH, bytes_of(&dir)), (LOAD, b"@rpath/shim.dylib")],
        ),
    );
    assert!(examine(&[rpath], &[], &table).file_not_examined);
    let link = own.join("share/link");
    std::fs::create_dir_all(own.join("share")).unwrap();
    symlink(&shim, &link).unwrap();
    assert!(examine(&[link], &[], &table).file_not_examined);
}

/// `swiftformat`'s shape: its system libraries are in the shared cache, with no file.
#[test]
fn an_absent_rpath_candidate_under_a_system_root_answers_the_name_and_none_elsewhere_does() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let file = put(
        &own.join("bin/a"),
        &thin(
            EXECUTE,
            &[
                (RPATH, b"/usr/lib/swift"),
                (LOAD, b"@rpath/libnone-such.dylib"),
            ],
        ),
    );
    assert!(!examine(&[file], &[], &table).rpath_unresolved);
    for rpath in [
        &b"/System/Library/Caches"[..],
        b"/System/Volumes/Data/usr/lib",
        b"/usr/local/lib",
    ] {
        let file = put(
            &own.join("bin/b"),
            &thin(
                EXECUTE,
                &[(RPATH, rpath), (LOAD, b"@rpath/libnone-such.dylib")],
            ),
        );
        let facts = examine(&[file], &[], &table);
        assert!(
            facts.rpath_unresolved,
            "{}: {facts:?}",
            String::from_utf8_lossy(rpath)
        );
    }
}

/// An interpreter nobody can place, read by nobody, was once a clean answer.
#[test]
fn a_real_interpreter_outside_every_prefix_is_not_examined() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let interp = put(&scratch.0.join("outside/interp"), b"x");
    let at = interp.display();
    for (name, line) in [
        ("direct", format!("#!{at}\n")),
        ("env", format!("#!/usr/bin/env {at}\n")),
        ("word", format!("#!/bin/sh {at}\n")),
        ("split", format!("#!/usr/bin/env -S{at}\n")),
    ] {
        let file = put(&own.join("share").join(name), line.as_bytes());
        let facts = examine(&[file], &[], &table);
        assert!(facts.file_not_examined, "{name}: {facts:?}");
    }
    #[cfg(target_os = "macos")]
    for line in [
        "#!/bin/sh\n",
        "#!/usr/bin/env python3\n",
        "#!/bin/sh /usr/lib/dyld\n",
    ] {
        let file = put(&own.join("share/fine"), line.as_bytes());
        let facts = examine(&[file], &[], &table);
        assert!(!facts.file_not_examined, "{line}: {facts:?}");
    }
}

/// A search path in the destination is a word, not a program, and says nothing. A
/// regular file of the destination that any word names is read, since it may be what
/// runs: `sh <file>`, `env -i <file>`, `env -S<file>`.
#[test]
fn a_shebang_word_naming_a_regular_file_of_the_destination_is_read_and_a_directory_is_not() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    std::fs::create_dir_all(own.join("bin")).unwrap();
    let text = format!("#!/usr/bin/env -P{}/bin python3\n", own.display());
    let file = put(&own.join("share/a"), text.as_bytes());
    assert!(!examine(&[file], &[], &table).file_not_examined);
    let tainted = put(
        &own.join("lib/tainted"),
        &thin(DYLIB, &[(LOAD, bytes_of(&other.join("lib/x")))]),
    );
    let at = tainted.display();
    for (name, line) in [
        ("sh", format!("#!/bin/sh {at}\n")),
        ("env", format!("#!/usr/bin/env {at}\n")),
        ("flags", format!("#!/usr/bin/env -i A=1 {at}\n")),
        ("clear", format!("#!/usr/bin/env -i {at}\n")),
        ("assign", format!("#!/usr/bin/env FOO=1 {at}\n")),
        ("unset", format!("#!/usr/bin/env -u X {at}\n")),
        ("split", format!("#!/usr/bin/env -S{at}\n")),
        // An argument of the program `env` runs: this reader cannot tell it from the
        // program, so it is read, which is the stricter answer.
        ("argument", format!("#!/usr/bin/env python3 {at}\n")),
    ] {
        let file = put(&own.join("share").join(name), line.as_bytes());
        let facts = examine(&[file], &[], &table);
        assert!(facts.links_detected_provider, "{name}: {facts:?}");
    }
}

/// A word that is no flag and holds no `/` is found by `PATH`, whatever the
/// interpreter (`nohup python3` and `caffeinate -i python3` ran so), and a list of such
/// programs would always be short. Flags alone leave the line determinate.
#[test]
fn a_bare_word_on_any_interpreter_is_found_by_path_and_indeterminate_on_the_list() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let bin = own.join("bin");
    for (name, line) in [
        ("nohup", "#!/usr/bin/nohup python3\n"),
        ("caffeinate", "#!/usr/bin/caffeinate -i python3\n"),
        ("arch", "#!/usr/bin/arch -x86_64 python3\n"),
        ("time", "#!/usr/bin/time -p python3\n"),
    ] {
        let on = put(&bin.join(name), line.as_bytes());
        let facts = examine(&[on], std::slice::from_ref(&bin), &table);
        assert!(facts.env_shebang_on_list, "{name}: {facts:?}");
        let off = put(&own.join("share").join(name), line.as_bytes());
        let facts = examine(&[off], std::slice::from_ref(&bin), &table);
        assert!(!facts.env_shebang_on_list, "{name}: {facts:?}");
    }
    for line in ["#!/bin/sh -e\n", "#!/bin/sh\n", "#!/usr/bin/perl -w -T\n"] {
        let on = put(&bin.join("determinate"), line.as_bytes());
        let facts = examine(&[on], std::slice::from_ref(&bin), &table);
        assert!(!facts.env_shebang_on_list, "{line}: {facts:?}");
    }
    let tainted = put(
        &own.join("lib/tainted"),
        &thin(DYLIB, &[(LOAD, bytes_of(&other.join("lib/x")))]),
    );
    let word = put(
        &own.join("share/word"),
        format!("#!/usr/bin/nohup {}\n", tainted.display()).as_bytes(),
    );
    assert!(examine(&[word], &[], &table).links_detected_provider);
}

/// The kernel keeps a CR in the interpreter's path, and ran the other provider's
/// program through a directory so named. Read as `/x` this was a miss.
#[test]
fn invariant_07_a_carriage_return_is_part_of_the_interpreters_path() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    symlink(&other, scratch.0.join("x\r")).unwrap();
    let mut line = b"#!".to_vec();
    line.extend_from_slice(bytes_of(&scratch.0.join("x\r/prog")));
    line.push(b'\n');
    let file = put(&own.join("share/a"), &line);
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

/// dyld's run path list belongs to the chain of loaders. `libD`'s own rpath is absent
/// under a system root, which alone answers the name, and the executable's serves it
/// from a file that does link the other prefix. Either order of the list.
#[test]
fn invariant_07_an_rpath_name_is_tried_against_every_rpath_in_the_walk() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let lib = own.join("lib");
    put(
        &lib.join("libE.dylib"),
        &thin(DYLIB, &[(LOAD, bytes_of(&other.join("lib/x")))]),
    );
    let library = put(
        &own.join("private/libD.dylib"),
        &thin(
            DYLIB,
            &[(RPATH, b"/usr/lib/swift"), (LOAD, b"@rpath/libE.dylib")],
        ),
    );
    let executable = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(RPATH, bytes_of(&lib))]),
    );
    for list in [
        [executable.clone(), library.clone()],
        [library.clone(), executable.clone()],
    ] {
        let facts = examine(&list, &[], &table);
        assert!(facts.links_detected_provider, "{facts:?}");
    }
    // A rpath met only through a file the walk reaches counts too.
    let reached = put(
        &own.join("private/libR.dylib"),
        &thin(DYLIB, &[(RPATH, bytes_of(&lib))]),
    );
    let first = put(
        &own.join("bin/b"),
        &thin(
            EXECUTE,
            &[(LOAD, b"@rpath/libE.dylib"), (LOAD, bytes_of(&reached))],
        ),
    );
    let facts = examine(&[first], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

/// A real directory nobody can place may serve any file of the chain.
#[test]
fn an_rpath_entry_that_is_a_real_directory_nobody_can_place_is_not_examined() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    std::fs::create_dir_all(scratch.0.join("nowhere")).unwrap();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(RPATH, bytes_of(&scratch.0.join("nowhere")))]),
    );
    assert!(examine(&[file], &[], &table).file_not_examined);
    // One that is not there says nothing by itself.
    let file = put(
        &own.join("bin/b"),
        &thin(EXECUTE, &[(RPATH, bytes_of(&scratch.0.join("absent")))]),
    );
    assert!(!examine(&[file], &[], &table).file_not_examined);
}

/// dyld's default fallback, with another provider at its first location: the project's
/// founding direction. A doubt and never a failure, since which loads cannot be told.
#[test]
fn invariant_07_a_name_that_resolves_to_nothing_is_tried_where_dyld_falls_back() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let lib = other.join("lib");
    let frameworks = other.join("Frameworks");
    put(&lib.join("libz.dylib"), &thin(DYLIB, &[]));
    put(
        &frameworks.join("Foo.framework/Versions/A/Foo"),
        &thin(DYLIB, &[]),
    );
    let places = Fallbacks {
        libraries: vec![lib.clone()],
        frameworks: vec![frameworks.clone()],
    };
    let run = |name: &str, commands: &[(u32, &[u8])]| {
        let file = put(&own.join("bin").join(name), &thin(EXECUTE, commands));
        examine_with(&listed(Path::new("/"), &[file]), &[], &table, &places).0
    };
    for (name, commands) in [
        ("absolute", vec![(LOAD, &b"/nowhere/at/all/libz.dylib"[..])]),
        ("system", vec![(LOAD, &b"/usr/lib/libz.dylib"[..])]),
        (
            "framework",
            vec![(LOAD, &b"/nowhere/Foo.framework/Versions/A/Foo"[..])],
        ),
        (
            "absent-system-rpath",
            vec![
                (RPATH, &b"/usr/lib/swift"[..]),
                (LOAD, &b"@rpath/libz.dylib"[..]),
            ],
        ),
        ("no-rpath", vec![(LOAD, &b"@rpath/libz.dylib"[..])]),
    ] {
        let facts = run(name, &commands);
        assert!(facts.fallback_under_other_prefix, "{name}: {facts:?}");
        assert!(
            !facts.links_detected_provider,
            "a doubt, not a link: {name}: {facts:?}"
        );
    }
    // A name that resolved, a leaf nothing has, and a real candidate: no fallback.
    let dep = put(&own.join("lib/libz.dylib"), &thin(DYLIB, &[]));
    for (name, commands) in [
        ("resolved", vec![(LOAD, bytes_of(&dep))]),
        ("no-leaf", vec![(LOAD, &b"/nowhere/libnothing.dylib"[..])]),
        (
            "real-candidate",
            vec![
                (RPATH, bytes_of(dep.parent().unwrap())),
                (LOAD, &b"@rpath/libz.dylib"[..]),
            ],
        ),
    ] {
        let facts = run(name, &commands);
        assert!(!facts.fallback_under_other_prefix, "{name}: {facts:?}");
    }
}

/// The real locations, with whatever this machine has at `/usr/local/lib`.
#[cfg(target_os = "macos")]
#[test]
fn the_default_fallback_is_what_man_dyld_states_and_it_is_searched() {
    let defaults = Fallbacks::dyld();
    assert_eq!(
        defaults.libraries,
        [Path::new("/usr/local/lib"), Path::new("/usr/lib")]
    );
    assert_eq!(
        defaults.frameworks,
        [
            Path::new("/Library/Frameworks"),
            Path::new("/System/Library/Frameworks")
        ]
    );
    let found = std::fs::read_dir("/usr/local/lib")
        .ok()
        .and_then(|mut entries| {
            entries.find_map(|entry| {
                let name = entry.ok()?.file_name();
                let text = name.to_str()?.to_owned();
                (Path::new(&text).extension().is_some_and(|e| e == "dylib")
                    && Path::new("/usr/local/lib").join(&text).exists())
                .then_some(text)
            })
        });
    let Some(leaf) = found else {
        eprintln!("skipped: nothing named *.dylib in /usr/local/lib here");
        return;
    };
    let scratch = Scratch::new();
    let own = scratch.0.join("own");
    let mut table = PrefixTable::default();
    table.add(bytes_of(&own), PrefixClass::Destination);
    table.add(b"/usr/local", PrefixClass::DetectedOther);
    let name = format!("/nowhere/{leaf}");
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, name.as_bytes())]),
    );
    assert!(examine(&[file], &[], &table).fallback_under_other_prefix);
}

/// A tree deep enough costs a `stat` per level per reference: the bound has to see it.
#[test]
fn each_ancestor_of_a_resolved_path_is_charged_to_the_bound() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let deep = own.join("d/".repeat(400));
    std::fs::create_dir_all(&deep).unwrap();
    // `@rpath`, because a candidate need not exist: only the count can stop this.
    let names = numbered("@rpath/n", 200);
    let mut commands: Vec<(u32, &[u8])> = vec![(RPATH, bytes_of(&deep))];
    commands.extend(names.iter().map(|n| (LOAD, n.as_slice())));
    let file = put(&own.join("bin/a"), &thin(EXECUTE, &commands));
    let (facts, work) = examine_counted(&[file], &[], &table);
    assert!(work.resolutions < 203, "{work:?}");
    assert!(facts.walk_over_bound, "{facts:?} {work:?}");
}

/// 256 KB of path names nothing the system could open, so it costs no `stat` per
/// ancestor and no entry in the cache, however many names use it.
#[test]
fn a_path_over_the_systems_limit_is_unresolved_before_any_filesystem_call() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let long = "/a".repeat(128 * 1024).into_bytes();
    let file = put(&own.join("bin/a"), &thin(EXECUTE, &[(RPATH, &long)]));
    let (facts, work) = examine_counted(&[file], &[], &table);
    assert!(facts.rpath_unresolved, "{facts:?}");
    assert_eq!(work.resolutions, 1, "the file alone: {work:?}");

    let names = numbered("@rpath/n", 200);
    let mut commands: Vec<(u32, &[u8])> = vec![(RPATH, long.as_slice())];
    commands.extend(names.iter().map(|n| (LOAD, n.as_slice())));
    let file = put(&own.join("bin/b"), &thin(EXECUTE, &commands));
    let (facts, work) = examine_counted(&[file], &[], &table);
    assert!(facts.rpath_unresolved, "{facts:?}");
    assert_eq!(work.resolutions, 1, "{work:?}");

    // Joined past the limit, each part short of it.
    let base = "/b".repeat(300).into_bytes();
    let rest = format!("@rpath/{}", "c/".repeat(300)).into_bytes();
    let file = put(
        &own.join("bin/c"),
        &thin(EXECUTE, &[(RPATH, &base), (LOAD, &rest)]),
    );
    let (facts, work) = examine_counted(&[file], &[], &table);
    assert!(facts.rpath_unresolved, "{facts:?}");
    assert_eq!(work.resolutions, 2, "the file and the rpath: {work:?}");

    // As a load command and as an interpreter.
    let file = put(&own.join("bin/d"), &thin(EXECUTE, &[(LOAD, &long[..4096])]));
    assert!(examine(&[file], &[], &table).rpath_unresolved);
}

/// `<token>//x` is `<dir>/x` to dyld. Read as `/x` it missed `..` out of the
/// destination and named the wrong file. One case per token.
#[test]
fn invariant_07_a_token_followed_by_slashes_is_relative_to_its_directory() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    put(&scratch.0.join("other/lib/x"), b"x");
    for (name, command) in [
        ("loader", (LOAD, &b"@loader_path//../../other/lib/x"[..])),
        ("many", (LOAD, &b"@loader_path////../../other/lib/x"[..])),
        (
            "executable",
            (LOAD, &b"@executable_path//../../other/lib/x"[..]),
        ),
        (
            "rpath-entry",
            (RPATH, &b"@loader_path//../../other/lib"[..]),
        ),
    ] {
        let file = put(&own.join("bin").join(name), &thin(EXECUTE, &[command]));
        let facts = examine(&[file], &[], &table);
        assert!(facts.links_detected_provider, "{name}: {facts:?}");
    }
    let file = put(
        &own.join("bin/rpath-name"),
        &thin(
            EXECUTE,
            &[
                (RPATH, b"@loader_path"),
                (LOAD, b"@rpath//../../other/lib/x"),
            ],
        ),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "@rpath: {facts:?}");
    // And it never reaches an absolute path: `/x` under the other prefix is not what
    // `@loader_path//<that>` names.
    let absolute = bytes_of(&scratch.0.join("other/lib/x")).to_vec();
    let mut name = b"@loader_path/".to_vec();
    name.extend_from_slice(&absolute);
    let file = put(
        &own.join("bin/not-absolute"),
        &thin(EXECUTE, &[(LOAD, &name)]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(!facts.links_detected_provider, "{facts:?}");
    // A longer word is not the token, and says nothing.
    let file = put(
        &own.join("bin/other-token"),
        &thin(EXECUTE, &[(LOAD, b"@loader_pathology/x")]),
    );
    assert!(examine(&[file], &[], &table).rpath_unresolved);
}

/// `bin -> libexec/bin`: the list names the file under its real directory.
#[test]
fn an_executable_directory_that_is_a_link_is_compared_by_where_it_leads() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let real = own.join("libexec/bin");
    let script = put(&real.join("tool"), b"#!/usr/bin/env python3\n");
    symlink(&real, own.join("bin")).unwrap();
    let facts = examine(&[script], &[own.join("bin")], &table);
    assert!(facts.env_shebang_on_list, "{facts:?}");
}

#[test]
fn any_path_inside_a_shebang_word_is_a_reference() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let at = other.display();
    for (name, line) in [
        ("split", format!("#!/usr/bin/env -S{at}/bin/python3\n")),
        (
            "path",
            format!("#!/usr/bin/env -P/nowhere:{at}/bin python3\n"),
        ),
        ("assign", format!("#!/usr/bin/env PATH={at}/bin python3\n")),
        ("quoted", format!("#!/usr/bin/env -S\"{at}/bin/python3\"\n")),
    ] {
        let file = put(&own.join("share").join(name), line.as_bytes());
        let facts = examine(&[file], &[], &table);
        assert!(facts.links_detected_provider, "{name}: {facts:?}");
    }
}

/// The file is not there, and the link it goes through is below a prefix, so no
/// ancestor is a prefix by inode. Only the deepest existing ancestor's real path says.
#[test]
fn invariant_07_an_absent_file_through_a_link_below_a_prefix_is_found() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    std::fs::create_dir_all(other.join("lib/deep")).unwrap();
    let outside = scratch.0.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    symlink(other.join("lib/deep"), outside.join("door")).unwrap();
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&outside.join("door/absent/x")))]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

/// What a script runs, when it is the destination's own, is read as a load target.
#[test]
fn invariant_07_an_interpreter_in_the_destination_is_read_and_one_elsewhere_is_not() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let interpreter = put(
        &own.join("libexec/interp"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&other.join("lib/x")))]),
    );
    let text = format!("#!{}\n", interpreter.display());
    let script = put(&own.join("share/script"), text.as_bytes());
    let facts = examine(&[script], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
    assert_eq!(facts.loadable_files, 1);
}

#[test]
fn a_relative_shebang_on_the_list_is_indeterminate() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let bin = own.join("bin");
    let file = put(&bin.join("a"), b"#!python3\n");
    assert!(examine(&[file], std::slice::from_ref(&bin), &table).env_shebang_on_list);
}

/// `lstat` only: a dangling `bin/x` is the placed entry, whatever `sbin/x` is.
#[test]
fn a_dangling_bin_entry_is_the_placed_entry_though_sbin_has_one() {
    let scratch = Scratch::new();
    put(&scratch.0.join("sbin/x"), b"");
    std::fs::create_dir_all(scratch.0.join("bin")).unwrap();
    symlink(scratch.0.join("nowhere"), scratch.0.join("bin/x")).unwrap();
    assert_eq!(
        placed_entry(&scratch.0, "x"),
        Ok(Some(scratch.0.join("bin/x")))
    );
}

#[test]
fn invariant_07_a_listed_entry_through_a_directory_link_is_measured_where_it_is() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    put(&other.join("lib/x"), b"plain");
    symlink(other.join("lib"), own.join("linked")).unwrap();
    assert!(examine(&[own.join("linked/x")], &[], &table).links_detected_provider);
}

/// Sparse files whose `sizeofcmds` asks for a mebibyte each: more than the bound
/// together, and only a bound in bytes stops them.
#[test]
fn passing_the_byte_bound_says_so() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let mut head = thin(EXECUTE, &[]);
    head[20..24].copy_from_slice(&1_048_576u32.to_le_bytes());
    let list: Vec<PathBuf> = (0..260)
        .map(|n| {
            let path = put(&own.join(format!("bin/{n}")), &head);
            std::fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .unwrap()
                .set_len(2 * 1_048_576)
                .unwrap();
            path
        })
        .collect();
    assert!(examine(&list, &[], &table).walk_over_bound);
}

// The second mutation proof (`b70f512`): each closes the survivor it names.

/// A name of exactly `length` bytes under `prefix`.
fn name_of_length(prefix: &Path, length: usize) -> Vec<u8> {
    let mut name = prefix.as_os_str().as_bytes().to_vec();
    name.push(b'/');
    name.resize(length, b'a');
    name
}

/// Closes `b03b` and `b03c`. A name of the limit less one is the longest the system
/// opens, so what it names still counts; a name of the limit is unresolved, before
/// any filesystem call, so no class of it is noted.
#[cfg(target_os = "macos")]
#[test]
fn the_path_limit_is_exact() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let longest = name_of_length(&other, 1023);
    let file = put(&own.join("bin/a"), &thin(EXECUTE, &[(LOAD, &longest)]));
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
    assert!(!facts.rpath_unresolved, "{facts:?}");

    let at_limit = name_of_length(&other, 1024);
    let file = put(&own.join("bin/b"), &thin(EXECUTE, &[(LOAD, &at_limit)]));
    let facts = examine(&[file], &[], &table);
    assert!(facts.rpath_unresolved, "{facts:?}");
    assert!(!facts.links_detected_provider, "{facts:?}");
}

/// Closes `b10c`. The reference is spelled under the other provider's prefix and
/// leads out of it by a link inside it: its canonical path and every inode above it
/// say nothing, and the text it was written in is the only evidence.
#[test]
fn invariant_07_a_reference_spelled_under_a_prefix_counts_though_it_leads_out() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let elsewhere = scratch.0.join("elsewhere");
    put(&elsewhere.join("libz.dylib"), b"x");
    symlink(&elsewhere, other.join("out")).unwrap();
    let target = other.join("out/libz.dylib");
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, target.as_os_str().as_bytes())]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

/// Closes `b12c`. A listed entry's parent is a link to an executable directory, a
/// name the directory's own canonical form does not contain.
#[test]
fn an_env_script_listed_through_a_link_to_an_executable_directory_is_on_the_list() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let bin = own.join("bin");
    put(&bin.join("tool"), b"#!/usr/bin/env python3\n");
    symlink(&bin, own.join("alias")).unwrap();
    let facts = examine(
        &[own.join("alias/tool")],
        std::slice::from_ref(&bin),
        &table,
    );
    assert!(facts.env_shebang_on_list, "{facts:?}");
}

/// A control for `b10b`: the canonical spelling of an existing file under a prefix is
/// found with no help from the written text.
#[test]
fn a_link_outside_every_prefix_into_another_counts_by_where_it_leads() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    put(&other.join("lib/libz.dylib"), b"x");
    let elsewhere = scratch.0.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    symlink(other.join("lib"), elsewhere.join("in")).unwrap();
    let target = elsewhere.join("in/libz.dylib");
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, target.as_os_str().as_bytes())]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

/// Closes `b10b`, in the one shape that tells it apart: the destination's prefix is
/// a link to the other's, so an inode answers for whichever was added first and only
/// the canonical text says it is also the other's.
#[test]
fn a_prefix_that_is_a_link_to_another_still_counts_by_its_canonical_text() {
    let scratch = Scratch::new();
    let real = scratch.0.join("other");
    let own = scratch.0.join("own");
    std::fs::create_dir_all(&real).unwrap();
    symlink(&real, &own).unwrap();
    let mut table = PrefixTable::default();
    table.add(own.as_os_str().as_bytes(), PrefixClass::Destination);
    table.add(real.as_os_str().as_bytes(), PrefixClass::DetectedOther);
    put(&real.join("lib/libz.dylib"), b"x");
    let elsewhere = scratch.0.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    symlink(real.join("lib"), elsewhere.join("in")).unwrap();
    let target = elsewhere.join("in/libz.dylib");
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, target.as_os_str().as_bytes())]),
    );
    let facts = examine(&[file], &[], &table);
    assert!(facts.links_detected_provider, "{facts:?}");
}

// The third mutation proof (`56742e2`).

/// Closes `b4`. A directory in the destination says nothing; one nobody can place may
/// be where the program is found.
#[test]
fn a_real_directory_in_a_word_outside_every_prefix_is_not_examined() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let elsewhere = scratch.0.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let file = put(
        &own.join("share/a"),
        format!("#!/usr/bin/env -P{} python3\n", elsewhere.display()).as_bytes(),
    );
    assert!(examine(&[file], &[], &table).file_not_examined);
}

/// Closes `e2`: an absent path costs a failed `canonicalize` per missing level, and
/// then a `stat` per ancestor.
#[test]
fn an_absent_paths_failed_ancestors_are_charged_too() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let depth = 300;
    let absent = scratch
        .0
        .join("gone")
        .join("a/".repeat(depth))
        .join("libx.dylib");
    let file = put(
        &own.join("bin/a"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&absent))]),
    );
    let (_, work) = examine_counted(&[file], &[], &table);
    assert!(work.operations >= 2 * depth, "{work:?}");
}

// Round 5.

/// `man dyld`'s run path list runs up the chain "that lead to the current dylib load".
/// `libB` has no rpath; a sibling's finds the name. A real binary loaded the other
/// provider's file this way. Either order, since a dedupe must not merge the bit away.
#[test]
fn invariant_07_a_name_found_only_through_a_siblings_rpath_still_tries_the_fallback() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let lib = own.join("lib");
    put(&lib.join("libX.dylib"), &thin(DYLIB, &[]));
    put(&other.join("fb/libX.dylib"), &thin(DYLIB, &[]));
    let places = Fallbacks {
        libraries: vec![other.join("fb")],
        frameworks: vec![other.join("nothing")],
    };
    let sibling = put(
        &own.join("private/libA.dylib"),
        &thin(
            DYLIB,
            &[(RPATH, bytes_of(&lib)), (LOAD, b"@rpath/libX.dylib")],
        ),
    );
    let without = put(
        &own.join("private/libB.dylib"),
        &thin(DYLIB, &[(LOAD, b"@rpath/libX.dylib")]),
    );
    for list in [
        [sibling.clone(), without.clone()],
        [without.clone(), sibling.clone()],
    ] {
        let facts = examine_with(&listed(Path::new("/"), &list), &[], &table, &places).0;
        assert!(facts.fallback_under_other_prefix, "{facts:?}");
        // The union found it, so the name itself is answered.
        assert!(!facts.rpath_unresolved, "{facts:?}");
    }
    // Alone, the file whose own rpath finds it has no doubt.
    let facts = examine_with(
        &listed(Path::new("/"), std::slice::from_ref(&sibling)),
        &[],
        &table,
        &places,
    )
    .0;
    assert!(!facts.fallback_under_other_prefix, "{facts:?}");
}

/// A real binary loaded from an outside directory through this command, by search
/// rules the reader does not model. Whatever the value names, it is a doubt.
#[test]
fn any_dyld_environment_command_leaves_the_check_unable_to_say() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    for value in [
        &b"DYLD_LIBRARY_PATH=/nowhere/at/all"[..],
        b"DYLD_VERSIONED_LIBRARY_PATH=@loader_path",
        b"X",
    ] {
        let file = put(
            &own.join("bin/a"),
            &thin(EXECUTE, &[(DYLD_ENVIRONMENT, value)]),
        );
        let facts = examine(&[file], &[], &table);
        assert!(facts.dyld_environment, "{facts:?}");
    }
    let plain = put(&own.join("bin/b"), &thin(EXECUTE, &[]));
    assert!(!examine(&[plain], &[], &table).dyld_environment);
}

/// A fallback location is measured as `reached` measures: the destination's own file
/// joins the walk, a real file nobody placed was examined by nobody, and either class
/// of another prefix is a doubt.
#[test]
fn a_fallback_location_is_measured_by_its_class() {
    let scratch = Scratch::new();
    let (own, other, table) = scratch.table();
    let run = |dir: PathBuf, name: &str| {
        let file = put(
            &own.join("bin").join(name),
            &thin(EXECUTE, &[(LOAD, b"/nowhere/libF.dylib")]),
        );
        let places = Fallbacks {
            libraries: vec![dir],
            frameworks: vec![own.join("nothing")],
        };
        examine_with(&listed(Path::new("/"), &[file]), &[], &table, &places).0
    };
    let in_destination = own.join("fb");
    put(
        &in_destination.join("libF.dylib"),
        &thin(DYLIB, &[(LOAD, bytes_of(&other.join("lib/x")))]),
    );
    let facts = run(in_destination, "a");
    assert!(facts.links_detected_provider, "{facts:?}");
    assert_eq!(facts.loadable_files, 2, "{facts:?}");

    let nowhere = scratch.0.join("nowhere-placed");
    put(&nowhere.join("libF.dylib"), b"x");
    let facts = run(nowhere, "b");
    assert!(facts.file_not_examined, "{facts:?}");
    assert!(!facts.fallback_under_other_prefix, "{facts:?}");

    let candidate = scratch.0.join("candidate");
    put(&candidate.join("libF.dylib"), b"x");
    let facts = run(candidate, "c");
    assert!(facts.fallback_under_other_prefix, "{facts:?}");
    assert!(!facts.file_not_examined, "{facts:?}");
}
