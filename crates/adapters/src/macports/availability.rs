// SPDX-License-Identifier: Apache-2.0

//! Prebuilt-archive availability for MacPorts, assessed offline.
//!
//! R-4 Finding 9 is the rule this module exists to obey: **availability is
//! not determinable offline in the positive direction.** Nothing in the
//! registry, the PortIndex or any `port` output says whether an archive is
//! published for a port. So this module never returns
//! [`ArtifactAvailability::BinaryAvailable`]; it returns the negative
//! determinations R-4 Finding 10 says *are* offline-decidable, and
//! [`ArtifactAvailability::Unknown`] with its evidence for everything else.
//!
//! The online check R-4 Finding 8 names is left for a later pass:
//! [`archive_query_url`] builds the URL and nothing here fetches it.

use super::names::is_safe_name;
use macportsbrew_domain::availability::SourceBuildReason;
use macportsbrew_domain::{
    ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, Machine,
};
use std::collections::BTreeSet;

/// The documented per-port build-status endpoint (R-4 E-8, Finding 8), as a
/// format whose only variable part is the port name.
pub const ARCHIVE_QUERY_URL_FORMAT: &str =
    "https://ports.macports.org/api/v1/builds/?port_name=<name>";

/// The constant part of [`ARCHIVE_QUERY_URL_FORMAT`].
const ARCHIVE_QUERY_URL_PREFIX: &str = "https://ports.macports.org/api/v1/builds/?port_name=";

/// The URL a later, network-enabled pass would query for `name`.
///
/// `None` for a name this adapter would not pass anywhere, so provider output
/// can never become a URL component unchecked. Nothing in this pass fetches
/// it; `analyze` is offline (INV-15).
pub fn archive_query_url(name: &str) -> Option<String> {
    is_safe_name(name).then(|| format!("{ARCHIVE_QUERY_URL_PREFIX}{name}"))
}

/// The platform tuple an archive filename is keyed on (R-4 E-4, Finding 2),
/// rendered for evidence. `None` when the assessment was made before the
/// machine was read, which is the case during `inventory()`.
fn platform_tuple(machine: Option<&Machine>) -> String {
    machine.map_or_else(
        || "this machine's (darwin_<macos-major>, arch) tuple".to_string(),
        |m| format!("darwin_{}.{}", m.macos.major, m.architecture),
    )
}

/// Whether this variant set is *known* to have no published archive.
///
/// R-4 Finding 2: the archive filename carries the canonical **active**
/// variant string, so any variant set other than the one the build farm
/// installs has no published file. Offline, though, the default variant set
/// is not knowable: R-4 E-5 shows `wget-1.25.0_1+gnutls...` published by the
/// farm, i.e. `+gnutls` is `wget`'s *default* set and appears in the filename
/// anyway. So the presence of a `+variant` alone proves nothing.
///
/// Two cases do prove it without the PortIndex's default-variant list:
///
/// - `+universal`, which changes the architecture component as well as the
///   variant component (R-4 Finding 4, `get_canonical_archs`), and which the
///   build farm never selects (R-4 E-6: `mpbb-install-port` passes bare
///   subport names);
/// - an explicitly negated variant (`-x11`), which can only come from the
///   user turning a default off.
///
/// Everything else stays `None` and is reported as unknown, because saying
/// `SOURCE_BUILD_REQUIRED` for every port that has any variant would be
/// wrong for the majority of a normal installation.
pub fn forced_source_build(variants: &BTreeSet<String>) -> Option<String> {
    if variants.iter().any(|v| v == "+universal") {
        return Some(
            "the +universal variant changes both the variant and the architecture component of \
             the archive filename, and the build farm publishes neither (R-4 Findings 3 and 4)"
                .to_string(),
        );
    }
    let negated: Vec<&str> = variants
        .iter()
        .filter(|v| v.starts_with('-'))
        .map(String::as_str)
        .collect();
    if negated.is_empty() {
        return None;
    }
    Some(format!(
        "the variant set explicitly turns off {}, so the canonical variant string differs from \
         the one the build farm publishes (R-4 Finding 3)",
        negated.join(" ")
    ))
}

/// What is known about `name` at read time.
pub struct Facts<'a> {
    /// The port name.
    pub name: &'a str,
    /// Whether the local PortIndex answered for this name.
    pub in_index: bool,
    /// The successor the tree names, when any.
    pub replaced_by: Option<&'a str>,
    /// Variants on the active installed row, sign included.
    pub variants: &'a BTreeSet<String>,
}

/// Assesses availability from local facts only.
///
/// `machine` sharpens the evidence text with the platform tuple the archive
/// filename would carry; it never changes the state, because no state here
/// is platform-dependent offline.
pub fn assess(facts: &Facts<'_>, machine: Option<&Machine>) -> AvailabilityAssessment {
    if let Some(successor) = facts.replaced_by {
        return one(
            ArtifactAvailability::Deprecated,
            "port info --index --replaced_by",
            format!("the ports tree names `{successor}` as the replacement for this port"),
        );
    }
    if let Some(detail) = forced_source_build(facts.variants) {
        return AvailabilityAssessment {
            availability: ArtifactAvailability::SourceBuildRequired {
                reason: SourceBuildReason::NonDefaultOptions,
            },
            evidence: vec![
                AvailabilityEvidence {
                    source: "port -q installed (variant set on the active row)".to_string(),
                    detail,
                    offline: true,
                },
                AvailabilityEvidence {
                    source: "archive_sites".to_string(),
                    detail: format!(
                        "the archive filename is keyed on the canonical variant set and on \
                         {} (R-4 Finding 2)",
                        platform_tuple(machine)
                    ),
                    offline: true,
                },
            ],
            cached: false,
        };
    }
    if !facts.in_index {
        return one(
            ArtifactAvailability::Unknown,
            "port info --index",
            format!(
                "the local port index did not answer for `{}`; whether an archive exists is not \
                 decidable from it",
                facts.name
            ),
        );
    }
    let mut evidence = vec![AvailabilityEvidence {
        source: "archive_sites".to_string(),
        detail: format!(
            "binary archive availability requires the network (ports.macports.org builds API); \
             not queried offline. The filename would be keyed on {} (R-4 Findings 2 and 9).",
            platform_tuple(machine)
        ),
        offline: true,
    }];
    if !facts.variants.is_empty() {
        evidence.push(AvailabilityEvidence {
            source: "port -q installed (variant set on the active row)".to_string(),
            detail: format!(
                "the active row carries {}; whether that is this port's default set is not \
                 decidable offline (R-4 E-5)",
                facts
                    .variants
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            offline: true,
        });
    }
    AvailabilityAssessment {
        availability: ArtifactAvailability::Unknown,
        evidence,
        cached: false,
    }
}

fn one(availability: ArtifactAvailability, source: &str, detail: String) -> AvailabilityAssessment {
    AvailabilityAssessment {
        availability,
        evidence: vec![AvailabilityEvidence {
            source: source.to_string(),
            detail,
            offline: true,
        }],
        cached: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::{Architecture, CommandLineTools, MacosVersion, Translation};

    fn machine() -> Machine {
        Machine {
            architecture: Architecture::X86_64,
            macos: MacosVersion::new(12, 7, 6),
            translation: Translation::Native,
            command_line_tools: CommandLineTools::Unknown,
        }
    }

    fn variants(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_replaced_port_is_deprecated() {
        let v = variants(&[]);
        let a = assess(
            &Facts {
                name: "py-pygments",
                in_index: true,
                replaced_by: Some("py312-pygments"),
                variants: &v,
            },
            Some(&machine()),
        );
        assert_eq!(a.availability, ArtifactAvailability::Deprecated);
        assert!(!a.evidence.is_empty());
    }

    #[test]
    fn universal_forces_a_source_build() {
        let v = variants(&["+universal"]);
        let a = assess(
            &Facts {
                name: "gnutls",
                in_index: true,
                replaced_by: None,
                variants: &v,
            },
            Some(&machine()),
        );
        assert_eq!(
            a.availability,
            ArtifactAvailability::SourceBuildRequired {
                reason: SourceBuildReason::NonDefaultOptions
            }
        );
        assert!(a.evidence.iter().all(|e| e.offline));
    }

    #[test]
    fn a_negated_variant_forces_a_source_build() {
        let v = variants(&["-x11"]);
        assert!(forced_source_build(&v).is_some());
    }

    #[test]
    fn a_plain_variant_proves_nothing_offline() {
        let v = variants(&["+gnutls"]);
        assert!(forced_source_build(&v).is_none());
        let a = assess(
            &Facts {
                name: "wget",
                in_index: true,
                replaced_by: None,
                variants: &v,
            },
            Some(&machine()),
        );
        assert_eq!(a.availability, ArtifactAvailability::Unknown);
        assert_eq!(a.evidence.len(), 2);
    }

    #[test]
    fn a_plain_port_is_unknown_with_the_network_evidence() {
        let v = variants(&[]);
        let a = assess(
            &Facts {
                name: "zlib",
                in_index: true,
                replaced_by: None,
                variants: &v,
            },
            Some(&machine()),
        );
        assert_eq!(a.availability, ArtifactAvailability::Unknown);
        assert_eq!(a.evidence[0].source, "archive_sites");
        assert!(a.evidence[0].detail.contains("requires the network"));
        assert!(a.evidence[0].offline);
        assert!(!a.is_prebuilt());
    }

    #[test]
    fn a_name_missing_from_the_index_is_unknown_not_unavailable() {
        let v = variants(&[]);
        let a = assess(
            &Facts {
                name: "local-only-port",
                in_index: false,
                replaced_by: None,
                variants: &v,
            },
            Some(&machine()),
        );
        assert_eq!(a.availability, ArtifactAvailability::Unknown);
    }

    #[test]
    fn the_query_url_is_built_only_for_a_safe_name() {
        assert_eq!(
            archive_query_url("wget").as_deref(),
            Some("https://ports.macports.org/api/v1/builds/?port_name=wget")
        );
        assert_eq!(archive_query_url("../etc"), None);
        assert_eq!(archive_query_url("-flag"), None);
        assert!(ARCHIVE_QUERY_URL_FORMAT.contains("<name>"));
    }
}
