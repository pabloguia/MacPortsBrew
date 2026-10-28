// SPDX-License-Identifier: Apache-2.0

//! The JSON half of `PROVIDER_REPORTS_ACTIVE` (work order 16 part S, §3; R-20
//! findings 8-13, 20, 21): what a provider's JSON inventory says about one
//! package, as a `LinkedReport` the domain's `evaluate_with` judges.
//!
//! **A typed parse of the whole document, and nothing else.** The structs below
//! name the only fields read; everything else is ignored. `serde_json::from_str`
//! over the complete stdout refuses leading or trailing data, a duplicate field
//! the structs name, and a value of the wrong type — each is
//! [`LinkedReport::Unparseable`], never a guess. Stderr is never read. Nothing
//! the document says becomes evidence, a path or a flag: the answer is one of
//! four values, and `ReadEvidence` keeps only the line count, the truncation
//! flag and the cause (C3).
//!
//! **The rule.** The entries of `formulae` whose `name` equals the destination
//! name exactly: none is [`LinkedReport::NotLinked`] (not installed, or a
//! package kind the list does not hold); two or more is
//! [`LinkedReport::Ambiguous`]; exactly one is linked only when its
//! `linked_keg` equals one of **its own** `installed[].version` — never another
//! entry's, never a stable version the document advertises. `casks` must be
//! present and is never consulted: a cask has no record that means "linked"
//! (R-20 finding 13), so it cannot pass (SE-107).

use macportsbrew_domain::LinkedReport;
use serde::de::IgnoredAny;
use serde::Deserialize;

#[derive(Deserialize)]
struct Document {
    formulae: Vec<Formula>,
    #[serde(rename = "casks")]
    _casks: IgnoredAny,
}

#[derive(Deserialize)]
struct Formula {
    name: String,
    linked_keg: Option<String>,
    installed: Vec<Installed>,
}

#[derive(Deserialize)]
struct Installed {
    version: String,
}

/// What `stdout`, a JSON inventory, says about `name`.
#[must_use]
pub fn linked_report(stdout: &str, name: &str) -> LinkedReport {
    let Ok(document) = serde_json::from_str::<Document>(stdout) else {
        return LinkedReport::Unparseable;
    };
    let mut named = document.formulae.iter().filter(|f| f.name == name);
    let Some(entry) = named.next() else {
        return LinkedReport::NotLinked;
    };
    if named.next().is_some() {
        return LinkedReport::Ambiguous;
    }
    match &entry.linked_keg {
        Some(keg) if entry.installed.iter().any(|i| &i.version == keg) => LinkedReport::Linked,
        Some(_) | None => LinkedReport::NotLinked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document shaped like R-20 E7's: the five representative entries'
    /// fields that matter, beside fields this parse ignores.
    const DOCUMENT: &str = r#"{
      "formulae": [
        {"name": "gh", "full_name": "gh", "versions": {"stable": "2.98.0"},
         "linked_keg": "2.98.0", "keg_only": false,
         "installed": [{"version": "2.98.0", "installed_on_request": true}]},
        {"name": "argon2", "versions": {"stable": "20190702"},
         "linked_keg": "20190702_1", "keg_only": false,
         "installed": [{"version": "20190702_1"}]},
        {"name": "openssl@3", "linked_keg": "3.6.4", "keg_only": true,
         "installed": [{"version": "3.6.3"}, {"version": "3.6.4"}]},
        {"name": "openssl@4", "linked_keg": null, "keg_only": false,
         "installed": [{"version": "4.0.1"}, {"version": "4.0.2"}]},
        {"name": "curl", "linked_keg": null, "keg_only": true,
         "installed": [{"version": "8.21.0"}]}
      ],
      "casks": [{"token": "meld", "installed": "3.22.2", "version": "3.22.2"}]
    }"#;

    fn with(entries: &str) -> String {
        format!("{{\"formulae\": [{entries}], \"casks\": []}}")
    }

    #[test]
    fn linked_keg_equal_to_its_own_installed_version_is_linked() {
        assert_eq!(linked_report(DOCUMENT, "gh"), LinkedReport::Linked);
        // The revision suffix is part of the comparison: `versions.stable`
        // (`20190702`) is not what is linked.
        assert_eq!(linked_report(DOCUMENT, "argon2"), LinkedReport::Linked);
        let stable_only = with(
            r#"{"name": "argon2", "versions": {"stable": "20190702"},
                "linked_keg": "20190702", "installed": [{"version": "20190702_1"}]}"#,
        );
        assert_eq!(
            linked_report(&stable_only, "argon2"),
            LinkedReport::NotLinked,
            "a linked_keg matching versions.stable but no installed version is not linked"
        );
    }

    #[test]
    fn a_linked_keg_matching_another_entrys_installed_is_not_linked() {
        let doc = with(
            r#"{"name": "aa", "linked_keg": "2.0", "installed": [{"version": "1.0"}]},
               {"name": "bb", "linked_keg": null, "installed": [{"version": "2.0"}]}"#,
        );
        assert_eq!(linked_report(&doc, "aa"), LinkedReport::NotLinked);
    }

    #[test]
    fn unlinked_cask_missing_or_malformed_is_not_linked() {
        assert_eq!(
            linked_report(DOCUMENT, "openssl@4"),
            LinkedReport::NotLinked
        );
        assert_eq!(
            linked_report(DOCUMENT, "meld"),
            LinkedReport::NotLinked,
            "a cask"
        );
        assert_eq!(
            linked_report(DOCUMENT, "cowsay"),
            LinkedReport::NotLinked,
            "no entry"
        );
        let number = with(r#"{"name": "gh", "linked_keg": 2, "installed": [{"version": "2"}]}"#);
        assert_eq!(linked_report(&number, "gh"), LinkedReport::Unparseable);
        let version_number =
            with(r#"{"name": "gh", "linked_keg": "2", "installed": [{"version": 2}]}"#);
        assert_eq!(
            linked_report(&version_number, "gh"),
            LinkedReport::Unparseable
        );
        let truncated = &DOCUMENT[..DOCUMENT.len() / 2];
        assert_eq!(linked_report(truncated, "gh"), LinkedReport::Unparseable);
        assert_eq!(
            linked_report("gh 2.98.0\n", "gh"),
            LinkedReport::Unparseable
        );
        assert_eq!(
            linked_report(r#"{"formulae": []}"#, "gh"),
            LinkedReport::Unparseable,
            "a document without its casks list is not the shape this build knows"
        );
    }

    #[test]
    fn leading_or_trailing_text_is_unparseable() {
        assert_eq!(
            linked_report(&format!("Warning: x\n{DOCUMENT}"), "gh"),
            LinkedReport::Unparseable
        );
        assert_eq!(
            linked_report(&format!("{DOCUMENT}\n{{}}"), "gh"),
            LinkedReport::Unparseable
        );
        assert_eq!(
            linked_report(&format!("{DOCUMENT}\n"), "gh"),
            LinkedReport::Linked,
            "trailing whitespace is not data"
        );
    }

    #[test]
    fn a_duplicate_linked_keg_is_unparseable() {
        let doc = with(
            r#"{"name": "gh", "linked_keg": null, "linked_keg": "2.98.0",
                "installed": [{"version": "2.98.0"}]}"#,
        );
        assert_eq!(linked_report(&doc, "gh"), LinkedReport::Unparseable);
    }

    #[test]
    fn a_keg_only_formula_that_is_linked_is_active() {
        assert_eq!(linked_report(DOCUMENT, "openssl@3"), LinkedReport::Linked);
        assert_eq!(
            linked_report(DOCUMENT, "curl"),
            LinkedReport::NotLinked,
            "keg-only and unlinked halts (R-20 finding 10)"
        );
    }

    #[test]
    fn two_entries_of_one_name_are_ambiguous() {
        let doc = with(
            r#"{"name": "gh", "linked_keg": "1", "installed": [{"version": "1"}]},
               {"name": "gh", "linked_keg": "1", "installed": [{"version": "1"}]}"#,
        );
        assert_eq!(linked_report(&doc, "gh"), LinkedReport::Ambiguous);
    }

    #[test]
    fn a_linked_keg_spoofed_in_caveats_is_not_read() {
        let doc = with(
            r#"{"name": "gh", "caveats": "\"linked_keg\": \"2.98.0\"",
                "linked_keg": null, "installed": [{"version": "2.98.0"}]},
               {"name": "other", "caveats": "{\"name\": \"gh\", \"linked_keg\": \"2.98.0\"}",
                "linked_keg": null, "installed": []}"#,
        );
        assert_eq!(linked_report(&doc, "gh"), LinkedReport::NotLinked);
    }

    #[test]
    fn the_name_is_matched_exactly() {
        assert_eq!(linked_report(DOCUMENT, "GH"), LinkedReport::NotLinked);
        assert_eq!(linked_report(DOCUMENT, "openssl"), LinkedReport::NotLinked);
    }

    /// The JSON half of INV-17's one rule, the same four cases the domain's
    /// `invariant_17_exactly_one_record_marked_active_is_the_rule_for_both_providers`
    /// runs as plain-text rows: exactly one record marked active passes; one
    /// marked beside another installed version passes; none marked fails; two
    /// records of the name fail.
    #[test]
    fn invariant_17_exactly_one_record_marked_active_holds_for_the_json_parse() {
        let doc = |formulae: &str| format!(r#"{{"formulae": [{formulae}], "casks": []}}"#);
        let entry = |linked: &str, installed: &[&str]| {
            let installed: Vec<String> = installed
                .iter()
                .map(|v| format!(r#"{{"version": "{v}"}}"#))
                .collect();
            format!(
                r#"{{"name": "xz", "linked_keg": {linked}, "installed": [{}]}}"#,
                installed.join(", ")
            )
        };
        let one = doc(&entry("\"5.8.3\"", &["5.8.3"]));
        let beside = doc(&entry("\"5.8.3\"", &["5.8.2", "5.8.3"]));
        let none = doc(&entry("null", &["5.8.3"]));
        let two = doc(&format!(
            "{}, {}",
            entry("\"5.8.3\"", &["5.8.3"]),
            entry("\"5.8.3\"", &["5.8.3"])
        ));
        assert_eq!(linked_report(&one, "xz"), LinkedReport::Linked);
        assert_eq!(linked_report(&beside, "xz"), LinkedReport::Linked);
        assert_eq!(linked_report(&none, "xz"), LinkedReport::NotLinked);
        assert_eq!(linked_report(&two, "xz"), LinkedReport::Ambiguous);
    }
}
