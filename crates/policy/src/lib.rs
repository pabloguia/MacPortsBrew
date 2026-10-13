// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew `policy` layer: the rule engine and the versioned data it
//! reads (`docs/spec/03-policy-and-recommendations.md`).
//!
//! Three properties hold across this crate and are what its tests defend:
//!
//! * **Pure.** [`engine::recommend`] is a function of `(facts, data)`. No
//!   I/O, no clock, no environment, no language model (INV-14). The current
//!   date is a field on [`facts::Facts`], because a rule that reads a clock
//!   is a rule whose output cannot be reproduced.
//! * **Reproducible.** Every collection that reaches a serialised surface is
//!   ordered, rules are evaluated in identifier order, and ties are broken by
//!   identifier (INV-15).
//! * **Symmetric.** No rule names a provider. Rules read facts and data, and
//!   the provider ids are values that flow through them, so renaming the two
//!   providers in the input renames them in the output and changes nothing
//!   else (INV-17).
//!
//! Everything a rule knows about a provider lives in `data/policy/*.toml`
//! with a source and a read date, embedded at build time (INV-20).

pub mod data;
pub mod engine;
pub mod facts;
pub mod reasons;
pub mod rules;

pub use data::{
    days_between, shift_date, BuildDefence, CrossProviderAdvice, PolicyData, PolicySources,
};
pub use engine::{explain, recommend, Derivation, FiredRule};
pub use facts::{Facts, PackageFacts, PackageProviderFacts, ProviderFacts};
pub use rules::{labels, Rule, RuleVerdict};
