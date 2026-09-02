// SPDX-License-Identifier: Apache-2.0

//! Reasons: stable identifiers with parameters, rendered by the UI in the
//! user's locale. Domain and policy emit reasons; they never contain display
//! text (`06-cli-and-ux.md`, i18n).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A stable reason identifier (`reason.homebrew_support_ending` style,
/// lower-case, dotted).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReasonId(String);

impl ReasonId {
    /// Creates an id.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A reason with the values a sentence about it needs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reason {
    /// Which reason.
    pub id: ReasonId,
    /// Named parameters, sorted, all rendered as text.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
    /// The rule that emitted it, when a rule did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

impl Reason {
    /// A reason without parameters.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: ReasonId::new(id),
            params: BTreeMap::new(),
            rule: None,
        }
    }

    /// Adds a parameter.
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: &dyn std::fmt::Display) -> Self {
        self.params.insert(key.into(), value.to_string());
        self
    }

    /// Records the emitting rule.
    #[must_use]
    pub fn from_rule(mut self, rule: impl Into<String>) -> Self {
        self.rule = Some(rule.into());
        self
    }
}
