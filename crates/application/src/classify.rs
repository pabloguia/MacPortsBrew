// SPDX-License-Identifier: Apache-2.0

//! Package class detection (`02-domain-model.md`, "Package classes";
//! R-8 for the detection rules per provider). Every rule records its
//! evidence; the curated stateful data is consulted through policy data.

use macportsbrew_domain::{ClassSet, PackageClass, PackageKind, PackageMetadata, ProviderId};
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
    if let Some(meta) = metadata {
        if let Some(origin) = &meta.origin {
            set.add(PackageClass::ThirdPartySourced, "non_core_origin", origin);
        }
        if meta.defines_service {
            set.add(PackageClass::Service, "provider_service_definition", name);
        }
        if !meta.versioned_siblings.is_empty() {
            set.add(
                PackageClass::Runtime,
                "versioned_family",
                meta.versioned_siblings
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(","),
            );
        } else if meta
            .not_on_default_path_reason
            .as_deref()
            .is_some_and(|r| r.contains("versioned_formula"))
        {
            set.add(PackageClass::Runtime, "versioned_formula_reason", name);
        }
    }
    let family = family_of(name);
    if !set.has(PackageClass::Runtime) && !policy.runtime_managers_for(&family).is_empty() {
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
            defines_service: true,
            versioned_siblings: ["php@8.3".to_string()].into_iter().collect(),
            ..PackageMetadata::default()
        };
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
