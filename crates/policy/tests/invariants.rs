// SPDX-License-Identifier: Apache-2.0

//! The invariants this crate is answerable for, each named after the one it
//! defends (`docs/spec/01-invariants.md`: "Each invariant MUST have at least
//! one test named after it").
//!
//! These are integration tests on purpose. They see the crate exactly as the
//! layers above it will, which is the point of view an invariant has to hold
//! from — a property that only holds when you can reach inside the crate is
//! not a property of the crate.

use macportsbrew_policy::data::{shift_date, PolicyData, PolicySources};
use macportsbrew_policy::rules::{testing, SUPPORT_END_HORIZON_DAYS};
use macportsbrew_policy::{engine, facts::Facts, reasons, rules};

use macportsbrew_domain::{
    ArtifactAvailability, Confidence, Outcome, PackageClass, ProviderId, SupportTier,
};

fn data() -> PolicyData {
    PolicyData::load_embedded().expect("the shipped data set must parse")
}

/// A data set in which two providers are described identically, so that a
/// difference in the engine's answer can only come from the rules.
fn symmetric_data() -> PolicyData {
    const MANIFEST: &str = "version = \"test\"\nstale_after_days = 180\n";
    const SUPPORT: &str = r#"
[[entry]]
id = "alpha-any"
provider = "alpha"
architecture = "any"
tier = "SUPPORTED"
source = "fixture"
read_on = "2026-09-02"

[[entry]]
id = "beta-any"
provider = "beta"
architecture = "any"
tier = "SUPPORTED"
source = "fixture"
read_on = "2026-09-02"
"#;
    const TAGS: &str = r#"
[[tagset]]
provider = "alpha"
universal_tag = "any"
older_compatible_same_architecture = false
source = "fixture"
read_on = "2026-09-02"
[[tagset.pattern]]
architecture = "x86_64"
template = "alpha-{release}"
[[tagset.release]]
macos = "26"
name = "twentysix"

[[tagset]]
provider = "beta"
universal_tag = "any"
older_compatible_same_architecture = false
source = "fixture"
read_on = "2026-09-02"
[[tagset.pattern]]
architecture = "x86_64"
template = "beta-{release}"
[[tagset.release]]
macos = "26"
name = "twentysix"
"#;
    const STATEFUL: &str = r#"
[[entry]]
family = "ledger"
names = { alpha = ["ledger*"], beta = ["ledger*"] }
source = "fixture"
read_on = "2026-09-02"
rationale = "fixture"
"#;
    const MANAGERS: &str = r#"
[[entry]]
id = "vm"
for_families = ["sample"]
source = "fixture"
read_on = "2026-09-02"
confirmed = false
"#;
    const MAPPINGS: &str = r#"
[[entry]]
source_provider = "alpha"
source_name = "sample"
destination_provider = "beta"
destination_name = "sample"
confidence = "EXACT"
source = "fixture"
read_on = "2026-09-02"
"#;

    PolicyData::load_from(PolicySources {
        manifest: MANIFEST,
        support: SUPPORT,
        artifact_tags: TAGS,
        stateful: STATEFUL,
        runtime_managers: MANAGERS,
        curated_mappings: MAPPINGS,
    })
    .expect("the symmetric fixture must parse")
}

/// Every shape of facts the invariant tests exercise, so that a property
/// claimed for "the engine" is claimed against more than one input.
fn scenarios() -> Vec<(&'static str, Facts)> {
    let horizon = shift_date(testing::TODAY, SUPPORT_END_HORIZON_DAYS / 2).unwrap();

    let mut ending = testing::baseline();
    ending.providers.get_mut(&testing::alpha()).unwrap().support =
        testing::support_ending(SupportTier::Degraded, &horizon, true);

    let mut compiling = testing::baseline();
    compiling
        .package
        .per_provider
        .get_mut(&testing::alpha())
        .unwrap()
        .availability = testing::availability(ArtifactAvailability::SourceBuildRequired {
        reason: macportsbrew_domain::availability::SourceBuildReason::NoArtifactForThisPlatform,
    });

    let mut both = testing::baseline();
    both.package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .installed = true;

    let mut reviewable = testing::baseline();
    reviewable
        .package
        .classes
        .add(PackageClass::Service, "fixture", "fixture");

    let mut weak = testing::baseline();
    weak.package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .mapping_confidence = Confidence::Probable;

    let mut vetoed = compiling.clone();
    vetoed.providers.get_mut(&testing::beta()).unwrap().support =
        testing::support(SupportTier::Unsupported, false);

    let mut runtime = testing::baseline();
    runtime
        .package
        .classes
        .add(PackageClass::Runtime, "fixture", "fixture");

    let mut dependents = testing::baseline();
    dependents.package.reverse_dependency_count = 3;

    vec![
        ("baseline", testing::baseline()),
        ("support-ending", ending),
        ("compiling-here", compiling),
        ("held-by-both", both),
        ("reviewable-class", reviewable),
        ("weak-evidence", weak),
        ("vetoed-destination", vetoed),
        ("runtime", runtime),
        ("has-dependents", dependents),
    ]
}

// ---------------------------------------------------------------- INV-14

#[test]
fn invariant_14_no_llm_no_io_engine_is_pure() {
    // Purity, observed the only way a test can observe it: the same inputs
    // give the same output, repeatedly, with no argument changing in
    // between and nothing outside the two arguments to consult.
    let data = data();
    for (name, facts) in scenarios() {
        let first = engine::recommend(&facts, &data);
        for _ in 0..8 {
            assert_eq!(engine::recommend(&facts, &data), first, "{name}");
        }
        assert_eq!(facts, facts.clone(), "{name}: the engine mutated its input");
    }
}

#[test]
fn invariant_14_the_current_date_is_an_input_not_a_clock_read() {
    // Moving `today` forward past a published end date changes the answer.
    // If the engine were reading a clock this would be impossible to arrange,
    // and if it ignored the date the answer would not move at all.
    let data = data();
    let mut facts = testing::baseline();
    let end = shift_date(testing::TODAY, SUPPORT_END_HORIZON_DAYS * 2).unwrap();
    facts.providers.get_mut(&testing::alpha()).unwrap().support =
        testing::support_ending(SupportTier::Degraded, &end, true);

    let far = engine::recommend(&facts, &data);
    assert_eq!(
        far.outcome,
        Outcome::Keep {
            provider: testing::alpha()
        }
    );

    facts.today = shift_date(testing::TODAY, SUPPORT_END_HORIZON_DAYS + 1).unwrap();
    let near = engine::recommend(&facts, &data);
    assert_eq!(
        near.outcome,
        Outcome::MigrateTo {
            provider: testing::beta()
        }
    );
}

// ---------------------------------------------------------------- INV-15

#[test]
fn invariant_15_reproducible_recommendations() {
    let data = data();
    for (name, facts) in scenarios() {
        let first = engine::explain(&facts, &data);
        let second = engine::explain(&facts, &data);
        assert_eq!(first, second, "{name}");

        // Byte-identical, not merely equal: the serialised form is what
        // `analyze --from-snapshot` compares, and an unordered collection
        // that happens to compare equal would still differ on the wire.
        let a = serde_json::to_string(&first).unwrap();
        let b = serde_json::to_string(&second).unwrap();
        assert_eq!(a, b, "{name}");

        let reparsed: engine::Derivation = serde_json::from_str(&a).unwrap();
        assert_eq!(serde_json::to_string(&reparsed).unwrap(), a, "{name}");
    }
}

#[test]
fn invariant_15_ordering_is_fixed_not_incidental() {
    let data = data();
    for (name, facts) in scenarios() {
        let derivation = engine::explain(&facts, &data);

        let mut not_fired = derivation.not_fired.clone();
        not_fired.sort();
        assert_eq!(derivation.not_fired, not_fired, "{name}: rules not fired");

        let fired: Vec<&str> = derivation.fired.iter().map(|f| f.rule.as_str()).collect();
        let mut sorted = fired.clone();
        sorted.sort_unstable();
        assert_eq!(fired, sorted, "{name}: rules fired");

        let keys: Vec<&str> = derivation
            .facts_used
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort_unstable();
        assert_eq!(keys, sorted_keys, "{name}: facts used");

        assert_eq!(
            derivation.fired.len() + derivation.not_fired.len(),
            rules::all().len(),
            "{name}: every rule must be accounted for"
        );
    }
}

// ---------------------------------------------------------------- INV-17

#[test]
fn invariant_17_symmetric_rules() {
    // Rename the two providers to each other everywhere in the facts, against
    // a data set that describes them identically. The recommendation must
    // come back renamed and otherwise unchanged. Anything a rule held about
    // one side rather than about the facts would show up here.
    let data = symmetric_data();
    let (a, b) = (testing::alpha(), testing::beta());

    for (name, facts) in scenarios() {
        let original = engine::recommend(&facts, &data);
        let swapped = engine::recommend(&testing::swap_providers(&facts, &a, &b), &data);

        assert_eq!(
            swapped.outcome,
            testing::swap_outcome(&original.outcome, &a, &b),
            "{name}: the outcome did not follow the rename"
        );

        let original_rules: Vec<Option<&String>> =
            original.reasons.iter().map(|r| r.rule.as_ref()).collect();
        let swapped_rules: Vec<Option<&String>> =
            swapped.reasons.iter().map(|r| r.rule.as_ref()).collect();
        assert_eq!(
            original_rules, swapped_rules,
            "{name}: a different set of rules fired for the mirrored facts"
        );
        assert_eq!(
            original.rules_not_fired, swapped.rules_not_fired,
            "{name}: a different set of rules stayed silent"
        );
    }
}

#[test]
fn invariant_17_every_rule_individually_follows_the_rename() {
    let data = symmetric_data();
    let (a, b) = (testing::alpha(), testing::beta());

    for (name, facts) in scenarios() {
        let mirrored = testing::swap_providers(&facts, &a, &b);
        for rule in rules::all() {
            let here = rule.evaluate(&facts, &data);
            let there = rule.evaluate(&mirrored, &data);
            match (here, there) {
                (None, None) => {}
                (Some(here), Some(there)) => assert_eq!(
                    there.outcome,
                    testing::swap_outcome(&here.outcome, &a, &b),
                    "{name}/{}: outcome did not follow the rename",
                    rule.id()
                ),
                (here, there) => panic!(
                    "{name}/{}: fired on one side only ({}, {})",
                    rule.id(),
                    here.is_some(),
                    there.is_some()
                ),
            }
        }
    }
}

#[test]
fn invariant_17_curated_data_answers_in_both_directions() {
    let data = data();
    for mapping in data.curated_mappings() {
        let forward = data.curated_targets(
            &mapping.source_provider,
            &mapping.source_name,
            &mapping.destination_provider,
        );
        assert!(
            forward.contains(&mapping.destination_name),
            "{} -> {} is missing forwards",
            mapping.source_name,
            mapping.destination_name
        );
        let back = data.curated_targets(
            &mapping.destination_provider,
            &mapping.destination_name,
            &mapping.source_provider,
        );
        assert!(
            back.contains(&mapping.source_name),
            "{} -> {} is missing backwards",
            mapping.destination_name,
            mapping.source_name
        );
    }
}

// ---------------------------------------------------------------- INV-19

#[test]
fn invariant_19_stateful_and_service_force_manual_review() {
    let data = data();

    // By class, for each of the classes the domain marks as forcing review.
    for class in [
        PackageClass::Stateful,
        PackageClass::Service,
        PackageClass::GuiApp,
        PackageClass::ThirdPartySourced,
    ] {
        let mut facts = testing::baseline();
        facts.package.classes.add(class, "fixture", "fixture");
        assert_eq!(
            engine::recommend(&facts, &data).outcome,
            Outcome::ManualReview,
            "{class:?} must not be actionable"
        );
    }

    // And by curated data alone, with no class set at all: R-8 finding 7
    // measured the classifier missing packages that unambiguously own data,
    // which is why the data is the primary authority and this path exists.
    let mut facts = testing::baseline();
    facts.package.source = ProviderId::new("homebrew");
    facts.package.name = "postgresql@17".to_string();
    assert!(facts.package.classes.is_simple());
    let recommendation = engine::recommend(&facts, &data);
    assert_eq!(recommendation.outcome, Outcome::ManualReview);
    assert!(recommendation
        .reasons
        .iter()
        .any(|r| r.id.as_str() == reasons::PACKAGE_OWNS_PERSISTENT_DATA));
}

#[test]
fn invariant_19_manual_review_cannot_be_outvoted() {
    // A reviewable class alongside every condition that would otherwise
    // produce an actionable outcome. Manual review still wins, and it wins by
    // outcome rather than by holding the largest number.
    let data = data();
    let mut facts = testing::baseline();
    facts.package.coexistence_requested = true;
    facts
        .package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .installed = true;
    facts.providers.get_mut(&testing::alpha()).unwrap().support = testing::support_ending(
        SupportTier::Degraded,
        &shift_date(testing::TODAY, 1).unwrap(),
        true,
    );
    facts
        .package
        .classes
        .add(PackageClass::Stateful, "fixture", "fixture");

    let recommendation = engine::recommend(&facts, &data);
    assert_eq!(recommendation.outcome, Outcome::ManualReview);
    assert!(!recommendation.outcome.is_actionable());
}

// ---------------------------------------------------------------- INV-20

#[test]
fn invariant_20_provider_facts_are_dated_data() {
    let data = data();

    for entry in data.support_entries() {
        assert!(
            !entry.source.trim().is_empty(),
            "{}: a support row with no source is a claim from memory",
            entry.id
        );
        assert_eq!(entry.read_on.len(), 10, "{}", entry.id);
        assert!(
            shift_date(&entry.read_on, 0).is_some(),
            "{}: read_on must be a date",
            entry.id
        );
    }
    for family in data.stateful_families() {
        assert!(!family.source.trim().is_empty(), "{}", family.family);
        assert!(
            shift_date(&family.read_on, 0).is_some(),
            "{}",
            family.family
        );
    }
    for mapping in data.curated_mappings() {
        assert!(
            !mapping.source.trim().is_empty(),
            "{} -> {}",
            mapping.source_name,
            mapping.destination_name
        );
        assert!(shift_date(&mapping.read_on, 0).is_some());
        assert_eq!(mapping.confidence, Confidence::Exact);
    }

    // At least one published end date exists, and it lives in the data.
    assert!(
        data.support_entries().iter().any(|e| e.end_date.is_some()),
        "the data set records no stated end of support at all"
    );
}

#[test]
fn invariant_20_no_provider_deadline_is_written_into_source() {
    // The one thing INV-20 exists to stop: a date a provider published,
    // compiled into the binary where re-checking it means re-releasing.
    // Rule metadata dates are not that — they say when a rule was reviewed,
    // not what a provider promised — so the check names the deadline years
    // rather than every date-shaped literal.
    let src = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut offenders: Vec<String> = Vec::new();
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_rust_files(std::path::Path::new(src), &mut files);
    assert!(files.len() > 5, "expected to find the crate's sources");

    for path in files {
        let text = std::fs::read_to_string(&path).expect("readable source file");
        for (number, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for year in ["2027", "2028", "2029", "2030"] {
                if code.contains(year) {
                    offenders.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a provider deadline appears in source; it belongs in data/policy/ with its \
         source and read date (INV-20):\n{}",
        offenders.join("\n")
    );
}

fn collect_rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

// ------------------------------------------------- no rule prefers a provider

#[test]
fn no_rule_fires_unconditionally_on_a_preference() {
    // "No rule may encode 'provider X is better'. Every rule is conditioned
    // on machine and package" (03-policy-and-recommendations.md, MUST NOT).
    //
    // Stated as two checks a test can actually make. First: every rule except
    // the documented fallback must stay silent on some input, so none of them
    // is a standing opinion. Second: no rule may name a provider that is not
    // in the facts it was given — a preference has to be written down
    // somewhere, and the only place left is the outcome.
    let data = data();
    let all_scenarios = scenarios();

    for rule in rules::all() {
        let silent_somewhere = all_scenarios
            .iter()
            .any(|(_, facts)| rule.evaluate(facts, &data).is_none());
        if rule.id() == "KEEP_SOURCE_DEFAULT" {
            assert!(
                !silent_somewhere,
                "the fallback must always fire, or a recommendation can have no reason"
            );
            continue;
        }
        assert!(
            silent_somewhere,
            "{} fired on every input; a rule that always fires is a preference, not a rule",
            rule.id()
        );
    }

    for (name, facts) in &all_scenarios {
        let known: Vec<&str> = facts.providers.keys().map(ProviderId::as_str).collect();
        for rule in rules::all() {
            let Some(verdict) = rule.evaluate(facts, &data) else {
                continue;
            };
            let named = match &verdict.outcome {
                Outcome::Keep { provider } | Outcome::MigrateTo { provider } => Some(provider),
                Outcome::KeepBoth { primary } => Some(primary),
                _ => None,
            };
            if let Some(provider) = named {
                assert!(
                    known.contains(&provider.as_str()),
                    "{name}/{}: named {provider}, which is not in the facts",
                    rule.id()
                );
            }
        }
    }
}

#[test]
fn every_reason_the_rules_can_emit_is_in_the_catalogue() {
    let data = data();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (_, facts) in scenarios() {
        for rule in rules::all() {
            if let Some(verdict) = rule.evaluate(&facts, &data) {
                seen.insert(verdict.reason.id.as_str().to_string());
            }
        }
    }
    for id in &seen {
        assert!(
            reasons::all().contains(&id.as_str()),
            "{id} is emitted but absent from reasons::all()"
        );
    }
    // The scenarios are supposed to exercise most of the catalogue; if this
    // drops, the scenario list has stopped keeping up with the rules.
    assert!(
        seen.len() >= 8,
        "only {} reasons were exercised",
        seen.len()
    );
}

// ---------------------------------------------------------------- INV-04

#[test]
fn invariant_04_reverse_dependencies_are_read_only_from_the_source_provider() {
    // Partly type-level, and deliberately so: `reverse_dependency_count` is a
    // field of `PackageFacts`, which is scoped to the package on the side
    // holding it, and `PackageProviderFacts` — the per-provider row — carries
    // no dependents field at all. There is therefore no way to express a
    // cross-provider count in the engine's input, which is a stronger
    // guarantee than any rule checking itself.
    let data = data();
    let mut facts = testing::baseline();
    facts.package.reverse_dependency_count = 4;

    let recommendation = engine::recommend(&facts, &data);
    assert_eq!(recommendation.outcome, Outcome::ManualReview);
    let reason = recommendation
        .reasons
        .iter()
        .find(|r| r.id.as_str() == reasons::REVERSE_DEPENDENCIES_PRESENT)
        .expect("the dependents reason must be attached");
    assert_eq!(
        reason.params.get("provider").map(String::as_str),
        Some(facts.package.source.as_str()),
        "the count must be attributed to the provider it was counted in"
    );

    // And it follows the facts rather than a name: mirror the providers and
    // the count is attributed to the other side.
    let mirrored = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
    let mirrored_reason = engine::recommend(&mirrored, &data)
        .reasons
        .iter()
        .find(|r| r.id.as_str() == reasons::REVERSE_DEPENDENCIES_PRESENT)
        .cloned()
        .expect("the dependents reason must be attached");
    assert_eq!(
        mirrored_reason.params.get("provider").map(String::as_str),
        Some(testing::beta().as_str())
    );
}

// ---------------------------------------------------------------- INV-08

#[test]
fn invariant_08_no_rule_reads_availability_or_dependents_across_providers() {
    // One provider's package never satisfies another's requirement "in any
    // calculation". The observable form in this layer: availability is keyed
    // by provider and never substituted. Change only the other side's facts
    // and everything the engine says about this side must be unchanged.
    let data = data();

    let mut here_compiles = testing::baseline();
    here_compiles
        .package
        .per_provider
        .get_mut(&testing::alpha())
        .unwrap()
        .availability = testing::availability(ArtifactAvailability::Unavailable);

    let before = engine::explain(&here_compiles, &data);

    let mut other_side_is_ready = here_compiles.clone();
    {
        let per = other_side_is_ready
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap();
        per.installed = true;
        per.availability = testing::availability(ArtifactAvailability::BinaryAvailable);
    }
    let after = engine::explain(&other_side_is_ready, &data);

    let source_key = format!("package.{}.availability", testing::alpha());
    let read_source = |d: &engine::Derivation| {
        d.facts_used
            .iter()
            .find(|(k, _)| *k == source_key)
            .map(|(_, v)| v.clone())
            .expect("the source side's availability must be reported")
    };
    assert_eq!(
        read_source(&before),
        "UNAVAILABLE",
        "the source side's own availability must be reported as its own"
    );
    assert_eq!(
        read_source(&before),
        read_source(&after),
        "the other side's artifact changed the source side's availability"
    );

    // The two sides are separate entries; nothing merges them.
    let destination_key = format!("package.{}.availability", testing::beta());
    let destination = after
        .facts_used
        .iter()
        .find(|(k, _)| *k == destination_key)
        .map(|(_, v)| v.as_str())
        .expect("the other side must have its own entry");
    assert_eq!(destination, "BINARY_AVAILABLE");
    assert_ne!(read_source(&after), destination);

    // A curated equivalence is an equivalence, not a substitution: the pair
    // INV-08 names by example resolves both ways and still says nothing about
    // satisfying a dependency.
    assert_eq!(
        data.curated_targets("homebrew", "openssl@3", "macports"),
        vec!["openssl3".to_string()]
    );
}

// ---------------------------------------------------------------- INV-09

#[test]
fn invariant_09_duplication_is_reported_not_consolidated() {
    // A package under both providers is reported, never consolidated. There
    // is no outcome that expresses "remove" at all — `Outcome` has six
    // variants and none of them is a removal — so the strongest thing the
    // engine can say about a duplicate is which one to reach first.
    let data = data();
    let mut both = testing::baseline();
    both.package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .installed = true;

    let recommendation = engine::recommend(&both, &data);
    assert!(
        matches!(recommendation.outcome, Outcome::KeepBoth { .. }),
        "a healthy duplicate must stay a duplicate, got {:?}",
        recommendation.outcome
    );

    // Even when one side would otherwise look better, the duplicate is not
    // resolved by moving: only a rule that reads a fact about this machine
    // can produce a move, and it must not be triggered by duplication alone.
    let mut lopsided = both.clone();
    lopsided
        .package
        .per_provider
        .get_mut(&testing::alpha())
        .unwrap()
        .availability = testing::availability(ArtifactAvailability::Unavailable);
    assert!(
        matches!(
            engine::recommend(&lopsided, &data).outcome,
            Outcome::KeepBoth { .. }
        ),
        "duplication must be reported, not consolidated"
    );
}

// ---------------------------------------------------------------- INV-11

#[test]
fn invariant_11_primary_selection_does_not_change_ownership() {
    // Type-level, and stated as such. `Outcome::KeepBoth` carries a single
    // `primary: ProviderId` and nothing else; `Recommendation` carries an
    // outcome, reasons, the rules that did not fire and a policy version.
    // Neither type can express an ownership change, and `recommend` takes
    // `&Facts`, so it has no way to write one back either.
    let data = data();
    let mut both = testing::baseline();
    both.package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .installed = true;

    let ownership_before = both.package.ownership;
    let recommendation = engine::recommend(&both, &data);
    assert!(matches!(recommendation.outcome, Outcome::KeepBoth { .. }));
    assert_eq!(
        both.package.ownership, ownership_before,
        "the engine may not restate ownership"
    );

    // And the serialised recommendation says nothing about ownership.
    let json = serde_json::to_string(&recommendation).unwrap();
    assert!(!json.contains("ownership"), "{json}");
}

// ---------------------------------------------------------------- INV-12

#[test]
fn invariant_12_package_choice_is_not_global_priority() {
    // Also type-level in part: nothing on `Recommendation` or `Outcome` names
    // a global ordering, and the assertion below checks the serialised form
    // for one. The behavioural half is that the choice is genuinely
    // per-package — two packages, the same two providers, different primaries.
    let data = data();

    let mut first = testing::baseline();
    first.package.name = "first".to_string();
    first
        .package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .installed = true;

    let mut second = first.clone();
    second.package.name = "second".to_string();
    second
        .package
        .per_provider
        .get_mut(&testing::alpha())
        .unwrap()
        .availability = testing::availability(ArtifactAvailability::Unavailable);

    let first_out = engine::recommend(&first, &data).outcome;
    let second_out = engine::recommend(&second, &data).outcome;
    assert_eq!(
        first_out,
        Outcome::KeepBoth {
            primary: testing::alpha()
        }
    );
    assert_eq!(
        second_out,
        Outcome::KeepBoth {
            primary: testing::beta()
        }
    );
    assert_ne!(
        first_out, second_out,
        "the same provider pair must be able to yield different primaries"
    );

    for outcome in [&first_out, &second_out] {
        let json = serde_json::to_string(outcome).unwrap();
        for forbidden in ["global", "priority", "order", "rank"] {
            assert!(!json.contains(forbidden), "{json} names {forbidden}");
        }
    }
}

// ---------------------------------------------------------------- INV-18

#[test]
fn invariant_18_coexistence_is_a_first_class_outcome() {
    let data = data();

    // Reachable as a final answer by both routes the policy names.
    let mut held_by_both = testing::baseline();
    held_by_both
        .package
        .per_provider
        .get_mut(&testing::beta())
        .unwrap()
        .installed = true;
    assert!(matches!(
        engine::recommend(&held_by_both, &data).outcome,
        Outcome::KeepBoth { .. }
    ));

    let mut asked_for = testing::baseline();
    asked_for.package.coexistence_requested = true;
    assert!(matches!(
        engine::recommend(&asked_for, &data).outcome,
        Outcome::KeepBoth { .. }
    ));

    // It is an end state, not a step: the outcome is actionable in its own
    // right rather than a marker for something else to resolve.
    assert!(Outcome::KeepBoth {
        primary: testing::alpha()
    }
    .is_actionable());

    // And nothing downstream of the rules may demote it. This is the
    // regression the engine's removed destination guard used to cause: with a
    // package held under two healthy providers it rewrote a legitimate
    // KeepBoth into a Keep whenever the primary happened to be the
    // unsupported side. Across every scenario, no outcome the engine produces
    // may name a provider that states it does not support this machine — and
    // no KeepBoth may be silently turned into something else.
    for (name, facts) in scenarios() {
        let outcome = engine::recommend(&facts, &data).outcome;
        let named = match &outcome {
            Outcome::MigrateTo { provider } | Outcome::KeepBoth { primary: provider } => {
                Some(provider.clone())
            }
            _ => None,
        };
        if let Some(provider) = named {
            assert_ne!(
                facts.support(&provider).tier,
                SupportTier::Unsupported,
                "{name}: the engine named an unsupported provider"
            );
        }
    }

    // The one shape that would have tripped the old guard, checked directly.
    let mut unsupported_primary = held_by_both.clone();
    unsupported_primary
        .package
        .per_provider
        .get_mut(&testing::alpha())
        .unwrap()
        .availability = testing::availability(ArtifactAvailability::Unavailable);
    unsupported_primary
        .providers
        .get_mut(&testing::beta())
        .unwrap()
        .support = testing::support(SupportTier::Unsupported, true);
    let outcome = engine::recommend(&unsupported_primary, &data).outcome;
    assert!(
        !matches!(outcome, Outcome::KeepBoth { primary } if primary == testing::beta()),
        "an unsupported side must never be named primary"
    );
}
