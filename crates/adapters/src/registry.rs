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
use macportsbrew_domain::ProviderEnvironmentContract;
use macportsbrew_domain::{ProviderId, ProviderRead};
use std::collections::BTreeMap;
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

/// What each provider's adapter could have put in an invocation environment.
///
/// The domain holds the rule and names no provider, so it is handed these
/// facts. Both halves matter. The prefixes come from [`candidate_prefixes`] and
/// not only from the snapshot's own `detection.prefix`, because a file that
/// understated its prefix would otherwise widen what it is allowed to say. The
/// keys come from each adapter's own list, so adding a quieting variable
/// changes the builder and the reader together.
pub fn environment_contracts() -> BTreeMap<String, ProviderEnvironmentContract> {
    known_provider_ids()
        .into_iter()
        .map(|id| {
            let extra = match id.as_str() {
                crate::homebrew::PROVIDER_ID => crate::homebrew::QUIETING_KEYS
                    .iter()
                    .map(|k| ((*k).to_string(), "1".to_string()))
                    .collect(),
                _ => BTreeMap::new(),
            };
            let contract = ProviderEnvironmentContract {
                prefixes: candidate_prefixes(&id),
                extra,
            };
            (id.as_str().to_string(), contract)
        })
        .collect()
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
    ///
    /// Honoured **only** when the runner is a replay. A prefix decides which
    /// executable is run, so on a live machine it must come from probing the
    /// documented candidates and from nothing else — an override that a file
    /// or a flag could reach would be a way to point the tool at an arbitrary
    /// binary (`04-migration-and-safety.md`, "Process execution"). Against a
    /// recording there is no executable to run and the recording is the
    /// machine, so its own answers are the right source.
    pub prefix_override: Option<Vec<(ProviderId, PathBuf)>>,
}

/// Builds one adapter per known provider. A provider whose prefix is not
/// found is still constructed (over its first candidate prefix) so that it
/// reports `Absent` through the same contract.
pub fn build<'r>(ctx: &RegistryContext<'r>) -> Vec<Box<dyn ProviderRead + 'r>> {
    let mut out: Vec<Box<dyn ProviderRead + 'r>> = Vec::new();
    for id in known_provider_ids() {
        let candidates = candidate_prefixes(&id);
        let overridden = override_for(ctx, &id);
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

/// The prefix override that applies to `id`, if any.
///
/// The replay test is the whole of it. A prefix decides which executable is
/// run, so against the live machine it must come from probing the documented
/// candidates and from nowhere a file or a flag can reach
/// (`04-migration-and-safety.md`, "Process execution"). Against a recording
/// there is no executable to run, and the recording's own `--prefix` answer is
/// the only prefix that makes sense.
fn override_for(ctx: &RegistryContext<'_>, id: &ProviderId) -> Option<PathBuf> {
    if !ctx.runner.is_replay() {
        return None;
    }
    ctx.prefix_override
        .as_ref()?
        .iter()
        .find(|(p, _)| p == id)
        .map(|(_, path)| path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{CommandSpec, Invocation, Recording, RecordingProvenance, ReplayRunner};
    use macportsbrew_domain::DomainError;

    /// A runner that claims to be live and refuses to run anything: the tests
    /// below only ever ask it `is_replay`.
    struct LiveRunner;

    impl ProcessRunner for LiveRunner {
        fn run(&self, _spec: &CommandSpec) -> Result<Invocation, DomainError> {
            panic!("these tests never spawn anything")
        }
    }

    fn replay() -> ReplayRunner {
        ReplayRunner::new(Recording {
            schema_version: 1,
            provenance: RecordingProvenance {
                macos: "26.0.0".into(),
                architecture: "arm64".into(),
                provider_versions: Vec::new(),
                captured_at: "2026-09-02T00:00:00Z".into(),
                captured_by: "registry tests".into(),
                origin: "synthesised".into(),
                note: None,
            },
            invocations: Vec::new(),
        })
    }

    fn context(runner: &dyn ProcessRunner) -> RegistryContext<'_> {
        RegistryContext {
            runner,
            home: None,
            platform_tags: Box::new(|_| Vec::new()),
            prefix_override: Some(vec![(
                ProviderId::new("homebrew"),
                PathBuf::from("/tmp/anything"),
            )]),
        }
    }

    #[test]
    fn an_override_applies_under_replay() {
        let runner = replay();
        assert_eq!(
            override_for(&context(&runner), &ProviderId::new("homebrew")),
            Some(PathBuf::from("/tmp/anything"))
        );
    }

    #[test]
    fn an_override_is_ignored_against_the_live_machine() {
        let runner = LiveRunner;
        assert_eq!(
            override_for(&context(&runner), &ProviderId::new("homebrew")),
            None,
            "a live run resolves its prefix by probing, and by nothing else"
        );
    }

    #[test]
    fn an_override_for_another_provider_is_not_borrowed() {
        let runner = replay();
        assert_eq!(
            override_for(&context(&runner), &ProviderId::new("macports")),
            None
        );
    }
}
