// SPDX-License-Identifier: Apache-2.0

//! The internationalisation layer.
//!
//! The domain, the policy engine and the adapters emit stable identifiers and
//! never display text (`docs/spec/06-cli-and-ux.md`). This module is where an
//! identifier becomes a sentence:
//!
//! ```
//! use macportsbrew::i18n::{resolve, Messages};
//! use macportsbrew::params;
//!
//! let locale = resolve(Some("pt-BR"), None);
//! let messages = Messages::for_locale(&locale);
//! let text = messages.render("policy.stale_notice", &params! {
//!     readOn: "2026-09-02",
//!     url: "https://docs.brew.sh/Support-Tiers",
//! });
//! assert!(text.contains("2026-09-02"));
//! ```
//!
//! English is the only catalogue this build ships; the four other target
//! locales resolve to a recognised [`Locale`] so that plural rules and layout
//! can be exercised now, and receive their catalogues in a later pass.

pub mod catalogue;
pub mod format;
pub mod locale;

pub use catalogue::Catalogue;
pub use locale::{Locale, PluralCategory};

use macportsbrew_domain::{DomainError, Reason};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

/// Names a message identifier, and marks it for the parity check.
///
/// `scripts/check-i18n-parity.sh` scans the sources for `msg!("…")` and fails
/// the build when an identifier the code names is absent from
/// `locales/en.toml`. The macro expands to the literal itself, so it costs
/// nothing at runtime and reads as what it is at the call site:
///
/// ```
/// use macportsbrew::{msg, i18n::Messages};
/// let messages = Messages::english();
/// assert!(!messages.render_id(msg!("cli.about")).is_empty());
/// ```
#[macro_export]
macro_rules! msg {
    ($id:literal) => {
        $id
    };
}

/// Builds the parameter map [`Messages::render`] takes.
///
/// Keys are written as identifiers and become the placeholder names; values
/// are anything that implements [`std::fmt::Display`].
///
/// ```
/// use macportsbrew::params;
/// let p = params! { count: 3, provider: "macports" };
/// assert_eq!(p["count"], "3");
/// ```
#[macro_export]
macro_rules! params {
    () => {
        ::std::collections::BTreeMap::<::std::string::String, ::std::string::String>::new()
    };
    ($($key:ident : $value:expr),+ $(,)?) => {{
        let mut map = ::std::collections::BTreeMap::<
            ::std::string::String, ::std::string::String>::new();
        $(
            map.insert(
                ::std::string::ToString::to_string(stringify!($key)),
                ::std::string::ToString::to_string(&$value),
            );
        )+
        map
    }};
}

/// Chooses the locale to present in.
///
/// Precedence, highest first: the `--language` flag, then the environment, then
/// English. Anything this build does not recognise falls back to English rather
/// than failing — a user who mistypes a tag should get a working tool and, from
/// the caller, a note about it, not a usage error.
///
/// The function takes values and never reads the environment itself, so it is
/// testable and so that the process's environment is read in exactly one place.
/// Use [`env_preference`] to collapse the environment variables into the second
/// argument in the documented order.
///
/// A language subtag alone is honoured when exactly one target locale uses it:
/// `pt` resolves to `pt-BR` and `zh` to `zh-CN`, because those are the target
/// regional variants. The POSIX `C` and `POSIX` locales mean "no localisation"
/// and resolve to English.
pub fn resolve(requested: Option<&str>, env_lang: Option<&str>) -> Locale {
    for candidate in [requested, env_lang].into_iter().flatten() {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("C") || trimmed.eq_ignore_ascii_case("POSIX") {
            return Locale::english();
        }
        let locale = Locale::new(trimmed);
        if locale.is_known() {
            return locale;
        }
        if let Some(known) = locale::KNOWN
            .iter()
            .find(|known| Locale::new(known).language() == locale.language())
        {
            return Locale::new(known);
        }
    }
    Locale::english()
}

/// Collapses the environment variables into the single value [`resolve`] takes,
/// in the documented order: the application's own override first, then
/// `LC_ALL`, then `LANG`. Empty values are treated as unset, which is how the
/// shell spells "no preference".
pub fn env_preference<'a>(
    app_override: Option<&'a str>,
    lc_all: Option<&'a str>,
    lang: Option<&'a str>,
) -> Option<&'a str> {
    [app_override, lc_all, lang]
        .into_iter()
        .flatten()
        .find(|value| !value.trim().is_empty())
}

/// A catalogue bound to a locale: the thing the UI renders through.
///
/// Identifiers this catalogue does not carry render as `[the.identifier]` and
/// are recorded. Rendering therefore never fails and never produces an empty
/// string — a blank line in a table is a defect nobody reports, whereas a
/// bracketed identifier is one anybody can paste into an issue. Read the
/// record with [`Messages::missing`]; the CLI reports it under `--verbose` and
/// the parity test asserts it is empty for the catalogues that ship.
///
/// The record is interior-mutable, so `Messages` is `!Sync` by design: it is a
/// single-threaded presentation object, and sharing one across threads would
/// make the miss record a race rather than a diagnostic.
#[derive(Debug)]
pub struct Messages {
    locale: Locale,
    catalogue: Catalogue,
    missing: RefCell<BTreeSet<String>>,
}

impl Messages {
    /// Binds a catalogue to a locale.
    pub fn new(locale: Locale, catalogue: Catalogue) -> Self {
        Self {
            locale,
            catalogue,
            missing: RefCell::new(BTreeSet::new()),
        }
    }

    /// English, the source locale.
    pub fn english() -> Self {
        Self::new(Locale::english(), Catalogue::english())
    }

    /// The catalogue this build has for a locale.
    ///
    /// The pseudolocale is generated; every other tag currently gets English,
    /// because the translations land in a later pass. The locale is kept as
    /// asked for even so, so plural rules and layout are already the target
    /// locale's while the text is still English.
    pub fn for_locale(locale: &Locale) -> Self {
        let catalogue = if locale.is_pseudo() {
            Catalogue::pseudolocale()
        } else {
            Catalogue::english()
        };
        Self::new(locale.clone(), catalogue)
    }

    /// The locale being presented in.
    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    /// The catalogue in use.
    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    /// Renders an identifier with parameters.
    ///
    /// **Every parameter value is escaped here** with
    /// [`macportsbrew_domain::escape_hostile`], before it is substituted. This
    /// is the one place catalogue parameters reach text, and parameters are
    /// where snapshot-derived values travel into human output - a `PATH` entry,
    /// a `reported` variable's name and value in `doctor`, a provider's
    /// version. A snapshot keeps format characters and the line separators
    /// (legitimate names carry them); this is where they stop being able to
    /// reorder text or break a line, and stop hiding text as far as Unicode
    /// format characters can (other default-ignorable characters - Hangul
    /// fillers, variation selectors - are not in the classification). The
    /// template is the catalogue's own text and is not escaped;
    /// `catalogue::tests::no_template_carries_a_hostile_character` holds it
    /// to that.
    ///
    /// **Every** parameter is escaped, with no exception: a multi-line value,
    /// or a fragment already rendered from the catalogue and passed in as a
    /// parameter, has its line breaks and format characters escaped too. No
    /// caller does that today. One that needs to compose trusted fragments is
    /// owed a separate, explicitly trusted path - not a way round this one.
    pub fn render(&self, id: &str, params: &BTreeMap<String, String>) -> String {
        if let Some(template) = self.catalogue.get(id) {
            let params: BTreeMap<String, String> = params
                .iter()
                .map(|(k, v)| (k.clone(), macportsbrew_domain::escape_hostile(v)))
                .collect();
            let params = &self.resolve_identifiers(&params);
            return format::format_message(&self.locale, template, params);
        }
        self.missing.borrow_mut().insert(id.to_string());
        format!("[{id}]")
    }

    /// Parameters whose value is a stable domain identifier, rendered through
    /// the catalogue key for that identifier before substitution.
    ///
    /// The families are named in `locales/en.toml`, which has described this
    /// mechanism since it was written while nothing implemented it: a rule
    /// emitting `labels::tier(SupportTier::Degraded)` put the literal
    /// `DEGRADED` into a sentence a person reads, and `doctor` put
    /// `format!("{:?}", tier)` there - a `Debug` rendering built in the
    /// *application* layer, which is a display string above the layer allowed to
    /// have one (F-20).
    ///
    /// A family is keyed by parameter *name*, across every template: a
    /// `{confidence}` or `{threshold}` anywhere is read as a mapping
    /// confidence. `crates/cli/tests/reasons_catalogue.rs` holds every such
    /// parameter to that vocabulary, so a later reason cannot reuse the name
    /// for something else and have it rendered as a confidence label.
    ///
    /// A value that names no key is left exactly as it is. That is deliberate
    /// and not a fallback to be tidied away: silently blanking a value no
    /// family covers would be worse than showing an identifier.
    fn resolve_identifiers(&self, params: &BTreeMap<String, String>) -> BTreeMap<String, String> {
        const FAMILIES: &[(&str, &str)] = &[
            ("tier", "support"),
            ("class", "class"),
            ("cause", "source_build"),
            ("confidence", "confidence"),
            ("threshold", "confidence"),
            ("state", "state"),
        ];
        let mut out = params.clone();
        for (name, family) in FAMILIES {
            let Some(value) = out.get_mut(*name) else {
                continue;
            };
            let key = format!("{family}.{}", value.to_ascii_lowercase());
            if let Some(template) = self.catalogue.get(&key) {
                *value = format::format_message(&self.locale, template, &BTreeMap::new());
            }
        }
        out
    }

    /// Renders an identifier that takes no parameters.
    pub fn render_id(&self, id: &str) -> String {
        self.render(id, &BTreeMap::new())
    }

    /// The identifiers that were asked for and not found, in sorted order.
    pub fn missing(&self) -> BTreeSet<String> {
        self.missing.borrow().clone()
    }

    /// Renders a reason.
    ///
    /// The catalogue key is `reason.<suffix>`, where the suffix is the
    /// [`macportsbrew_domain::ReasonId`] with any leading `reason.` removed —
    /// so a rule may emit either spelling. An identifier the catalogue does not
    /// carry falls back to `reason.unknown`, which names it rather than
    /// inventing a sentence; the miss is still recorded, because an unrendered
    /// reason is a translation gap and not a normal outcome.
    pub fn render_reason(&self, reason: &Reason) -> String {
        let raw = reason.id.as_str();
        // A reason id that is itself a catalogue key (doctor findings reuse
        // `doctor.*` keys; the staleness notice reuses `policy.*`) renders
        // directly; otherwise the `reason.` namespace applies.
        if self.catalogue.get(raw).is_some() {
            return self.render(raw, &reason.params);
        }
        let suffix = raw.strip_prefix("reason.").unwrap_or(raw);
        let key = format!("reason.{suffix}");
        if self.catalogue.get(&key).is_some() {
            return self.render(&key, &reason.params);
        }
        self.missing.borrow_mut().insert(key);
        let mut params = reason.params.clone();
        params.insert("id".to_string(), suffix.to_string());
        self.render("reason.unknown", &params)
    }

    /// Renders an error: its remediation sentence, with the untranslated
    /// diagnostic detail substituted for `{detail}`.
    pub fn render_error(&self, error: &DomainError) -> String {
        let mut params = BTreeMap::new();
        params.insert("detail".to_string(), error.detail.clone());
        self.render(error.id.message_id(), &params)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_stable_identifier_parameter_is_rendered_through_its_catalogue_key() {
        // `locales/en.toml` has described this since it was written and nothing
        // implemented it, so a rule emitting `labels::tier(Unsupported)` put the
        // literal `UNSUPPORTED` into a sentence a person reads — while
        // `support.unsupported = "Not supported"` sat in the catalogue unused
        // (F-20).
        let messages = Messages::english();
        let with = |name: &str, value: &str| {
            let mut p = BTreeMap::new();
            p.insert(name.to_string(), value.to_string());
            p
        };
        let rendered = messages.render("doctor.provider_support", &with("tier", "UNSUPPORTED"));
        assert!(
            rendered.contains("Not supported"),
            "the catalogue's wording, not the identifier: {rendered}"
        );
        assert!(
            !rendered.contains("UNSUPPORTED"),
            "and the identifier does not survive: {rendered}"
        );
        // Every variant the domain has.
        for (identifier, text) in [
            ("SUPPORTED", "Supported"),
            ("DEGRADED", "Degraded"),
            ("UNSUPPORTED", "Not supported"),
            ("UNKNOWN", "Unknown"),
        ] {
            let out = messages.render("doctor.provider_support", &with("tier", identifier));
            assert!(
                out.contains(text),
                "{identifier} should render {text}: {out}"
            );
        }
        // A value naming no key is left exactly as it is, rather than blanked.
        // `{state}` and friends carry values from families this does not cover.
        let out = messages.render("doctor.provider_support", &with("tier", "SOMETHING_NEW"));
        assert!(out.contains("SOMETHING_NEW"), "{out}");
    }

    use super::*;
    use macportsbrew_domain::ErrorId;

    #[test]
    fn an_explicit_language_beats_the_environment() {
        assert_eq!(resolve(Some("ja"), Some("pt_BR.UTF-8")).as_str(), "ja");
    }

    #[test]
    fn the_environment_is_used_when_no_flag_was_given() {
        assert_eq!(resolve(None, Some("pt_BR.UTF-8")).as_str(), "pt-BR");
        assert_eq!(resolve(None, Some("zh_CN.UTF-8")).as_str(), "zh-CN");
    }

    #[test]
    fn nothing_at_all_is_english() {
        assert_eq!(resolve(None, None).as_str(), "en");
        assert_eq!(resolve(Some("   "), None).as_str(), "en");
    }

    #[test]
    fn an_unknown_tag_falls_back_to_english_rather_than_failing() {
        assert_eq!(resolve(Some("tlh"), None).as_str(), "en");
        assert_eq!(resolve(None, Some("de_DE.UTF-8")).as_str(), "en");
    }

    #[test]
    fn an_unknown_flag_still_lets_the_environment_be_tried() {
        assert_eq!(resolve(Some("tlh"), Some("ja_JP.UTF-8")).as_str(), "ja");
    }

    #[test]
    fn a_bare_language_resolves_to_its_target_variant() {
        assert_eq!(resolve(Some("pt"), None).as_str(), "pt-BR");
        assert_eq!(resolve(Some("zh"), None).as_str(), "zh-CN");
    }

    #[test]
    fn the_posix_locales_mean_english() {
        assert_eq!(resolve(None, Some("C")).as_str(), "en");
        assert_eq!(resolve(None, Some("POSIX")).as_str(), "en");
    }

    #[test]
    fn the_pseudolocale_is_selectable() {
        assert!(resolve(Some("qps"), None).is_pseudo());
    }

    #[test]
    fn the_environment_order_is_override_then_lc_all_then_lang() {
        assert_eq!(
            env_preference(Some("ja"), Some("es"), Some("en")),
            Some("ja")
        );
        assert_eq!(env_preference(None, Some("es"), Some("en")), Some("es"));
        assert_eq!(env_preference(None, None, Some("en")), Some("en"));
        assert_eq!(env_preference(None, None, None), None);
        // An empty variable is how a shell spells "unset".
        assert_eq!(env_preference(Some(""), Some(" "), Some("ja")), Some("ja"));
    }

    #[test]
    fn a_missing_identifier_renders_bracketed_and_is_recorded() {
        let messages = Messages::english();
        assert_eq!(messages.render_id("no.such.key"), "[no.such.key]");
        assert!(messages.missing().contains("no.such.key"));
    }

    #[test]
    fn a_present_identifier_records_nothing() {
        let messages = Messages::english();
        assert!(!messages.render_id("cli.about").is_empty());
        assert!(messages.missing().is_empty());
    }

    #[test]
    fn every_hostile_parameter_value_is_escaped_and_benign_text_is_not() {
        use macportsbrew_domain::{CHARACTER_SAMPLES, LEGITIMATE_TEXT_WITH_FORMAT};
        let m = Messages::english();
        // `doctor`'s leaked-variable finding, whose `{variable}` comes straight
        // from a snapshot's `reported` keys.
        let template = m
            .catalogue()
            .get("doctor.build_variable_names_prefix")
            .expect("the finding exists");
        assert!(template.contains("{variable}"), "{template}");
        for &(c, class) in CHARACTER_SAMPLES {
            let value = format!("CPP{c}FLAGS");
            let text = m.render(
                "doctor.build_variable_names_prefix",
                &crate::params! { variable: value.clone(), provider: "p" },
            );
            if class.is_some() {
                assert!(!text.contains(c), "{c:?} survived: {text:?}");
                assert!(
                    text.contains(&format!("CPP{}FLAGS", c.escape_unicode())),
                    "{text}"
                );
            } else {
                assert!(text.contains(&value), "{c:?} was altered: {text:?}");
            }
        }
        for name in LEGITIMATE_TEXT_WITH_FORMAT {
            let text = m.render(
                "doctor.build_variable_names_prefix",
                &crate::params! { variable: *name, provider: "p" },
            );
            assert!(
                !text.chars().any(macportsbrew_domain::is_hostile),
                "{text:?}"
            );
        }
    }

    #[test]
    fn an_error_renders_its_remediation_with_the_detail() {
        let messages = Messages::english();
        let error = DomainError::new(ErrorId::PackageNotFound, "wget under macports");
        let text = messages.render_error(&error);
        assert!(text.contains("wget under macports"), "{text}");
        assert!(!text.contains("{detail}"), "{text}");
        assert!(messages.missing().is_empty());
    }

    #[test]
    fn an_unknown_reason_falls_back_to_naming_itself() {
        let messages = Messages::english();
        let reason = Reason::new("some_rule_not_yet_translated");
        assert_eq!(
            messages.render_reason(&reason),
            "Reason some_rule_not_yet_translated."
        );
        assert!(messages
            .missing()
            .contains("reason.some_rule_not_yet_translated"));
    }

    #[test]
    fn a_reason_id_may_carry_the_prefix_already() {
        let messages = Messages::english();
        let reason = Reason::new("reason.some_rule");
        assert_eq!(messages.render_reason(&reason), "Reason some_rule.");
    }

    #[test]
    fn the_params_macro_builds_the_map_render_takes() {
        let map = crate::params! { count: 3, provider: "macports" };
        assert_eq!(map["count"], "3");
        assert_eq!(map["provider"], "macports");
        assert!(crate::params!().is_empty());
    }

    #[test]
    fn the_msg_macro_is_the_identifier() {
        assert_eq!(crate::msg!("cli.about"), "cli.about");
    }
}
