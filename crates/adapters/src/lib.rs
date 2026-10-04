// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew adapters: the only layer that touches the machine.
//!
//! `process` is the one place a subprocess is spawned
//! (`docs/spec/04-migration-and-safety.md`, "Process execution");
//! `scripts/check-layering.sh` asserts that textually. Provider adapters
//! implement the domain's read contract over a [`process::ProcessRunner`],
//! which is either the real system or a recorded fixture - so the same
//! adapter code runs against a live machine and against captured output,
//! and tests never touch a developer's real installation.
//!
//! The provider names appear here and nowhere above this layer.

pub mod configuration;
pub mod filesystem;
pub mod homebrew;
pub mod macports;
pub mod persistence;
pub mod prefixes;
pub mod process;
pub mod registry;
pub mod system;
