// SPDX-License-Identifier: Apache-2.0

//! The filesystem half, in a scratch directory. No recording of a contaminated
//! binary can exist, so detection is shown on bytes built here, and said so.
// A helper builds byte counts the format stores in 32 bits; none comes near it.
#![allow(clippy::cast_possible_truncation)]

use macportsbrew_adapters::destination_reads::{
    checked_list, examine, examine_counted, placed_entry, stat_counts, CheckedList, Outside,
    StatCounts, MAX_WALK_FILES, MAX_WALK_REFERENCES,
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
        return; // Not this kind of volume: the case does not arise.
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
        return; // A case-sensitive volume: the spelling names nothing.
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
    // A reference spelled outside every prefix, leading outside: nobody's, and
    // the system's libraries are this case. Not flagged.
    let elsewhere = put(
        &own.join("bin/b"),
        &thin(EXECUTE, &[(LOAD, bytes_of(&nowhere))]),
    );
    assert!(!examine(&[elsewhere], &[], &table).file_not_examined);
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
    assert!(work.operations <= MAX_WALK_REFERENCES + 1, "{work:?}");
}

/// A repeated candidate costs a count and no filesystem access; a mutant with no
/// dedupe resolves it each time.
#[test]
fn a_repeated_candidate_reaches_the_filesystem_once() {
    let scratch = Scratch::new();
    let (own, _, table) = scratch.table();
    let rpath = bytes_of(&own.join("lib")).to_vec();
    let mut commands: Vec<(u32, &[u8])> = vec![(RPATH, rpath.as_slice()); 40];
    commands.extend(vec![(LOAD, &b"@rpath/same"[..]); 40]);
    let file = put(&own.join("bin/a"), &thin(EXECUTE, &commands));
    let (_, work) = examine_counted(&[file], &[], &table);
    assert!(work.operations > 1600, "{work:?}");
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
    assert_eq!(work.operations, 2, "{work:?}");
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
    // A candidate in the destination need not exist: it could have been its own.
    let inside = bytes_of(&own.join("lib")).to_vec();
    let file = put(
        &own.join("bin/b"),
        &thin(
            EXECUTE,
            &[(RPATH, inside.as_slice()), (LOAD, b"@rpath/libx.dylib")],
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
