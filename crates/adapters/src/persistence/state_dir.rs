// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew's own state directory.
//!
//! `04-migration-and-safety.md` ("Threat model", state and configuration
//! integrity) asks for one that is user-owned with restrictive permissions,
//! and for corrupt state to fail closed rather than to produce a plan. This
//! module is the whole of "user-owned with restrictive permissions": a private
//! directory at mode `0o700`, created without following a symbolic reference,
//! refused rather than repaired when it is not what it should be.
//!
//! The location is `~/Library/Application Support/macportsbrew`. Apple's *File
//! System Programming Guide* describes `~/Library/Application Support` as the
//! place for "all app-specific data and support files … that your app creates
//! and manages on behalf of the user", organised in a per-application
//! subdirectory the application creates itself.
//! Source: <https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html>,
//! read 2026-09-02. Apple's convention is a bundle identifier; MacPortsBrew is
//! a command-line tool with no bundle, so the subdirectory is the tool's name.
//!
//! Three subdirectories, named by what `04-migration-and-safety.md` puts in
//! them: `snapshots/` for the automatic pre-mutation captures that are never
//! pruned automatically, `history/` for the run records `macportsbrew history`
//! lists, and `cache/` for anything that may be deleted without losing a fact.

use macportsbrew_domain::{DomainError, ErrorId};
use std::fs::DirBuilder;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The directory mode: the owner, and nobody else.
pub const STATE_DIR_MODE: u32 = 0o700;

/// The path under a home directory, as Apple's guide lays it out.
const RELATIVE_LOCATION: &str = "Library/Application Support/macportsbrew";

/// A resolved, not yet necessarily created, state directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDir {
    root: PathBuf,
}

impl StateDir {
    /// Resolves the state directory without touching the filesystem.
    ///
    /// `override_env` is the value of
    /// [`crate::system::STATE_DIR_VARIABLE`] when it is set — the only thing
    /// that relocates the directory. It must be absolute, because a state
    /// directory resolved against the working directory is the same class of
    /// problem as a relative `PATH` entry. With no override, `home` is
    /// required.
    pub fn resolve(home: Option<&Path>, override_env: Option<&str>) -> Result<Self, DomainError> {
        if let Some(raw) = override_env.map(str::trim).filter(|s| !s.is_empty()) {
            let root = PathBuf::from(raw);
            if !root.is_absolute() {
                return Err(DomainError::new(
                    ErrorId::Io,
                    format!("the state directory override must be an absolute path, not {raw:?}"),
                ));
            }
            return Ok(Self { root });
        }
        let home = home.ok_or_else(|| {
            DomainError::new(
                ErrorId::Io,
                "no home directory is known, so the state directory cannot be placed",
            )
        })?;
        if !home.is_absolute() {
            return Err(DomainError::new(
                ErrorId::Io,
                format!(
                    "the home directory must be absolute, not {}",
                    home.display()
                ),
            ));
        }
        Ok(Self {
            root: home.join(RELATIVE_LOCATION),
        })
    }

    /// The root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where automatic pre-mutation snapshots live.
    pub fn snapshots(&self) -> PathBuf {
        self.root.join("snapshots")
    }

    /// Where run records live.
    pub fn history(&self) -> PathBuf {
        self.root.join("history")
    }

    /// Where discardable data lives.
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Every directory this owns, root first, in creation order.
    pub fn directories(&self) -> Vec<PathBuf> {
        vec![
            self.root.clone(),
            self.snapshots(),
            self.history(),
            self.cache(),
        ]
    }

    /// Creates the directory tree, or verifies what is already there.
    ///
    /// Idempotent (INV-16): a second call on a correct tree changes nothing
    /// and reports success. It refuses, rather than repairing, when
    ///
    /// - a path is a symbolic reference — following one is how a state
    ///   directory becomes a write into somewhere else entirely;
    /// - a path exists and is not a directory;
    /// - somebody else owns it. The comparison is against this process's own
    ///   effective uid, obtained without a `libc` dependency: the standard
    ///   library exposes no `getuid`, but it does expose the uid of a file, so
    ///   a probe file created inside the tree names its creator. That works
    ///   for an overridden location too, which a comparison against the home
    ///   directory's owner would not.
    ///
    /// Ownership is checked *before* the mode is repaired, so a directory
    /// somebody else owns is refused rather than chmod-ed. The repair itself
    /// only ever reduces access: a directory at a looser mode is set back to
    /// `0o700`, and it fails closed if even that does not work.
    ///
    /// "Changes nothing" means the tree, its modes and its contents. The
    /// ownership probe is created and removed inside the root, which moves the
    /// root's modification time on every call; nothing is left behind.
    ///
    /// **What it does not check.** Only the four directories it owns are
    /// inspected. The intermediates that `create_dir_all` traverses —
    /// `~/Library` and `~/Library/Application Support` — are not, so a user
    /// who has relocated `Application Support` behind a symbolic link (an
    /// ordinary thing to do on a small disk) keeps their state behind it. That
    /// is deliberate: refusing would break those users, and the alternative —
    /// silently rejecting a location the user chose — is worse than
    /// documenting the boundary. It does mean the guarantee is "this code
    /// never *creates* or *follows* a symbolic reference at the four paths it
    /// owns", not "no symbolic reference is anywhere in the path".
    pub fn ensure(&self) -> Result<(), DomainError> {
        let directories = self.directories();
        // The root first, so there is somewhere private to put the probe.
        create_private_directory(&directories[0], None)?;
        let owner = effective_uid(&directories[0])?;
        for directory in &directories {
            create_private_directory(directory, Some(owner))?;
        }
        Ok(())
    }
}

/// This process's effective uid, read as the owner of a file it just created.
/// `std` has no `getuid`; it does have `MetadataExt::uid`, and the owner of a
/// file this process made is this process. A directory that will not accept
/// the probe is one this process cannot safely keep state in either, so the
/// failure is the answer rather than a reason to skip the check.
fn effective_uid(inside: &Path) -> Result<u32, DomainError> {
    let probe = inside.join(format!(".owner-probe.{}", std::process::id()));
    let uid = (|| -> std::io::Result<u32> {
        std::fs::remove_file(&probe).ok();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&probe)?;
        file.metadata().map(|m| m.uid())
    })();
    std::fs::remove_file(&probe).ok();
    uid.map_err(|e| {
        DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "{} could not be written to, so its ownership cannot be established: {e}",
                inside.display()
            ),
        )
    })
}

/// Creates one directory at [`STATE_DIR_MODE`], or verifies the existing one.
/// When `owner` is given, refuses a directory somebody else owns before
/// touching its permissions.
fn create_private_directory(path: &Path, owner: Option<u32>) -> Result<(), DomainError> {
    // `read_link` succeeding is the whole test: the path is a symbolic
    // reference, and every later operation on it would land somewhere this
    // code did not choose.
    if std::fs::read_link(path).is_ok() {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!(
                "{} is a symbolic reference, not a directory; refusing to keep state behind it",
                path.display()
            ),
        ));
    }
    if let Err(e) = DirBuilder::new()
        .recursive(true)
        .mode(STATE_DIR_MODE)
        .create(path)
    {
        return Err(DomainError::new(
            ErrorId::Io,
            format!("{} could not be created: {e}", path.display()),
        ));
    }
    // `symlink_metadata` does not follow, so the type and mode inspected are
    // the ones the check above refused — one resolution of the name rather
    // than two, which closes the window between them.
    let meta = std::fs::symlink_metadata(path).map_err(|e| {
        DomainError::new(
            ErrorId::Io,
            format!("{} could not be inspected: {e}", path.display()),
        )
    })?;
    if !meta.is_dir() {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            format!("{} exists and is not a directory", path.display()),
        ));
    }
    if let Some(owner) = owner {
        if meta.uid() != owner {
            return Err(DomainError::new(
                ErrorId::SafetyBlocked,
                format!(
                    "{} is owned by uid {} but this process runs as uid {owner}; \
                     refusing to keep state in a directory somebody else owns",
                    path.display(),
                    meta.uid()
                ),
            ));
        }
    }
    if meta.permissions().mode() & 0o777 != STATE_DIR_MODE {
        let private = std::fs::Permissions::from_mode(STATE_DIR_MODE);
        std::fs::set_permissions(path, private).map_err(|e| {
            DomainError::new(
                ErrorId::SafetyBlocked,
                format!(
                    "{} is not private and could not be made private: {e}",
                    path.display()
                ),
            )
        })?;
        // A filesystem that ignores permissions (a FAT or exFAT volume, some
        // network shares) accepts the call and changes nothing. Read the mode
        // back rather than trust the call, or every later private read of the
        // tree fails while this reported success (pass 08).
        let repaired =
            std::fs::symlink_metadata(path).map_or(0, |m| m.permissions().mode() & 0o777);
        if repaired != STATE_DIR_MODE {
            return Err(DomainError::new(
                ErrorId::SafetyBlocked,
                format!(
                    "{} could not be made private: its filesystem does not keep permissions",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_location_is_apples_application_support_directory() {
        let dir = StateDir::resolve(Some(Path::new("/Users/ada")), None).unwrap();
        assert_eq!(
            dir.root(),
            Path::new("/Users/ada/Library/Application Support/macportsbrew")
        );
        assert!(dir.snapshots().ends_with("macportsbrew/snapshots"));
        assert!(dir.history().ends_with("macportsbrew/history"));
        assert!(dir.cache().ends_with("macportsbrew/cache"));
        assert_eq!(dir.directories().len(), 4);
        assert_eq!(dir.directories()[0], dir.root());
    }

    #[test]
    fn an_override_replaces_the_location_and_must_be_absolute() {
        let dir = StateDir::resolve(Some(Path::new("/Users/ada")), Some("/var/tmp/mpb")).unwrap();
        assert_eq!(dir.root(), Path::new("/var/tmp/mpb"));
        // Blank is not an override.
        assert_eq!(
            StateDir::resolve(Some(Path::new("/Users/ada")), Some("  "))
                .unwrap()
                .root(),
            Path::new("/Users/ada/Library/Application Support/macportsbrew")
        );
        assert_eq!(
            StateDir::resolve(None, Some("relative/state"))
                .unwrap_err()
                .id,
            ErrorId::Io
        );
    }

    #[test]
    fn without_a_home_and_without_an_override_there_is_nowhere_to_put_it() {
        assert_eq!(StateDir::resolve(None, None).unwrap_err().id, ErrorId::Io);
        assert_eq!(
            StateDir::resolve(Some(Path::new("relative")), None)
                .unwrap_err()
                .id,
            ErrorId::Io
        );
    }
}
