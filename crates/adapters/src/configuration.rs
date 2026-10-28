// SPDX-License-Identifier: Apache-2.0

//! A provider's own configuration files, and the keys they set.
//!
//! R-15 (Homebrew) and R-16 (MacPorts) found that each provider reads
//! configuration that applies over the environment this tool invokes it with,
//! and that no variable an invocation sets can switch it off. DP-027 ruled the
//! answer: detect it, report it on reads, refuse a mutating step against it.
//! This module is the detection, for any provider the data names, so neither
//! direction is a special case (INV-17).
//!
//! **Where the files are, and how a line names a key, are dated data**
//! (`data/policy/provider-configuration.toml`, INV-20), read here for the same
//! layering reason [`crate::prefixes`] reads its own file.
//!
//! **Every key the provider's reader would accept is recorded, by name.** The
//! first version kept only a reviewed list, and review showed what a denylist
//! does: it let `SUDO_ASKPASS`, `HOMEBREW_XDG_CONFIG_HOME` - which moves the
//! user file somewhere this reader never looks - and the rest of R-15's own
//! minimum list through as "nothing found" (security-reviewer and
//! spec-guardian, 2026-10-04). A key nobody reviewed is still a key the
//! provider applies over this tool's environment, so it counts. A name that is
//! not one a snapshot may carry makes the file unreadable rather than dropped.
//! **Values are never kept**: one can carry a credential (a proxy URL with a
//! password in it), and nothing this tool decides needs one.
//!
//! **What is read, and how.** Each file is stat'd, refused unless it is a
//! regular file, and opened once through
//! [`crate::persistence::bounded_read::read_bounded`] - non-blocking, a regular
//! file again on the handle, bounded. A symlink at the file is followed,
//! because the provider follows it: a dotfile manager's link is the user's
//! file. Lines are split the way the provider's own reader splits them.
//! Nothing is written, created or executed, and an absent file is simply not
//! listed.
//!
//! **Under replay the answer is [`ProviderConfiguration::Unknowable`].** A
//! recording carries no files, and reading this host's would describe the
//! replaying machine (INV-15) - the same rule cask receipts follow.

use crate::filesystem::FilesystemScope;
use macportsbrew_domain::{
    is_safe_configuration_key, ConfigurationFile, ConfigurationScope, ProviderConfiguration,
};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SOURCE: &str = include_str!("../../../data/policy/provider-configuration.toml");

/// The bytes of one configuration file this reader will look at. A `brew.env`
/// or a `macports.conf` is a few kilobytes; the default MacPorts system file,
/// comments included, is under 10 KiB.
const MAX_CONFIGURATION_BYTES: u64 = 256 * 1024;

#[derive(Debug, Deserialize)]
struct Document {
    entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    provider: String,
    syntax: Syntax,
    files: Vec<FileSpec>,
    /// The keys that decide whether the provider sends its own analytics
    /// (DP-028): a file that sets one is disclosed as such by `doctor`.
    #[serde(default)]
    analytics_keys: BTreeSet<String>,
    #[allow(dead_code, reason = "provenance is the point of the file; INV-20")]
    source: String,
    #[allow(dead_code, reason = "provenance is the point of the file; INV-20")]
    read_on: String,
}

#[derive(Debug, Deserialize)]
struct FileSpec {
    path: String,
    scope: ConfigurationScope,
}

/// How a line names a key, mirroring each provider's own reader.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Syntax {
    /// Homebrew's `export_homebrew_env_file`: `read -r` lines (split on `\n`
    /// only, surrounding whitespace trimmed) starting `HOMEBREW_`,
    /// `SUDO_ASKPASS=` or a proxy variable, then `export "$line"` - so the key
    /// is what precedes `=`, and `KEY+=value` sets `KEY` too (bash appends to
    /// an unset variable, which is every variable this tool does not set).
    EnvAssignment,
    /// MacPorts' configuration reader, `^(\w+)([ \t]+(.*))?$` over Tcl `gets`
    /// lines, which end at `\n`, `\r\n` *or* a lone `\r`: the key is the
    /// leading word.
    WordValue,
}

impl Syntax {
    /// The lines of a file as the provider's reader sees them.
    fn lines(self, text: &str) -> Vec<&str> {
        match self {
            Self::EnvAssignment => text.split('\n').collect(),
            // Splitting on both is a superset of `\r\n`, `\r` and `\n`; the
            // empty piece between `\r` and `\n` names no key.
            Self::WordValue => text.split(['\r', '\n']).collect(),
        }
    }

    /// The key a line sets, if it sets one.
    fn key(self, line: &str) -> Option<&str> {
        match self {
            Self::EnvAssignment => {
                let line = line.trim();
                let homebrew = line.starts_with("HOMEBREW_");
                let named = [
                    "SUDO_ASKPASS=",
                    "all_proxy=",
                    "no_proxy=",
                    "ftp_proxy=",
                    "http_proxy=",
                    "https_proxy=",
                ]
                .iter()
                .any(|p| line.starts_with(p));
                if !(homebrew || named) {
                    return None;
                }
                let (key, _) = line.split_once('=')?;
                Some(key.strip_suffix('+').unwrap_or(key))
            }
            Self::WordValue => {
                let end = line
                    .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .unwrap_or(line.len());
                let (key, rest) = line.split_at(end);
                let separated = rest.is_empty() || rest.starts_with([' ', '\t']);
                (!key.is_empty() && separated).then_some(key)
            }
        }
    }
}

/// Reads `provider`'s configuration files as this tool invokes it: `prefix` is
/// the detected installation, `home` the `HOME` the invocation carries.
///
/// A provider the data does not describe is [`ProviderConfiguration::NotRecorded`]:
/// nothing was looked for, which is not "nothing found".
pub fn read(
    provider: &str,
    prefix: &Path,
    home: Option<&Path>,
    filesystem: FilesystemScope,
) -> ProviderConfiguration {
    read_under(provider, prefix, home, filesystem, Path::new("/"))
}

/// [`read`], with every absolute path in the data taken under `root` - `/` in
/// the program, a scratch directory in a test, so no test reads this host's
/// `/etc`.
fn read_under(
    provider: &str,
    prefix: &Path,
    home: Option<&Path>,
    filesystem: FilesystemScope,
    root: &Path,
) -> ProviderConfiguration {
    if !filesystem.is_this_machine() {
        return ProviderConfiguration::Unknowable;
    }
    let Some(entry) = document().entry.iter().find(|e| e.provider == provider) else {
        return ProviderConfiguration::NotRecorded;
    };
    let mut files = Vec::new();
    for spec in &entry.files {
        let (label, located) = locate(&spec.path, prefix, home, root);
        let Some(path) = located else {
            // A user file with no home to find it under: whether it is there
            // cannot be said, so it is listed as unreadable rather than
            // dropped as absent.
            files.push(ConfigurationFile {
                path: label,
                scope: spec.scope,
                readable: false,
                keys: BTreeSet::new(),
                analytics_keys: BTreeSet::new(),
            });
            continue;
        };
        if let Some(mut file) = read_one(&path, label, spec.scope, entry.syntax) {
            file.analytics_keys = file
                .keys
                .intersection(&entry.analytics_keys)
                .cloned()
                .collect();
            files.push(file);
        }
    }
    ProviderConfiguration::Read { files }
}

/// The label a file is recorded under, and the path to open (`None` when a
/// `~/` path has no home to resolve against).
fn locate(
    template: &str,
    prefix: &Path,
    home: Option<&Path>,
    root: &Path,
) -> (String, Option<PathBuf>) {
    if let Some(rest) = template.strip_prefix("{prefix}/") {
        let path = prefix.join(rest);
        (path.display().to_string(), Some(path))
    } else if let Some(rest) = template.strip_prefix("~/") {
        (template.to_string(), home.map(|h| h.join(rest)))
    } else {
        let relative = template.trim_start_matches('/');
        (template.to_string(), Some(root.join(relative)))
    }
}

/// One file: `None` when it does not exist, otherwise what was found.
fn read_one(
    path: &Path,
    label: String,
    scope: ConfigurationScope,
    syntax: Syntax,
) -> Option<ConfigurationFile> {
    let unreadable = |label: String| ConfigurationFile {
        path: label,
        scope,
        readable: false,
        keys: BTreeSet::new(),
        analytics_keys: BTreeSet::new(),
    };
    match std::fs::metadata(path) {
        // Not a regular file - a directory, a device a link points at - is
        // refused before anything opens it: some devices act on open.
        Ok(meta) if !meta.is_file() => return Some(unreadable(label)),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        // It may be there; whether it is cannot be told, which is not "absent".
        Err(_) => return Some(unreadable(label)),
    }
    let Ok(text) = crate::persistence::bounded_read::read_bounded(
        path,
        MAX_CONFIGURATION_BYTES,
        crate::persistence::bounded_read::OpenPolicy::FollowAsNamed,
        |_| Ok(()),
    ) else {
        return Some(unreadable(label));
    };
    let mut keys = BTreeSet::new();
    for key in syntax
        .lines(&text)
        .into_iter()
        .filter_map(|l| syntax.key(l))
    {
        if !is_safe_configuration_key(key) {
            // The provider applies a key this record cannot name: an unknown,
            // never a "nothing found".
            return Some(unreadable(label));
        }
        keys.insert(key.to_string());
    }
    Some(ConfigurationFile {
        path: label,
        scope,
        readable: true,
        keys,
        analytics_keys: BTreeSet::new(),
    })
}

/// Parses the embedded document; a malformed file is this repository's own
/// mistake and fails at the first call, as [`crate::prefixes`] does.
fn document() -> &'static Document {
    use std::sync::OnceLock;
    static PARSED: OnceLock<Document> = OnceLock::new();
    PARSED.get_or_init(|| {
        toml::from_str(SOURCE)
            .unwrap_or_else(|e| panic!("data/policy/provider-configuration.toml is malformed: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch tree with `root` (standing in for `/`), `home` and `prefix`.
    struct Tree {
        base: PathBuf,
    }

    impl Tree {
        fn new(label: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "mpb-configuration-{label}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&base);
            for d in ["root", "home", "prefix"] {
                std::fs::create_dir_all(base.join(d)).expect("scratch");
            }
            Self { base }
        }
        fn path(&self, d: &str) -> PathBuf {
            self.base.join(d)
        }
        fn write(&self, relative: &str, text: &str) {
            let p = self.base.join(relative);
            std::fs::create_dir_all(p.parent().expect("a parent")).expect("dirs");
            std::fs::write(p, text).expect("write");
        }
        fn read(&self, provider: &str) -> ProviderConfiguration {
            read_under(
                provider,
                &self.path("prefix"),
                Some(&self.path("home")),
                FilesystemScope::ThisMachine,
                &self.path("root"),
            )
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn files(found: &ProviderConfiguration) -> &[ConfigurationFile] {
        match found {
            ProviderConfiguration::Read { files } => files,
            other => panic!("{other:?}"),
        }
    }

    fn keys(file: &ConfigurationFile) -> Vec<&str> {
        file.keys.iter().map(String::as_str).collect()
    }

    #[test]
    fn every_entry_carries_its_provenance_and_rooted_paths() {
        let doc = document();
        for e in &doc.entry {
            assert!(!e.source.trim().is_empty(), "{}: no source", e.provider);
            assert!(
                e.read_on.len() == 10 && e.read_on.chars().filter(|c| *c == '-').count() == 2,
                "{}: read_on is not YYYY-MM-DD",
                e.provider
            );
            assert!(!e.files.is_empty(), "{}", e.provider);
            for f in &e.files {
                let rooted = f.path.starts_with('/')
                    || f.path.starts_with("{prefix}/")
                    || f.path.starts_with("~/");
                assert!(
                    rooted && !f.path.contains(".."),
                    "{}: {}",
                    e.provider,
                    f.path
                );
            }
        }
    }

    #[test]
    fn every_provider_the_registry_knows_is_described() {
        // A provider with no entry is `NOT_RECORDED`, an unknown; the recipe
        // that adds one owes its entry, and this is where it is held to it.
        for id in crate::registry::known_provider_ids() {
            assert!(
                document().entry.iter().any(|e| e.provider == id.as_str()),
                "{id} has no entry in data/policy/provider-configuration.toml"
            );
        }
        assert_eq!(
            read(
                "pkgsrc",
                Path::new("/x"),
                None,
                FilesystemScope::ThisMachine
            ),
            ProviderConfiguration::NotRecorded
        );
    }

    #[test]
    fn a_replayed_capture_reads_nothing_and_says_it_cannot_know() {
        let tree = Tree::new("replay");
        tree.write("home/.homebrew/brew.env", "HOMEBREW_NO_ANALYTICS=\n");
        assert_eq!(
            read_under(
                "homebrew",
                &tree.path("prefix"),
                Some(&tree.path("home")),
                FilesystemScope::Elsewhere,
                &tree.path("root"),
            ),
            ProviderConfiguration::Unknowable
        );
    }

    #[test]
    fn every_key_homebrew_would_export_is_recorded_by_name_and_no_value_is_kept() {
        let tree = Tree::new("brew");
        tree.write(
            "home/.homebrew/brew.env",
            "# a comment\n  HOMEBREW_NO_ANALYTICS=\nHOMEBREW_GITHUB_API_TOKEN=secret\n\
             https_proxy=http://user:pw@proxy\nNOT_HOMEBREW=1\nSUDO_ASKPASS=/x/askpass\n\
             HOMEBREW_API_DOMAIN+=https://elsewhere\nHOMEBREW_BARE\n",
        );
        let found = tree.read("homebrew");
        let files = files(&found);
        assert_eq!(files.len(), 1, "{files:?}");
        assert_eq!(
            files[0].path, "~/.homebrew/brew.env",
            "the home is never named"
        );
        assert_eq!(files[0].scope, ConfigurationScope::User);
        assert!(files[0].readable);
        assert_eq!(
            keys(&files[0]),
            [
                "HOMEBREW_API_DOMAIN",
                "HOMEBREW_GITHUB_API_TOKEN",
                "HOMEBREW_NO_ANALYTICS",
                "SUDO_ASKPASS",
                "https_proxy"
            ],
            "`+=` sets the key; a line with no `=` sets nothing"
        );
        let shown = format!("{found:?}");
        assert!(
            !shown.contains("secret") && !shown.contains("pw@") && !shown.contains("askpass"),
            "a value was kept: {shown}"
        );
        assert_eq!(found.outranks(), Some(true));
        assert_eq!(
            files[0].analytics_keys.iter().collect::<Vec<_>>(),
            ["HOMEBREW_NO_ANALYTICS"],
            "exactly the keys the data names as governing analytics (DP-028)"
        );
    }

    #[test]
    fn a_file_with_no_analytics_key_marks_none() {
        let tree = Tree::new("brew-no-analytics");
        tree.write("home/.homebrew/brew.env", "HOMEBREW_CACHE=/x\n");
        let found = tree.read("homebrew");
        assert!(files(&found)[0].analytics_keys.is_empty());
    }

    #[test]
    fn a_prefix_file_that_moves_the_user_file_is_reported_and_outranks() {
        // `bin/brew` exports the prefix file before it computes where the user
        // file lives, so a prefix file can point the user file anywhere. The
        // relocation itself is a key, and it is recorded (spec-guardian,
        // 2026-10-04).
        let tree = Tree::new("xdg");
        tree.write(
            "prefix/etc/homebrew/brew.env",
            "HOMEBREW_XDG_CONFIG_HOME=/somewhere/else\n",
        );
        let found = tree.read("homebrew");
        let prefix_file = files(&found)
            .iter()
            .find(|f| f.scope == ConfigurationScope::Prefix)
            .expect("the prefix file");
        assert_eq!(keys(prefix_file), ["HOMEBREW_XDG_CONFIG_HOME"]);
        assert_eq!(found.outranks(), Some(true));
    }

    #[test]
    fn the_system_file_is_read_under_the_root_and_never_this_hosts() {
        let tree = Tree::new("system");
        tree.write(
            "root/etc/homebrew/brew.env",
            "HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY=1\n",
        );
        let found = tree.read("homebrew");
        let system = files(&found)
            .iter()
            .find(|f| f.scope == ConfigurationScope::System)
            .expect("the system file");
        assert_eq!(
            system.path, "/etc/homebrew/brew.env",
            "labelled as the provider names it"
        );
        assert_eq!(keys(system), ["HOMEBREW_SYSTEM_ENV_TAKES_PRIORITY"]);
    }

    #[test]
    fn a_macports_user_file_is_read_by_its_leading_word_on_any_line_ending() {
        // Tcl `gets` ends a line at a lone carriage return, so a key after one
        // is a key MacPorts applies (security-reviewer, 2026-10-04).
        let tree = Tree::new("port");
        tree.write(
            "home/.macports/macports.conf",
            "# portdbpath /nowhere\nportdbpath\t/tmp/db\r\nbinpath /usr/local/bin\n\
             # c\rprefix /evil\n\n",
        );
        let found = tree.read("macports");
        let files = files(&found);
        assert_eq!(keys(&files[0]), ["binpath", "portdbpath", "prefix"]);
        assert_eq!(files[0].path, "~/.macports/macports.conf");
    }

    #[test]
    fn a_key_this_record_cannot_name_makes_the_file_unreadable() {
        let tree = Tree::new("unnamed");
        tree.write("home/.macports/macports.conf", "pörtdbpath /tmp/db\n");
        let found = tree.read("macports");
        assert!(!files(&found)[0].readable);
        assert_eq!(found.outranks(), Some(true), "an unknown is never a no");
    }

    #[test]
    fn absent_files_are_not_listed_and_nothing_found_is_not_an_outranking() {
        let tree = Tree::new("empty");
        let found = tree.read("macports");
        assert_eq!(found, ProviderConfiguration::Read { files: Vec::new() });
        assert_eq!(found.outranks(), Some(false));
    }

    #[test]
    fn a_user_file_with_no_home_to_look_under_is_an_unknown_not_an_absence() {
        let tree = Tree::new("nohome");
        let found = read_under(
            "macports",
            &tree.path("prefix"),
            None,
            FilesystemScope::ThisMachine,
            &tree.path("root"),
        );
        let files = files(&found);
        assert_eq!(files.len(), 1);
        assert!(!files[0].readable);
        assert_eq!(found.outranks(), Some(true));
    }

    #[test]
    fn a_file_that_is_not_a_regular_file_is_unreadable_not_absent() {
        let tree = Tree::new("dir");
        std::fs::create_dir_all(tree.path("home/.macports/macports.conf")).expect("a directory");
        let found = tree.read("macports");
        assert!(!files(&found)[0].readable, "{found:?}");
    }
}
