// SPDX-License-Identifier: Apache-2.0

//! The provider registry: the one place that knows which providers exist
//! and where their prefixes are expected. Everything above this layer works
//! with `dyn ProviderRead` and never names a provider.
//!
//! Adding a provider (`08-distribution-and-project.md`, contributor recipe)
//! means an adapter module plus one entry here.

use crate::filesystem::FilesystemScope;
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

/// Candidate prefixes per provider, in detection order.
///
/// The values, their sources and their read dates are in
/// `data/policy/provider-prefixes.toml`; INV-20 names prefixes as dated data
/// and this was the last category held in a `match` (DP-014). See
/// [`crate::prefixes`] for why the loader lives in this layer.
pub fn candidate_prefixes(provider: &ProviderId) -> Vec<PathBuf> {
    crate::prefixes::candidates(provider.as_str())
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
    /// Whether this host's filesystem describes the machine being read.
    ///
    /// Deliberately **not** derived from `runner.is_replay()` inside this
    /// function: they are different questions, and `override_for` below still
    /// asks the runner because its question really is the runner's. See
    /// [`crate::filesystem`] (F-25).
    pub filesystem: FilesystemScope,
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
    // A live runner spawns real commands on this host, so this host *is* the
    // machine being read: `Elsewhere` alongside it is not a configuration, it
    // is a contradiction. Collapsing it here rather than trusting the caller
    // is what keeps the pair from disabling `detect_uncached`'s no-spawn
    // check in front of an unprobed prefix — the fallback `candidates[N]`,
    // which on a Mac that once had an Intel Homebrew is a writable
    // `/usr/local`. A runtime check and not a `debug_assert!`, for the reason
    // the executor's own allowlist is one (F-25 review).
    let filesystem = effective_scope(ctx.runner, ctx.filesystem);
    let mut out: Vec<Box<dyn ProviderRead + 'r>> = Vec::new();
    for id in known_provider_ids() {
        let candidates = candidate_prefixes(&id);
        let overridden = override_for(ctx, &id);
        match id.as_str() {
            "homebrew" => {
                let prefix = overridden
                    .or_else(|| crate::homebrew::detect_prefix(&candidates, filesystem))
                    .unwrap_or_else(|| candidates[1].clone());
                let tags = (ctx.platform_tags)(id.as_str());
                out.push(Box::new(HomebrewAdapter::new(
                    ctx.runner,
                    prefix,
                    ctx.home.clone(),
                    tags,
                    filesystem,
                )));
            }
            "macports" => {
                let prefix = overridden
                    .or_else(|| {
                        MacPortsAdapter::<&dyn ProcessRunner>::detect_prefix(
                            &candidates,
                            filesystem,
                        )
                    })
                    .unwrap_or_else(|| candidates[0].clone());
                out.push(Box::new(MacPortsAdapter::new(
                    ctx.runner, prefix, filesystem,
                )));
            }
            _ => {}
        }
    }
    out
}

/// The filesystem scope an adapter is actually built with.
///
/// A live runner spawns real commands on this host, so this host **is** the
/// machine being read. `Elsewhere` beside it is not a configuration but a
/// contradiction, and it is collapsed here rather than trusted: left standing
/// it would switch off `detect_uncached`'s no-spawn check in front of a prefix
/// nobody probed — the fallback candidate, which on a Mac that once carried an
/// Intel Homebrew is a writable `/usr/local` (F-25 review).
///
/// The converse pair is legitimate and stays: a contract test replays
/// subprocesses while reading a scratch tree that really is its subject.
fn effective_scope(runner: &dyn ProcessRunner, requested: FilesystemScope) -> FilesystemScope {
    if runner.is_replay() {
        requested
    } else {
        FilesystemScope::ThisMachine
    }
}

/// The prefix override that applies to `id`, if any.
///
/// **This asks `runner.is_replay()` on purpose, and must keep asking it.** Its
/// question is whether a real executable could be spawned, which is the
/// runner's question and not [`FilesystemScope`]'s. Answering it with the
/// filesystem scope instead would let a caller holding a live runner supply a
/// prefix and so choose which binary the tool runs (F-25 changed the reads
/// around this guard and deliberately not the guard).
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
    use super::effective_scope;
    use crate::filesystem::FilesystemScope;
    use crate::process::SystemRunner;

    #[test]
    fn a_live_runner_is_always_reading_this_machine() {
        // The incoherent pair, refused. A runner that can spawn on this host
        // is reading this host, whatever the caller says (F-25 review).
        let live = SystemRunner::new();
        assert!(!live.is_replay());
        assert_eq!(
            effective_scope(&live, FilesystemScope::Elsewhere),
            FilesystemScope::ThisMachine
        );
        assert_eq!(
            effective_scope(&live, FilesystemScope::ThisMachine),
            FilesystemScope::ThisMachine
        );
    }

    #[test]
    fn a_replay_runner_keeps_whichever_scope_it_was_given() {
        // Both halves are legitimate: `--replay` reads a recording, and a
        // contract test replays subprocesses over a tree that is its subject.
        let runner = replay();
        assert_eq!(
            effective_scope(&runner, FilesystemScope::Elsewhere),
            FilesystemScope::Elsewhere
        );
        assert_eq!(
            effective_scope(&runner, FilesystemScope::ThisMachine),
            FilesystemScope::ThisMachine
        );
    }

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
            filesystem: FilesystemScope::Elsewhere,
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
