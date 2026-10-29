// SPDX-License-Identifier: Apache-2.0

//! Package class detection (`02-domain-model.md`, "Package classes";
//! R-8 for the detection rules per provider). Every rule records its
//! evidence; the curated stateful data is consulted through policy data.

use macportsbrew_domain::{
    Answer, ClassSet, PackageClass, PackageKind, PackageMetadata, ProviderId,
};
use macportsbrew_policy::PolicyData;

/// Classifies one package from what its provider reported.
pub fn classify(
    provider: &ProviderId,
    name: &str,
    kind: PackageKind,
    metadata: Option<&PackageMetadata>,
    policy: &PolicyData,
) -> ClassSet {
    let mut set = ClassSet::simple();
    if kind == PackageKind::GuiApp {
        set.add(PackageClass::GuiApp, "kind_gui_app", name);
    }
    if let Some(family) = policy.stateful_family_of(provider.as_str(), name) {
        set.add(
            PackageClass::Stateful,
            "curated_stateful_family",
            family.family.clone(),
        );
    }
    // `RUNTIME` is "a language runtime or toolchain with its own ecosystem"
    // (`02-domain-model.md`). Versioned siblings are evidence of one only for a
    // family the policy data knows as a language family: `ffmpeg`, `openssl`
    // and `llvm` have versioned formulae too, and calling them runtimes made
    // the engine suggest a language runtime manager for a video tool (truth
    // sweep, pass 10, M10).
    let family = family_of(name);
    let language_family = !policy.runtime_managers_for(&family).is_empty();
    if let Some(meta) = metadata {
        if let Some(origin) = &meta.origin {
            set.add(PackageClass::ThirdPartySourced, "non_core_origin", origin);
        }
        // DP-032: the rule's answer, never the bool. `UNKNOWN` adds no class:
        // the planner refuses a plan from it instead (INV-19).
        if meta.service_answer() == Answer::Yes {
            set.add(PackageClass::Service, "provider_service_definition", name);
        }
        if language_family && !meta.versioned_siblings.is_empty() {
            set.add(
                PackageClass::Runtime,
                "versioned_family",
                meta.versioned_siblings
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(","),
            );
        } else if language_family
            && meta
                .not_on_default_path_reason
                .as_deref()
                .is_some_and(|r| r.contains("versioned_formula"))
        {
            set.add(PackageClass::Runtime, "versioned_formula_reason", name);
        }
    }
    if !set.has(PackageClass::Runtime) && language_family {
        set.add(PackageClass::Runtime, "runtime_manager_family", family);
    }
    set
}

/// The family part of a name: `python@3.12` and `python312` give `python`.
pub fn family_of(name: &str) -> String {
    let head = name.split('@').next().unwrap_or(name);
    let trimmed = head.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let trimmed = trimmed.trim_end_matches(['-', '_']);
    if trimmed.is_empty() { head } else { trimmed }.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formula_kept_off_the_default_path_for_being_versioned_is_a_runtime() {
        // The second of the two routes to `RUNTIME`, and the one no test
        // reached (DP-015). Homebrew keeps a versioned formula out of the
        // default `PATH` and says so in its own words; that sentence is the
        // evidence, so the class is recorded with the reason rather than
        // inferred from the name.
        //
        // It matters because `RUNTIME` is what makes the policy engine suggest
        // a runtime manager instead of a plain migration — the wrong class
        // here is the wrong recommendation, not a wrong label.
        let meta = PackageMetadata {
            not_on_default_path_reason: Some(
                "this is a versioned_formula and is keg-only".to_string(),
            ),
            ..PackageMetadata::default()
        };
        let set = classify(
            &ProviderId::new("homebrew"),
            "python@3.12",
            PackageKind::Package,
            Some(&meta),
            &policy(),
        );
        assert!(
            set.has(PackageClass::Runtime),
            "a versioned-formula reason must reach RUNTIME"
        );
        assert!(
            set.evidence
                .iter()
                .any(|e| e.class == PackageClass::Runtime && e.rule == "versioned_formula_reason"),
            "and it is recorded by that route: {set:?}"
        );
    }

    #[test]
    fn versioned_siblings_make_a_runtime_only_of_a_language_family() {
        let meta = PackageMetadata {
            versioned_siblings: ["ffmpeg@7".to_string()].into_iter().collect(),
            not_on_default_path_reason: Some("this is a versioned_formula".to_string()),
            ..PackageMetadata::default()
        };
        let homebrew = ProviderId::new("homebrew");
        let video = classify(
            &homebrew,
            "ffmpeg",
            PackageKind::Package,
            Some(&meta),
            &policy(),
        );
        assert!(!video.has(PackageClass::Runtime), "{video:?}");
        let language = PackageMetadata {
            versioned_siblings: ["node@22".to_string()].into_iter().collect(),
            ..PackageMetadata::default()
        };
        let node = classify(
            &homebrew,
            "node",
            PackageKind::Package,
            Some(&language),
            &policy(),
        );
        assert!(
            node.evidence
                .iter()
                .any(|e| e.class == PackageClass::Runtime && e.rule == "versioned_family"),
            "{node:?}"
        );
    }

    #[test]
    fn a_keg_only_formula_for_another_reason_is_not_made_a_runtime_by_it() {
        // The negative half: keg-only is not the same fact as versioned, and
        // without this the test above would pass on a rule that fired for any
        // reason string at all.
        let meta = PackageMetadata {
            not_on_default_path_reason: Some("macOS already provides this software".to_string()),
            ..PackageMetadata::default()
        };
        let set = classify(
            &ProviderId::new("homebrew"),
            "someformula",
            PackageKind::Package,
            Some(&meta),
            &policy(),
        );
        assert!(!set.has(PackageClass::Runtime));
    }

    #[test]
    fn a_name_that_is_only_digits_keeps_its_head_as_the_family() {
        // `family_of` trims trailing digits, so a name made entirely of them
        // would trim to nothing. The guard returns the head instead, and this
        // is the case that reaches it.
        assert_eq!(family_of("312"), "312");
        assert_eq!(family_of("python@3.12"), "python");
        assert_eq!(family_of("python312"), "python");
    }

    fn policy() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn a_plain_formula_is_simple() {
        let set = classify(
            &ProviderId::new("homebrew"),
            "wget",
            PackageKind::Package,
            Some(&PackageMetadata::default()),
            &policy(),
        );
        assert!(set.is_simple());
    }

    #[test]
    fn service_and_runtime_stack_into_a_set() {
        let meta = PackageMetadata {
            versioned_siblings: ["php@8.3".to_string()].into_iter().collect(),
            ..PackageMetadata::default()
        }
        .with_service(macportsbrew_domain::ServiceDefinition::both(Answer::Yes));
        let set = classify(
            &ProviderId::new("homebrew"),
            "php@8.4",
            PackageKind::Package,
            Some(&meta),
            &policy(),
        );
        assert!(set.has(PackageClass::Service));
        assert!(set.has(PackageClass::Runtime));
        assert_eq!(set.dominant(), PackageClass::Service);
    }

    #[test]
    fn curated_stateful_families_are_detected_from_data() {
        let set = classify(
            &ProviderId::new("homebrew"),
            "postgresql@17",
            PackageKind::Package,
            None,
            &policy(),
        );
        assert!(set.has(PackageClass::Stateful), "{set:?}");
    }

    #[test]
    fn casks_and_third_party_origins_force_review() {
        let meta = PackageMetadata {
            origin: Some("someone/tap".into()),
            ..PackageMetadata::default()
        };
        let set = classify(
            &ProviderId::new("homebrew"),
            "thing",
            PackageKind::GuiApp,
            Some(&meta),
            &policy(),
        );
        assert!(set.has(PackageClass::GuiApp));
        assert!(set.has(PackageClass::ThirdPartySourced));
        assert!(set.forces_manual_review());
    }

    #[test]
    fn families() {
        assert_eq!(family_of("python@3.12"), "python");
        assert_eq!(family_of("python312"), "python");
        assert_eq!(family_of("node"), "node");
    }
}
