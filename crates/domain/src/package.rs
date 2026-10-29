// SPDX-License-Identifier: Apache-2.0

//! Packages as installed and as described by a provider.

use crate::provider::ProviderId;
use crate::version::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

/// What kind of thing a provider calls a package. `GuiApp` is the Homebrew
/// cask concept; the other provider has no equivalent (R-1, SE-012).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackageKind {
    /// A formula, a port: built or prebuilt software under the prefix.
    Package,
    /// A prebuilt, vendor-signed application (a cask). Out of migration scope.
    GuiApp,
}

/// A package name within one provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIdentity {
    /// Owning provider.
    pub provider: ProviderId,
    /// Name as the provider spells it (`python@3.12`, `python312`).
    pub name: String,
}

impl PackageIdentity {
    /// Creates an identity.
    pub fn new(provider: ProviderId, name: impl Into<String>) -> Self {
        Self {
            provider,
            name: name.into(),
        }
    }
}

impl fmt::Display for PackageIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.name)
    }
}

/// One installed row. A provider may report several rows for one name -
/// an active version and inactive ones (R-3) - so this is a row, not a
/// summary.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackage {
    /// Which package.
    pub identity: PackageIdentity,
    /// Package or GUI application.
    pub kind: PackageKind,
    /// Installed version.
    pub version: Version,
    /// Provider-side revision, when the provider has the concept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// Build variants or options selected at install time, sorted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub variants: BTreeSet<String>,
    /// Whether this row is the active (linked, selected) one.
    pub active: bool,
    /// Whether the provider recorded this as installed on the user's
    /// request. `None` when the provider keeps no such record for this kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested: Option<bool>,
    /// Whether the prebuilt artifact was used at install time, when the
    /// provider records it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_from_prebuilt: Option<bool>,
    /// Where the provider says the package lives, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_path: Option<PathBuf>,
}

/// A directed edge: `from` depends on `to`, both within one provider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyEdge {
    /// Dependent.
    pub from: String,
    /// Dependency.
    pub to: String,
    /// Build-time only, when the provider distinguishes.
    #[serde(default)]
    pub build_only: bool,
}

/// Descriptive metadata about a package, provider-neutral.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMetadata {
    /// One-line description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Upstream homepage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Licence expression as the provider states it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Newest version the provider offers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_version: Option<Version>,
    /// Origin: a tap name, a source, a local file. `None` means the
    /// provider's core catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Deprecated by the provider, with the reason it gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    /// Disabled (no longer installable) with the reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<String>,
    /// The provider says another package replaces this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<String>,
    /// Pinned or held by the user.
    #[serde(default)]
    pub pinned: bool,
    /// Kept out of the default executable path by the provider's design,
    /// with the provider's stated reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_on_default_path_reason: Option<String>,
    /// Whether the package defines a background service, **as a bool for older
    /// readers**. Written from [`ServiceDefinition::defines_service`] (`true`
    /// only for [`Answer::Yes`]) and never read to decide anything: a `false`
    /// here cannot tell "no" from "unknown". [`Self::service_answer`] is the
    /// question this build asks (DP-032, SE-109).
    #[serde(default)]
    pub defines_service: bool,
    /// The two halves of the service question (DP-032). Absent from a document
    /// written before it existed, which answers [`Answer::Unknown`] whatever
    /// the bool beside it says: that bool was one half only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceDefinition>,
    /// Executables the package provides, sorted, when known.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub executables: BTreeSet<String>,
    /// Alternative names the provider accepts, sorted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub aliases: BTreeSet<String>,
    /// Versioned siblings (`python@3.11`, `python@3.13`), sorted.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub versioned_siblings: BTreeSet<String>,
    /// Text the provider shows after install (caveats, notes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl PackageMetadata {
    /// Whether this package defines a background service: the rule's answer
    /// over [`Self::service`], and [`Answer::Unknown`] when the row does not
    /// carry it — never the bool, which held one half only (DP-032, V1).
    #[must_use]
    pub fn service_answer(&self) -> Answer {
        self.service
            .map_or(Answer::Unknown, ServiceDefinition::defines_service)
    }

    /// Records `service` and writes the older bool from the rule, so the two
    /// cannot disagree in anything this build produces.
    #[must_use]
    pub fn with_service(mut self, service: ServiceDefinition) -> Self {
        self.defines_service = service.defines_service() == Answer::Yes;
        self.service = Some(service);
        self
    }
}

/// A three-valued answer: a yes, a no, or the honest "could not tell".
///
/// `UNKNOWN` is a real state (`02-domain-model.md`): it is never read as `NO`.
/// A value written by a newer build that this one does not know reads as
/// `UNKNOWN` (`serde(other)`), the additive rule of `snapshot.v1.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Answer {
    /// Established, and true.
    Yes,
    /// Established, and false.
    No,
    /// Not established.
    #[serde(other)]
    Unknown,
}

impl From<bool> for Answer {
    /// An established answer. There is no `bool` that means "unknown".
    fn from(value: bool) -> Self {
        if value {
            Self::Yes
        } else {
            Self::No
        }
    }
}

/// What a provider says about a package's background service, in the two
/// halves DP-032 ruled: what the package **declares**, and what the provider
/// **materialised** for it. Each adapter supplies both; the domain decides from
/// them with one rule, [`Self::defines_service`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDefinition {
    /// The package's own description declares a service.
    pub declared: Answer,
    /// The provider wrote launch machinery for it on disk.
    pub materialised: Answer,
}

impl ServiceDefinition {
    /// The same answer for both halves.
    #[must_use]
    pub const fn both(answer: Answer) -> Self {
        Self {
            declared: answer,
            materialised: answer,
        }
    }

    /// **The rule, stated once** (DP-032): the union. `YES` if either half is
    /// `YES`; `NO` only if both are `NO`; otherwise `UNKNOWN`. A service either
    /// half can see is a service, and one half that cannot look means the
    /// question is not answered.
    #[must_use]
    pub const fn defines_service(self) -> Answer {
        match (self.declared, self.materialised) {
            (Answer::Yes, _) | (_, Answer::Yes) => Answer::Yes,
            (Answer::No, Answer::No) => Answer::No,
            (Answer::Unknown, _) | (_, Answer::Unknown) => Answer::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_union_is_the_rule_in_all_nine_cells() {
        use Answer::{No, Unknown, Yes};
        for (declared, materialised, want) in [
            (Yes, Yes, Yes),
            (Yes, No, Yes),
            (Yes, Unknown, Yes),
            (No, Yes, Yes),
            (Unknown, Yes, Yes),
            (No, No, No),
            (No, Unknown, Unknown),
            (Unknown, No, Unknown),
            (Unknown, Unknown, Unknown),
        ] {
            assert_eq!(
                ServiceDefinition {
                    declared,
                    materialised
                }
                .defines_service(),
                want,
                "{declared:?} + {materialised:?}"
            );
        }
    }

    #[test]
    fn the_bool_is_written_from_the_rule() {
        for (definition, want) in [
            (
                ServiceDefinition {
                    declared: Answer::Yes,
                    materialised: Answer::No,
                },
                true,
            ),
            (
                ServiceDefinition {
                    declared: Answer::No,
                    materialised: Answer::Yes,
                },
                true,
            ),
            (
                ServiceDefinition {
                    declared: Answer::No,
                    materialised: Answer::Unknown,
                },
                false,
            ),
            (ServiceDefinition::both(Answer::No), false),
        ] {
            let meta = PackageMetadata::default().with_service(definition);
            assert_eq!(meta.defines_service, want, "{definition:?}");
            assert_eq!(meta.service, Some(definition));
        }
    }

    #[test]
    fn a_row_without_the_service_question_answers_unknown_whatever_its_bool_says() {
        // V1: an older document's bool held one half only.
        for bool_says in [true, false] {
            let meta = PackageMetadata {
                defines_service: bool_says,
                ..PackageMetadata::default()
            };
            assert_eq!(meta.service_answer(), Answer::Unknown);
        }
        let read: PackageMetadata =
            serde_json::from_str(r#"{"definesService":true}"#).expect("an older row");
        assert_eq!(read.service_answer(), Answer::Unknown);
    }

    #[test]
    fn an_answer_from_a_newer_build_reads_as_unknown() {
        let read: ServiceDefinition =
            serde_json::from_str(r#"{"declared":"PROBABLY","materialised":"NO"}"#)
                .expect("an unknown variant parses");
        assert_eq!(read.declared, Answer::Unknown);
        assert_eq!(read.defines_service(), Answer::Unknown);
        assert_eq!(
            serde_json::to_string(&ServiceDefinition::both(Answer::Yes)).unwrap(),
            r#"{"declared":"YES","materialised":"YES"}"#
        );
    }
}
