// SPDX-License-Identifier: Apache-2.0

//! The reason catalogue.
//!
//! A reason is a stable lower-case dotted identifier plus named parameters.
//! It carries no display text: the UI renders it in the user's locale
//! (`docs/spec/06-cli-and-ux.md`, i18n). Every identifier this crate can emit
//! is listed here, and [`all`] exists so that the UI can assert its
//! catalogue covers the engine rather than discovering a gap in production.
//!
//! Adding an identifier is a compatibility change: it needs a translation in
//! every supported locale before it ships.

/// A class on the package forces a human to look (INV-19).
/// Parameters: `class`, `package`.
pub const CLASS_REQUIRES_REVIEW: &str = "reason.class_requires_review";

/// The package belongs to a curated family that owns persistent data outside
/// its own prefix (INV-19). Parameters: `family`, `package`, `provider`.
pub const PACKAGE_OWNS_PERSISTENT_DATA: &str = "reason.package_owns_persistent_data";

/// A candidate equivalent exists but the evidence is too thin to act on.
/// Parameters: `destination`, `confidence`, `threshold`.
pub const MAPPING_CONFIDENCE_BELOW_THRESHOLD: &str = "reason.mapping_confidence_below_threshold";

/// Whether a prebuilt artifact applies could not be determined.
/// Parameters: `provider`, `package`.
pub const AVAILABILITY_UNKNOWN: &str = "reason.availability_unknown";

/// Other packages inside the same provider depend on this one (INV-04).
/// Parameters: `count`, `provider`.
pub const REVERSE_DEPENDENCIES_PRESENT: &str = "reason.reverse_dependencies_present";

/// The destination provider marks the equivalent deprecated, disabled or
/// superseded. Parameters: `destination`, `state`, optionally `replacedBy`.
pub const DESTINATION_DEPRECATED: &str = "reason.destination_deprecated";

/// The package is a language runtime; a version manager is the right tool.
/// Parameters: `family`, `managers`.
pub const RUNTIME_FAMILY_MANAGED_ELSEWHERE: &str = "reason.runtime_family_managed_elsewhere";

/// The provider holding the package has stated an end to support for this
/// machine. Parameters: `provider`, `tier`, `endDate`, `daysRemaining`,
/// `weight`, `destination`.
pub const SOURCE_SUPPORT_ENDING: &str = "reason.source_support_ending";

/// The current side would compile from source where the other side offers a
/// prebuilt artifact. Parameters: `source`, `destination`, `cause`.
pub const DESTINATION_OFFERS_PREBUILT: &str = "reason.destination_offers_prebuilt";

/// The other side states it does not support this machine, so it is not a
/// destination. Parameters: `destination`, `entry`.
pub const DESTINATION_UNSUPPORTED_HERE: &str = "reason.destination_unsupported_here";

/// The package is in place under both providers and both answer.
/// Parameters: `primary`, `other`, `criterion`.
pub const PRESENT_UNDER_BOTH_PROVIDERS: &str = "reason.present_under_both_providers";

/// The user asked for the package to exist under both providers (INV-18).
/// Parameters: `primary`, `other`.
pub const COEXISTENCE_REQUESTED: &str = "reason.coexistence_requested";

/// Nothing argues for moving it, so it stays where it is.
/// Parameters: `provider`, `package`.
pub const KEEP_SOURCE_DEFAULT: &str = "reason.keep_source_default";

/// There is no other provider on this machine to move it to.
/// Parameters: `provider`, `package`.
pub const NO_ALTERNATIVE_PROVIDER_PRESENT: &str = "reason.no_alternative_provider_present";

/// Every identifier this crate can emit, sorted.
pub fn all() -> &'static [&'static str] {
    &[
        AVAILABILITY_UNKNOWN,
        CLASS_REQUIRES_REVIEW,
        COEXISTENCE_REQUESTED,
        DESTINATION_DEPRECATED,
        DESTINATION_OFFERS_PREBUILT,
        DESTINATION_UNSUPPORTED_HERE,
        KEEP_SOURCE_DEFAULT,
        MAPPING_CONFIDENCE_BELOW_THRESHOLD,
        NO_ALTERNATIVE_PROVIDER_PRESENT,
        PACKAGE_OWNS_PERSISTENT_DATA,
        PRESENT_UNDER_BOTH_PROVIDERS,
        REVERSE_DEPENDENCIES_PRESENT,
        RUNTIME_FAMILY_MANAGED_ELSEWHERE,
        SOURCE_SUPPORT_ENDING,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_catalogue_is_sorted_distinct_and_well_formed() {
        let mut sorted = all().to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted.as_slice(), all(), "reasons::all() must stay sorted");

        let distinct: BTreeSet<_> = all().iter().collect();
        assert_eq!(distinct.len(), all().len());

        for id in all() {
            assert!(id.starts_with("reason."), "{id}");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_'),
                "{id} must be lower-case dotted"
            );
        }
    }
}
