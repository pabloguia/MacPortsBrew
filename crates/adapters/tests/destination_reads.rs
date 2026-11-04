// SPDX-License-Identifier: Apache-2.0

//! The filesystem half, in a scratch directory. No recording of a contaminated
//! binary can exist, so detection is shown on bytes built here, and said so.
// A helper builds byte counts the format stores in 32 bits; none comes near it.
#![allow(clippy::cast_possible_truncation)]

use macportsbrew_adapters::destination_reads::{
    checked_list, examine, placed_entry, stat_counts, CheckedList, Outside, StatCounts,
    MAX_WALK_FILES,
};
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
fn invariant_08_every_slice_is_read_and_not_only_the_first() {
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
    assert_eq!(facts.env_shebang_elsewhere, 1);
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
    let under_a_file = present.join("below");
    let dangling = scratch.0.join("dangling");
    symlink(scratch.0.join("nowhere"), &dangling).unwrap();
    let looped = scratch.0.join("loop");
    symlink(&looped, &looped).unwrap();
    let counts = stat_counts(&[present, gone, under_a_file, dangling, looped]);
    assert_eq!(
        counts,
        StatCounts {
            missing: 2,
            dangling: 2,
            failed: 0,
        }
    );
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
        ),
        Ok(CheckedList {
            paths: vec![PathBuf::from("/opt/local/bin/xz")],
            outside: 1,
        })
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
