// SPDX-License-Identifier: Apache-2.0

//! Every place a provider is installed, not only the one this tool runs.
//!
//! Detection takes the first candidate prefix that holds the provider's
//! executable and stops (`homebrew::detect_prefix`, `macports::detect_prefix`).
//! On a Mac migrated from an Intel one that hides a second, working
//! installation under the old prefix - the case `04-migration-and-safety.md`
//! ("Cross-provider contamination") asks `doctor` to report. This module is
//! the search that does not stop, for any provider the data names, so neither
//! direction is a special case (INV-17).
//!
//! **The candidates are the caller's, from `data/policy/provider-prefixes.toml`
//! through [`crate::prefixes`]; there is no path in this file** (INV-20), and
//! nothing a flag or a snapshot can reach adds one.
//!
//! **Filesystem answers follow the machine, not this host** (INV-15, F-25).
//! Where [`FilesystemScope`] says the recording is the machine, this host's disk
//! is not consulted at all and the answer is [`Installations::Unknowable`] - it
//! is never "one" because the single prefix a recording names was the only one
//! looked for. With no candidates there was nothing to search, which is
//! [`Installations::NotRecorded`], also not "one".
//!
//! The test is the same one detection uses - the executable is a regular file,
//! reached through the prefix and never through `PATH` - so a prefix counts here
//! exactly when detection would have chosen it.

use std::path::PathBuf;

use macportsbrew_domain::Installations;

use crate::filesystem::FilesystemScope;

/// Searches every candidate for the provider's executable.
///
/// `executable_relative` is the provider's own constant (`bin/brew`,
/// `bin/port`), joined to each candidate; the result lists the candidates that
/// hold it, in the order given, as text for a snapshot.
pub fn read(
    candidates: &[PathBuf],
    executable_relative: &str,
    filesystem: FilesystemScope,
) -> Installations {
    if candidates.is_empty() {
        return Installations::NotRecorded;
    }
    if !filesystem.is_this_machine() {
        return Installations::Unknowable;
    }
    Installations::Read {
        prefixes: candidates
            .iter()
            .filter(|prefix| prefix.join(executable_relative).is_file())
            .map(|prefix| prefix.to_string_lossy().into_owned())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mpb-installations-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn install(prefix: &std::path::Path) {
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::write(prefix.join("bin/tool"), "").unwrap();
    }

    #[test]
    fn every_candidate_holding_the_executable_is_listed_in_order() {
        let root = scratch("two");
        let (a, b, empty) = (root.join("a"), root.join("b"), root.join("empty"));
        install(&a);
        install(&b);
        fs::create_dir_all(&empty).unwrap();
        let found = read(
            &[a.clone(), empty, b.clone()],
            "bin/tool",
            FilesystemScope::ThisMachine,
        );
        assert_eq!(
            found,
            Installations::Read {
                prefixes: vec![
                    a.to_string_lossy().into_owned(),
                    b.to_string_lossy().into_owned()
                ]
            }
        );
        assert_eq!(found.count(), Some(2));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn one_installation_is_one_and_none_is_none() {
        let root = scratch("one");
        let (a, b) = (root.join("a"), root.join("b"));
        install(&a);
        let one = read(&[a, b.clone()], "bin/tool", FilesystemScope::ThisMachine);
        assert_eq!(one.count(), Some(1));
        let none = read(&[b], "bin/tool", FilesystemScope::ThisMachine);
        assert_eq!(none.count(), Some(0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_where_the_executable_should_be_is_not_an_installation() {
        let root = scratch("dir");
        fs::create_dir_all(root.join("bin/tool")).unwrap();
        let found = read(
            std::slice::from_ref(&root),
            "bin/tool",
            FilesystemScope::ThisMachine,
        );
        assert_eq!(found.count(), Some(0));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_recording_never_answers_from_this_hosts_disk() {
        // The prefix really holds the executable here; under `Elsewhere` the
        // disk is not the machine, so the answer is unknown, never "one".
        let root = scratch("elsewhere");
        install(&root);
        let found = read(
            std::slice::from_ref(&root),
            "bin/tool",
            FilesystemScope::Elsewhere,
        );
        assert_eq!(found, Installations::Unknowable);
        assert_eq!(found.count(), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_to_search_is_not_recorded_rather_than_none() {
        assert_eq!(
            read(&[], "bin/tool", FilesystemScope::ThisMachine),
            Installations::NotRecorded
        );
    }
}
