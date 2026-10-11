// SPDX-License-Identifier: Apache-2.0

//! The i18n layer's contract with the rest of the system.
//!
//! `scripts/check-i18n-parity.sh` enforces parity between catalogue *files*.
//! These tests enforce the part it cannot see: that the generated pseudolocale
//! matches English, that the error taxonomy is fully covered, that plural
//! selection follows each locale's own rule, and that no template in the
//! shipped catalogue is malformed.

use std::collections::BTreeSet;

use macportsbrew::i18n::format::is_balanced;
use macportsbrew::i18n::{env_preference, resolve, Catalogue, Locale, Messages};
use macportsbrew::render::display_width;
use macportsbrew::{msg, params};
use macportsbrew_domain::{DomainError, ErrorId, Reason};

// --- Catalogue parity -------------------------------------------------------

#[test]
fn the_pseudolocale_has_exactly_the_english_keys() {
    let source = Catalogue::english();
    let generated = Catalogue::pseudolocale();
    let english: BTreeSet<&str> = source.keys().collect();
    let pseudo: BTreeSet<&str> = generated.keys().collect();
    assert_eq!(english, pseudo);
    assert!(english.len() > 40, "the catalogue looks truncated");
}

#[test]
fn every_error_identifier_has_an_english_message() {
    let catalogue = Catalogue::english();
    let absent: Vec<&str> = ErrorId::all()
        .iter()
        .map(|id| id.message_id())
        .filter(|id| catalogue.get(id).is_none())
        .collect();
    assert!(absent.is_empty(), "no English message for {absent:?}");
}

#[test]
fn every_error_message_carries_the_diagnostic_detail_and_a_remediation() {
    let catalogue = Catalogue::english();
    for id in ErrorId::all() {
        let template = catalogue.get(id.message_id()).unwrap();
        assert!(
            template.contains("{detail}"),
            "{} does not place the detail",
            id.message_id()
        );
        // A remediation is a second sentence: "it failed" with no next step is
        // exactly what the output contract forbids.
        let sentences = template.matches(". ").count();
        assert!(
            sentences >= 1,
            "{} has no remediation sentence: {template}",
            id.message_id()
        );
    }
}

#[test]
fn every_error_renders_without_leaving_a_placeholder_behind() {
    let messages = Messages::english();
    for id in ErrorId::all() {
        let error = DomainError::new(*id, "the-detail");
        let text = messages.render_error(&error);
        assert!(text.contains("the-detail"), "{}: {text}", id.message_id());
        assert!(!text.contains('{'), "{}: {text}", id.message_id());
        assert!(!text.starts_with('['), "{}: {text}", id.message_id());
    }
    assert!(messages.missing().is_empty(), "{:?}", messages.missing());
}

#[test]
fn no_english_template_has_an_unmatched_brace() {
    let catalogue = Catalogue::english();
    let broken: Vec<&str> = catalogue
        .keys()
        .filter(|key| !is_balanced(catalogue.get(key).unwrap()))
        .collect();
    assert!(broken.is_empty(), "unbalanced braces in {broken:?}");
}

#[test]
fn no_pseudolocale_template_has_an_unmatched_brace() {
    let catalogue = Catalogue::pseudolocale();
    let broken: Vec<&str> = catalogue
        .keys()
        .filter(|key| !is_balanced(catalogue.get(key).unwrap()))
        .collect();
    assert!(broken.is_empty(), "unbalanced braces in {broken:?}");
}

#[test]
fn the_keys_the_ui_depends_on_are_present() {
    let messages = Messages::english();
    for id in [
        msg!("cli.about"),
        msg!("cli.alias_note"),
        msg!("table.package"),
        msg!("table.provider"),
        msg!("table.version"),
        msg!("table.ownership"),
        msg!("table.class"),
        msg!("table.availability"),
        msg!("table.support"),
        msg!("table.recommendation"),
        msg!("outcome.keep"),
        msg!("outcome.migrate_to"),
        msg!("outcome.keep_both"),
        msg!("outcome.use_runtime_manager"),
        msg!("outcome.manual_review"),
        msg!("outcome.no_action"),
        msg!("doctor.ok"),
        msg!("doctor.warning"),
        msg!("doctor.error"),
        msg!("reason.unknown"),
        msg!("advanced.warning"),
        msg!("count.packages"),
        msg!("policy.stale_notice"),
    ] {
        assert!(!messages.render_id(id).starts_with('['), "{id} is absent");
    }
    assert!(messages.missing().is_empty(), "{:?}", messages.missing());
}

// --- Rendering --------------------------------------------------------------

#[test]
fn placeholders_are_substituted_and_literal_braces_survive() {
    let messages = Messages::english();
    let notice = messages.render(
        msg!("plan.heading"),
        &params! { package: "wget", source: "homebrew", destination: "macports" },
    );
    assert!(notice.contains("wget"), "{notice}");
    assert!(notice.contains("macports"), "{notice}");
    assert!(!notice.contains('{'), "{notice}");

    // Escaping is `{{` and `}}`, in the catalogue and anywhere else.
    let catalogue = Catalogue::from_toml("t = \"a {{x}} {y}\"\n").unwrap();
    let braces = Messages::new(Locale::english(), catalogue);
    assert_eq!(braces.render("t", &params! { y: 1 }), "a {x} 1");
}

#[test]
fn plural_selection_follows_the_locale_rule() {
    let english = Messages::english();
    assert_eq!(
        english.render(msg!("count.packages"), &params! { count: 1 }),
        "1 package"
    );
    assert_eq!(
        english.render(msg!("count.packages"), &params! { count: 0 }),
        "0 packages"
    );
    assert_eq!(
        english.render(msg!("count.packages"), &params! { count: 42 }),
        "42 packages"
    );

    // Japanese has no singular; the English catalogue is what ships today, so
    // the assertion is that selection under `ja` is stable and never reaches
    // for an arm the locale does not have.
    let japanese = Messages::for_locale(&Locale::new("ja"));
    let one = japanese.render(msg!("count.packages"), &params! { count: 1 });
    let many = japanese.render(msg!("count.packages"), &params! { count: 9 });
    assert_eq!(one, "1 packages");
    assert_eq!(many, "9 packages");

    let exact = english.render(msg!("count.findings"), &params! { count: 0 });
    assert_eq!(exact, "no findings");
}

#[test]
fn a_reason_without_a_catalogue_entry_names_itself() {
    let messages = Messages::english();
    let reason = Reason::new("homebrew_support_ending").with("year", &2027);
    assert_eq!(
        messages.render_reason(&reason),
        "Reason homebrew_support_ending."
    );
    assert!(messages
        .missing()
        .contains("reason.homebrew_support_ending"));
}

// --- Locale resolution ------------------------------------------------------

#[test]
fn resolution_precedence_is_flag_then_environment_then_english() {
    // The flag wins.
    assert_eq!(resolve(Some("ja"), Some("pt_BR.UTF-8")).as_str(), "ja");
    // Then the environment, in the order env_preference collapses it.
    let env = env_preference(Some("es"), Some("ja_JP.UTF-8"), Some("pt_BR.UTF-8"));
    assert_eq!(resolve(None, env).as_str(), "es");
    let env = env_preference(None, Some("ja_JP.UTF-8"), Some("pt_BR.UTF-8"));
    assert_eq!(resolve(None, env).as_str(), "ja");
    let env = env_preference(None, None, Some("pt_BR.UTF-8"));
    assert_eq!(resolve(None, env).as_str(), "pt-BR");
    // Then English.
    assert_eq!(
        resolve(None, env_preference(None, None, None)).as_str(),
        "en"
    );
    // An unrecognised tag is not an error.
    assert_eq!(resolve(Some("xx-YY"), None).as_str(), "en");
}

#[test]
fn every_target_locale_resolves_to_itself() {
    for tag in ["en", "pt-BR", "es", "ja", "zh-CN", "qps"] {
        assert_eq!(resolve(Some(tag), None).as_str(), tag);
    }
}

// --- Pseudolocale -----------------------------------------------------------

#[test]
fn every_pseudolocale_string_is_wider_than_its_english_source() {
    let english = Catalogue::english();
    let pseudo = Catalogue::pseudolocale();
    for key in english.keys() {
        let source = english.get(key).unwrap();
        let widened = pseudo.get(key).unwrap();
        assert!(
            display_width(widened) > display_width(source),
            "{key}: {widened:?} is not wider than {source:?}"
        );
    }
}

#[test]
fn the_pseudolocale_still_substitutes_and_still_pluralises() {
    let messages = Messages::for_locale(&Locale::pseudo());
    let notice = messages.render(
        msg!("plan.heading"),
        &params! { package: "wget", source: "homebrew", destination: "example-test" },
    );
    assert!(notice.contains("wget"), "{notice}");
    assert!(notice.contains("example-test"), "{notice}");
    assert!(notice.starts_with('['), "{notice}");
    assert!(notice.ends_with(']'), "{notice}");

    // The pseudolocale follows English plural rules, so both arms are reachable
    // when a layout is being checked.
    let one = messages.render(msg!("count.packages"), &params! { count: 1 });
    let other = messages.render(msg!("count.packages"), &params! { count: 5 });
    assert!(one.contains("1 package"), "{one}");
    assert!(other.contains("5 packages"), "{other}");
}
