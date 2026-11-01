// SPDX-License-Identifier: Apache-2.0

//! Mapping a package toward another provider (`02-domain-model.md`,
//! "Mapping"). Never by name alone: every candidate carries evidence, the
//! curated table is consulted first, and the discovered path only sees the
//! destination's *installed* packages in this pass - the destination's full
//! catalogue is a later pass, so an absent counterpart is reported as
//! `NONE`, not guessed.

use macportsbrew_domain::{
    Confidence, MappingCandidate, MappingEvidence, PackageIdentity, PackageMapping,
    PackageMetadata, ProviderId, ProviderInventory,
};
use macportsbrew_policy::PolicyData;
use std::collections::BTreeMap;

/// Below this score the evidence is a hint (a family member with a different
/// version), not a candidate: listing it at the same confidence as a real
/// match would make every versioned family ambiguous.
const CANDIDATE_THRESHOLD: u32 = 30;

/// Normalises a name for comparison: lower case, `@`/`-`/`_` removed.
pub fn normalise(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Splits `python@3.12` / `python312` into (family, version digits).
fn family_and_version(name: &str) -> (String, String) {
    let family = crate::classify::family_of(name);
    let digits: String = name
        .split('@')
        .next_back()
        .unwrap_or("")
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    (family, digits)
}

/// Maps `source` toward `destination`.
pub fn map_package(
    source: &PackageIdentity,
    source_meta: Option<&PackageMetadata>,
    destination: &ProviderId,
    destination_inventory: Option<&ProviderInventory>,
    policy: &PolicyData,
) -> PackageMapping {
    let mut scored: BTreeMap<String, (u32, Vec<MappingEvidence>)> = BTreeMap::new();
    let mut add = |name: &str, kind: &str, detail: String, weight: u32| {
        let entry = scored.entry(name.to_string()).or_default();
        entry.0 += weight;
        entry.1.push(MappingEvidence {
            kind: kind.into(),
            detail,
            weight,
        });
    };

    for target in
        policy.curated_targets(source.provider.as_str(), &source.name, destination.as_str())
    {
        add(
            &target,
            "curated",
            format!("{} -> {target}", source.name),
            100,
        );
    }

    if let Some(inv) = destination_inventory {
        let src_norm = normalise(&source.name);
        let (src_family, src_ver) = family_and_version(&source.name);
        for candidate in inv.installed_names() {
            let cand_norm = normalise(&candidate);
            if cand_norm == src_norm {
                add(&candidate, "normalised_name", cand_norm.clone(), 40);
            } else {
                let (fam, ver) = family_and_version(&candidate);
                if fam == src_family && !src_family.is_empty() {
                    if !src_ver.is_empty() && src_ver == ver {
                        add(&candidate, "versioned_family", format!("{fam} {ver}"), 40);
                    } else if !src_ver.is_empty() || !ver.is_empty() {
                        add(
                            &candidate,
                            "versioned_family",
                            format!("{fam} (version differs)"),
                            10,
                        );
                    }
                }
            }
            if let (Some(sm), Some(cm)) = (source_meta, inv.metadata.get(&candidate)) {
                if let (Some(a), Some(b)) = (&sm.homepage, &cm.homepage) {
                    if !a.is_empty() && normalise_url(a) == normalise_url(b) {
                        add(&candidate, "homepage", b.clone(), 30);
                    }
                }
                // The list, never the older set beside it (F.md §1): equal
                // non-empty `KNOWN` lists are evidence. Only a capture that
                // named the package reads a list (F-2b), so for every other
                // snapshot this adds nothing, as before.
                if let (Some(a), Some(b)) = (sm.executable_list.known(), cm.executable_list.known())
                {
                    if !a.is_empty() && a == b {
                        add(&candidate, "executables", a.len().to_string(), 20);
                    }
                }
            }
        }
    }

    let candidates: Vec<MappingCandidate> = scored
        .into_iter()
        .filter(|(_, (score, _))| *score >= CANDIDATE_THRESHOLD)
        .map(|(name, (score, mut evidence))| {
            evidence.sort();
            MappingCandidate {
                package: PackageIdentity::new(destination.clone(), name),
                confidence: confidence_for(score),
                evidence,
            }
        })
        .collect();
    PackageMapping::from_candidates(source.clone(), destination.clone(), candidates)
}

fn confidence_for(score: u32) -> Confidence {
    match score {
        s if s >= 100 => Confidence::Exact,
        s if s >= 60 => Confidence::Strong,
        _ => Confidence::Probable,
    }
}

fn normalise_url(url: &str) -> String {
    url.trim()
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::{InstalledPackage, PackageKind, Version};

    fn inventory(names: &[(&str, Option<&str>)]) -> ProviderInventory {
        let mut inv = ProviderInventory::default();
        for (n, home) in names {
            inv.installed.push(InstalledPackage {
                identity: PackageIdentity::new(ProviderId::new("macports"), *n),
                kind: PackageKind::Package,
                version: Version::new("1"),
                revision: None,
                variants: std::collections::BTreeSet::default(),
                active: true,
                requested: Some(true),
                installed_from_prebuilt: None,
                install_path: None,
            });
            if let Some(h) = home {
                inv.metadata.insert(
                    (*n).to_string(),
                    PackageMetadata {
                        homepage: Some((*h).to_string()),
                        ..PackageMetadata::default()
                    },
                );
            }
        }
        inv
    }

    fn policy() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    #[test]
    fn same_name_alone_is_only_probable() {
        let m = map_package(
            &PackageIdentity::new(ProviderId::new("homebrew"), "wget"),
            None,
            &ProviderId::new("macports"),
            Some(&inventory(&[("wget", None)])),
            &policy(),
        );
        assert_eq!(m.confidence, Confidence::Probable);
        assert_eq!(m.selected.unwrap().evidence[0].kind, "normalised_name");
    }

    #[test]
    fn name_plus_homepage_is_strong() {
        let meta = PackageMetadata {
            homepage: Some("https://www.gnu.org/software/wget/".into()),
            ..PackageMetadata::default()
        };
        let m = map_package(
            &PackageIdentity::new(ProviderId::new("homebrew"), "wget"),
            Some(&meta),
            &ProviderId::new("macports"),
            Some(&inventory(&[(
                "wget",
                Some("http://gnu.org/software/wget"),
            )])),
            &policy(),
        );
        assert_eq!(m.confidence, Confidence::Strong);
        assert_eq!(m.selected.as_ref().unwrap().evidence.len(), 2);
    }

    #[test]
    fn curated_entries_are_exact_even_when_the_destination_is_absent() {
        let m = map_package(
            &PackageIdentity::new(ProviderId::new("homebrew"), "python@3.12"),
            None,
            &ProviderId::new("macports"),
            None,
            &policy(),
        );
        assert_eq!(m.confidence, Confidence::Exact, "{m:?}");
        assert_eq!(m.selected.unwrap().package.name, "python312");
    }

    #[test]
    fn versioned_family_with_matching_version() {
        let m = map_package(
            &PackageIdentity::new(ProviderId::new("homebrew"), "llvm@18"),
            None,
            &ProviderId::new("macports"),
            Some(&inventory(&[("llvm-18", None), ("llvm-17", None)])),
            &policy(),
        );
        assert_eq!(m.selected.unwrap().package.name, "llvm-18");
    }

    #[test]
    fn nothing_matching_is_none() {
        let m = map_package(
            &PackageIdentity::new(ProviderId::new("homebrew"), "zzz"),
            None,
            &ProviderId::new("macports"),
            Some(&inventory(&[("wget", None)])),
            &policy(),
        );
        assert_eq!(m.confidence, Confidence::None);
        assert!(m.candidates.is_empty());
    }
}
