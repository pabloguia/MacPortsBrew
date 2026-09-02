// SPDX-License-Identifier: Apache-2.0

//! The message format: named placeholders and a minimal ICU-style plural.
//!
//! The grammar is deliberately small — it is the smallest thing that satisfies
//! "do not concatenate sentence fragments" (`docs/spec/06-cli-and-ux.md`) for
//! the five target locales:
//!
//! ```text
//! message     := (text | escape | group)*
//! escape      := "{{" | "}}"
//! group       := "{" name "}"
//!              | "{" name "," "plural" "," arm+ "}"
//! arm         := selector "{" message "}"
//! selector    := "=" digits | "one" | "other"
//! ```
//!
//! Inside a plural arm, `#` renders the count. An `=N` arm wins over the
//! category arms, so "no packages" can be phrased as a sentence rather than
//! built from a zero and a plural noun.
//!
//! Malformed input degrades rather than panicking or vanishing: an unbalanced
//! brace, an unknown function and an absent parameter all render as the source
//! text, so the defect is visible on screen and in a bug report instead of
//! silently producing an empty sentence.

use std::collections::BTreeMap;

use super::locale::Locale;

/// Renders a template in a locale.
pub fn format_message(
    locale: &Locale,
    template: &str,
    params: &BTreeMap<String, String>,
) -> String {
    let chars: Vec<char> = template.chars().collect();
    render_chars(locale, &chars, params)
}

/// True when every `{` in the template has a matching `}`, counting `{{` and
/// `}}` as escapes. Used by the catalogue tests: a template with an unbalanced
/// brace renders its own source, which is a defect a test should catch and not
/// a user should read.
pub fn is_balanced(template: &str) -> bool {
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    let mut depth = 0i32;
    while i < chars.len() {
        match chars[i] {
            // Escapes are recognised at the top level only, exactly as the
            // renderer recognises them: inside a group, `}}` is a nested arm
            // closing and then the group closing, not a literal brace.
            '{' if depth == 0 && chars.get(i + 1) == Some(&'{') => i += 2,
            '}' if depth == 0 && chars.get(i + 1) == Some(&'}') => i += 2,
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    depth == 0
}

/// The index of the `}` that closes the `{` at `start`, or `None` when the
/// template is unbalanced.
///
/// Escapes are recognised at the top level only: within a group, `{` and `}`
/// are structure. That is what lets a plural arm carry nested groups without
/// the parser needing a full escape state machine.
pub fn matching_brace(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start) != Some(&'{') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, c) in chars.iter().enumerate().skip(start) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn render_chars(locale: &Locale, chars: &[char], params: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                out.push('{');
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                out.push('}');
                i += 2;
            }
            '{' => {
                if let Some(end) = matching_brace(chars, i) {
                    let inner: String = chars[i + 1..end].iter().collect();
                    out.push_str(&render_group(locale, &inner, params));
                    i = end + 1;
                } else {
                    // Unbalanced: show the source rather than swallowing it.
                    out.push('{');
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn render_group(locale: &Locale, inner: &str, params: &BTreeMap<String, String>) -> String {
    let Some((name, rest)) = inner.split_once(',') else {
        let name = inner.trim();
        return params
            .get(name)
            .cloned()
            .unwrap_or_else(|| format!("{{{name}}}"));
    };
    let name = name.trim();
    let (kind, arms) = rest.split_once(',').unwrap_or((rest, ""));
    if kind.trim() == "plural" {
        render_plural(locale, name, arms, params)
    } else {
        // An unknown function: the catalogue asked for something this build
        // does not implement. Render the source so it is reportable.
        format!("{{{inner}}}")
    }
}

fn render_plural(
    locale: &Locale,
    name: &str,
    arms_source: &str,
    params: &BTreeMap<String, String>,
) -> String {
    let raw = params.get(name).map_or("", String::as_str);
    let count: Option<u64> = raw.parse().ok();
    let arms = parse_arms(arms_source);
    if arms.is_empty() {
        return format!("{{{name}}}");
    }

    let exact = count.map(|n| format!("={n}"));
    let category = count.map_or("other", |n| locale.plural_category(n).selector());

    let body = exact
        .as_deref()
        .and_then(|selector| find_arm(&arms, selector))
        .or_else(|| find_arm(&arms, category))
        .or_else(|| find_arm(&arms, "other"))
        // A catalogue with neither the locale's category nor `other` is a
        // parity failure; render the first arm rather than nothing so the
        // sentence is still readable.
        .unwrap_or(arms[0].1.as_str());

    // `#` stands for the count. Substituting it before the recursive pass
    // keeps a value containing `#` from being re-substituted.
    let expanded = body.replace('#', raw);
    format_message(locale, &expanded, params)
}

fn find_arm<'a>(arms: &'a [(String, String)], selector: &str) -> Option<&'a str> {
    arms.iter()
        .find(|(s, _)| s == selector)
        .map(|(_, body)| body.as_str())
}

/// Parses `one {…} other {…}` into selector/body pairs, stopping at the first
/// thing that is not an arm.
fn parse_arms(source: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = source.chars().collect();
    let mut arms = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let selector_start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '{' {
            i += 1;
        }
        let selector: String = chars[selector_start..i].iter().collect();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if selector.is_empty() || chars.get(i) != Some(&'{') {
            break;
        }
        let Some(end) = matching_brace(&chars, i) else {
            break;
        };
        arms.push((selector, chars[i + 1..end].iter().collect()));
        i = end + 1;
    }
    arms
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn a_named_placeholder_is_substituted() {
        let en = Locale::english();
        assert_eq!(
            format_message(&en, "Reason {id}.", &params(&[("id", "tier3")])),
            "Reason tier3."
        );
    }

    #[test]
    fn several_placeholders_are_substituted_in_place() {
        let en = Locale::english();
        let out = format_message(
            &en,
            "verified on {readOn}; see {url}.",
            &params(&[("readOn", "2026-09-02"), ("url", "https://example.test")]),
        );
        assert_eq!(out, "verified on 2026-09-02; see https://example.test.");
    }

    #[test]
    fn an_absent_parameter_renders_its_own_name() {
        let en = Locale::english();
        assert_eq!(
            format_message(&en, "Reason {id}.", &BTreeMap::new()),
            "Reason {id}."
        );
    }

    #[test]
    fn braces_are_escaped_by_doubling() {
        let en = Locale::english();
        assert_eq!(
            format_message(&en, "a {{literal}} brace", &BTreeMap::new()),
            "a {literal} brace"
        );
        assert_eq!(
            format_message(&en, "{{{name}}}", &params(&[("name", "x")])),
            "{x}"
        );
    }

    #[test]
    fn an_unbalanced_brace_renders_its_source() {
        let en = Locale::english();
        assert_eq!(format_message(&en, "a { b", &BTreeMap::new()), "a { b");
        assert!(!is_balanced("a { b"));
        assert!(!is_balanced("a } b"));
        assert!(is_balanced("a {{ b }} c"));
        assert!(is_balanced("{count, plural, one {#} other {#}}"));
    }

    #[test]
    fn plural_selects_by_the_locale_rule() {
        let template = "{count, plural, one {# package} other {# packages}}";
        let en = Locale::english();
        assert_eq!(
            format_message(&en, template, &params(&[("count", "1")])),
            "1 package"
        );
        assert_eq!(
            format_message(&en, template, &params(&[("count", "2")])),
            "2 packages"
        );
        assert_eq!(
            format_message(&en, template, &params(&[("count", "0")])),
            "0 packages"
        );

        // Japanese has only `other`, and picking it is the correct answer for
        // every count rather than a fallback.
        let ja = Locale::new("ja");
        let japanese = "{count, plural, other {パッケージ # 件}}";
        assert_eq!(
            format_message(&ja, japanese, &params(&[("count", "1")])),
            "パッケージ 1 件"
        );
        assert_eq!(
            format_message(&ja, japanese, &params(&[("count", "7")])),
            "パッケージ 7 件"
        );
    }

    #[test]
    fn an_exact_arm_wins_over_the_category() {
        let template = "{count, plural, =0 {no findings} one {# finding} other {# findings}}";
        let en = Locale::english();
        assert_eq!(
            format_message(&en, template, &params(&[("count", "0")])),
            "no findings"
        );
        assert_eq!(
            format_message(&en, template, &params(&[("count", "1")])),
            "1 finding"
        );
        assert_eq!(
            format_message(&en, template, &params(&[("count", "3")])),
            "3 findings"
        );
    }

    #[test]
    fn portuguese_takes_the_singular_at_zero() {
        let template = "{count, plural, one {# pacote} other {# pacotes}}";
        let pt = Locale::new("pt-BR");
        assert_eq!(
            format_message(&pt, template, &params(&[("count", "0")])),
            "0 pacote"
        );
        assert_eq!(
            format_message(&pt, template, &params(&[("count", "2")])),
            "2 pacotes"
        );
    }

    #[test]
    fn a_plural_arm_may_carry_its_own_placeholder() {
        let template = "{count, plural, one {# under {provider}} other {# under {provider}}}";
        let en = Locale::english();
        assert_eq!(
            format_message(
                &en,
                template,
                &params(&[("count", "2"), ("provider", "macports")])
            ),
            "2 under macports"
        );
    }

    #[test]
    fn a_missing_category_falls_back_to_other() {
        let template = "{count, plural, other {# things}}";
        let en = Locale::english();
        assert_eq!(
            format_message(&en, template, &params(&[("count", "1")])),
            "1 things"
        );
    }

    #[test]
    fn a_non_numeric_count_takes_the_other_arm() {
        let template = "{count, plural, one {# package} other {# packages}}";
        let en = Locale::english();
        assert_eq!(
            format_message(&en, template, &params(&[("count", "many")])),
            "many packages"
        );
    }

    #[test]
    fn an_unknown_function_renders_its_source() {
        let en = Locale::english();
        assert_eq!(
            format_message(&en, "{n, select, a {x}}", &params(&[("n", "a")])),
            "{n, select, a {x}}"
        );
    }
}
