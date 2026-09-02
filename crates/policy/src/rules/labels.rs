// SPDX-License-Identifier: Apache-2.0

//! Stable text for domain enumerations, for use as reason parameters.
//!
//! Reason parameters are rendered into a sentence by the UI, so their values
//! have to be stable identifiers rather than `Debug` output: `Debug` is a
//! developer convenience that no compatibility promise covers, and a
//! translation keyed on it would break silently. These functions name the
//! same text the serialised form uses.

// `SourceBuildReason` is not re-exported from the domain crate root, only
// from its module; the path below is the only way to reach it today.
use macportsbrew_domain::availability::SourceBuildReason;
use macportsbrew_domain::{
    ArtifactAvailability, Confidence, Ownership, PackageClass, SupportTier, Translation,
};

/// Stable text for a translation state.
pub fn translation(value: Translation) -> &'static str {
    match value {
        Translation::Native => "NATIVE",
        Translation::Translated => "TRANSLATED",
        Translation::Unknown => "UNKNOWN",
    }
}

/// Stable text for an ownership state.
pub fn ownership(value: Ownership) -> &'static str {
    match value {
        Ownership::TopLevel => "TOP_LEVEL",
        Ownership::Dependency => "DEPENDENCY",
        Ownership::Orphan => "ORPHAN",
        Ownership::Unknown => "UNKNOWN",
    }
}

/// Stable text for a package class.
pub fn class(value: PackageClass) -> &'static str {
    match value {
        PackageClass::Simple => "SIMPLE",
        PackageClass::Stateful => "STATEFUL",
        PackageClass::Service => "SERVICE",
        PackageClass::Runtime => "RUNTIME",
        PackageClass::GuiApp => "GUI_APP",
        PackageClass::ThirdPartySourced => "THIRD_PARTY_SOURCED",
        PackageClass::ProviderInternal => "PROVIDER_INTERNAL",
    }
}

/// Stable text for a mapping confidence.
pub fn confidence(value: Confidence) -> &'static str {
    match value {
        Confidence::None => "NONE",
        Confidence::Ambiguous => "AMBIGUOUS",
        Confidence::Probable => "PROBABLE",
        Confidence::Strong => "STRONG",
        Confidence::Exact => "EXACT",
    }
}

/// Stable text for a support tier.
pub fn tier(value: SupportTier) -> &'static str {
    match value {
        SupportTier::Supported => "SUPPORTED",
        SupportTier::Degraded => "DEGRADED",
        SupportTier::Unsupported => "UNSUPPORTED",
        SupportTier::Unknown => "UNKNOWN",
    }
}

/// Stable text for an artifact-availability state.
pub fn availability(value: &ArtifactAvailability) -> &'static str {
    match value {
        ArtifactAvailability::BinaryAvailable => "BINARY_AVAILABLE",
        ArtifactAvailability::SourceBuildRequired { .. } => "SOURCE_BUILD_REQUIRED",
        ArtifactAvailability::Unavailable => "UNAVAILABLE",
        ArtifactAvailability::UnsupportedPlatform => "UNSUPPORTED_PLATFORM",
        ArtifactAvailability::Deprecated => "DEPRECATED",
        ArtifactAvailability::Disabled => "DISABLED",
        ArtifactAvailability::Unknown => "UNKNOWN",
    }
}

/// Stable text for why a source build would happen.
pub fn source_build_reason(value: SourceBuildReason) -> &'static str {
    match value {
        SourceBuildReason::NoArtifactForThisPlatform => "NO_ARTIFACT_FOR_THIS_PLATFORM",
        SourceBuildReason::NonDefaultOptions => "NON_DEFAULT_OPTIONS",
        SourceBuildReason::LicenceRestricted => "LICENCE_RESTRICTED",
        SourceBuildReason::ForcedByConfiguration => "FORCED_BY_CONFIGURATION",
    }
}
