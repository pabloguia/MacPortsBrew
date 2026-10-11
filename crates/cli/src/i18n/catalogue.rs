// SPDX-License-Identifier: Apache-2.0

//! The message catalogue: a sorted map from stable identifier to template.

use macportsbrew_domain::{DomainError, ErrorId};
use std::collections::BTreeMap;

use super::format::matching_brace;

/// The English source catalogue, compiled into the binary.
///
/// Embedding it is deliberate: the tool must be able to explain a failure
/// before it has read anything from disk, and a catalogue that could go
/// missing at runtime would make the error taxonomy's remediation strings
/// conditional on the filesystem.
const EN_TOML: &str = include_str!("../../../../locales/en.toml");

/// A set of message templates, keyed by stable identifier.
///
/// The map is a `BTreeMap`, so iteration order is the identifiers' order on
/// every machine and every run: catalogue dumps and parity diagnostics are
/// reproducible (`docs/spec/07-quality-and-testing.md`, determinism).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    entries: BTreeMap<String, String>,
}

impl Catalogue {
    /// The English source catalogue.
    ///
    /// # Panics
    ///
    /// If the embedded `locales/en.toml` is not a flat table of strings. That
    /// is a build defect rather than a runtime condition — the file is part of
    /// the binary — and `english_parses` in this module fails first.
    pub fn english() -> Self {
        Self::from_toml(EN_TOML).expect("the embedded English catalogue must parse")
    }

    /// The pseudolocale, derived from English.
    ///
    /// Every string is accented and lengthened by roughly a third and wrapped
    /// in brackets, so that a layout which only fits English is visibly wrong
    /// and a string that escaped the catalogue is visibly bare. Placeholders
    /// and plural syntax are copied through untouched: the point is to test
    /// layout, and a pseudolocale that corrupted `{count, plural, …}` would
    /// test the renderer's tolerance of broken input instead.
    pub fn pseudolocale() -> Self {
        let english = Self::english();
        Self {
            entries: english
                .entries
                .iter()
                .map(|(key, value)| (key.clone(), pseudo_value(value)))
                .collect(),
        }
    }

    /// Parses a catalogue from TOML.
    ///
    /// Dotted keys nest in TOML, so `cli.about = "…"` arrives as a table; the
    /// nesting is flattened back to the dotted identifier the rest of the
    /// system uses. A non-string leaf is rejected rather than stringified,
    /// because a number or a boolean in a catalogue is a translator's mistake
    /// that would otherwise reach a user as text.
    pub fn from_toml(source: &str) -> Result<Self, DomainError> {
        let table: toml::Table = source.parse().map_err(|e| {
            DomainError::new(ErrorId::Internal, format!("catalogue is not TOML: {e}"))
        })?;
        let mut entries = BTreeMap::new();
        flatten(&table, &mut String::new(), &mut entries)?;
        Ok(Self { entries })
    }

    /// The template for an identifier.
    pub fn get(&self, id: &str) -> Option<&str> {
        self.entries.get(id).map(String::as_str)
    }

    /// Every identifier, in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the catalogue carries nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Flattens TOML's nested tables back into dotted identifiers.
fn flatten(
    table: &toml::Table,
    prefix: &mut String,
    out: &mut BTreeMap<String, String>,
) -> Result<(), DomainError> {
    for (key, value) in table {
        let restore = prefix.len();
        if !prefix.is_empty() {
            prefix.push('.');
        }
        prefix.push_str(key);
        match value {
            toml::Value::String(text) => {
                out.insert(prefix.clone(), text.clone());
            }
            toml::Value::Table(nested) => flatten(nested, prefix, out)?,
            other => {
                return Err(DomainError::new(
                    ErrorId::Internal,
                    format!(
                        "catalogue entry {prefix} is a {} where a string was expected",
                        other.type_str()
                    ),
                ));
            }
        }
        prefix.truncate(restore);
    }
    Ok(())
}

/// The accent applied to one ASCII letter. Latin-1 and Latin Extended-A only,
/// so the pseudolocale stays single-width and does not accidentally test
/// double-width handling as well.
fn accent(c: char) -> char {
    match c {
        'a' => 'á',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'ð',
        'e' => 'é',
        'f' => 'ƒ',
        'g' => 'ǵ',
        'h' => 'ĥ',
        'i' => 'í',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ĺ',
        'm' => 'ḿ',
        'n' => 'ñ',
        'o' => 'ø',
        'p' => 'þ',
        'q' => 'ɋ',
        'r' => 'ŕ',
        's' => 'š',
        't' => 'ţ',
        'u' => 'ü',
        'v' => 'ṽ',
        'w' => 'ŵ',
        'x' => 'ẋ',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Á',
        'B' => 'Ɓ',
        'C' => 'Ç',
        'D' => 'Ð',
        'E' => 'É',
        'F' => 'Ƒ',
        'G' => 'Ǵ',
        'H' => 'Ĥ',
        'I' => 'Í',
        'J' => 'Ĵ',
        'K' => 'Ķ',
        'L' => 'Ĺ',
        'M' => 'Ḿ',
        'N' => 'Ñ',
        'O' => 'Ø',
        'P' => 'Þ',
        'Q' => 'Ɋ',
        'R' => 'Ŕ',
        'S' => 'Š',
        'T' => 'Ţ',
        'U' => 'Ü',
        'V' => 'Ṽ',
        'W' => 'Ŵ',
        'X' => 'Ẋ',
        'Y' => 'Ý',
        'Z' => 'Ž',
        other => other,
    }
}

/// Accents and lengthens one template, leaving every `{…}` group alone.
fn pseudo_value(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut body = String::new();
    let mut translatable = 0usize;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' if chars.get(i + 1) == Some(&'{') => {
                body.push_str("{{");
                i += 2;
            }
            '}' if chars.get(i + 1) == Some(&'}') => {
                body.push_str("}}");
                i += 2;
            }
            '{' => {
                // A placeholder or a plural: copied through verbatim, so the
                // pseudolocale substitutes exactly as English does.
                let end = matching_brace(&chars, i).unwrap_or(chars.len() - 1);
                body.extend(chars[i..=end].iter());
                i = end + 1;
            }
            c => {
                body.push(accent(c));
                if !c.is_whitespace() {
                    translatable += 1;
                }
                i += 1;
            }
        }
    }
    // Roughly a third longer, and never shorter than the brackets alone: a
    // template that is nothing but a placeholder still has to grow, or the
    // layout test it exists for would pass by accident.
    let padding = (translatable * 35 / 100).max(1);
    let mut out = String::with_capacity(body.len() + padding + 2);
    out.push('[');
    out.push_str(&body);
    for _ in 0..padding {
        out.push('~');
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Templates are trusted text and `Messages::render` does not escape them,
    /// so none may carry a hostile character (`macportsbrew_domain::text`): a
    /// right-to-left mark or an embedded line break added to a template must
    /// fail here rather than reorder or break what a user reads. A template
    /// that genuinely needs one is listed here, by key and character, with
    /// the reason beside it.
    #[test]
    fn no_template_carries_a_hostile_character() {
        const ALLOWED: &[(&str, char)] = &[];
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../locales");
        let mut catalogues = vec![("pseudolocale".to_string(), Catalogue::pseudolocale())];
        for entry in std::fs::read_dir(&locales).expect("locales/ is readable") {
            let path = entry.expect("a directory entry").path();
            if path.extension().is_some_and(|e| e == "toml") {
                let text = std::fs::read_to_string(&path).expect("a readable catalogue");
                let catalogue = Catalogue::from_toml(&text).expect("a catalogue that parses");
                catalogues.push((path.display().to_string(), catalogue));
            }
        }
        assert!(
            catalogues.len() >= 2,
            "no catalogue was read from {}",
            locales.display()
        );
        let mut offenders = Vec::new();
        for (name, catalogue) in &catalogues {
            for key in catalogue.keys() {
                let template = catalogue.get(key).unwrap_or_default();
                for c in template.chars() {
                    if macportsbrew_domain::is_hostile(c) && !ALLOWED.contains(&(key, c)) {
                        offenders.push(format!("{name}: {key}: {}", c.escape_unicode()));
                    }
                }
            }
        }
        assert!(offenders.is_empty(), "{}", offenders.join("\n"));
    }

    #[test]
    fn english_parses() {
        let catalogue = Catalogue::english();
        assert!(!catalogue.is_empty());
        assert!(catalogue.get("cli.about").is_some());
    }

    #[test]
    fn dotted_keys_flatten_rather_than_nest() {
        let catalogue = Catalogue::from_toml("a.b.c = \"x\"\nd = \"y\"\n").unwrap();
        assert_eq!(catalogue.get("a.b.c"), Some("x"));
        assert_eq!(catalogue.get("d"), Some("y"));
        assert_eq!(catalogue.len(), 2);
    }

    #[test]
    fn a_non_string_leaf_is_rejected() {
        let error = Catalogue::from_toml("a.b = 3\n").unwrap_err();
        assert_eq!(error.id, ErrorId::Internal);
        assert!(error.detail.contains("a.b"), "{}", error.detail);
    }

    #[test]
    fn malformed_toml_is_an_error_not_a_panic() {
        assert!(Catalogue::from_toml("a = \n").is_err());
    }

    #[test]
    fn the_pseudolocale_has_english_keys_exactly() {
        let english = Catalogue::english();
        let pseudo = Catalogue::pseudolocale();
        let english_keys: Vec<&str> = english.keys().collect();
        let pseudo_keys: Vec<&str> = pseudo.keys().collect();
        assert_eq!(english_keys, pseudo_keys);
    }

    #[test]
    fn the_pseudolocale_accents_text_and_preserves_placeholders() {
        let pseudo = Catalogue::pseudolocale();
        let notice = pseudo.get("plan.heading").unwrap();
        assert!(notice.starts_with('['), "{notice}");
        assert!(notice.ends_with(']'), "{notice}");
        assert!(notice.contains("{package}"), "{notice}");
        assert!(notice.contains("{destination}"), "{notice}");
        // Accented, so an untranslated string is obvious on screen.
        assert!(notice.contains('ø') || notice.contains('á'), "{notice}");
    }

    #[test]
    fn the_pseudolocale_leaves_plural_syntax_alone() {
        let pseudo = Catalogue::pseudolocale();
        let packages = pseudo.get("count.packages").unwrap();
        assert!(
            packages.contains("{count, plural, one {# package} other {# packages}}"),
            "{packages}"
        );
    }

    #[test]
    fn escaped_braces_survive_the_pseudolocale() {
        assert!(pseudo_value("a {{ b }} c").contains("{{"));
        assert!(pseudo_value("a {{ b }} c").contains("}}"));
    }
}
