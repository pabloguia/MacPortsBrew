// SPDX-License-Identifier: Apache-2.0

//! The system adapter: what this machine is, and what its `PATH` looks like.
//!
//! Provider-neutral by construction. It implements the domain's
//! [`MachineRead`](macportsbrew_domain::MachineRead) contract over a
//! [`ProcessRunner`](crate::process::ProcessRunner), which is either the live
//! machine or a recording, and it reports the environment rather than managing
//! it (`05-environment-and-primary-provider.md`, "Environment variables").
//!
//! Only [`machine::from_process_environment`] reads the process environment,
//! and only [`crate::persistence::time::now_secs`] reads the clock. Everything
//! else is a pure function of what it is handed, which is what makes an
//! analysis reproducible from a snapshot (INV-14, INV-15).

pub mod machine;
pub mod path_audit;

pub use machine::{
    from_process_environment, is_reported, redact_home, state_dir_override, SystemMachine,
    STATE_DIR_VARIABLE,
};
pub use macportsbrew_domain::{PathFinding, PathFindingKind};
pub use path_audit::audit_path;
