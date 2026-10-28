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
    /// Read date - the newest read in the entry, which the loader holds every
    /// cell's own date to.
    pub read_on: String,
    /// Where one cell was read, for a cell whose evidence is not the entry's
    /// (F-35). Keyed by table (`elevation`, `success`), then by the cell's key.
    ///
    /// An entry's `source` describes the entry, and two cells had come to rest
    /// on reads the entry-level fields could only describe in prose: MacPorts'
    /// `verify` elevation on an observation dated after the documentation the
    /// entry cited, Homebrew's on a commit of `brew.sh` where the entry cited
    /// the FAQ. The comments said so; INV-20 wants provenance in a field, and
    /// the loader checks fields.
    #[serde(default)]
    pub provenance: BTreeMap<String, BTreeMap<String, CellProvenance>>,
}

/// One cell's `source` and `read_on`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CellProvenance {
    /// What was read, precisely enough to read it again.
    pub source: String,
    /// When, `YYYY-MM-DD`.
    pub read_on: String,
}

/// `YYYY-MM-DD`, with a month and a day in range - which is what makes two
/// dates comparable as strings. Not a calendar: `2026-02-31` passes, and the
/// comparison it serves is unaffected by that.
fn is_iso_date(s: &str) -> bool {
    let shaped = s.len() == 10
        && s.char_indices().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    shaped && ("01"..="12").contains(&&s[5..7]) && ("01"..="31").contains(&&s[8..10])
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

/// The operations a `VERIFY_DESTINATION` reads, in order: `verify`, then
/// `verify_active` when the provider's entry holds that verb (work order 16
/// part S, §3, V2).
///
/// **The one rule the planner and the execution gate share.** The planner
/// builds the step's commands from it and the gate's pin derives the expected
/// list from it, element by element and in length, so the two cannot drift.
/// Without `verify_active` in the table — every table this build ships — the
/// list is `[verify]`, which is exactly what both did before.
#[must_use]
pub fn verify_destination_operations(entry: Option<&ProviderCommands>) -> Vec<&'static str> {
    let mut operations = vec!["verify"];
    if entry.is_some_and(|e| e.verbs.contains_key("verify_active")) {
        operations.push("verify_active");
    }
    operations
}

/// What a `VERIFY_DESTINATION` of `name` toward `provider` carries, command by
/// command: the argv and the success semantic the planner writes for each of
/// [`verify_destination_operations`], with the operation it came from.
#[must_use]
pub fn verify_destination_reads(
    table: &BTreeMap<String, ProviderCommands>,
    provider: &str,
    name: &str,
) -> Vec<(
    &'static str,
    Vec<String>,
    macportsbrew_domain::SuccessSemantic,
)> {
    let entry = table.get(provider);
    verify_destination_operations(entry)
        .into_iter()
        .map(|operation| {
            (
                operation,
                crate::planner::planned_argv(entry, operation, name),
                crate::planner::planned_success(entry, operation),
            )
        })
        .collect()
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
        if e.source.trim().is_empty() || e.read_on.is_empty() {
            return Err(DomainError::new(
                ErrorId::PolicyDataInvalid,
                format!(
                    "provider-commands.toml: entry {} lacks source or read_on",
                    e.provider
                ),
            ));
        }
        // The entry's own date is the one every cell's is compared against
        // below; unchecked, `"Sept 19"` sorts after every real date and the
        // "newer than its entry" refusal could never fire (spec-guardian,
        // 2026-09-25).
        if !is_iso_date(&e.read_on) {
            return Err(DomainError::new(
                ErrorId::PolicyDataInvalid,
                format!(
                    "provider-commands.toml: entry {} has read_on {:?}, which is not \
                     YYYY-MM-DD",
                    e.provider, e.read_on
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
        // A verification verb is a read, and nothing said so (F-53).
        //
        // Every check above is about a cell's *shape*; none asked what the verb
        // under `verify` or `verify_absent` does. `verify = ["install"]` loaded
        // clean, planned a VERIFY_DESTINATION carrying an install command, and
        // passed `check_ordering` and every predicate the executor's refusal
        // gate was going to apply - so a loop that runs verification steps
        // would have run it. Compared by word rather than by argv, because a
        // mutating verb with a different flag beside it is the same mutation.
        //
        // This refuses the one overlap the table itself can name. A read verb
        // that is some *other* mutation is the runner's to refuse - its
        // subcommand allowlist beside `PERMITTED_PROGRAMS` holds reads only -
        // and that is the second half of this finding, not a gap in this one.
        let words = |op: &str| -> BTreeSet<&str> {
            e.verbs
                .get(op)
                .into_iter()
                .flatten()
                .map(String::as_str)
                .filter(|w| !w.starts_with('-'))
                .collect()
        };
        let mutating: BTreeSet<&str> = words("add").union(&words("remove")).copied().collect();
        for read in ["verify", "verify_absent", "verify_active"] {
            if let Some(word) = words(read).intersection(&mutating).next() {
                return Err(DomainError::new(
                    ErrorId::PolicyDataInvalid,
                    format!(
                        "provider-commands.toml: entry {} gives {read} the word {word:?}, \
                         which is this entry's add or remove verb. A verification step is \
                         a read; one that installs or removes would be run by a loop that \
                         runs verification steps (F-53)",
                        e.provider
                    ),
                ));
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
            // The same coupling for the two "active" semantics (work order 16
            // part S, V1): each reads a whole inventory and judges one row or
            // entry of it, so a verb that appends the package name would hand
            // the judgement an answer to a different question.
            if let Some(
                semantic @ ("stdout_marks_package_active" | "json_reports_package_linked"),
            ) = e.success.get(verb).map(String::as_str)
            {
                if verb != "verify_active" || !crate::planner::names_no_package(verb) {
                    return Err(DomainError::new(
                        ErrorId::PolicyDataInvalid,
                        format!(
                            "provider-commands.toml: entry {} gives {verb} the success semantic \
                             {semantic}, but {verb} is a verb the planner appends the package \
                             name to. An active read asks for the whole inventory and finds the \
                             package's record in it",
                            e.provider
                        ),
                    ));
                }
            }
            if verb == "verify_active"
                && !matches!(
                    e.success.get(verb).map(String::as_str),
                    Some("stdout_marks_package_active" | "json_reports_package_linked")
                )
            {
                return Err(DomainError::new(
                    ErrorId::PolicyDataInvalid,
                    format!(
                        "provider-commands.toml: entry {} gives verify_active a success semantic \
                         that is not stdout_marks_package_active or json_reports_package_linked; \
                         the active read answers PROVIDER_REPORTS_ACTIVE and nothing else does",
                        e.provider
                    ),
                ));
            }
            match e.success.get(verb).map(String::as_str) {
                Some(
                    "exit_status"
                    | "stdout_names_package"
                    | "stdout_omits_package"
                    | "stdout_marks_package_active"
                    | "json_reports_package_linked",
                ) => {}
                Some(other) => {
                    return Err(DomainError::new(
                        ErrorId::PolicyDataInvalid,
                        format!(
                            "provider-commands.toml: entry {} gives {verb} the success \
                             semantic {other:?}, which is not exit_status, \
                             stdout_names_package, stdout_omits_package, \
                             stdout_marks_package_active or json_reports_package_linked; an \
                             unrecognised \
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
        // A cell's provenance must describe a cell, say what was read and when,
        // and not be newer than the entry (F-35). Each of the three is the way
        // a provenance row goes quietly wrong: a misspelt key that attaches a
        // citation to nothing while the cell it meant keeps the entry's, a row
        // that is a citation in name only, and an entry `read_on` that stops
        // being the newest read in it - which `policy.toml`'s version is built
        // from.
        for (table, cells) in &e.provenance {
            let values = match table.as_str() {
                "elevation" => &e.elevation,
                "success" => &e.success,
                other => {
                    return Err(DomainError::new(
                        ErrorId::PolicyDataInvalid,
                        format!(
                            "provider-commands.toml: entry {} gives provenance for the table \
                             {other:?}, which has no cells; only elevation and success do",
                            e.provider
                        ),
                    ));
                }
            };
            for (key, cell) in cells {
                let refuse = |why: &str| {
                    Err(DomainError::new(
                        ErrorId::PolicyDataInvalid,
                        format!(
                            "provider-commands.toml: entry {} gives provenance for \
                             {table}.{key}, which {why}",
                            e.provider
                        ),
                    ))
                };
                if !values.contains_key(key) {
                    return refuse("is not a cell of that table, so it describes nothing");
                }
                if cell.source.trim().is_empty() || !is_iso_date(&cell.read_on) {
                    return refuse("lacks a source or a YYYY-MM-DD read_on");
                }
                if cell.read_on > e.read_on {
                    return refuse(
                        "was read after the entry's read_on - that field is the newest \
                         read in the entry, and the policy version is built from it",
                    );
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

    /// The shipped table with a `verify_active` verb added to the MacPorts
    /// entry: `verb` as its argv, `success` as its semantic, and the elevation
    /// cells `elevation` (each a whole line). A test-only shape; no table this
    /// build ships holds the verb (work order 16 part S, §10).
    fn with_verify_active(verb: &str, success: &str, elevation: &[&str]) -> String {
        let text = SOURCE.replacen(
            "verify_absent = [\"-q\", \"installed\"]",
            &format!("verify_absent = [\"-q\", \"installed\"]\nverify_active = {verb}"),
            1,
        );
        // The MacPorts entry's success cell is the second occurrence.
        let at = text
            .match_indices("verify_absent = \"stdout_omits_package\"")
            .nth(1)
            .expect("the MacPorts success cell")
            .0;
        let mut text = text;
        text.insert_str(at, &format!("verify_active = {success}\n"));
        text.replacen(
            "verify_absent_gui_app = \"unknown\"",
            &format!(
                "verify_absent_gui_app = \"unknown\"\n{}",
                elevation.join("\n")
            ),
            1,
        )
    }

    const ACTIVE_ELEVATION: &[&str] = &[
        "verify_active = \"none\"",
        "verify_active_gui_app = \"unknown\"",
    ];

    #[test]
    fn the_new_semantics_are_accepted_on_a_no_package_verb() {
        for success in [
            "\"stdout_marks_package_active\"",
            "\"json_reports_package_linked\"",
        ] {
            let text = with_verify_active("[\"-q\", \"installed\"]", success, ACTIVE_ELEVATION);
            let table = parse(&text).unwrap_or_else(|e| panic!("{success}: {e:?}"));
            let entry = table.get("macports");
            assert_eq!(
                verify_destination_operations(entry),
                vec!["verify", "verify_active"]
            );
            let reads = verify_destination_reads(&table, "macports", "xz");
            assert_eq!(reads.len(), 2);
            assert_eq!(reads[1].1, vec!["-q".to_string(), "installed".to_string()]);
        }
    }

    #[test]
    fn the_new_semantics_require_a_no_package_verb() {
        for semantic in ["stdout_marks_package_active", "json_reports_package_linked"] {
            let text = SOURCE.replacen(
                "verify = \"stdout_names_package\"",
                &format!("verify = \"{semantic}\""),
                1,
            );
            let err = parse(&text).expect_err("a name-taking verb with an active semantic");
            assert_eq!(err.id, ErrorId::PolicyDataInvalid);
            assert!(err.detail.contains(semantic), "{}", err.detail);
            assert!(err.detail.contains("appends the package"), "{}", err.detail);
        }
    }

    #[test]
    fn verify_active_takes_an_active_semantic_and_only_it_does() {
        // An installed read on the active verb proves presence, not activation.
        let text = with_verify_active(
            "[\"-q\", \"installed\"]",
            "\"stdout_names_package\"",
            ACTIVE_ELEVATION,
        );
        let err = parse(&text).expect_err("verify_active with an installed semantic");
        assert!(err.detail.contains("verify_active"), "{}", err.detail);
        // And an active semantic on the removal inventory is refused too.
        let text = SOURCE.replacen(
            "verify_absent = \"stdout_omits_package\"",
            "verify_absent = \"stdout_marks_package_active\"",
            1,
        );
        assert!(parse(&text).is_err(), "an active semantic on verify_absent");
    }

    #[test]
    fn verify_active_requires_its_gui_app_elevation_cell() {
        let text = with_verify_active(
            "[\"-q\", \"installed\"]",
            "\"stdout_marks_package_active\"",
            &["verify_active = \"none\""],
        );
        let err = parse(&text).expect_err("verify_active without verify_active_gui_app");
        assert_eq!(err.id, ErrorId::PolicyDataInvalid);
        assert!(
            err.detail.contains("verify_active_gui_app"),
            "{}",
            err.detail
        );
    }

    #[test]
    fn verify_active_sharing_a_word_with_a_mutation_is_refused() {
        let text = with_verify_active(
            "[\"-q\", \"uninstall\"]",
            "\"stdout_marks_package_active\"",
            ACTIVE_ELEVATION,
        );
        let err = parse(&text).expect_err("verify_active naming the remove verb");
        assert!(err.detail.contains("F-53"), "{}", err.detail);
    }

    #[test]
    fn the_shipped_table_derives_verify_alone() {
        // S-1 is inert: no shipped entry holds `verify_active`, so every
        // VERIFY_DESTINATION reads exactly what it read before (§10).
        let table = load().expect("the shipped table parses");
        for (id, entry) in &table {
            assert_eq!(
                verify_destination_operations(Some(entry)),
                vec!["verify"],
                "{id}"
            );
        }
        assert_eq!(verify_destination_operations(None), vec!["verify"]);
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
    fn invariant_01_a_verification_verb_that_is_the_entrys_own_mutation_is_refused() {
        // F-53, as the finding states it: the shipped table with one read verb
        // replaced by the same entry's add or remove verb. The replacement is
        // read out of the table rather than written here, which is both the
        // honest case - the entry's own mutation - and what keeps the literal
        // out of `src/`, where the Stop gate greps for it.
        let table = load().unwrap();
        for (provider, shipped) in [
            ("homebrew", "[\"list\", \"--versions\"]"),
            ("macports", "[\"-q\", \"installed\"]"),
        ] {
            for read in ["verify", "verify_absent"] {
                for op in ["add", "remove"] {
                    let word = &table[provider].verbs[op][0];
                    let line = format!("{read} = {shipped}");
                    // Each provider's argv is distinct, so the first match is
                    // that provider's line and no other.
                    assert!(SOURCE.contains(&line), "{line} is not in the table");
                    let text = SOURCE.replacen(&line, &format!("{read} = [{word:?}]"), 1);
                    let err = parse(&text).expect_err(&format!(
                        "{provider}: {read} = [<{op} verb>] must be refused"
                    ));
                    assert_eq!(err.id, ErrorId::PolicyDataInvalid);
                    assert!(
                        err.detail.contains("F-53") && err.detail.contains(read),
                        "{provider} {read} {op}: {}",
                        err.detail
                    );
                    // A flag beside it is the same mutation.
                    let text = SOURCE.replacen(
                        &line,
                        &format!("{read} = [\"--quiet\", {word:?}, \"x\"]"),
                        1,
                    );
                    assert!(parse(&text).is_err(), "{provider} {read} {op} with a flag");
                }
            }
        }
    }

    /// `(cell, read_on)` pairs for one provider's table.
    type Rows<'a> = &'a [(&'a str, &'a str)];

    #[test]
    fn invariant_20_every_cell_with_its_own_evidence_says_so_in_a_field() {
        // F-35's second half. Both cells used to be explained in comments and in
        // a mixed entry-level `source`; if either loses its field, this says so.
        let table = load().unwrap();
        for (provider, read_on, cites) in [
            (
                "homebrew",
                "2026-09-04",
                "560147012b9678b42ef5e83b690f0895552d1366",
            ),
            ("macports", "2026-09-08", "UNRESOLVED-1"),
        ] {
            let cell = &table[provider].provenance["elevation"]["verify"];
            assert_eq!(cell.read_on, read_on, "{provider}");
            assert!(cell.source.contains(cites), "{provider}: {}", cell.source);
            assert!(cell.read_on <= table[provider].read_on, "{provider}");
        }
        // The same read with one argument fewer, on the same evidence; the entry
        // `source` no longer carries it, so without this row it would carry
        // nothing specific (spec-guardian, 2026-09-25).
        assert_eq!(
            table["macports"].provenance["elevation"]["verify_absent"],
            table["macports"].provenance["elevation"]["verify"]
        );
        // Every other cell whose evidence is dated apart from its entry
        // (SE-091), with the date it rests on - a key alone would let a wrong
        // date through (spec-guardian, 2026-09-25). Homebrew's `success.add`
        // and `success.remove` are absent on purpose: no artefact records the
        // Manpage stating an exit status for either.
        let expected: &[(&str, &str, Rows)] = &[
            (
                "homebrew",
                "elevation",
                &[
                    ("verify", "2026-09-04"),
                    ("verify_absent", "2026-09-04"),
                    ("verify_absent_gui_app", "2026-09-04"),
                    ("verify_gui_app", "2026-09-04"),
                ],
            ),
            (
                "homebrew",
                "success",
                &[("verify", "2026-09-06"), ("verify_absent", "2026-09-19")],
            ),
            (
                "macports",
                "elevation",
                &[("verify", "2026-09-08"), ("verify_absent", "2026-09-08")],
            ),
            (
                "macports",
                "success",
                &[
                    ("add", "2026-09-02"),
                    ("remove", "2026-09-02"),
                    ("verify", "2026-09-08"),
                    ("verify_absent", "2026-09-19"),
                ],
            ),
        ];
        for (provider, table_name, rows) in expected {
            let have: Vec<(&str, &str)> = table[*provider].provenance[*table_name]
                .iter()
                .map(|(k, c)| (k.as_str(), c.read_on.as_str()))
                .collect();
            assert_eq!(have, rows.to_vec(), "{provider}.{table_name}");
        }
    }

    #[test]
    fn invariant_20_a_provenance_row_that_describes_no_cell_is_refused() {
        // A misspelt key would attach the citation to nothing while the cell it
        // meant silently kept the entry's - the exact state F-35 closed.
        for (from, to) in [
            (
                "\n[entry.provenance.elevation.verify]\n",
                "\n[entry.provenance.elevation.verfy]\n",
            ),
            (
                "\n[entry.provenance.elevation.verify]\n",
                "\n[entry.provenance.elevaton.verify]\n",
            ),
        ] {
            // Anchored on the line: the header is also named, in backticks, in
            // the comments that point at it.
            assert!(SOURCE.contains(from));
            let err = parse(&SOURCE.replacen(from, to, 1)).expect_err(to);
            assert_eq!(err.id, ErrorId::PolicyDataInvalid, "{to}");
            assert!(err.detail.contains("provenance"), "{to}: {}", err.detail);
        }
    }

    /// The shipped table with the `read_on` of Homebrew's `verify` elevation
    /// row replaced - located by its header, because a row's position in the
    /// entry is not something a test should depend on.
    fn with_homebrew_verify_read_on(date: &str) -> String {
        let header = "\n[entry.provenance.elevation.verify]\nsource = \"Homebrew/brew";
        let start = SOURCE.find(header).expect("the Homebrew verify row");
        let line = "read_on = \"2026-09-04\"";
        let at = start + SOURCE[start..].find(line).expect("its read_on");
        format!(
            "{}read_on = \"{date}\"{}",
            &SOURCE[..at],
            &SOURCE[at + line.len()..]
        )
    }

    #[test]
    fn invariant_20_a_provenance_row_without_a_source_or_a_date_is_refused() {
        for bad in [
            "",
            "Sept 4",
            "2026-9-4",
            "2026/09/04",
            "2026-13-04",
            "2026-09-00",
        ] {
            let text = with_homebrew_verify_read_on(bad);
            let err = parse(&text).expect_err(&format!("read_on {bad:?}"));
            assert!(err.detail.contains("YYYY-MM-DD"), "{bad}: {}", err.detail);
        }
        let text = SOURCE.replacen(
            "[entry.provenance.elevation.verify]\nsource = \"Homebrew/brew",
            "[entry.provenance.elevation.verify]\nsource = \"\"\nx_was = \"",
            1,
        );
        // `deny_unknown_fields` refuses the stray key as well; either refusal
        // is a refusal, and the point is that an empty source does not load.
        assert!(parse(&text).is_err(), "an empty source loaded");
        let text = SOURCE.replacen(
            "[entry.provenance.elevation.verify]\nsource = \"Homebrew/brew",
            "[entry.provenance.elevation.verify]\nsource = \"\"\n#",
            1,
        );
        let err = parse(&text).expect_err("an empty source must be refused");
        assert!(err.detail.contains("lacks a source"), "{}", err.detail);
    }

    #[test]
    fn invariant_20_an_entry_read_on_that_is_not_a_date_is_refused() {
        // The comparison below it is only sound when both sides are dates.
        let line = "read_on = \"2026-09-19\"";
        assert!(SOURCE.contains(line));
        for bad in ["Sept 19", "2026-9-19", "2026-19-09"] {
            let text = SOURCE.replacen(line, &format!("read_on = \"{bad}\""), 1);
            let err = parse(&text).expect_err(bad);
            assert!(err.detail.contains("YYYY-MM-DD"), "{bad}: {}", err.detail);
        }
        let text = SOURCE.replacen(
            "\n[entry.provenance.elevation.verify]\nsource = \"Homebrew/brew",
            "\n[entry.provenance.elevation.verify]\nsource = \"   \"\n#",
            1,
        );
        assert!(parse(&text).is_err(), "a whitespace-only source loaded");
    }

    #[test]
    fn invariant_20_a_provenance_row_newer_than_its_entry_is_refused() {
        // The entry's `read_on` is the newest read in it, and `policy.toml`'s
        // version is built from the newest across the directory. A cell dated
        // after its entry would make both quietly wrong.
        let text = with_homebrew_verify_read_on("2026-09-20");
        let err = parse(&text).expect_err("a cell newer than its entry must be refused");
        assert!(err.detail.contains("after the entry"), "{}", err.detail);
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
