// SPDX-License-Identifier: Apache-2.0

//! The commands a plan step would run, per provider, as dated data.
//!
//! Verbs live in `data/policy/provider-commands.toml`, never in code: the
//! foundation pass ships no mutating code path and the Stop gate greps
//! `src/` for the verbs (SE-020). A plan renders them; nothing executes
//! them. Elevation per operation comes from the same file (R-5: it varies
//! by provider *and* by package kind).

use macportsbrew_domain::{DomainError, ErrorId};
use serde::Deserialize;
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../data/policy/provider-commands.toml");

/// One provider's command table.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ProviderCommands {
    /// Provider id.
    pub provider: String,
    /// Executable path relative to the prefix (`bin/brew`).
    pub executable: String,
    /// Verb argv per operation (`add`, `remove`, `verify`).
    pub verbs: BTreeMap<String, Vec<String>>,
    /// Elevation per operation: `none`, `required`, `unknown`.
    pub elevation: BTreeMap<String, String>,
    /// Why elevation is needed where it is.
    #[serde(default)]
    pub elevation_reason: Option<String>,
    /// Words this provider reads as something other than a package name,
    /// wherever a package name goes.
    ///
    /// A provider-neutral predicate can say "this is not a flag"; it cannot say
    /// "this is not a keyword", because the keywords belong to the provider. So
    /// they are data, dated and sourced like every other provider fact (INV-20).
    #[serde(default)]
    pub reserved_names: Vec<String>,
    /// Provenance.
    pub source: String,
    /// Read date.
    pub read_on: String,
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    entry: Vec<ProviderCommands>,
}

/// Loads the embedded table.
pub fn load() -> Result<BTreeMap<String, ProviderCommands>, DomainError> {
    let file: File = toml::from_str(SOURCE).map_err(|e| {
        DomainError::new(
            ErrorId::PolicyDataInvalid,
            format!("provider-commands.toml: {e}"),
        )
    })?;
    let mut out = BTreeMap::new();
    for e in file.entry {
        if e.source.is_empty() || e.read_on.is_empty() {
            return Err(DomainError::new(
                ErrorId::PolicyDataInvalid,
                format!(
                    "provider-commands.toml: entry {} lacks source or read_on",
                    e.provider
                ),
            ));
        }
        out.insert(e.provider.clone(), e);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_table_loads_with_provenance() {
        let table = load().unwrap();
        assert!(table.len() >= 2);
        for (id, e) in &table {
            assert_eq!(id, &e.provider);
            assert!(e.verbs.contains_key("add"), "{id} lacks add");
            assert!(e.verbs.contains_key("remove"), "{id} lacks remove");
            assert!(e.elevation.contains_key("add"), "{id} lacks add elevation");
            assert!(!e.source.is_empty() && !e.read_on.is_empty());
        }
    }
}
