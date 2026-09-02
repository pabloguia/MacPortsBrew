// SPDX-License-Identifier: Apache-2.0

//! The provider registry: the one place that knows which providers exist
//! and where their prefixes are expected. Everything above this layer works
//! with `dyn ProviderRead` and never names a provider.
//!
//! Adding a provider (`08-distribution-and-project.md`, contributor recipe)
//! means an adapter module plus one entry here.

use crate::homebrew::HomebrewAdapter;
use crate::macports::MacPortsAdapter;
use crate::process::ProcessRunner;
use macportsbrew_domain::{ProviderId, ProviderRead};
use std::path::PathBuf;

/// The provider ids this build knows, in a stable order.
pub fn known_provider_ids() -> Vec<ProviderId> {
    vec![
        ProviderId::new(crate::homebrew::PROVIDER_ID),
        ProviderId::new(crate::macports::PROVIDER_ID),
    ]
}

/// Candidate prefixes per provider, in detection order. Homebrew's default
/// prefix is `/opt/homebrew` on Apple Silicon and `/usr/local` on Intel
/// (<https://docs.brew.sh/Installation>, read 2026-09-02); MacPorts installs
/// to `/opt/local` (<https://guide.macports.org>, read 2026-09-02). Both
/// candidates are probed on every machine so a migrated Mac carrying an
/// Intel Homebrew under Rosetta is found and reported, not missed.
pub fn candidate_prefixes(provider: &ProviderId) -> Vec<PathBuf> {
    match provider.as_str() {
        "homebrew" => vec![PathBuf::from("/opt/homebrew"), PathBuf::from("/usr/local")],
        "macports" => vec![PathBuf::from("/opt/local")],
        _ => Vec::new(),
    }
}

/// Resolves the prebuilt-artifact tags for a provider id.
pub type PlatformTags<'r> = Box<dyn Fn(&str) -> Vec<String> + 'r>;

/// What the registry needs to construct adapters.
pub struct RegistryContext<'r> {
    /// The runner (live or replay).
    pub runner: &'r dyn ProcessRunner,
    /// The user's home, for the provider cache probe. `None` under replay.
    pub home: Option<PathBuf>,
    /// The prebuilt-artifact tags that satisfy this machine, per provider
    /// id, from the policy data.
    pub platform_tags: PlatformTags<'r>,
    /// Prefix overrides for tests and fixtures; `None` probes the defaults.
    pub prefix_override: Option<Vec<(ProviderId, PathBuf)>>,
}

/// Builds one adapter per known provider. A provider whose prefix is not
/// found is still constructed (over its first candidate prefix) so that it
/// reports `Absent` through the same contract.
pub fn build<'r>(ctx: &RegistryContext<'r>) -> Vec<Box<dyn ProviderRead + 'r>> {
    let mut out: Vec<Box<dyn ProviderRead + 'r>> = Vec::new();
    for id in known_provider_ids() {
        let candidates = candidate_prefixes(&id);
        let overridden = ctx.prefix_override.as_ref().and_then(|o| {
            o.iter()
                .find(|(p, _)| p == &id)
                .map(|(_, path)| path.clone())
        });
        match id.as_str() {
            "homebrew" => {
                let prefix = overridden
                    .or_else(|| {
                        if ctx.runner.is_replay() {
                            None
                        } else {
                            crate::homebrew::detect_prefix(&candidates)
                        }
                    })
                    .unwrap_or_else(|| candidates[1].clone());
                let tags = (ctx.platform_tags)(id.as_str());
                out.push(Box::new(HomebrewAdapter::new(
                    ctx.runner,
                    prefix,
                    ctx.home.clone(),
                    tags,
                )));
            }
            "macports" => {
                let prefix = overridden
                    .or_else(|| {
                        if ctx.runner.is_replay() {
                            None
                        } else {
                            MacPortsAdapter::<&dyn ProcessRunner>::detect_prefix(&candidates)
                        }
                    })
                    .unwrap_or_else(|| candidates[0].clone());
                out.push(Box::new(MacPortsAdapter::new(ctx.runner, prefix)));
            }
            _ => {}
        }
    }
    out
}
