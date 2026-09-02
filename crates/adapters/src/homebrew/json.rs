// SPDX-License-Identifier: Apache-2.0

//! The `brew info --json=v2 --installed` document, parsed.
//!
//! Provider output is untrusted input (`04-migration-and-safety.md`, threat
//! model). Four rules follow from that and are enforced here rather than at
//! every call site:
//!
//! 1. **Bounded.** The document is rejected above [`MAX_INPUT_BYTES`] before
//!    a parser ever sees it.
//! 2. **Shape-asserted.** A key that decides identity, ownership or artifact
//!    availability is *required*. Its absence is
//!    [`macportsbrew_domain::ErrorId::ProviderOutputChanged`] with a detail
//!    naming the field and the package, never a silent default
//!    (`07-quality-and-testing.md`).
//! 3. **Permissive elsewhere.** Homebrew documents its JSON stability as
//!    weak-additive - "fields may be added to the schema as needed without
//!    incrementing the schema" (R-1 finding 9) - so unknown keys are ignored
//!    and every other key is absent-able.
//! 4. **Never a path element, never a flag.** Every name and version that
//!    this adapter later joins onto the Cellar or the Caskroom is validated
//!    first: no separators, no `.`/`..`, no leading `-`, bounded length.
//!
//! The raw `Raw*` types mirror the document; [`parse_installed_json`] turns
//! them into the validated types the adapter maps from, so an invalid
//! document cannot reach the mapping code at all.

use crate::process::DEFAULT_MAX_OUTPUT_BYTES;
use macportsbrew_domain::{DomainError, ErrorId};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Largest document this adapter will parse. A whole-inventory answer is
/// under a megabyte on a full machine (876 KB for 180 formulae and 9 casks,
/// R-1); this cap exists so a hostile or broken provider cannot exhaust
/// memory, not as a working limit.
///
/// It is the executor's own cap, and it has to be: the text handed here was
/// captured by [`crate::process`], which stops reading at
/// [`DEFAULT_MAX_OUTPUT_BYTES`] and marks the invocation truncated. A larger
/// bound here could never be reached and so bounded nothing.
pub const MAX_INPUT_BYTES: usize = DEFAULT_MAX_OUTPUT_BYTES;

/// Longest accepted package name or version. Both are used as single path
/// components under the Cellar and the Caskroom.
const MAX_COMPONENT_LEN: usize = 128;

/// Characters a package name may contain, beyond ASCII alphanumerics.
const NAME_EXTRA: &[char] = &['@', '+', '.', '_', '-'];

/// Characters a version may contain, beyond ASCII alphanumerics. Casks use
/// a comma (`16.0.1,9`) and a plus (`3.22.3+105`).
const VERSION_EXTRA: &[char] = &['@', '+', '.', '_', '-', ',', '~', ':'];

fn changed(detail: impl Into<String>) -> DomainError {
    DomainError::new(ErrorId::ProviderOutputChanged, detail)
}

/// Rejects any text that must not become a path component or a command-line
/// flag. `field` names the JSON field for the error detail.
fn checked_component(raw: &str, allowed: &[char], field: &str) -> Result<String, DomainError> {
    if raw.is_empty() {
        return Err(changed(format!("{field} is empty")));
    }
    if raw.len() > MAX_COMPONENT_LEN {
        return Err(changed(format!(
            "{field} is {} bytes, over the {MAX_COMPONENT_LEN}-byte limit",
            raw.len()
        )));
    }
    if raw == "." || raw == ".." || raw.starts_with('-') {
        return Err(changed(format!(
            "{field} is not a usable component: {raw:?}"
        )));
    }
    if !raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || allowed.contains(&c))
    {
        return Err(changed(format!(
            "{field} contains characters this adapter refuses to use as a path component: {raw:?}"
        )));
    }
    Ok(raw.to_string())
}

/// Validates a package name or a dependency name.
pub fn checked_name(raw: &str, field: &str) -> Result<String, DomainError> {
    checked_component(raw, NAME_EXTRA, field)
}

/// Validates a version string.
pub fn checked_version(raw: &str, field: &str) -> Result<String, DomainError> {
    checked_component(raw, VERSION_EXTRA, field)
}

/// Validates a list of dependency names belonging to one formula.
fn checked_names(raw: &[String], owner: &str, field: &str) -> Result<Vec<String>, DomainError> {
    raw.iter()
        .enumerate()
        .map(|(index, name)| checked_name(name, &format!("formulae[{owner}].{field}[{index}]")))
        .collect()
}

// ---------------------------------------------------------------------------
// The document as Homebrew emits it.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawDocument {
    #[serde(default)]
    formulae: Option<Vec<RawFormula>>,
    #[serde(default)]
    casks: Option<Vec<RawCask>>,
}

#[derive(Debug, Deserialize)]
struct RawFormula {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    installed: Option<Vec<RawKeg>>,
    #[serde(default)]
    versions: Option<Versions>,
    #[serde(default)]
    bottle: Option<Bottle>,
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    tap: Option<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    revision: u32,
    #[serde(default)]
    linked_keg: Option<String>,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    keg_only: bool,
    #[serde(default)]
    keg_only_reason: Option<KegOnlyReason>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    versioned_formulae: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    build_dependencies: Vec<String>,
    #[serde(default)]
    caveats: Option<String>,
    #[serde(default)]
    service: Option<Value>,
    #[serde(default)]
    deprecated: Option<bool>,
    #[serde(default)]
    deprecation_reason: Option<String>,
    #[serde(default)]
    deprecation_replacement_formula: Option<String>,
    #[serde(default)]
    deprecation_replacement_cask: Option<String>,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    disable_reason: Option<String>,
    #[serde(default)]
    disable_replacement_formula: Option<String>,
    #[serde(default)]
    disable_replacement_cask: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawKeg {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    used_options: Vec<String>,
    #[serde(default)]
    poured_from_bottle: Option<bool>,
    #[serde(default)]
    built_as_bottle: Option<bool>,
    #[serde(default)]
    installed_on_request: Option<bool>,
    #[serde(default)]
    runtime_dependencies: Vec<RawRuntimeDependency>,
}

#[derive(Debug, Deserialize)]
struct RawRuntimeDependency {
    #[serde(default)]
    full_name: Option<String>,
    #[serde(default)]
    declared_directly: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawCask {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    installed: Option<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    tap: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    auto_updates: Option<bool>,
    #[serde(default)]
    deprecated: Option<bool>,
    #[serde(default)]
    deprecation_reason: Option<String>,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default)]
    disable_reason: Option<String>,
    #[serde(default)]
    pinned: bool,
}

// ---------------------------------------------------------------------------
// The validated shapes the adapter maps from.
// ---------------------------------------------------------------------------

/// The `versions` object. Only the stable version is read; `head` is not a
/// version this adapter can compare or migrate.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Versions {
    /// Newest stable version the provider offers, when it offers one.
    #[serde(default)]
    pub stable: Option<String>,
}

/// The `bottle` object. `{}` parses to a `Bottle` whose `stable` is `None`,
/// which is the observed representation of "no prebuilt artifact applies to
/// this machine" (R-2 finding 3). `null` is a shape change and is refused.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Bottle {
    /// The stable-version artifact record.
    #[serde(default)]
    pub stable: Option<BottleStable>,
}

/// The stable artifact record.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct BottleStable {
    /// Artifacts keyed by platform tag (`tahoe`, `sonoma`, `all`, ...).
    /// Under the default configuration this is a *resolution result* for
    /// this machine, not the published set (R-2 finding 3).
    #[serde(default)]
    pub files: BTreeMap<String, Value>,
}

/// Why a formula is kept off the default executable path.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct KegOnlyReason {
    /// A symbol (`:provided_by_macos`) or free text.
    #[serde(default)]
    pub reason: String,
    /// Prose, frequently empty (14 of 19 cases observed, R-1/R-6).
    #[serde(default)]
    pub explanation: String,
}

/// One row of `installed[]`: a keg in the rack. The array is ordered oldest
/// keg first (R-1 finding 7).
#[derive(Clone, Debug)]
pub struct Keg {
    /// Validated version, usable as a path component.
    pub version: String,
    /// Build options selected for this keg, sorted.
    pub used_options: Vec<String>,
    /// Whether the prebuilt artifact was used, per this keg.
    pub poured_from_bottle: Option<bool>,
    /// Whether the keg was itself built as a prebuilt artifact.
    pub built_as_bottle: Option<bool>,
    /// Homebrew's own record of user intent for this keg.
    pub installed_on_request: Option<bool>,
    /// Runtime dependency names, validated.
    pub runtime_dependencies: Vec<RuntimeDependency>,
}

/// One runtime dependency of a keg.
#[derive(Clone, Debug)]
pub struct RuntimeDependency {
    /// Validated dependency name.
    pub full_name: String,
    /// Whether the formula declares it directly.
    pub declared_directly: Option<bool>,
}

/// A validated formula.
#[derive(Clone, Debug)]
pub struct Formula {
    /// Validated name.
    pub name: String,
    /// Fully qualified name, when given.
    pub full_name: Option<String>,
    /// Kegs, oldest first.
    pub installed: Vec<Keg>,
    /// Catalogue versions.
    pub versions: Versions,
    /// Artifact record.
    pub bottle: Bottle,
    /// Tap the formula came from.
    pub tap: Option<String>,
    /// One-line description.
    pub desc: Option<String>,
    /// Licence expression.
    pub license: Option<String>,
    /// Upstream homepage.
    pub homepage: Option<String>,
    /// Provider-side revision.
    pub revision: u32,
    /// The version currently on the default path, when there is one.
    pub linked_keg: Option<String>,
    /// Held at its current version by the user.
    pub pinned: bool,
    /// Kept off the default path by design.
    pub keg_only: bool,
    /// Why, when the provider says.
    pub keg_only_reason: Option<KegOnlyReason>,
    /// Alternative names.
    pub aliases: Vec<String>,
    /// Versioned siblings.
    pub versioned_formulae: Vec<String>,
    /// Declared dependencies, used when a keg carries no runtime record.
    pub dependencies: Vec<String>,
    /// Build-time dependencies.
    pub build_dependencies: Vec<String>,
    /// Text shown after a change.
    pub caveats: Option<String>,
    /// Whether a background service block is defined.
    pub has_service: bool,
    /// Deprecation state, when the field was present.
    pub deprecated: Option<bool>,
    /// Stated deprecation reason.
    pub deprecation_reason: Option<String>,
    /// Disabled state, when the field was present.
    pub disabled: Option<bool>,
    /// Stated disable reason.
    pub disable_reason: Option<String>,
    /// What the provider says replaces this, formula first then cask.
    pub replaced_by: Option<String>,
}

impl Formula {
    /// The keg that is on the default path, or - when nothing is linked, as
    /// for a keg-only formula - the newest one (R-1 finding 7: reading
    /// `installed[0]` gives the wrong answer).
    pub fn active_keg(&self) -> Option<&Keg> {
        match &self.linked_keg {
            Some(linked) => self.installed.iter().find(|k| &k.version == linked),
            None => self.installed.last(),
        }
    }

    /// The platform tags this formula publishes an artifact for, as the
    /// document reports them.
    pub fn bottle_tags(&self) -> Vec<&str> {
        self.bottle
            .stable
            .as_ref()
            .map(|s| s.files.keys().map(String::as_str).collect())
            .unwrap_or_default()
    }
}

/// A validated cask. A cask carries no record of user intent and no keg
/// (R-1 finding 10), so it is a single row with no `requested` answer.
#[derive(Clone, Debug)]
pub struct Cask {
    /// Validated token: the cask's identity.
    pub token: String,
    /// Validated version as recorded.
    pub installed: String,
    /// One-line description.
    pub desc: Option<String>,
    /// Upstream homepage.
    pub homepage: Option<String>,
    /// Tap the cask came from, when recorded.
    pub tap: Option<String>,
    /// Newest catalogue version.
    pub version: Option<String>,
    /// Whether the vendor updates the application outside the provider.
    pub auto_updates: Option<bool>,
    /// Deprecation state, when the field was present.
    pub deprecated: Option<bool>,
    /// Stated deprecation reason.
    pub deprecation_reason: Option<String>,
    /// Disabled state, when the field was present.
    pub disabled: Option<bool>,
    /// Stated disable reason.
    pub disable_reason: Option<String>,
    /// Held at its current version by the user.
    pub pinned: bool,
}

/// A fact the parser actually saw in the document, as opposed to one the
/// adapter would otherwise have to assume from a version string. These are
/// what turn into detected capabilities (`02-domain-model.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Observed {
    /// At least one keg carried Homebrew's own record of user intent.
    RequestedRecord,
    /// At least one package carried a deprecation or disable flag.
    DeprecationState,
    /// At least one formula defines a background service.
    ServiceMetadata,
    /// At least one dependency edge could be read.
    DependencyRecords,
}

/// A parsed and validated document.
#[derive(Clone, Debug)]
pub struct Parsed {
    /// Formulae, in document order.
    pub formulae: Vec<Formula>,
    /// Casks, in document order.
    pub casks: Vec<Cask>,
    /// What the parser saw, which is the evidence behind every detected
    /// capability. An empty set never becomes a negative claim.
    pub observed: BTreeSet<Observed>,
}

impl Parsed {
    /// Whether the document demonstrated this fact.
    pub fn saw(&self, fact: Observed) -> bool {
        self.observed.contains(&fact)
    }
}

/// Parses `brew info --json=v2 --installed`.
///
/// The document is bounded first, then required keys are asserted with the
/// field named in the error, then every name and version that will become a
/// path component is validated.
pub fn parse_installed_json(text: &str) -> Result<Parsed, DomainError> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(changed(format!(
            "document is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            text.len()
        )));
    }
    let raw: RawDocument = serde_json::from_str(text).map_err(|e| {
        changed(format!(
            "`info --json=v2` document did not parse: {}",
            e.to_string().chars().take(300).collect::<String>()
        ))
    })?;
    let raw_formulae = raw
        .formulae
        .ok_or_else(|| changed("`info --json=v2` document has no `formulae` key"))?;
    let raw_casks = raw
        .casks
        .ok_or_else(|| changed("`info --json=v2` document has no `casks` key"))?;

    let mut parsed = Parsed {
        formulae: Vec::with_capacity(raw_formulae.len()),
        casks: Vec::with_capacity(raw_casks.len()),
        observed: BTreeSet::new(),
    };
    for (index, formula) in raw_formulae.into_iter().enumerate() {
        parsed.push_formula(index, formula)?;
    }
    for (index, cask) in raw_casks.into_iter().enumerate() {
        parsed.push_cask(index, cask)?;
    }
    Ok(parsed)
}

impl Parsed {
    fn push_formula(&mut self, index: usize, raw: RawFormula) -> Result<(), DomainError> {
        let label = raw.name.clone().unwrap_or_else(|| format!("index {index}"));
        let name = checked_name(
            raw.name
                .as_deref()
                .ok_or_else(|| changed(format!("formulae[{label}] has no `name`")))?,
            &format!("formulae[{label}].name"),
        )?;
        let raw_kegs = raw
            .installed
            .ok_or_else(|| changed(format!("formulae[{name}] has no `installed`")))?;
        let versions = raw
            .versions
            .ok_or_else(|| changed(format!("formulae[{name}] has no `versions`")))?;
        let bottle = raw
            .bottle
            .ok_or_else(|| changed(format!("formulae[{name}] has no `bottle` object")))?;
        let installed = self.parse_kegs(&name, raw_kegs)?;

        if raw.deprecated.is_some() || raw.disabled.is_some() {
            self.observed.insert(Observed::DeprecationState);
        }
        let has_service = raw.service.as_ref().is_some_and(|v| !v.is_null());
        if has_service {
            self.observed.insert(Observed::ServiceMetadata);
        }
        if !raw.dependencies.is_empty() {
            self.observed.insert(Observed::DependencyRecords);
        }
        let replaced_by = raw
            .disable_replacement_formula
            .or(raw.disable_replacement_cask)
            .or(raw.deprecation_replacement_formula)
            .or(raw.deprecation_replacement_cask);
        let dependencies = checked_names(&raw.dependencies, &name, "dependencies")?;
        let build_dependencies =
            checked_names(&raw.build_dependencies, &name, "build_dependencies")?;

        self.formulae.push(Formula {
            name,
            full_name: raw.full_name,
            installed,
            versions,
            bottle,
            tap: raw.tap,
            desc: raw.desc,
            license: raw.license,
            homepage: raw.homepage,
            revision: raw.revision,
            linked_keg: raw.linked_keg,
            pinned: raw.pinned,
            keg_only: raw.keg_only,
            keg_only_reason: raw.keg_only_reason,
            aliases: raw.aliases,
            versioned_formulae: raw.versioned_formulae,
            dependencies,
            build_dependencies,
            caveats: raw.caveats,
            has_service,
            deprecated: raw.deprecated,
            deprecation_reason: raw.deprecation_reason,
            disabled: raw.disabled,
            disable_reason: raw.disable_reason,
            replaced_by,
        });
        Ok(())
    }

    fn parse_kegs(&mut self, name: &str, raw_kegs: Vec<RawKeg>) -> Result<Vec<Keg>, DomainError> {
        let mut kegs = Vec::with_capacity(raw_kegs.len());
        for (index, raw) in raw_kegs.into_iter().enumerate() {
            let version = checked_version(
                raw.version.as_deref().ok_or_else(|| {
                    changed(format!(
                        "formulae[{name}].installed[{index}] has no `version`"
                    ))
                })?,
                &format!("formulae[{name}].installed[{index}].version"),
            )?;
            if raw.installed_on_request.is_some() {
                self.observed.insert(Observed::RequestedRecord);
            }
            let runtime_dependencies = self.parse_runtime(name, index, raw.runtime_dependencies)?;
            let mut used_options = raw.used_options;
            used_options.sort_unstable();
            used_options.dedup();
            kegs.push(Keg {
                version,
                used_options,
                poured_from_bottle: raw.poured_from_bottle,
                built_as_bottle: raw.built_as_bottle,
                installed_on_request: raw.installed_on_request,
                runtime_dependencies,
            });
        }
        Ok(kegs)
    }

    fn parse_runtime(
        &mut self,
        name: &str,
        keg: usize,
        raw: Vec<RawRuntimeDependency>,
    ) -> Result<Vec<RuntimeDependency>, DomainError> {
        let mut out = Vec::with_capacity(raw.len());
        for (index, dep) in raw.into_iter().enumerate() {
            let field = format!(
                "formulae[{name}].installed[{keg}].runtime_dependencies[{index}].full_name"
            );
            let full_name = checked_name(
                dep.full_name
                    .as_deref()
                    .ok_or_else(|| changed(format!("{field} is absent")))?,
                &field,
            )?;
            out.push(RuntimeDependency {
                full_name,
                declared_directly: dep.declared_directly,
            });
        }
        if !out.is_empty() {
            self.observed.insert(Observed::DependencyRecords);
        }
        Ok(out)
    }

    fn push_cask(&mut self, index: usize, raw: RawCask) -> Result<(), DomainError> {
        let label = raw
            .token
            .clone()
            .unwrap_or_else(|| format!("index {index}"));
        let token = checked_name(
            raw.token
                .as_deref()
                .ok_or_else(|| changed(format!("casks[{label}] has no `token`")))?,
            &format!("casks[{label}].token"),
        )?;
        let version = checked_version(
            raw.installed
                .as_deref()
                .ok_or_else(|| changed(format!("casks[{token}] has no `installed` version")))?,
            &format!("casks[{token}].installed"),
        )?;
        if raw.deprecated.is_some() || raw.disabled.is_some() {
            self.observed.insert(Observed::DeprecationState);
        }
        self.casks.push(Cask {
            token,
            installed: version,
            desc: raw.desc,
            homepage: raw.homepage,
            tap: raw.tap,
            version: raw.version,
            auto_updates: raw.auto_updates,
            deprecated: raw.deprecated,
            deprecation_reason: raw.deprecation_reason,
            disabled: raw.disabled,
            disable_reason: raw.disable_reason,
            pinned: raw.pinned,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
      "formulae": [{
        "name": "wget",
        "versions": {"stable": "1.25.0"},
        "bottle": {"stable": {"files": {"sonoma": {"url": "x"}}}},
        "linked_keg": "1.25.0",
        "installed": [{"version": "1.25.0", "poured_from_bottle": true,
                       "installed_on_request": true,
                       "runtime_dependencies": [{"full_name": "libidn2"}]}]
      }],
      "casks": [{"token": "gimp", "installed": "3.2.4"}]
    }"#;

    #[test]
    fn a_minimal_document_parses_and_reports_its_capabilities() {
        let parsed = parse_installed_json(MINIMAL).unwrap();
        assert_eq!(parsed.formulae.len(), 1);
        assert_eq!(parsed.casks.len(), 1);
        assert!(parsed.saw(Observed::RequestedRecord));
        assert!(parsed.saw(Observed::DependencyRecords));
        assert!(!parsed.saw(Observed::ServiceMetadata));
        assert_eq!(parsed.formulae[0].bottle_tags(), vec!["sonoma"]);
        assert_eq!(parsed.formulae[0].active_keg().unwrap().version, "1.25.0");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let text = MINIMAL.replace(r#""name": "wget""#, r#""name": "wget", "brand_new_key": 7"#);
        assert!(parse_installed_json(&text).is_ok());
    }

    #[test]
    fn a_missing_required_key_names_the_field() {
        for (needle, field) in [
            (r#""installed": [{"version": "1.25.0""#, "installed"),
            (r#""versions": {"stable": "1.25.0"}"#, "versions"),
        ] {
            let text = MINIMAL.replace(needle, r#""absent": [{"version": "1.25.0""#);
            let err = parse_installed_json(&text).unwrap_err();
            assert_eq!(err.id, ErrorId::ProviderOutputChanged);
            assert!(
                err.detail.contains(field) || err.detail.contains("did not parse"),
                "{}",
                err.detail
            );
        }
    }

    #[test]
    fn a_bottle_that_is_not_an_object_is_a_shape_change() {
        let text = MINIMAL.replace(
            r#""bottle": {"stable": {"files": {"sonoma": {"url": "x"}}}}"#,
            r#""bottle": null"#,
        );
        let err = parse_installed_json(&text).unwrap_err();
        assert_eq!(err.id, ErrorId::ProviderOutputChanged);
        assert!(err.detail.contains("bottle"), "{}", err.detail);
    }

    #[test]
    fn an_empty_bottle_object_is_valid_and_carries_no_tag() {
        let text = MINIMAL.replace(
            r#""bottle": {"stable": {"files": {"sonoma": {"url": "x"}}}}"#,
            r#""bottle": {}"#,
        );
        let parsed = parse_installed_json(&text).unwrap();
        assert!(parsed.formulae[0].bottle_tags().is_empty());
    }

    #[test]
    fn a_document_without_the_two_top_level_keys_is_a_shape_change() {
        let err = parse_installed_json(r#"{"formulae": []}"#).unwrap_err();
        assert!(err.detail.contains("casks"), "{}", err.detail);
        let err = parse_installed_json(r#"{"casks": []}"#).unwrap_err();
        assert!(err.detail.contains("formulae"), "{}", err.detail);
    }

    #[test]
    fn names_that_could_escape_a_directory_are_refused() {
        for hostile in ["../../etc/passwd", "..", "-s", "a/b", "a\u{0}b", "wgét"] {
            let err = checked_name(hostile, "formulae[x].name").unwrap_err();
            assert_eq!(err.id, ErrorId::ProviderOutputChanged);
            assert!(err.detail.contains("formulae[x].name"), "{}", err.detail);
        }
        assert_eq!(checked_name("php@8.4", "f").unwrap(), "php@8.4");
        assert_eq!(checked_name("openssl@3", "f").unwrap(), "openssl@3");
        assert_eq!(checked_version("16.0.1,9", "v").unwrap(), "16.0.1,9");
        assert_eq!(checked_version("3.22.3+105", "v").unwrap(), "3.22.3+105");
        assert!(checked_name(&"a".repeat(MAX_COMPONENT_LEN + 1), "f").is_err());
        assert!(checked_name("", "f").is_err());
    }

    #[test]
    fn the_newest_keg_is_active_when_nothing_is_on_the_default_path() {
        let text = MINIMAL
            .replace(r#""linked_keg": "1.25.0","#, r#""linked_keg": null,"#)
            .replace(
                r#""installed": [{"version": "1.25.0""#,
                r#""installed": [{"version": "1.24.0"}, {"version": "1.25.0""#,
            );
        let parsed = parse_installed_json(&text).unwrap();
        assert_eq!(parsed.formulae[0].installed.len(), 2);
        assert_eq!(parsed.formulae[0].active_keg().unwrap().version, "1.25.0");
    }
}
