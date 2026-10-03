// SPDX-License-Identifier: Apache-2.0

//! The UI's catalogue must cover the policy engine's reason vocabulary.
//!
//! `scripts/check-i18n-parity.sh` checks catalogue files against each other
//! and against message identifiers written as literals in code. It cannot see
//! this: the reason identifiers are `pub const`s in another crate, assembled
//! at run time, and a rule that emits one this catalogue lacks would degrade
//! to `reason.unknown` in front of a user without any check failing.
//!
//! `06-cli-and-ux.md` requires every reason to be renderable as a human
//! sentence in every supported locale, and `03-policy-and-recommendations.md`
//! that every recommendation carries its reasons. Together those mean the
//! catalogue is not allowed to lag the engine, and this is where that is
//! enforced.

use std::collections::BTreeSet;

use macportsbrew::i18n::Catalogue;
use macportsbrew_policy::reasons;

#[test]
fn english_covers_every_reason_the_policy_engine_can_emit() {
    let catalogue = Catalogue::english();
    let keys: BTreeSet<&str> = catalogue.keys().collect();

    let absent: Vec<&str> = reasons::all()
        .iter()
        .copied()
        .filter(|id| !keys.contains(id))
        .collect();

    assert!(
        absent.is_empty(),
        "locales/en.toml has no sentence for {absent:?}; a reason with no \
         translation reaches the user as reason.unknown"
    );
}

#[test]
fn the_catalogue_carries_no_orphaned_reason_key() {
    // The other direction: a `reason.*` key with no rule behind it is a
    // sentence nobody can trigger, and it will be translated into every
    // locale before anyone notices. `reason.unknown` is the one deliberate
    // exception — it is the fallback the renderer uses for an identifier the
    // catalogue does not carry, so by construction no rule emits it.
    let catalogue = Catalogue::english();
    let known: BTreeSet<&str> = reasons::all()
        .iter()
        .chain(macportsbrew_application::planner::REASON_IDS.iter())
        .copied()
        .collect();

    let orphaned: Vec<&str> = catalogue
        .keys()
        .filter(|k| k.starts_with("reason."))
        .filter(|k| **k != *"reason.unknown")
        .filter(|k| !known.contains(*k))
        .collect();

    assert!(
        orphaned.is_empty(),
        "locales/en.toml carries {orphaned:?}, which no rule emits"
    );
}

/// The label the catalogue gives a confidence identifier (`NONE` -> `None`).
fn confidence_label(catalogue: &Catalogue, identifier: &str) -> String {
    let key = format!("confidence.{}", identifier.to_ascii_lowercase());
    catalogue
        .get(&key)
        .unwrap_or_else(|| panic!("locales/en.toml has no {key}"))
        .to_string()
}

#[test]
fn a_mapping_confidence_is_named_in_the_sentence_never_used_as_an_adjective() {
    // The sentence used to read "is only a {confidence} match", which with the
    // planner's `None` printed "only a None match" - a label used as an
    // adjective it cannot be. It is now introduced as what it is ("at level
    // None"), rendered through `confidence.*`, for every value the domain has,
    // from the helper both emitters use.
    use macportsbrew::i18n::Messages;
    use macportsbrew_domain::{Confidence, Reason};

    let messages = Messages::english();
    let catalogue = messages.catalogue();
    let labels: Vec<String> = macportsbrew_policy::labels::confidence_vocabulary()
        .iter()
        .map(|id| confidence_label(catalogue, id))
        .collect();
    let threshold = confidence_label(
        catalogue,
        macportsbrew_policy::labels::confidence(
            macportsbrew_policy::labels::MAPPING_CONFIDENCE_THRESHOLD,
        ),
    );

    for confidence in [
        Confidence::None,
        Confidence::Ambiguous,
        Confidence::Probable,
        Confidence::Strong,
        Confidence::Exact,
    ] {
        let params = macportsbrew_policy::labels::mapping_confidence_params(confidence);
        let reason = params.iter().fold(
            Reason::new(reasons::MAPPING_CONFIDENCE_BELOW_THRESHOLD).with("destination", &"dest"),
            |reason, (name, value)| reason.with(*name, value),
        );
        let rendered = messages.render_reason(&reason);
        let label = confidence_label(catalogue, params[0].1);

        assert!(
            rendered.contains(&format!("level {label}")),
            "the label is introduced as a level: {rendered}"
        );
        assert!(
            rendered.contains(&format!("reaches confidence level {threshold}")),
            "the threshold renders through its label too: {rendered}"
        );
        for identifier in macportsbrew_policy::labels::confidence_vocabulary() {
            assert!(
                !rendered.contains(identifier),
                "no identifier survives into the sentence: {rendered}"
            );
        }
        for any in &labels {
            for bare in [
                format!("a {any} match"),
                format!("an {any} match"),
                format!("a {any} "),
                format!("an {any} "),
                format!("{any} match"),
            ] {
                assert!(
                    !rendered.contains(&bare),
                    "a confidence label used as an adjective ({bare:?}): {rendered}"
                );
            }
        }
    }
}

#[test]
fn confidence_and_threshold_parameters_only_ever_carry_a_confidence() {
    // `resolve_identifiers` resolves a family by parameter *name*, in every
    // template. So a `{confidence}` or `{threshold}` is only safe where its
    // value is a Confidence identifier. Two checks hold that:
    //
    // 1. The only template that interpolates either is the mapping-confidence
    //    reason. A new one has to be added here deliberately, after checking
    //    what its emitter passes.
    // 2. No source file in the workspace passes either name as a literal to
    //    `Reason::with`; both emitters take name and value from
    //    `labels::mapping_confidence_params`, which only produces the
    //    vocabulary. The scan removes whitespace from the whole file, not line
    //    by line, so a call rustfmt wrapped across lines is still caught.
    let catalogue = Catalogue::english();
    let using: BTreeSet<&str> = catalogue
        .keys()
        .filter(|key| {
            let template = catalogue.get(key).unwrap_or_default();
            template.contains("{confidence") || template.contains("{threshold")
        })
        .collect();
    assert_eq!(
        using,
        BTreeSet::from([reasons::MAPPING_CONFIDENCE_BELOW_THRESHOLD]),
        "a template interpolating {{confidence}} or {{threshold}} renders it \
         through confidence.*; check its emitter passes a Confidence identifier"
    );

    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut offenders = Vec::new();
    let mut pending = vec![crates];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).expect("the workspace crates are readable") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    pending.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("a source file is UTF-8");
                let compact: String = text.split_whitespace().collect();
                if compact.contains(".with(\"confidence\"")
                    || compact.contains(".with(\"threshold\"")
                {
                    offenders.push(path.display().to_string());
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "set confidence/threshold through labels::mapping_confidence_params: {offenders:?}"
    );
}
