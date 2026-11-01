// SPDX-License-Identifier: Apache-2.0

//! A provider's installed-file listing, parsed into the executables it ships.
//!
//! Part F's executable list (`docs/passes/16-plans/F.md`, Design §2). Both
//! providers answer with one absolute path per line - MacPorts' `port -q
//! contents` indents each with two spaces, Homebrew's `brew list --formula`
//! does not (R-21 E-4, E-9) - so one parser serves both and the INV-17 shape is
//! one rule, not two copies of it:
//!
//! * every line is the indent, then an absolute path with no empty, `.` or
//!   `..` component, no trailing `/` and no NUL; anything else is
//!   `ProviderOutputChanged`, and so is an empty answer, because an installed
//!   package lists at least one file (SR2-N1);
//! * an executable is a path whose parent equals `<root>/bin` or `<root>/sbin`
//!   **component-wise**, never by string prefix (SR-10); deeper paths are
//!   ignored, and so is an entry that is itself the parent of another entry
//!   (a directory, not a file);
//! * nothing is stat-ed or followed.
//!
//! The failure mapping (SR-8, SR2-N4) is here too, so both adapters degrade the
//! same way.

use crate::process::Invocation;
use macportsbrew_domain::{
    DomainError, ErrorId, ExecutableList, ListReason, Placement, UnknownReason,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

fn changed(read: &str, detail: impl std::fmt::Display) -> DomainError {
    DomainError::new(ErrorId::ProviderOutputChanged, format!("{read}: {detail}"))
}

/// The paths of a listing, in the order printed.
///
/// `indent` is what precedes every path (`"  "` for MacPorts, `""` for
/// Homebrew); `read` names the read in an error.
pub(crate) fn parse(stdout: &str, indent: &str, read: &str) -> Result<Vec<PathBuf>, DomainError> {
    if stdout.is_empty() {
        return Err(changed(
            read,
            "exit 0 with nothing on stdout; an installed package lists at least one file",
        ));
    }
    stdout
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let path = line
                .strip_prefix(indent)
                .filter(|rest| rest.starts_with('/'))
                .ok_or_else(|| {
                    changed(
                        read,
                        format!("line {} is not the indent and an absolute path", index + 1),
                    )
                })?;
            if !is_plain_absolute(path) {
                return Err(changed(
                    read,
                    format!(
                        "line {} has an empty, `.` or `..` component, a trailing `/` or a NUL",
                        index + 1
                    ),
                ));
            }
            Ok(PathBuf::from(path))
        })
        .collect()
}

/// An absolute path every component of which is a real name.
fn is_plain_absolute(path: &str) -> bool {
    path.len() > 1
        && !path.contains('\0')
        && path[1..]
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}

/// Whether `path` lies under `root`, compared component by component.
pub(crate) fn is_under(path: &Path, root: &Path) -> bool {
    let mut path = path.components();
    root.components().all(|c| path.next() == Some(c)) && path.next().is_some()
}

/// The leaf names directly under each of `roots`, with the indices of the
/// roots each was found under. A path's parent is compared with a root
/// component-wise; an entry that is the parent of another entry is a directory
/// and is skipped.
pub(crate) fn direct_children(
    paths: &[PathBuf],
    roots: &[PathBuf],
) -> BTreeMap<String, BTreeSet<usize>> {
    let parents: BTreeSet<&Path> = paths.iter().filter_map(|p| p.parent()).collect();
    let mut found: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for path in paths {
        if parents.contains(path.as_path()) {
            continue;
        }
        let (Some(parent), Some(Component::Normal(leaf))) =
            (path.parent(), path.components().next_back())
        else {
            continue;
        };
        for (index, root) in roots.iter().enumerate() {
            if parent.components().eq(root.components()) {
                if let Some(leaf) = leaf.to_str() {
                    found.entry(leaf.to_string()).or_default().insert(index);
                }
            }
        }
    }
    found
}

/// The list and, when `placed` names the directory each root links into, the
/// placement, from what [`direct_children`] found.
///
/// The list is all or nothing ([`ExecutableList::from_leaves`]). A name found
/// under two roots has no single directory, so the placement is
/// [`ListReason::AmbiguousDirectory`] while the list keeps the name.
pub(crate) fn outcome(
    found: &BTreeMap<String, BTreeSet<usize>>,
    placed: Option<&[PathBuf]>,
) -> (ExecutableList, Option<Placement>) {
    let list = ExecutableList::from_leaves(found.keys().map(String::as_str));
    let Some(executables) = list.known() else {
        let reason = match &list {
            ExecutableList::Unknown { reason } => *reason,
            ExecutableList::NotRecorded | ExecutableList::Known { .. } => {
                UnknownReason::Listed(ListReason::NotRecorded)
            }
        };
        return (list, Some(Placement::unknown(reason)));
    };
    let Some(dirs) = placed else {
        return (list, None);
    };
    let mut placement = BTreeMap::new();
    for name in executables {
        let roots = &found[name.as_str()];
        let (Some(only), 1) = (roots.iter().next(), roots.len()) else {
            return (
                list,
                Some(Placement::unknown(ListReason::AmbiguousDirectory)),
            );
        };
        placement.insert(name.clone(), dirs[*only].display().to_string());
    }
    let placement = Placement::OnPathDir { dirs: placement };
    (list, Some(placement))
}

/// What a listing read answered, mapped per SR-8 and SR2-N4.
///
/// * The runner's `Internal` **while replaying** - a recording with no
///   invocation for this read - is a per-package unknown, and the capture
///   continues.
/// * Any other runner error propagates and aborts the capture, live or
///   replayed. The adapter's **own** allowlist refusal never reaches here: it
///   is checked before the runner is called and always propagates.
/// * A program that could not be started (`ProviderNotFound`: the provider
///   vanished mid-capture) propagates as detection's error does.
/// * Any other way the provider answered badly - a non-zero exit, a timeout,
///   truncated output - is a per-package unknown carrying the error id.
pub(crate) fn stdout_of(
    run: Result<Invocation, DomainError>,
    is_replay: bool,
) -> Result<Result<String, UnknownReason>, DomainError> {
    let invocation = match run {
        Ok(invocation) => invocation,
        Err(error) if error.id == ErrorId::Internal && is_replay => {
            return Ok(Err(UnknownReason::Read(ErrorId::Internal)));
        }
        Err(error) => return Err(error),
    };
    match invocation.require_success() {
        Ok(invocation) => Ok(Ok(invocation.stdout.clone())),
        Err(error) if error.id == ErrorId::ProviderNotFound => Err(error),
        Err(error) => Ok(Err(UnknownReason::Read(error.id))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ExitStatus;

    fn paths(lines: &[&str]) -> Vec<PathBuf> {
        lines.iter().map(PathBuf::from).collect()
    }

    fn roots(root: &str) -> Vec<PathBuf> {
        vec![Path::new(root).join("bin"), Path::new(root).join("sbin")]
    }

    #[test]
    fn a_path_with_an_empty_dot_or_dot_dot_component_or_a_trailing_slash_is_refused() {
        for bad in [
            "/opt/local//bin/xz",
            "/opt/local/./bin/xz",
            "/opt/local/../bin/xz",
            "/opt/local/bin/",
            "/",
            "/opt/local/bin/x\0z",
            "opt/local/bin/xz",
        ] {
            let error = parse(&format!("{bad}\n"), "", "test").unwrap_err();
            assert_eq!(error.id, ErrorId::ProviderOutputChanged, "{bad:?}");
        }
        assert_eq!(
            parse(
                "/opt/local/bin/xz\n/opt/local/share/doc/xz/README\n",
                "",
                "test"
            )
            .unwrap(),
            paths(&["/opt/local/bin/xz", "/opt/local/share/doc/xz/README"])
        );
    }

    #[test]
    fn the_indent_is_exactly_the_indent() {
        assert!(parse("  /opt/local/bin/xz\n", "  ", "test").is_ok());
        for bad in [
            " /opt/local/bin/xz\n",
            "   /opt/local/bin/xz\n",
            "\t/opt/local/bin/xz\n",
        ] {
            assert_eq!(
                parse(bad, "  ", "test").unwrap_err().id,
                ErrorId::ProviderOutputChanged,
                "{bad:?}"
            );
        }
        assert_eq!(
            parse("  /opt/local/bin/xz\n\n", "  ", "test")
                .unwrap_err()
                .id,
            ErrorId::ProviderOutputChanged,
            "an empty line is not a path"
        );
    }

    #[test]
    fn the_parent_is_compared_by_component() {
        // A string prefix would count `/opt/local/binx/a` and
        // `/opt/local/bin-extra/b` as under `/opt/local/bin`.
        let found = direct_children(
            &paths(&[
                "/opt/local/bin/xz",
                "/opt/local/binx/a",
                "/opt/local/bin-extra/b",
                "/opt/local/sbin/s",
                "/opt/localbin/c",
            ]),
            &roots("/opt/local"),
        );
        assert_eq!(found.keys().collect::<Vec<_>>(), vec!["s", "xz"]);
        assert!(is_under(
            Path::new("/opt/local/bin/xz"),
            Path::new("/opt/local")
        ));
        assert!(!is_under(
            Path::new("/opt/localx/bin"),
            Path::new("/opt/local")
        ));
        assert!(!is_under(Path::new("/opt/local"), Path::new("/opt/local")));
    }

    #[test]
    fn only_direct_entries_of_bin_and_sbin_are_executables() {
        let found = direct_children(
            &paths(&[
                "/opt/local/bin/xz",
                "/opt/local/bin/x/deeper",
                "/opt/local/bin/x/deeper2",
                "/opt/local/libexec/bin/hidden",
                "/opt/local/share/man/man1/xz.1",
                "/opt/local/sbin/daemon",
            ]),
            &roots("/opt/local"),
        );
        // `x` is a directory (it has entries), not an executable.
        assert_eq!(found.keys().collect::<Vec<_>>(), vec!["daemon", "xz"]);
    }

    #[test]
    fn a_name_under_both_bin_and_sbin_has_no_single_directory() {
        let found = direct_children(
            &paths(&["/p/bin/tool", "/p/sbin/tool", "/p/bin/other"]),
            &roots("/p"),
        );
        let (list, placement) = outcome(&found, Some(&roots("/p")));
        assert_eq!(list.known().unwrap().len(), 2);
        assert_eq!(
            placement,
            Some(Placement::unknown(ListReason::AmbiguousDirectory))
        );
    }

    #[test]
    fn a_bad_leaf_makes_list_and_placement_unknown() {
        let found = direct_children(&paths(&["/p/bin/ok", "/p/bin/-x"]), &roots("/p"));
        let (list, placement) = outcome(&found, Some(&roots("/p")));
        assert_eq!(list, ExecutableList::unknown(ListReason::NameClass));
        assert_eq!(placement, Some(Placement::unknown(ListReason::NameClass)));
    }

    fn invocation(status: ExitStatus, stdout: &str) -> Invocation {
        Invocation {
            program: "port".into(),
            argv: Vec::new(),
            environment: BTreeMap::new(),
            status,
            stdout: stdout.into(),
            stderr: String::new(),
            truncated: false,
            duration_ms: 0,
        }
    }

    #[test]
    fn the_failure_mapping_keeps_defects_and_unknowns_apart() {
        let internal = || DomainError::new(ErrorId::Internal, "no recorded invocation");
        assert_eq!(
            stdout_of(Err(internal()), true).unwrap(),
            Err(UnknownReason::Read(ErrorId::Internal)),
            "a replayed missing invocation is a per-package unknown"
        );
        assert_eq!(
            stdout_of(Err(internal()), false).unwrap_err().id,
            ErrorId::Internal,
            "a live Internal is a defect"
        );
        assert_eq!(
            stdout_of(Err(DomainError::new(ErrorId::ProviderUnhealthy, "x")), true)
                .unwrap_err()
                .id,
            ErrorId::ProviderUnhealthy
        );
        assert_eq!(
            stdout_of(Ok(invocation(ExitStatus::Exited { code: 1 }, "")), false).unwrap(),
            Err(UnknownReason::Read(ErrorId::ProviderUnhealthy))
        );
        assert_eq!(
            stdout_of(Ok(invocation(ExitStatus::TimedOut, "")), false).unwrap(),
            Err(UnknownReason::Read(ErrorId::ProviderTimeout))
        );
        assert_eq!(
            stdout_of(
                Ok(invocation(
                    ExitStatus::NotStarted {
                        error: "gone".into()
                    },
                    ""
                )),
                false
            )
            .unwrap_err()
            .id,
            ErrorId::ProviderNotFound
        );
        let mut truncated = invocation(ExitStatus::Exited { code: 0 }, "/p/bin/a\n");
        truncated.truncated = true;
        assert_eq!(
            stdout_of(Ok(truncated), false).unwrap(),
            Err(UnknownReason::Read(ErrorId::ProviderOutputChanged))
        );
        assert_eq!(
            stdout_of(Ok(invocation(ExitStatus::Exited { code: 0 }, "x")), false).unwrap(),
            Ok("x".to_string())
        );
    }
}
