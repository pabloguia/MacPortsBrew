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
    /// How success is established per operation: `exit_status`,
    /// `stdout_names_package`.
    ///
    /// Keyed by operation like `verbs`, and **not yet** per package kind.
    ///
    /// The first version of this comment said a kind-aware form "would be
    /// inventing a distinction no provider makes". That is wrong, and
    /// `planner.rs` records the counter-example twenty lines away:
    /// `brew list --versions <cask>` exits 1 for a *correctly installed* cask
    /// (F-22). The verb differs by kind, so the semantic that reads it differs
    /// by kind too.
    ///
    /// It is not kind-aware today because it cannot be reached: a `GuiApp`
    /// carries `PackageClass::GuiApp`, which forces `MANUAL_REVIEW` before any
    /// command is built. **Whoever closes F-22 adds the kind-aware cell in the
    /// same change as the kind-aware verb** - and until then the mismatch
    /// fails closed, because a verification that cannot succeed halts before
    /// `REMOVE_SOURCE` rather than after it (security-reviewer, 2026-09-18).
    pub success: BTreeMap<String, String>,
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
        // Every verb states how its success is established (F-33, INV-01).
        //
        // Keyed off `verbs` alone, not the union above, and the difference is
        // deliberate: a success semantic describes a command this table
        // supplies the argv for, and `set_primary` supplies none - the planner
        // builds that step's command itself, as a `CommandTarget::Tool`, and
        // states the semantic there. Requiring a cell here for an operation
        // with no verb would be a cell nothing reads.
        //
        // The refusal is at load rather than at use for the same reason the
        // elevation check is: the planner's fallback is
        // `SuccessSemantic::Unknown`, which an executor must refuse, so an
        // omission here would turn into a plan that cannot be run rather than
        // into a build that cannot ship.
        for verb in e.verbs.keys() {
            // The coupling the whole absence design rests on, and nothing
            // checked it until `security-reviewer` said so on 2026-09-19.
            //
            // `stdout_omits_package` is only safe because the command names no
            // package: asked about one name, both providers answer an absent
            // package, a name that never was one, and a failure identically.
            // The two halves live in different files - the semantic here, the
            // "no package in argv" rule in `planner::names_no_package` - so a
            // data-only edit could give a name-taking verb this semantic and
            // produce a plan telling an executor "absent iff the name is
            // missing" about a command that asks for one name. `port installed
            // <name>` without `-q` is the live example and prints a sentence
            // that is non-empty and does not contain the name: a certified
            // removal, for a port that is still there.
            if e.success.get(verb).map(String::as_str) == Some("stdout_omits_package")
                && !crate::planner::names_no_package(verb)
            {
                return Err(DomainError::new(
                    ErrorId::PolicyDataInvalid,
                    format!(
                        "provider-commands.toml: entry {} gives {verb} the success semantic \
                         stdout_omits_package, but {verb} is a verb the planner appends the \
                         package name to. Absence is established by asking for the whole \
                         inventory; asked about one name, both providers answer \"absent\", \
                         \"never existed\" and \"could not run\" the same way",
                        e.provider
                    ),
                ));
            }
            match e.success.get(verb).map(String::as_str) {
                Some("exit_status" | "stdout_names_package" | "stdout_omits_package") => {}
                Some(other) => {
                    return Err(DomainError::new(
                        ErrorId::PolicyDataInvalid,
                        format!(
                            "provider-commands.toml: entry {} gives {verb} the success \
                             semantic {other:?}, which is not exit_status, \
                             stdout_names_package or stdout_omits_package; an unrecognised \
                             value silently becomes \"unknown\", which an executor must \
                             refuse to run",
                            e.provider
                        ),
                    ));
                }
                None => {
                    return Err(DomainError::new(
                        ErrorId::PolicyDataInvalid,
                        format!(
                            "provider-commands.toml: entry {} declares the verb {verb} but \
                             states no success semantic for it, so an executor would be left \
                             to assume exit status - which on this provider's verify verb \
                             can never fail (F-33, INV-01)",
                            e.provider
                        ),
                    ));
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
                 success = {{}}\n\
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
            // Not in this list until 2026-09-19, and its absence is worse than
            // the others': a missing verb falls through to `<verify_absent>` as
            // a literal argv element with `placeholder: false`, which the human
            // plan renders as a real argument to a real provider binary (F-20).
            // It fails closed only incidentally, because the missing success
            // cell makes the semantic `Unknown`. `add-provider` names no verbs,
            // so a third provider would have omitted it silently
            // (`security-reviewer`, 2026-09-19).
            assert!(
                e.verbs.contains_key("verify_absent"),
                "{id} lacks verify_absent, so a plan's VERIFY_REMOVAL would carry a \
                 literal <verify_absent> argument"
            );
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
    fn every_verb_states_how_its_success_is_established() {
        // The elevation check above has a sibling because the two omissions
        // fail in opposite directions. A missing elevation makes the planner
        // reach `Unknown`, which is *cautious*. A missing success semantic
        // leaves an executor to assume exit status, and on the MacPorts verify
        // verb that is a check which cannot fail while standing in front of
        // `REMOVE_SOURCE` (F-33, INV-01). Caution and data loss are not the
        // same kind of default.
        for key in [
            "verify = \"stdout_names_package\"",
            "verify = \"exit_status\"",
            "add = \"exit_status\"",
            "remove = \"exit_status\"",
        ] {
            let Err(err) = without(key) else {
                panic!("removing {key} must be refused");
            };
            assert_eq!(err.id, ErrorId::PolicyDataInvalid, "{key}");
            assert!(
                err.detail.contains("success semantic"),
                "{key}: {}",
                err.detail
            );
        }
    }

    #[test]
    fn a_success_semantic_this_build_does_not_recognise_is_refused() {
        // Same reasoning as the misspelt elevation, and worse: the planner
        // folds an unrecognised value to `SuccessSemantic::Unknown`, which an
        // executor must refuse - so a typo here turns into a plan that cannot
        // be run, discovered at execution rather than at build.
        let text = SOURCE.replacen(
            "verify = \"stdout_names_package\"",
            "verify = \"stdout_names_pacakge\"",
            1,
        );
        let err = parse(&text).expect_err("a misspelt success semantic must be refused");
        assert_eq!(err.id, ErrorId::PolicyDataInvalid);
        assert!(
            err.detail.contains("stdout_names_pacakge"),
            "{}",
            err.detail
        );
    }

    #[test]
    fn the_shipped_table_states_the_semantics_that_were_observed() {
        // Not a restatement of the file: these two values are the finding.
        // `port -q installed <name>` exits 0 whether the port is installed,
        // absent, or not a port at all; `brew list --versions <name>` exits 1
        // when the name is absent. Both observed on this machine 2026-09-08
        // (R-5 E22, F-33). If either cell is ever "corrected" to exit status,
        // this fails and says why.
        //
        // The two `verify_absent` cells joined them on 2026-09-19 and were not
        // added here until `spec-guardian` noticed: a test whose stated job is
        // to stop an observed semantic being "corrected" had been left covering
        // half of them. Its name said "one of the two" then, which is the
        // drifting-count class this pass spent the night on, one commit later.
        let table = load().unwrap();
        assert_eq!(
            table["macports"].success["verify"], "stdout_names_package",
            "MacPorts' verify verb cannot be driven by exit status"
        );
        assert_eq!(
            table["homebrew"].success["verify"], "exit_status",
            "Homebrew's can, and saying so is what makes the other cell a fact \
             rather than a blanket rule"
        );
        for provider in ["macports", "homebrew"] {
            assert_eq!(
                table[provider].success["verify_absent"], "stdout_omits_package",
                "{provider}: absence is never exit status and never a single-name query - \
                 asked about one name both providers answer removed, never-existed and \
                 could-not-run identically (observed 2026-09-19)"
            );
        }
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
