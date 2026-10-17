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
use crate::prefixes;
use crate::process::{CommandSpec, Invocation, PlanReadRefusal, ProcessRunner};
use macportsbrew_domain::ProviderEnvironmentContract;
use macportsbrew_domain::{
    DomainError, ErrorId, PlanCommand, ProviderConfiguration, ProviderId, ProviderRead,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

/// How one provider's invocation is derived: the executable under its prefix,
/// and the function its adapter builds every environment with.
///
/// One row per provider and one code path for all of them (INV-17): nothing in
/// [`plan_read_spec`] branches on which provider it is deriving for, so a
/// third provider is a row here and not a second derivation.
#[derive(Clone, Copy)]
struct Derivation {
    executable: &'static str,
    environment: fn(&Path, Option<PathBuf>) -> BTreeMap<String, String>,
}

fn derivation(provider: &ProviderId) -> Option<Derivation> {
    match provider.as_str() {
        crate::homebrew::PROVIDER_ID => Some(Derivation {
            executable: crate::homebrew::EXECUTABLE_RELATIVE,
            environment: crate::homebrew::quiet_environment,
        }),
        crate::macports::PROVIDER_ID => Some(Derivation {
            executable: crate::macports::EXECUTABLE_RELATIVE,
            environment: crate::macports::invocation_environment,
        }),
        _ => None,
    }
}

/// A provider prefix this build detected — the only thing a plan read may be
/// derived from (DP-023).
///
/// Its fields are private and it has no public constructor: the one way to
/// obtain one is [`detected_prefix`], which resolves the prefix exactly as
/// [`build`] does for the adapters — probing the documented candidates on a
/// live run, honouring `prefix_override` only under replay — and takes `home`
/// from the same [`RegistryContext`] the adapters are built from. So no value
/// from a plan, a snapshot or a flag can be handed to [`plan_read_spec`] as
/// the directory a binary is resolved from: a `/tmp/evil` has no route into
/// this type. Unlike [`build`], there is no fallback to a first candidate: a
/// provider that was not detected has no `DetectedPrefix` at all.
#[derive(Clone)]
pub struct DetectedPrefix {
    provider: ProviderId,
    prefix: PathBuf,
    home: Option<PathBuf>,
    derivation: Derivation,
    /// Resolved through a replay runner. Every spec built from it is marked
    /// so that the live runner refuses to spawn it (SE-050, "Not closed" 7).
    replay: bool,
}

impl DetectedPrefix {
    /// The provider this prefix was detected for.
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// The detected prefix.
    pub fn prefix(&self) -> &Path {
        &self.prefix
    }
}

impl std::fmt::Debug for DetectedPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectedPrefix")
            .field("provider", &self.provider)
            .field("prefix", &self.prefix)
            .finish_non_exhaustive()
    }
}

/// The prefix of `provider`, as this build detects it for `ctx` — or `None`
/// when the provider is not one this build knows, or was not detected.
pub fn detected_prefix(ctx: &RegistryContext<'_>, provider: &ProviderId) -> Option<DetectedPrefix> {
    let derivation = derivation(provider)?;
    // Asked once: the scope, the override and the replay mark must all follow
    // from the same answer (a runner that answered differently on each call
    // could otherwise take a probed scope with an honoured override).
    let replay = ctx.runner.is_replay();
    let filesystem = effective_scope(replay, ctx.filesystem);
    let prefix = resolve_prefix(ctx, provider, filesystem, replay)?;
    Some(DetectedPrefix {
        provider: provider.clone(),
        prefix,
        home: ctx.home.clone(),
        derivation,
        replay,
    })
}

/// Turns a plan's provider command into something that may run (DP-023).
///
/// The plan supplies argv only. The program is re-derived from the prefix in
/// `site` and its adapter's executable, and the environment from the adapter's
/// own construction for that prefix and the home the adapters were built with
/// — `None` under replay (F17). A plan whose program disagrees with the derived
/// one is refused, and the argv must pass the shipped read allowlist at
/// construction (SE-050). The prefix and the home cannot come from the plan or
/// a snapshot: [`DetectedPrefix`] is constructed only by detection.
///
/// **`command.environment` and `command.elevation` are never read here.** The
/// first is a display copy — redacted by capture, so its `HOME` is `~` on any
/// live run — and the spawn site builds its own (`plan.v1.md`, obligation 10;
/// INV-10). The second is what the user is shown, not what decides whether to
/// ask (obligation 9); the refusal gate refuses anything but `NONE` in this
/// build. `command.program` is read only to refuse a disagreement.
///
/// **Safe only after the execution gate has passed the plan**
/// (`execution_refusals`, `plan.v1.md` "What an executor inherits"). The read
/// allowlist checked here fixes the subcommand and the exact `--long` options
/// that subcommand may take, and refuses separators, short options and path- or
/// URL-shaped arguments — but it checks positional arguments for shape only:
/// which package a read names, and how many, it does not decide. What keeps a
/// plan's argv to exactly the planner's read is the gate's
/// `COMMAND_IS_NOT_THE_PLANNERS_READ`; the caller owes that the gate passed
/// (SE-050, "Not closed").
pub fn plan_read_spec(
    site: &DetectedPrefix,
    command: &PlanCommand,
) -> Result<CommandSpec, PlanReadRefusal> {
    CommandSpec::plan_read(
        &site.prefix,
        site.derivation.executable,
        &command.target,
        &command.program,
        &command.argv,
        (site.derivation.environment)(&site.prefix, site.home.clone()),
        site.replay,
    )
}

/// `provider`'s configuration, read again immediately before a mutating step
/// (pass 08 slice 4, D7; SE-016 D-R1′).
///
/// The prefix is detected again here, exactly as [`detected_prefix`] detects
/// it, and the read is **refused** unless the program derived from it is the
/// step's `planned_program` — the same comparison the read site makes (DP-023)
/// — so the configuration that is re-checked and the command that would run
/// share one prefix, and a caller never holds a prefix of its own. The home is
/// the one `ctx` was built with, captured once at context build, never read
/// from the environment at this point. Under replay the scope makes the answer
/// `UNKNOWABLE`, which the re-check refuses.
///
/// # Errors
/// `SAFETY_BLOCKED` when the provider is not detected, or its derived program
/// is not the planned one. Nothing is read in either case.
pub fn reread_configuration(
    ctx: &RegistryContext<'_>,
    provider: &ProviderId,
    planned_program: &str,
) -> Result<ProviderConfiguration, DomainError> {
    let site = detected_prefix(ctx, provider).ok_or_else(|| {
        DomainError::new(
            ErrorId::SafetyBlocked,
            "the provider was not detected again before the step, so its configuration was \
             not re-read and nothing was run",
        )
    })?;
    let derived = site.prefix.join(site.derivation.executable);
    if derived.to_string_lossy() != planned_program {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "the provider detected before the step is not the one the plan was built for, so \
             nothing was run",
        ));
    }
    let filesystem = effective_scope(site.replay, ctx.filesystem);
    Ok(crate::configuration::read(
        provider.as_str(),
        &site.prefix,
        site.home.as_deref(),
        filesystem,
    ))
}

/// Every provider's plan read site, each bound to the runner its prefix was
/// detected through (SE-050, "Not closed" 7).
///
/// **The only way a plan's command is run.** [`PlanReadSites::run`] builds the
/// spec through [`plan_read_spec`] and hands it to the runner captured from
/// the same [`RegistryContext`] detection used, so a prefix a recording
/// answered is answered by that recording, and a prefix probed on this machine
/// is spawned on it. The binding is the structure, not the caller's
/// discipline; the replay mark [`plan_read_spec`] sets is the second line, for
/// a spec handed to another runner by some other route — the live runner
/// refuses it.
///
/// **Safe only after the execution gate has passed the plan**, for the reason
/// [`plan_read_spec`] gives.
pub struct PlanReadSites<'r> {
    runner: &'r dyn ProcessRunner,
    sites: Vec<DetectedPrefix>,
}

/// Why a plan read was not run.
#[derive(Debug)]
pub enum PlanReadError {
    /// The step's provider was not detected through this context, so no prefix
    /// exists to derive a program from (no fallback candidate is taken).
    NotDetected,
    /// The command was refused when its spec was built.
    Refused(PlanReadRefusal),
    /// The runner could not record an invocation at all. A process that ran
    /// and failed is an `Ok` invocation with its status, never this.
    Transport(DomainError),
}

impl PlanReadSites<'_> {
    /// Runs `command` against `provider`'s detected site, through the runner
    /// that detected it.
    pub fn run(
        &self,
        provider: &ProviderId,
        command: &PlanCommand,
    ) -> Result<Invocation, PlanReadError> {
        let site = self
            .sites
            .iter()
            .find(|s| s.provider() == provider)
            .ok_or(PlanReadError::NotDetected)?;
        let spec = plan_read_spec(site, command).map_err(PlanReadError::Refused)?;
        self.runner.run(&spec).map_err(PlanReadError::Transport)
    }

    /// The providers a site was detected for.
    pub fn providers(&self) -> Vec<&ProviderId> {
        self.sites.iter().map(DetectedPrefix::provider).collect()
    }
}

/// The plan read sites `ctx` detects, bound to `ctx.runner`.
pub fn plan_read_sites<'r>(ctx: &RegistryContext<'r>) -> PlanReadSites<'r> {
    PlanReadSites {
        runner: ctx.runner,
        sites: known_provider_ids()
            .iter()
            .filter_map(|id| detected_prefix(ctx, id))
            .collect(),
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

/// Under replay, each provider's prefix as the recording's own answer states
/// it (`brew --prefix`; the prefix above `var/macports` in `port dir`), rather
/// than this machine's filesystem. Here, beside the adapters, because it is
/// knowledge of what each provider's commands print.
///
/// A recording is a file the user may have been handed, so its contents are
/// untrusted input (`04-migration-and-safety.md`, "Malicious or malformed
/// provider metadata"). A prefix taken from one becomes the directory an
/// executable path is built from, so every candidate must pass
/// [`crate::process::is_safe_path`] - absolute, bounded, no `..`, no control
/// characters - and one that fails is dropped rather than repaired; the
/// default candidate is then used, as for a recording without the answer.
pub fn prefix_overrides_from(recording: &crate::process::Recording) -> Vec<(ProviderId, PathBuf)> {
    let mut out = Vec::new();
    let mut accept = |id: &str, path: PathBuf| {
        if crate::process::is_safe_path(&path) {
            out.push((ProviderId::new(id), path));
        }
    };
    for inv in recording.invocations.iter().map(|r| &r.invocation) {
        if !inv.status.success() {
            continue;
        }
        if inv.program == "brew" && inv.argv == ["--prefix"] {
            accept(
                crate::homebrew::PROVIDER_ID,
                PathBuf::from(inv.stdout.trim()),
            );
        }
        if inv.program == "port" && inv.argv == ["dir"] {
            // `port dir` prints a directory under the prefix's
            // `var/macports`; the prefix is what precedes that component.
            let text = inv.stdout.trim();
            let at = text
                .match_indices("/var/macports")
                .map(|(i, _)| i)
                .find(|i| {
                    let rest = &text[i + "/var/macports".len()..];
                    rest.is_empty() || rest.starts_with('/')
                });
            if let Some(i) = at {
                accept(crate::macports::PROVIDER_ID, PathBuf::from(&text[..i]));
            }
        }
    }
    out
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
    let replay = ctx.runner.is_replay();
    let filesystem = effective_scope(replay, ctx.filesystem);
    let mut out: Vec<Box<dyn ProviderRead + 'r>> = Vec::new();
    for id in known_provider_ids() {
        // Every known provider has data (`known_provider_ids` reads the same
        // file), and its fallback is required there, so this skips nothing;
        // `every_known_provider_gets_an_adapter` holds both.
        let Some(prefix) = resolve_prefix(ctx, &id, filesystem, replay)
            .or_else(|| prefixes::fallback(id.as_str()))
        else {
            continue;
        };
        match id.as_str() {
            "homebrew" => {
                let tags = (ctx.platform_tags)(id.as_str());
                out.push(Box::new(
                    HomebrewAdapter::new(ctx.runner, prefix, ctx.home.clone(), tags, filesystem)
                        .with_installation_candidates(candidate_prefixes(&id)),
                ));
            }
            "macports" => {
                out.push(Box::new(
                    MacPortsAdapter::new(ctx.runner, prefix, ctx.home.clone(), filesystem)
                        .with_installation_candidates(candidate_prefixes(&id)),
                ));
            }
            _ => {}
        }
    }
    out
}

/// Where `id` is installed, as detection sees it for `ctx`: the replay-only
/// override, else the first documented candidate holding the executable. One
/// function, so the adapters [`build`] constructs and the [`DetectedPrefix`] a
/// plan read derives from cannot be resolved two different ways.
fn resolve_prefix(
    ctx: &RegistryContext<'_>,
    id: &ProviderId,
    filesystem: FilesystemScope,
    replay: bool,
) -> Option<PathBuf> {
    let candidates = candidate_prefixes(id);
    override_for(ctx, id, replay).or_else(|| match id.as_str() {
        crate::homebrew::PROVIDER_ID => crate::homebrew::detect_prefix(&candidates, filesystem),
        crate::macports::PROVIDER_ID => {
            MacPortsAdapter::<&dyn ProcessRunner>::detect_prefix(&candidates, filesystem)
        }
        _ => None,
    })
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
fn effective_scope(replay: bool, requested: FilesystemScope) -> FilesystemScope {
    if replay {
        requested
    } else {
        FilesystemScope::ThisMachine
    }
}

/// The prefix override that applies to `id`, if any.
///
/// **`replay` is `runner.is_replay()`, asked once by the caller, on purpose,
/// and it must keep being the runner's answer.** Its
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
fn override_for(ctx: &RegistryContext<'_>, id: &ProviderId, replay: bool) -> Option<PathBuf> {
    if !replay {
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
            effective_scope(live.is_replay(), FilesystemScope::Elsewhere),
            FilesystemScope::ThisMachine
        );
        assert_eq!(
            effective_scope(live.is_replay(), FilesystemScope::ThisMachine),
            FilesystemScope::ThisMachine
        );
    }

    #[test]
    fn a_replay_runner_keeps_whichever_scope_it_was_given() {
        // Both halves are legitimate: `--replay` reads a recording, and a
        // contract test replays subprocesses over a tree that is its subject.
        let runner = replay();
        assert_eq!(
            effective_scope(runner.is_replay(), FilesystemScope::Elsewhere),
            FilesystemScope::Elsewhere
        );
        assert_eq!(
            effective_scope(runner.is_replay(), FilesystemScope::ThisMachine),
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

        fn is_replay(&self) -> bool {
            false
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
    fn every_known_provider_gets_an_adapter() {
        // A provider the data names but no arm builds, or one with no
        // fallback, would be dropped silently from every analysis.
        let runner = replay();
        assert_eq!(build(&context(&runner)).len(), known_provider_ids().len());
        for id in known_provider_ids() {
            assert!(
                prefixes::fallback(id.as_str()).is_some(),
                "{id} has no fallback prefix"
            );
        }
    }

    #[test]
    fn every_adapter_is_told_its_candidate_prefixes() {
        // Under replay the disk is not the machine, so a searched provider is
        // `Unknowable`; an adapter handed no candidates would say `NotRecorded`
        // instead, so this fails if the registry stops passing them (INV-15).
        let runner = replay();
        for adapter in build(&context(&runner)) {
            assert_eq!(
                adapter.installations(),
                macportsbrew_domain::Installations::Unknowable,
                "{}",
                adapter.id()
            );
        }
        // Against the live machine it is a real search whatever this host holds
        // (a `stat` of the documented prefixes on this host, which spawns nothing
        // and asserts nothing about what they hold, like the probes above)
        // (`Read`, possibly empty), never `Unknowable` or `NotRecorded`.
        let live = LiveRunner;
        let live_context = RegistryContext {
            filesystem: FilesystemScope::ThisMachine,
            runner: &live,
            home: None,
            platform_tags: Box::new(|_| Vec::new()),
            prefix_override: None,
        };
        for adapter in build(&live_context) {
            assert!(
                matches!(
                    adapter.installations(),
                    macportsbrew_domain::Installations::Read { .. }
                ),
                "{}",
                adapter.id()
            );
        }
    }

    #[test]
    fn an_override_applies_under_replay() {
        let runner = replay();
        assert_eq!(
            override_for(
                &context(&runner),
                &ProviderId::new("homebrew"),
                runner.is_replay()
            ),
            Some(PathBuf::from("/tmp/anything"))
        );
    }

    #[test]
    fn an_override_is_ignored_against_the_live_machine() {
        let runner = LiveRunner;
        assert_eq!(
            override_for(
                &context(&runner),
                &ProviderId::new("homebrew"),
                runner.is_replay()
            ),
            None,
            "a live run resolves its prefix by probing, and by nothing else"
        );
    }

    /// A runner whose `is_replay` answer changes between calls: true first.
    struct Flipping(std::cell::Cell<usize>);

    impl ProcessRunner for Flipping {
        fn run(&self, _spec: &CommandSpec) -> Result<Invocation, DomainError> {
            panic!("these tests never spawn anything")
        }

        fn is_replay(&self) -> bool {
            let n = self.0.get();
            self.0.set(n + 1);
            n == 0
        }
    }

    #[test]
    fn detection_asks_the_runner_whether_it_is_a_replay_exactly_once() {
        // One answer decides the scope, the override and the replay mark, so
        // a runner cannot be read as a replay for the override and as live for
        // the mark (SE-050, "Not closed" 7).
        let runner = Flipping(std::cell::Cell::new(0));
        let site = detected_prefix(&context(&runner), &ProviderId::new("homebrew"))
            .expect("the override is honoured on the first answer");
        assert_eq!(runner.0.get(), 1);
        assert!(site.replay, "the mark follows the same answer");
    }

    #[test]
    fn a_plan_read_site_on_the_live_machine_never_takes_a_prefix_it_was_handed() {
        // The context names `/tmp/anything` for Homebrew. Against a live runner
        // that is ignored, so the only prefixes a site can hold are the
        // documented candidates detection probes (or none at all, on a machine
        // without the provider). This probes this machine's filesystem with a
        // stat and spawns nothing; `LiveRunner` panics if asked to run.
        let runner = LiveRunner;
        let ctx = context(&runner);
        for id in known_provider_ids() {
            if let Some(site) = detected_prefix(&ctx, &id) {
                assert_ne!(site.prefix(), Path::new("/tmp/anything"));
                assert!(
                    candidate_prefixes(&id).iter().any(|c| c == site.prefix()),
                    "{id:?}: {site:?} is not a documented candidate"
                );
            }
        }
    }

    #[test]
    fn an_override_for_another_provider_is_not_borrowed() {
        let runner = replay();
        assert_eq!(
            override_for(
                &context(&runner),
                &ProviderId::new("macports"),
                runner.is_replay()
            ),
            None
        );
    }
}
