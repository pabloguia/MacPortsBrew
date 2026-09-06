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
use std::collections::{BTreeMap, BTreeSet};

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
    parse(SOURCE)
}

/// The same, over any text. Split out so the checks below can be exercised
/// against a deliberately broken copy of the table rather than only against the
/// one that ships - a validation nothing can fail is not a validation.
fn parse(source: &str) -> Result<BTreeMap<String, ProviderCommands>, DomainError> {
    let file: File = toml::from_str(source).map_err(|e| {
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
        // The executable is joined onto a detected prefix, and `Path::join`
        // discards the base when its argument is absolute - so `/bin/sh` here
        // would not be "the provider's executable under its prefix" but a
        // program of the file's own choosing. This file is `include_str!`-
        // embedded, so it is not attacker-controlled at run time; it is,
        // however, exactly the file where an edit that looks like a fix would
        // read as one (F-20).
        if e.executable.is_empty()
            || std::path::Path::new(&e.executable).is_absolute()
            || e.executable
                .split('/')
                .any(|part| part == ".." || part.is_empty())
        {
            return Err(DomainError::new(
                ErrorId::PolicyDataInvalid,
                format!(
                    "provider-commands.toml: entry {} names {:?} as its executable, \
                     which is not a path under the provider's own prefix",
                    e.provider, e.executable
                ),
            ));
        }
        // Every verb has an elevation, for both package kinds.
        //
        // The planner looks up `{operation}` for a package and
        // `{operation}_gui_app` for a GUI application, and a key it does not
        // find becomes `Elevation::Unknown`. Folding toward "might need rights"
        // is the right default for a value nobody stated, but under INV-06 the
        // effect of stating none is to show an administrator warning for a
        // read - which trains the user to accept one. `verify` reached that
        // value by omission in both entries while this file's own comment said
        // the `_gui_app` keys existed so it could not happen (F-10).
        // Every operation, not every verb. An operation reaches the planner as a
        // string, and `set_primary` is one that appears only in
        // `[entry.elevation]` — so keying off `verbs` left the one operation
        // that is elevation-only unchecked, and it was missing its `_gui_app`
        // cell. The check existed to forbid precisely that and could not see it.
        // Nothing routes `set_primary` through `command()` today, which is why
        // it was invisible rather than broken; the guarantee claimed is that the
        // *next* operation cannot repeat F-10, and that has to hold for one
        // added to either table.
        let mut operations: BTreeSet<&str> = e.verbs.keys().map(String::as_str).collect();
        operations.extend(
            e.elevation
                .keys()
                .map(|k| k.strip_suffix("_gui_app").unwrap_or(k)),
        );
        for verb in operations {
            for key in [verb.to_string(), format!("{verb}_gui_app")] {
                match e.elevation.get(&key).map(String::as_str) {
                    Some("none" | "required" | "unknown") => {}
                    Some(other) => {
                        return Err(DomainError::new(
                            ErrorId::PolicyDataInvalid,
                            format!(
                                "provider-commands.toml: entry {} gives {key} the elevation \
                                 {other:?}, which is not none, required or unknown; an \
                                 unrecognised value silently becomes \"unknown\"",
                                e.provider
                            ),
                        ));
                    }
                    None => {
                        return Err(DomainError::new(
                            ErrorId::PolicyDataInvalid,
                            format!(
                                "provider-commands.toml: entry {} declares the verb {verb} \
                                 but states no elevation for {key}, so the planner would \
                                 reach one by omission (INV-06)",
                                e.provider
                            ),
                        ));
                    }
                }
            }
        }
        out.insert(e.provider.clone(), e);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_table_that_is_not_toml_is_refused_as_invalid_policy_data() {
        // The first thing `parse` does, and the only branch of it that had no
        // test: a malformed file must name itself rather than panic or fall
        // back to an empty table, which would leave the planner with no verbs
        // and no explanation (DP-015).
        let err = parse("this is not [ toml").expect_err("malformed TOML must be refused");
        assert_eq!(err.id, ErrorId::PolicyDataInvalid);
        assert!(
            err.detail.contains("provider-commands.toml"),
            "the error must name the file: {}",
            err.detail
        );
    }

    #[test]
    fn an_entry_without_provenance_is_refused_however_complete_the_rest_is() {
        // INV-20 in the loader: a row with no `source` or no `read_on` is a
        // claim from memory, and this file is where that is caught. The entry
        // below is otherwise well-formed, so the refusal can only be about the
        // missing provenance.
        for (label, source, read_on) in [
            ("no source", "", "2026-09-05"),
            ("no read_on", "https://example.org", ""),
        ] {
            let text = format!(
                "[[entry]]\n\
                 provider = \"example\"\n\
                 executable = \"bin/example\"\n\
                 verbs = {{}}\n\
                 elevation = {{}}\n\
                 source = \"{source}\"\n\
                 read_on = \"{read_on}\"\n"
            );
            let err = parse(&text).expect_err(&format!("{label} must be refused, not loaded"));
            assert_eq!(err.id, ErrorId::PolicyDataInvalid, "{label}");
            assert!(
                err.detail.contains("lacks source or read_on"),
                "{label}: the refusal must say what is missing: {}",
                err.detail
            );
        }
    }

    use super::*;

    #[test]
    fn embedded_table_loads_with_provenance() {
        let table = load().unwrap();
        assert!(table.len() >= 2);
        for (id, e) in &table {
            assert_eq!(id, &e.provider);
            assert!(e.verbs.contains_key("add"), "{id} lacks add");
            assert!(e.verbs.contains_key("remove"), "{id} lacks remove");
            assert!(e.verbs.contains_key("verify"), "{id} lacks verify");
            assert!(!e.source.is_empty() && !e.read_on.is_empty());
        }
    }

    /// The table as shipped, minus whatever the caller wants to break.
    fn without(line: &str) -> Result<(), DomainError> {
        let text: String = SOURCE
            .lines()
            .filter(|l| l.trim() != line)
            .collect::<Vec<_>>()
            .join("\n");
        parse(&text).map(|_| ())
    }

    #[test]
    fn every_verb_states_an_elevation_for_both_package_kinds() {
        // The defect this closes was not a missing check; it was a *comment*
        // saying the keys existed so the planner could not reach a value by
        // omission, while `verify` did exactly that in both entries (F-10).
        for key in [
            "verify = \"none\"",
            "verify_gui_app = \"unknown\"",
            "add_gui_app = \"unknown\"",
            // The case the first version of this check could not see: an
            // operation that appears in `[entry.elevation]` and not in
            // `[entry.verbs]`. It iterated the verbs, so `set_primary` - which
            // was missing its `_gui_app` cell entirely - passed the check
            // written to forbid exactly that shape.
            "set_primary_gui_app = \"none\"",
            "set_primary = \"none\"",
        ] {
            let Err(err) = without(key) else {
                panic!("removing {key} must be refused");
            };
            assert_eq!(err.id, ErrorId::PolicyDataInvalid, "{key}: {err:?}");
            assert!(
                err.detail.contains("omission") || err.detail.contains("elevation"),
                "{key}: {}",
                err.detail
            );
        }
    }

    #[test]
    fn an_elevation_value_that_is_not_one_of_the_three_is_refused() {
        // Left unchecked this is the quietest failure of the three: the
        // planner's `match` folds any unrecognised string to `Unknown`, so a
        // typo reads as caution rather than as a mistake.
        let text = SOURCE.replacen("add = \"required\"", "add = \"requried\"", 1);
        let err = parse(&text).expect_err("a misspelt elevation must be refused");
        assert_eq!(err.id, ErrorId::PolicyDataInvalid);
        assert!(err.detail.contains("requried"), "{}", err.detail);
    }

    #[test]
    fn an_executable_that_escapes_the_prefix_is_refused() {
        // `Path::join` discards the base when the argument is absolute, so an
        // absolute `executable` is not "under the prefix" at all (F-20).
        for bad in ["/bin/sh", "../../bin/sh", ""] {
            let text = SOURCE.replacen(
                "executable = \"bin/brew\"",
                &format!("executable = \"{bad}\""),
                1,
            );
            let err = parse(&text).unwrap_err();
            assert_eq!(err.id, ErrorId::PolicyDataInvalid, "{bad}");
            assert!(err.detail.contains("executable"), "{bad}: {}", err.detail);
        }
    }
}
