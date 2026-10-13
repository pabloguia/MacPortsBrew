// SPDX-License-Identifier: Apache-2.0

//! `read_regular_file_for_replacement`: what `snapshot --out` and
//! `doctor --report` read to decide whether an existing file may be replaced.
//! A symlink, a directory and an oversized file are each "no" (security
//! re-review, SE-094). The non-blocking open that keeps a FIFO from stalling
//! the read is held by `bounded_read`'s own `open_flags` test: making a FIFO
//! here would mean spawning a program, which tests do not do.

use macportsbrew_adapters::persistence::read_regular_file_for_replacement as read;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mpb-replacement-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

#[test]
fn a_regular_file_within_the_cap_is_read() {
    let dir = scratch("ok");
    let file = dir.join("doc.json");
    std::fs::write(&file, "{}").expect("write");
    assert_eq!(read(&file, 16).as_deref(), Some("{}"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_symlink_at_the_end_is_refused_even_to_a_readable_file() {
    let dir = scratch("link");
    let file = dir.join("doc.json");
    std::fs::write(&file, "{}").expect("write");
    let link = dir.join("link.json");
    std::os::unix::fs::symlink(&file, &link).expect("symlink");
    assert_eq!(read(&link, 16), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_over_the_cap_a_directory_and_nothing_are_refused() {
    let dir = scratch("cap");
    let file = dir.join("big.json");
    std::fs::write(&file, "0123456789").expect("write");
    assert_eq!(read(&file, 9), None);
    assert_eq!(read(&dir, 1024), None);
    assert_eq!(read(&dir.join("absent"), 1024), None);
    let _ = std::fs::remove_dir_all(&dir);
}
