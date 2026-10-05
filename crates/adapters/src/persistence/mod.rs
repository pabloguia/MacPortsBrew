// SPDX-License-Identifier: Apache-2.0

//! The persistence adapter: MacPortsBrew's own state on disk.
//!
//! Four small modules, each answering one line of
//! `04-migration-and-safety.md`:
//!
//! - [`state_dir`] — a private, user-owned directory at
//!   `~/Library/Application Support/macportsbrew`, refused rather than
//!   repaired when it is not what it should be.
//! - [`atomic`] — every write is atomic, at `0o600`, through a temporary file
//!   that cannot be redirected by a symbolic reference.
//! - [`snapshot_store`] — snapshots and recordings, read as untrusted input
//!   with a size bound and a schema check, written byte-stably.
//! - [`time`] — the one clock read in the workspace, and the formatting the
//!   rest of the code would otherwise pull a crate in for.
//!
//! Nothing here knows a provider's name. The only writes are inside the state
//! directory, plus the intermediates `create_dir_all` must make to reach it on
//! an account that has no `~/Library/Application Support` yet.

pub mod atomic;
pub(crate) mod bounded_read;
pub mod history_store;
pub mod run_lock;
pub mod snapshot_store;
pub mod state_dir;
pub mod time;

pub use atomic::{write_atomic, write_new_atomic, PublishError, FILE_MODE};
pub use snapshot_store::{
    load_recording, load_snapshot, save_snapshot, snapshot_file_name, snapshot_to_json,
    MAX_DOCUMENT_BYTES,
};
pub use state_dir::{StateDir, STATE_DIR_MODE};
pub use time::{compact_stamp, now_secs, rfc3339_utc, today_from};
