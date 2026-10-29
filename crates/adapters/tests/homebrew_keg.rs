// SPDX-License-Identifier: Apache-2.0

//! The Homebrew keg reader (`homebrew::keg`), on scratch Cellars. These build
//! symbolic links to prove the reader refuses them, so they live here rather
//! than in `src/`, which the stop gate scans and no symlink call may appear in
//! (work order 16, part D).

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::homebrew::keg::{materialised, MAX_KEG_ENTRIES};
use macportsbrew_domain::Answer;
use std::path::PathBuf;

struct Cellar(PathBuf);

impl Cellar {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir()
            .join(format!("mpb-keg-{tag}-{}", std::process::id()))
            .join("Cellar");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn keg(&self, name: &str, version: &str) -> PathBuf {
        let keg = self.0.join(name).join(version);
        std::fs::create_dir_all(&keg).unwrap();
        std::fs::write(keg.join("INSTALL_RECEIPT.json"), "{}").unwrap();
        keg
    }
    fn answer(&self, name: &str, versions: &[&str]) -> Answer {
        materialised(&self.0, name, versions, FilesystemScope::ThisMachine)
    }
}

impl Drop for Cellar {
    fn drop(&mut self) {
        if let Some(parent) = self.0.parent() {
            std::fs::remove_dir_all(parent).ok();
        }
    }
}

#[test]
fn every_name_homebrew_writes_and_an_old_custom_label_are_yes() {
    let cellar = Cellar::new("names");
    for (i, file) in [
        "sh.brew.svc.plist",
        "homebrew.mxcl.svc.plist",
        "com.example.custom-label.plist",
        "homebrew.svc.service",
        "Something.PLIST",
    ]
    .iter()
    .enumerate()
    {
        let version = format!("1.{i}");
        let keg = cellar.keg("svc", &version);
        std::fs::write(keg.join(file), "").unwrap();
        assert_eq!(cellar.answer("svc", &[&version]), Answer::Yes, "{file}");
    }
}

#[test]
fn a_keg_with_no_launch_file_is_no() {
    let cellar = Cellar::new("plain");
    let keg = cellar.keg("jq", "1.8");
    std::fs::create_dir_all(keg.join("bin")).unwrap();
    assert_eq!(cellar.answer("jq", &["1.8"]), Answer::No);
}

#[test]
fn a_symlinked_plist_is_unknown() {
    let cellar = Cellar::new("symlink");
    let keg = cellar.keg("svc", "1.0");
    std::os::unix::fs::symlink("/etc/hosts", keg.join("sh.brew.svc.plist")).unwrap();
    assert_eq!(cellar.answer("svc", &["1.0"]), Answer::Unknown);
}

#[test]
fn an_absent_or_unreadable_keg_is_unknown_and_not_no() {
    use std::os::unix::fs::PermissionsExt;
    let cellar = Cellar::new("absent");
    cellar.keg("svc", "1.0");
    assert_eq!(
        cellar.answer("svc", &["2.0"]),
        Answer::Unknown,
        "absent keg"
    );
    assert_eq!(
        cellar.answer("other", &["1.0"]),
        Answer::Unknown,
        "absent formula"
    );
    let keg = cellar.keg("locked", "1.0");
    std::fs::set_permissions(&keg, std::fs::Permissions::from_mode(0o000)).unwrap();
    let answer = cellar.answer("locked", &["1.0"]);
    std::fs::set_permissions(&keg, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(answer, Answer::Unknown, "unreadable keg");
}

#[test]
fn a_hostile_cellar_name_or_version_is_unknown() {
    let cellar = Cellar::new("hostile");
    let keg = cellar.keg("svc", "1.0");
    std::fs::write(keg.join("sh.brew.svc.plist"), "").unwrap();
    for (root, name, version) in [
        (PathBuf::from("relative/Cellar"), "svc", "1.0"),
        (cellar.0.join("..").join("Cellar"), "svc", "1.0"),
        (cellar.0.clone(), "../Cellar/svc", "1.0"),
        (cellar.0.clone(), "svc", "../svc/1.0"),
        (cellar.0.clone(), "svc", ".."),
        (cellar.0.clone(), "svc", "-1.0"),
        (cellar.0.clone(), "homebrew/core/svc", "1.0"),
    ] {
        assert_eq!(
            materialised(&root, name, &[version], FilesystemScope::ThisMachine),
            Answer::Unknown,
            "{root:?} {name} {version}"
        );
    }
    assert_eq!(cellar.answer("svc", &[]), Answer::Unknown, "no version");
}

#[test]
fn a_cellar_that_is_a_symlink_is_unknown() {
    let cellar = Cellar::new("linked");
    cellar.keg("svc", "1.0");
    let link = cellar.0.with_file_name("LinkedCellar");
    std::os::unix::fs::symlink(&cellar.0, &link).unwrap();
    assert_eq!(
        materialised(&link, "svc", &["1.0"], FilesystemScope::ThisMachine),
        Answer::Unknown
    );
}

#[test]
fn more_entries_than_a_keg_has_is_unknown_not_truncated() {
    let cellar = Cellar::new("crowded");
    let keg = cellar.keg("svc", "1.0");
    for n in 0..MAX_KEG_ENTRIES {
        std::fs::write(keg.join(format!("f{n}")), "").unwrap();
    }
    assert_eq!(cellar.answer("svc", &["1.0"]), Answer::Unknown);
}

#[test]
fn a_launch_file_does_not_outrun_an_overflowing_listing() {
    // Whatever order the directory hands entries back in, a listing over
    // the bound is UNKNOWN even when it also holds a regular plist.
    let cellar = Cellar::new("crowded-plist");
    let keg = cellar.keg("svc", "1.0");
    std::fs::write(keg.join("homebrew.mxcl.svc.plist"), "").unwrap();
    for n in 0..MAX_KEG_ENTRIES {
        std::fs::write(keg.join(format!("f{n}")), "").unwrap();
    }
    assert_eq!(cellar.answer("svc", &["1.0"]), Answer::Unknown);
}

#[test]
fn versions_combine_yes_first_then_unknown() {
    let cellar = Cellar::new("versions");
    cellar.keg("svc", "1.0");
    let new = cellar.keg("svc", "2.0");
    assert_eq!(cellar.answer("svc", &["1.0", "2.0"]), Answer::No);
    assert_eq!(cellar.answer("svc", &["1.0", "3.0"]), Answer::Unknown);
    std::fs::write(new.join("homebrew.mxcl.svc.plist"), "").unwrap();
    assert_eq!(cellar.answer("svc", &["1.0", "3.0", "2.0"]), Answer::Yes);
}

#[test]
fn invariant_15_a_replayed_reader_never_consults_the_keg() {
    let cellar = Cellar::new("elsewhere");
    let keg = cellar.keg("svc", "1.0");
    std::fs::write(keg.join("sh.brew.svc.plist"), "").unwrap();
    assert_eq!(cellar.answer("svc", &["1.0"]), Answer::Yes);
    assert_eq!(
        materialised(&cellar.0, "svc", &["1.0"], FilesystemScope::Elsewhere),
        Answer::Unknown
    );
}
