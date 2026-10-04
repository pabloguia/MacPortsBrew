// SPDX-License-Identifier: Apache-2.0

//! MacPortsBrew domain layer: provider-neutral types and pure rules.
//!
//! No I/O, no clock, no environment access, no provider names in any
//! signature (`docs/spec/02-domain-model.md`, "Shape"). The layering is a
//! Cargo dependency edge: this crate depends on `serde` and nothing else in
//! the workspace, and `scripts/check-layering.sh` asserts it.
//!
//! Every collection that reaches a serialised surface is ordered
//! (`BTreeMap`, `BTreeSet`, sorted `Vec`) so that the same input produces the
//! same bytes (INV-15).

pub mod availability;
pub mod class;
pub mod concurrency;
pub mod error;
pub mod history;
pub mod machine;
pub mod mapping;
pub mod ownership;
pub mod package;
pub mod plan;
pub mod provider;
pub mod reason;
pub mod recommendation;
pub mod resolution;
pub mod snapshot;
pub mod support;
pub mod text;
pub mod verification;
pub mod version;

pub use availability::{
    ArtifactAvailability, AvailabilityAssessment, AvailabilityEvidence, SourceBuildReason,
};
pub use class::{ClassEvidence, ClassSet, PackageClass};
pub use concurrency::{ConcurrencyFlag, ConcurrentProviderActivity};
pub use error::{DomainError, ErrorId};
pub use history::{
    Direction, EnvironmentChange, HistoryView, MigrationKey, NotRestorable, RecordedStep,
    RecordedVersions, Restorability, RunEnding, RunRecord, RunStart, RunStatus, RunSubject,
    StepResult, StoredEnvelope, StoredRunRecord, HISTORY_SCHEMA_VERSION,
};
pub use machine::{
    Architecture, CommandLineTools, Machine, MachineRead, MacosVersion, Translation,
};
pub use mapping::{Confidence, MappingCandidate, MappingEvidence, PackageMapping};
pub use ownership::{classify_ownership as classify_ownership_for, Ownership, OwnershipEvidence};
pub use package::{
    DependencyEdge, InstalledPackage, PackageIdentity, PackageKind, PackageMetadata,
};
pub use plan::{
    CommandTarget, Elevation, MigrationPlan, MigrationStep, PlanCommand, PlanRisk, StepKind,
    SuccessSemantic, VerificationCheck,
};
pub use provider::{
    Capability, CapabilityState, ProviderCapabilities, ProviderDetection, ProviderId,
    ProviderInventory, ProviderPresence, ProviderRead, RequestedSet,
};
pub use reason::{Reason, ReasonId};
pub use recommendation::{Outcome, Recommendation};
pub use resolution::{ExecutableOwner, Resolution};
pub use snapshot::{
    is_build_flag, is_safe_captured_path, is_safe_configuration_key, is_safe_path,
    is_safe_provider_name, is_safe_provider_text, is_safe_provider_version,
    path_starts_with_insensitive, redact_home, ConfigurationFile, ConfigurationScope,
    EnvironmentSnapshot, HomeRedaction, PathFinding, PathFindingKind, Provenance,
    ProviderConfiguration, ProviderEnvironmentContract, ProviderSnapshot, Snapshot, MAX_PATH_LEN,
    MAX_TEXT_LEN, PINNED_ENVIRONMENT, SNAPSHOT_SCHEMA_VERSION, SYSTEM_PATH,
};
pub use support::{DatedFact, SupportState, SupportTier};
pub use text::{
    escape_hostile, hostile_class, is_control_class, is_hostile, CharacterSample, HostileClass,
    CHARACTER_SAMPLES, LEGITIMATE_TEXT_WITH_FORMAT,
};
pub use verification::{
    evaluate, expectation_for, stdout_names, DestinationName, Expectation, ExpectationError,
    FailureCause, ProcessEnd, ReadEvidence, ReadOutput, SourceName, Verdict,
};
pub use version::Version;
