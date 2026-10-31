// SPDX-License-Identifier: Apache-2.0

//! The provider configuration, checked again immediately before a mutating
//! step (`04-migration-and-safety.md`, Process execution; SE-016's
//! check-then-use window; DP-027 ruling (b); pass 08 slice 3, D-R1′).
//!
//! A plan is built from a capture, and the gate refuses a mutating step against
//! a provider whose captured configuration outranks the invocation. Between the
//! capture and the step, a `brew.env` or a user `macports.conf` can appear. The
//! executor shell re-reads the files (`adapters::configuration::read`, filesystem
//! only, `Unknowable` off this machine) and asks this function before each
//! mutating dispatch.
//!
//! **What it compares.** Both the captured and the fresh reading must outrank
//! nothing. Not equality: the capture's file labels are home-redacted and the
//! fresh read's are not, so equality fails for any installation under the home,
//! and once both readings outrank nothing equality adds only "the same empty
//! files" (security review, M1).
//!
//! **What it cannot do.** The window between this re-read and the provider's own
//! read at exec is inherent; a writer running as the same user can still race
//! it. It catches drift and accidents between the capture and the step, which is
//! what SE-016 asks.
//!
//! **Not wired** while this build dispatches no mutation. The result is
//! a [`ConfigurationRechecked`] token that is neither `Clone` nor `Copy`: the
//! wiring makes the mutating dispatch take it by value and check its provider
//! against the step's, which a textual tripwire cannot (spec-guardian, M2).

use macportsbrew_domain::{DomainError, ErrorId, ProviderConfiguration, ProviderId, Snapshot};

/// Why a step may not run against a provider's configuration as it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigurationDrift {
    /// A file sets a key that can outrank the invocation, or exists unreadable.
    Outranks,
    /// The configuration was not read (a replay, or a provider not recorded),
    /// so nothing says it outranks nothing.
    Unknown,
    /// The capture already said it outranks, so the plan should not have
    /// reached a mutating step against this provider.
    CapturedOutranks,
}

impl ConfigurationDrift {
    /// The refusal a caller reports: a safety check stopped the step and
    /// nothing was changed.
    #[must_use]
    pub fn error(&self, provider: &ProviderId) -> DomainError {
        let why = match self {
            Self::Outranks => "now has a configuration file that can change how it runs",
            Self::Unknown => "has a configuration that could not be read just before the step",
            Self::CapturedOutranks => {
                "had a configuration file that can change how it runs when the plan was built"
            }
        };
        DomainError::new(
            ErrorId::SafetyBlocked,
            format!("{} {why}, so nothing was changed", provider.as_str()),
        )
    }
}

/// Proof that a provider's configuration was re-checked and outranks nothing,
/// for one program of it. Neither `Clone` nor `Copy`, so one re-check cannot be
/// spent on two steps; every consumer compares both the provider and the
/// program against the commands it is spent on (work order 16 part E, SR 9).
#[derive(Debug, PartialEq, Eq)]
pub struct ConfigurationRechecked {
    provider: ProviderId,
    program: String,
}

impl ConfigurationRechecked {
    /// The provider it was checked for.
    #[must_use]
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The program the configuration was re-read for.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }
}

/// Whether a mutating step may run against `provider`, given the snapshot the
/// plan was built from and the provider's configuration as re-read just now for
/// `program`. The token records `program`; it compares nothing new, and the
/// consumers refuse it for any command that names another program.
///
/// The captured reading is looked up here, by `provider`, rather than handed
/// in: a caller cannot pair one provider's capture with another's id (security
/// review, pass 08 slice 3, L2). A provider the snapshot does not hold is
/// [`ConfigurationDrift::Unknown`].
pub fn recheck_provider_configuration(
    provider: &ProviderId,
    program: &str,
    snapshot: &Snapshot,
    fresh: &ProviderConfiguration,
) -> Result<ConfigurationRechecked, ConfigurationDrift> {
    let captured = snapshot
        .providers
        .get(provider)
        .map(|p| &p.configuration)
        .ok_or(ConfigurationDrift::Unknown)?;
    match captured.outranks() {
        Some(false) => {}
        Some(true) => return Err(ConfigurationDrift::CapturedOutranks),
        None => return Err(ConfigurationDrift::Unknown),
    }
    match fresh.outranks() {
        Some(false) => Ok(ConfigurationRechecked {
            provider: provider.clone(),
            program: program.to_owned(),
        }),
        Some(true) => Err(ConfigurationDrift::Outranks),
        None => Err(ConfigurationDrift::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_domain::{ConfigurationFile, ConfigurationScope};
    use std::collections::BTreeSet;

    fn file(path: &str, readable: bool, keys: &[&str]) -> ConfigurationFile {
        ConfigurationFile {
            path: path.to_string(),
            scope: ConfigurationScope::User,
            readable,
            keys: keys
                .iter()
                .map(|k| (*k).to_string())
                .collect::<BTreeSet<_>>(),
            analytics_keys: BTreeSet::new(),
        }
    }

    fn read(files: Vec<ConfigurationFile>) -> ProviderConfiguration {
        ProviderConfiguration::Read { files }
    }

    /// A snapshot holding `alpha` with `captured` as its configuration.
    fn snapshot(captured: ProviderConfiguration) -> Snapshot {
        use macportsbrew_domain::{
            Architecture, CommandLineTools, EnvironmentSnapshot, Machine, MacosVersion, Provenance,
            ProviderCapabilities, ProviderDetection, ProviderInventory, ProviderPresence,
            ProviderSnapshot, Translation, SNAPSHOT_SCHEMA_VERSION,
        };
        use std::collections::BTreeMap;
        let provider = ProviderSnapshot {
            detection: ProviderDetection {
                provider: id(),
                presence: ProviderPresence::Present,
                version: None,
                prefix: None,
                executable: None,
                architecture: None,
                prefix_redacted: false,
            },
            capabilities: ProviderCapabilities::default(),
            inventory: ProviderInventory::default(),
            environment: BTreeMap::new(),
            invocation_environment: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            configuration: captured,
        };
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            tool_version: "test".into(),
            policy_version: "test".into(),
            provenance: Provenance {
                captured_at: "2026-10-10T00:00:00Z".into(),
                captured_by: "configuration_recheck.rs".into(),
                origin: "synthesised".into(),
                note: None,
            },
            machine: Machine {
                macos: MacosVersion::new(26, 0, 0),
                architecture: Architecture::Arm64,
                translation: Translation::Native,
                command_line_tools: CommandLineTools::Missing,
            },
            environment: EnvironmentSnapshot::default(),
            providers: [(id(), provider)].into_iter().collect(),
        }
    }

    fn id() -> ProviderId {
        ProviderId::new("alpha")
    }

    #[test]
    fn a_configuration_that_outranks_nothing_then_and_now_passes_and_names_its_provider() {
        let token =
            recheck_provider_configuration(&id(), "port", &snapshot(read(vec![])), &read(vec![]))
                .unwrap();
        assert_eq!(token.provider(), &id());
        // A file that exists, is readable and sets nothing outranks nothing,
        // and the labels need not agree (redacted against raw).
        let captured = read(vec![file("~/.cfg", true, &[])]);
        let fresh = read(vec![file("/Users/someone/.cfg", true, &[])]);
        assert!(
            recheck_provider_configuration(&id(), "port", &snapshot(captured.clone()), &fresh)
                .is_ok()
        );
    }

    #[test]
    fn a_file_that_appeared_since_the_capture_refuses_the_step() {
        let fresh = read(vec![file("~/.cfg", true, &["SOME_KEY"])]);
        assert_eq!(
            recheck_provider_configuration(&id(), "port", &snapshot(read(vec![])), &fresh),
            Err(ConfigurationDrift::Outranks)
        );
        let unreadable = read(vec![file("~/.cfg", false, &[])]);
        assert_eq!(
            recheck_provider_configuration(&id(), "port", &snapshot(read(vec![])), &unreadable),
            Err(ConfigurationDrift::Outranks)
        );
    }

    #[test]
    fn an_unread_configuration_on_either_side_is_never_a_pass() {
        for unknown in [
            ProviderConfiguration::Unknowable,
            ProviderConfiguration::NotRecorded,
        ] {
            assert_eq!(
                recheck_provider_configuration(&id(), "port", &snapshot(read(vec![])), &unknown),
                Err(ConfigurationDrift::Unknown)
            );
            assert_eq!(
                recheck_provider_configuration(
                    &id(),
                    "port",
                    &snapshot(unknown.clone()),
                    &read(vec![])
                ),
                Err(ConfigurationDrift::Unknown)
            );
        }
    }

    #[test]
    fn a_provider_the_snapshot_does_not_hold_is_unknown_never_a_pass() {
        let mut other = snapshot(read(vec![]));
        let held = other.providers.remove(&id()).expect("alpha");
        other.providers.insert(ProviderId::new("beta"), held);
        assert_eq!(
            recheck_provider_configuration(&id(), "port", &other, &read(vec![])),
            Err(ConfigurationDrift::Unknown)
        );
    }

    #[test]
    fn a_capture_that_already_outranked_is_refused_even_if_it_is_clean_now() {
        let captured = read(vec![file("~/.cfg", true, &["SOME_KEY"])]);
        assert_eq!(
            recheck_provider_configuration(
                &id(),
                "port",
                &snapshot(captured.clone()),
                &read(vec![])
            ),
            Err(ConfigurationDrift::CapturedOutranks)
        );
    }

    #[test]
    fn every_refusal_is_a_safety_block_that_names_the_provider() {
        for drift in [
            ConfigurationDrift::Outranks,
            ConfigurationDrift::Unknown,
            ConfigurationDrift::CapturedOutranks,
        ] {
            let error = drift.error(&id());
            assert_eq!(error.id, ErrorId::SafetyBlocked);
            assert!(error.detail.starts_with("alpha "), "{}", error.detail);
            assert!(
                error.detail.ends_with("nothing was changed"),
                "{}",
                error.detail
            );
        }
    }

    #[test]
    fn a_recheck_records_the_program_it_was_made_for() {
        let checked = recheck_provider_configuration(
            &id(),
            "the-program",
            &snapshot(read(vec![])),
            &read(vec![]),
        )
        .unwrap();
        assert_eq!(checked.provider(), &id());
        assert_eq!(checked.program(), "the-program");
    }
}
