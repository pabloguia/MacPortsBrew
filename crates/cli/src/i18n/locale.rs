// SPDX-License-Identifier: Apache-2.0

//! Locale tags and the plural rules the target locales need.
//!
//! Tags are BCP-47-ish: a lower-case language subtag, optionally a hyphen and
//! an upper-case region subtag. POSIX spellings (`pt_BR.UTF-8`, `zh_CN@pinyin`)
//! are accepted and normalised, because that is what the environment supplies.

use std::fmt;

/// The locales this build knows how to present text in
/// (`docs/spec/06-cli-and-ux.md`: `en`, `pt-BR`, `es`, `ja`, `zh-CN`, plus the
/// pseudolocale). Translations for the four non-English ones land in later
/// passes; the tags are recognised now so that resolution, plural rules and
/// layout can be tested against them.
pub const KNOWN: &[&str] = &["en", "pt-BR", "es", "ja", "zh-CN", PSEUDO];

/// The pseudolocale tag. `qps` is the conventional private-use prefix for one;
/// it is generated from English at runtime and has no catalogue file.
pub const PSEUDO: &str = "qps";

/// The plural categories the target locales use.
///
/// CLDR defines six; English, Spanish and Portuguese need two of them and
/// Japanese and Chinese need one, so carrying the other four would be four
/// categories no message could ever select. A locale that needs `few`, `many`
/// or `two` is a catalogue this build does not yet accept, and adding one is a
/// deliberate change here rather than a silent mis-selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PluralCategory {
    /// The singular arm.
    One,
    /// Everything the locale does not treat as singular.
    Other,
}

impl PluralCategory {
    /// The selector this category is written as inside a plural message.
    pub fn selector(self) -> &'static str {
        match self {
            PluralCategory::One => "one",
            PluralCategory::Other => "other",
        }
    }
}

/// A normalised locale tag.
///
/// Constructing one never fails: an unrecognised tag is kept as written so the
/// caller can report what it was asked for, and [`resolve`](super::resolve) is
/// the function that decides whether to honour it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locale {
    tag: String,
}

impl Locale {
    /// Normalises a tag: `_` becomes `-`, any `.charset` or `@modifier` suffix
    /// is dropped, the language subtag is lower-cased and the region subtag
    /// upper-cased. `pt_br.UTF-8` and `PT-BR` both become `pt-BR`.
    pub fn new(tag: &str) -> Self {
        let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-");
        let mut parts = base.split('-').filter(|p| !p.is_empty());
        let Some(language) = parts.next() else {
            return Self { tag: String::new() };
        };
        let mut normalised = language.to_ascii_lowercase();
        for part in parts {
            normalised.push('-');
            // A two-letter subtag is a region and is conventionally upper-case;
            // anything longer is a script or a variant, which this build does
            // not distinguish, so it is kept as written.
            if part.len() == 2 {
                normalised.push_str(&part.to_ascii_uppercase());
            } else {
                normalised.push_str(part);
            }
        }
        Self { tag: normalised }
    }

    /// English, the source locale.
    pub fn english() -> Self {
        Self {
            tag: "en".to_string(),
        }
    }

    /// The pseudolocale used for layout testing.
    pub fn pseudo() -> Self {
        Self {
            tag: PSEUDO.to_string(),
        }
    }

    /// The normalised tag.
    pub fn as_str(&self) -> &str {
        &self.tag
    }

    /// The language subtag alone (`pt` for `pt-BR`).
    pub fn language(&self) -> &str {
        self.tag.split('-').next().unwrap_or("")
    }

    /// True when this is the generated pseudolocale.
    pub fn is_pseudo(&self) -> bool {
        self.tag == PSEUDO
    }

    /// True when this build carries, or can generate, a catalogue for the tag.
    pub fn is_known(&self) -> bool {
        KNOWN.contains(&self.tag.as_str())
    }

    /// The plural category for a count.
    ///
    /// The rules are CLDR's, restricted to the target locales. Portuguese is
    /// the one that surprises: CLDR gives `pt` the singular for both 0 and 1
    /// (`i = 0..1`), so "0 pacote" is correct and "0 pacotes" is not. English
    /// and Spanish take the singular only at 1; Japanese and Chinese have no
    /// singular at all, which is why a message must never be assembled from a
    /// stem plus an "s".
    pub fn plural_category(&self, count: u64) -> PluralCategory {
        match self.language() {
            "ja" | "zh" => PluralCategory::Other,
            "pt" => {
                if count <= 1 {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
            // `en`, `es`, the pseudolocale, and anything unrecognised, which
            // can only be reached through a catalogue this build accepted.
            _ => {
                if count == 1 {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posix_spellings_normalise() {
        assert_eq!(Locale::new("pt_BR.UTF-8").as_str(), "pt-BR");
        assert_eq!(Locale::new("pt_br").as_str(), "pt-BR");
        assert_eq!(Locale::new("ZH-cn").as_str(), "zh-CN");
        assert_eq!(Locale::new("zh_CN@pinyin").as_str(), "zh-CN");
        assert_eq!(Locale::new("EN").as_str(), "en");
        assert_eq!(Locale::new("qps").as_str(), "qps");
    }

    #[test]
    fn a_longer_subtag_is_not_treated_as_a_region() {
        assert_eq!(Locale::new("zh-Hans").as_str(), "zh-Hans");
    }

    #[test]
    fn an_empty_tag_normalises_to_nothing_rather_than_panicking() {
        assert_eq!(Locale::new("").as_str(), "");
        assert_eq!(Locale::new(".UTF-8").as_str(), "");
    }

    #[test]
    fn every_known_tag_is_already_normalised() {
        for tag in KNOWN {
            assert_eq!(Locale::new(tag).as_str(), *tag, "{tag} is not normalised");
        }
    }

    #[test]
    fn plural_rules_match_cldr_for_the_target_locales() {
        let en = Locale::new("en");
        assert_eq!(en.plural_category(0), PluralCategory::Other);
        assert_eq!(en.plural_category(1), PluralCategory::One);
        assert_eq!(en.plural_category(2), PluralCategory::Other);

        let es = Locale::new("es");
        assert_eq!(es.plural_category(0), PluralCategory::Other);
        assert_eq!(es.plural_category(1), PluralCategory::One);

        // CLDR `pt`: the singular covers 0 as well as 1.
        let pt = Locale::new("pt-BR");
        assert_eq!(pt.plural_category(0), PluralCategory::One);
        assert_eq!(pt.plural_category(1), PluralCategory::One);
        assert_eq!(pt.plural_category(2), PluralCategory::Other);

        for tag in ["ja", "zh-CN"] {
            let locale = Locale::new(tag);
            for count in [0, 1, 2, 11] {
                assert_eq!(
                    locale.plural_category(count),
                    PluralCategory::Other,
                    "{tag} has no singular"
                );
            }
        }

        // The pseudolocale follows English so layout tests exercise both arms.
        assert_eq!(Locale::pseudo().plural_category(1), PluralCategory::One);
        assert_eq!(Locale::pseudo().plural_category(2), PluralCategory::Other);
    }

    #[test]
    fn pseudo_is_recognised_but_english_is_not_pseudo() {
        assert!(Locale::pseudo().is_pseudo());
        assert!(Locale::pseudo().is_known());
        assert!(!Locale::english().is_pseudo());
        assert!(Locale::english().is_known());
        assert!(!Locale::new("de").is_known());
    }
}
