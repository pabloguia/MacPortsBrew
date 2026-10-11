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

use macportsbrew_domain::{DomainError, ErrorId, ProviderConfiguration, ProviderId};

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

/// Proof that a provider's configuration was re-checked and outranks nothing.
/// Neither `Clone` nor `Copy`, so one re-check cannot be spent on two steps.
#[derive(Debug, PartialEq, Eq)]
pub struct ConfigurationRechecked {
    provider: ProviderId,
}

impl ConfigurationRechecked {
    /// The provider it was checked for.
    #[must_use]
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }
}

/// Whether a mutating step may run against `provider`, given its configuration
/// as captured and as re-read just now.
pub fn recheck_provider_configuration(
    provider: &ProviderId,
    captured: &ProviderConfiguration,
    fresh: &ProviderConfiguration,
) -> Result<ConfigurationRechecked, ConfigurationDrift> {
    match captured.outranks() {
        Some(false) => {}
        Some(true) => return Err(ConfigurationDrift::CapturedOutranks),
        None => return Err(ConfigurationDrift::Unknown),
    }
    match fresh.outranks() {
        Some(false) => Ok(ConfigurationRechecked {
            provider: provider.clone(),
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
        }
    }

    fn read(files: Vec<ConfigurationFile>) -> ProviderConfiguration {
        ProviderConfiguration::Read { files }
    }

    fn id() -> ProviderId {
        ProviderId::new("alpha")
    }

    #[test]
    fn a_configuration_that_outranks_nothing_then_and_now_passes_and_names_its_provider() {
        let token = recheck_provider_configuration(&id(), &read(vec![]), &read(vec![])).unwrap();
        assert_eq!(token.provider(), &id());
        // A file that exists, is readable and sets nothing outranks nothing,
        // and the labels need not agree (redacted against raw).
        let captured = read(vec![file("~/.cfg", true, &[])]);
        let fresh = read(vec![file("/Users/someone/.cfg", true, &[])]);
        assert!(recheck_provider_configuration(&id(), &captured, &fresh).is_ok());
    }

    #[test]
    fn a_file_that_appeared_since_the_capture_refuses_the_step() {
        let fresh = read(vec![file("~/.cfg", true, &["SOME_KEY"])]);
        assert_eq!(
            recheck_provider_configuration(&id(), &read(vec![]), &fresh),
            Err(ConfigurationDrift::Outranks)
        );
        let unreadable = read(vec![file("~/.cfg", false, &[])]);
        assert_eq!(
            recheck_provider_configuration(&id(), &read(vec![]), &unreadable),
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
                recheck_provider_configuration(&id(), &read(vec![]), &unknown),
                Err(ConfigurationDrift::Unknown)
            );
            assert_eq!(
                recheck_provider_configuration(&id(), &unknown, &read(vec![])),
                Err(ConfigurationDrift::Unknown)
            );
        }
    }

    #[test]
    fn a_capture_that_already_outranked_is_refused_even_if_it_is_clean_now() {
        let captured = read(vec![file("~/.cfg", true, &["SOME_KEY"])]);
        assert_eq!(
            recheck_provider_configuration(&id(), &captured, &read(vec![])),
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
}
