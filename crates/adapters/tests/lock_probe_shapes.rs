// SPDX-License-Identifier: Apache-2.0

//! The lock probe against entries that are links (G item 1, mutant 15).
//!
//! Here rather than in `lock_probe/mod.rs`, because making a link is a token the
//! stop gate refuses in `src/`, tests included (the same reason part D moved its
//! readers' tests). The module is compiled again for this file, so its
//! crate-private seam is reachable and no public constructor takes a path (G item
//! 13). Only ever scratch directories this test made.

#[allow(dead_code, reason = "this file reaches only part of the module")]
#[path = "../src/lock_probe/mod.rs"]
mod lock_probe;

use lock_probe::{absence_is_certain, sample_site, Answer, LockSite};
use macportsbrew_domain::UndeterminedCause;
use std::ffi::OsString;
use std::fs::{self, File};
use std::os::unix::fs::symlink as link_to;
use std::path::{Path, PathBuf};

const SHAPE: Answer = Answer::NotDetermined(UndeterminedCause::UnexpectedShape);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mpb-lock-shapes-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn directory(dir: &Path) -> Answer {
    sample_site(&LockSite::Directory(dir.to_path_buf()), &|_| {})
}

fn named(dir: &Path, name: &str) -> Answer {
    let site = LockSite::File {
        dir: dir.to_path_buf(),
        name: OsString::from(name),
    };
    sample_site(&site, &|_| {})
}

#[test]
fn a_link_entry_is_not_determined_and_never_followed() {
    let root = scratch("entry");
    let target = scratch("entry-target");
    let holder = File::create(target.join("elsewhere")).expect("a scratch file");
    holder.lock().expect("locks");
    link_to(target.join("elsewhere"), root.join("x.lock")).expect("a link");
    assert_eq!(directory(&root), SHAPE);
    fs::remove_dir_all(&root).expect("cleanup");
    fs::remove_dir_all(&target).expect("cleanup");
}

#[test]
fn a_lock_directory_that_is_a_link_is_not_determined() {
    // Mutant 15: the lock directory replaced by a link.
    let real = scratch("dir-real");
    let root = scratch("dir");
    let holder = File::create(real.join("x.lock")).expect("a scratch file");
    holder.lock().expect("locks");
    link_to(&real, root.join("locks")).expect("a link");
    assert_eq!(directory(&root.join("locks")), SHAPE);
    assert_eq!(named(&root.join("locks"), "x.lock"), SHAPE);
    fs::remove_dir_all(&root).expect("cleanup");
    fs::remove_dir_all(&real).expect("cleanup");
}

#[test]
fn a_named_lock_file_that_is_a_link_or_a_directory_is_not_determined() {
    let root = scratch("named");
    let target = scratch("named-target");
    let holder = File::create(target.join("real")).expect("a scratch file");
    holder.lock().expect("locks");
    link_to(target.join("real"), root.join("to.lock")).expect("a link");
    fs::create_dir(root.join("dir.lock")).expect("a directory");
    for name in ["to.lock", "dir.lock"] {
        assert_eq!(named(&root, name), SHAPE, "{name}");
    }
    fs::remove_dir_all(&root).expect("cleanup");
    fs::remove_dir_all(&target).expect("cleanup");
}

#[test]
fn a_dangling_or_looping_executable_is_not_a_certain_absence() {
    // Each answers "not a file" to detection and is still not absent.
    let root = scratch("absence");
    for name in ["dangling/bin", "looping/bin"] {
        fs::create_dir_all(root.join(name)).expect("a scratch prefix");
    }
    link_to("/nonexistent/target", root.join("dangling/bin/tool")).expect("a dangling link");
    link_to("tool", root.join("looping/bin/tool")).expect("a loop");
    for broken in ["dangling", "looping"] {
        assert!(
            !absence_is_certain(&[root.join("empty"), root.join(broken)], "bin/tool"),
            "{broken}"
        );
    }
    assert!(absence_is_certain(&[root.join("empty")], "bin/tool"));
    fs::remove_dir_all(&root).expect("cleanup");
}
