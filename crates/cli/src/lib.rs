// SPDX-License-Identifier: Apache-2.0

//! The presentation layer: the i18n catalogue and the text renderer.
//!
//! Everything a user reads is produced here. The domain, the policy engine and
//! the adapters emit stable identifiers — [`macportsbrew_domain::ErrorId`],
//! [`macportsbrew_domain::Reason`] — and this crate turns them into sentences
//! in the user's locale (`docs/spec/06-cli-and-ux.md`, "Internationalisation").
//!
//! Two rules shape the whole crate:
//!
//! * **English is the source.** `locales/en.toml` is compiled into the binary
//!   and is the only catalogue this crate ships. Every other catalogue is a
//!   translation of exactly its keys, and `scripts/check-i18n-parity.sh` fails
//!   the build on a missing or an orphaned one.
//! * **Layout measures display width, never bytes or characters.** Japanese and
//!   Chinese text is double-width; column arithmetic done in `char` counts
//!   produces ragged tables for half the target locales. [`render::width`] is
//!   the only place that answers "how wide is this".

pub mod i18n;
pub mod render;
