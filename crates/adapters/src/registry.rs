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
    // Asked once: the scope, the override and the replay mark must all follow
    // from the same answer (a runner that answered differently on each call
    // could otherwise take a probed scope with an honoured override).
    let replay = ctx.runner.is_replay();
    detected_prefix_as(ctx, provider, replay)
}

/// [`detected_prefix`] with the runner's replay answer already asked, so a
/// caller detecting several providers asks it once for all of them.
fn detected_prefix_as(
    ctx: &RegistryContext<'_>,
    provider: &ProviderId,
    replay: bool,
) -> Option<DetectedPrefix> {
    let derivation = derivation(provider)?;
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

/// The lock probe over this machine's detected providers (DP-030 (b); R-17).
///
/// **The only constructor of a probe that looks at anything.** Each detected
/// provider's lock location is read from `data/policy/provider-locks.toml`
/// (INV-20) and resolved here, against the prefix detection found and, for a
/// `portdbpath` base, against [`crate::configuration::macports_portdbpath`];
/// the probe receives only the resolved shape and never learns which provider
/// a location is for (INV-17). A provider that is not detected is neither
/// probed nor reported. A detected provider with no entry, or whose base does
/// not resolve on this machine, is sampled as "not determined".
///
/// `runner.is_replay()` is asked **once**. A replay, or a filesystem that is
/// not the machine described, gets a probe that answers `Unknowable` and
/// touches nothing: nothing in a recording or a snapshot says what this
/// host's locks were when it was taken (INV-15). The scope is read as it was
/// requested, not through [`effective_scope`]: that collapse stops an unprobed
/// prefix being executed, and for a probe the contradiction of a live runner
/// and `Elsewhere` resolves towards not looking.
///
/// A provider whose detection failed only because its executable could not be
/// checked is not "not detected" ([`crate::lock_probe::undetected_site`]): it is sampled as
/// "not determined", so an error is never left out of the answer.
pub fn lock_probe(ctx: &RegistryContext<'_>) -> crate::lock_probe::SystemLockProbe {
    use crate::lock_probe::SystemLockProbe;
    let replay = ctx.runner.is_replay();
    if replay || !ctx.filesystem.is_this_machine() {
        return SystemLockProbe::unknowable();
    }
    let sites = known_provider_ids()
        .into_iter()
        .filter_map(|id| {
            let Some(site) = detected_prefix_as(ctx, &id, replay) else {
                let executable = derivation(&id)?.executable;
                let site = crate::lock_probe::undetected_site(&candidate_prefixes(&id), executable);
                return site.map(|site| (id, site));
            };
            Some((id, lock_site(&site)))
        })
        .collect();
    SystemLockProbe::over(sites)
}

/// Where `site`'s provider holds its locks, on this machine.
fn lock_site(site: &DetectedPrefix) -> crate::lock_probe::LockSite {
    use crate::lock_probe::LockSite;
    locks::entry(site.provider().as_str()).map_or(LockSite::Unresolved, |entry| {
        let base = match entry.base {
            locks::Base::Prefix => Some(site.prefix.clone()),
            locks::Base::Portdbpath => crate::configuration::macports_portdbpath(
                &site.prefix,
                site.home.as_deref(),
                FilesystemScope::ThisMachine,
            ),
        };
        base.map_or(LockSite::Unresolved, |base| entry.site_under(&base))
    })
}

/// `data/policy/provider-locks.toml`, read here and nowhere else (G item 18).
///
/// Embedded with `include_str!`, as [`crate::prefixes`] embeds its file. A
/// malformed or invalid file is this repository's own mistake and fails at
/// the first call, with the reason; there is no honest fallback, and a guessed
/// lock location would be a guessed "nothing held".
mod locks {
    use crate::lock_probe::LockSite;
    use serde::Deserialize;
    use std::ffi::OsString;
    use std::path::{Component, Path, PathBuf};

    const SOURCE: &str = include_str!("../../../data/policy/provider-locks.toml");

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Document {
        pub(super) entry: Vec<Entry>,
    }

    /// What a location is relative to: a closed set.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub(super) enum Base {
        Prefix,
        Portdbpath,
    }

    /// Where the lock files are under the base.
    #[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
    #[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
    pub(super) enum Location {
        /// Every entry of this directory is a lock file.
        Directory { dir: PathBuf },
        /// One lock file.
        File { file: PathBuf },
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub(super) struct Entry {
        pub(super) provider: String,
        pub(super) base: Base,
        pub(super) location: Location,
        pub(super) source: String,
        pub(super) read_on: String,
        #[allow(dead_code, reason = "provenance is the point of the file; INV-20")]
        pub(super) note: Option<String>,
    }

    impl Entry {
        /// The probe's site for this entry under `base`.
        pub(super) fn site_under(&self, base: &Path) -> LockSite {
            match &self.location {
                Location::Directory { dir } => LockSite::Directory(base.join(dir)),
                Location::File { file } => {
                    // Validated on load: at least one normal component, so it
                    // has a file name, and its parent is relative.
                    let (Some(name), Some(parent)) = (file.file_name(), file.parent()) else {
                        return LockSite::Unresolved;
                    };
                    LockSite::File {
                        dir: base.join(parent),
                        name: OsString::from(name),
                    }
                }
            }
        }
    }

    /// Whether `path` is relative and made only of normal components, read
    /// from its text: `Path::components` drops an interior `.` and a
    /// trailing `/`, which would let them through unseen.
    fn is_plain_relative(path: &Path) -> bool {
        let Some(text) = path.to_str() else {
            return false;
        };
        !text.is_empty()
            && text
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
            && path.components().all(|c| matches!(c, Component::Normal(_)))
    }

    fn is_date(text: &str) -> bool {
        let b = text.as_bytes();
        b.len() == 10
            && b.iter().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    *c == b'-'
                } else {
                    c.is_ascii_digit()
                }
            })
    }

    /// Parses and validates `text` against the providers the registry knows.
    pub(super) fn parse(text: &str, known: &[String]) -> Result<Document, String> {
        let doc: Document = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut seen = std::collections::BTreeSet::new();
        for e in &doc.entry {
            if !known.contains(&e.provider) {
                return Err(format!("{}: not a provider this build knows", e.provider));
            }
            if !seen.insert(e.provider.clone()) {
                return Err(format!("{}: more than one entry", e.provider));
            }
            if e.source.trim().is_empty() {
                return Err(format!("{}: no source", e.provider));
            }
            if !is_date(&e.read_on) {
                return Err(format!("{}: read_on is not YYYY-MM-DD", e.provider));
            }
            let path = match &e.location {
                Location::Directory { dir } => dir,
                Location::File { file } => file,
            };
            if !is_plain_relative(path) {
                return Err(format!(
                    "{}: {} must be relative, without `.`, `..` or a leading `/`",
                    e.provider,
                    path.display()
                ));
            }
        }
        Ok(doc)
    }

    fn document() -> &'static Document {
        use std::sync::OnceLock;
        static PARSED: OnceLock<Document> = OnceLock::new();
        PARSED.get_or_init(|| {
            let known: Vec<String> = super::known_provider_ids()
                .iter()
                .map(|p| p.as_str().to_string())
                .collect();
            parse(SOURCE, &known)
                .unwrap_or_else(|e| panic!("data/policy/provider-locks.toml is invalid: {e}"))
        })
    }

    /// `provider`'s entry, if the data has one.
    pub(super) fn entry(provider: &str) -> Option<&'static Entry> {
        document().entry.iter().find(|e| e.provider == provider)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn known() -> Vec<String> {
            vec!["a".into(), "b".into()]
        }

        #[test]
        fn the_shipped_file_is_valid_and_describes_every_known_provider() {
            let doc = document();
            for id in super::super::known_provider_ids() {
                assert!(
                    doc.entry.iter().any(|e| e.provider == id.as_str()),
                    "{id:?} has no lock entry"
                );
            }
        }

        #[test]
        fn an_invalid_file_is_refused_with_its_reason() {
            let row = |extra: &str| {
                format!(
                    "[[entry]]\nprovider = \"a\"\nbase = \"prefix\"\n{extra}\nsource = \"s\"\nread_on = \"2026-10-05\"\n"
                )
            };
            let good = row("location = { kind = \"directory\", dir = \"var/x\" }");
            assert!(
                parse(&good, &known()).is_ok(),
                "{:?}",
                parse(&good, &known())
            );
            for bad in [
                row("location = { kind = \"directory\", dir = \"/abs\" }"),
                row("location = { kind = \"directory\", dir = \"../up\" }"),
                row("location = { kind = \"file\", file = \"a/./b\" }"),
                row("location = { kind = \"file\", file = \"\" }"),
                row("location = { kind = \"directory\", dir = \"x/\" }"),
                row("location = { kind = \"directory\", dir = \"x//y\" }"),
                row("location = { kind = \"directory\", dir = \"x\", file = \"y\" }"),
                row("location = { kind = \"glob\", dir = \"x\" }"),
                row("location = { kind = \"directory\", dir = \"x\" }\nextra = 1"),
                good.replace("\"prefix\"", "\"home\""),
                good.replace("provider = \"a\"", "provider = \"z\""),
                good.replace("2026-10-05", "Oct 5"),
                good.replace("source = \"s\"", "source = \" \""),
                format!("{good}{good}"),
            ] {
                assert!(parse(&bad, &known()).is_err(), "accepted:\n{bad}");
            }
        }

        #[test]
        fn a_file_location_splits_into_its_directory_and_one_leaf() {
            let doc = parse(
                "[[entry]]\nprovider = \"b\"\nbase = \"prefix\"\nlocation = { kind = \"file\", file = \"d/e/f.lock\" }\nsource = \"s\"\nread_on = \"2026-10-05\"\n",
                &known(),
            )
            .expect("valid");
            assert_eq!(
                doc.entry[0].site_under(Path::new("/base")),
                LockSite::File {
                    dir: PathBuf::from("/base/d/e"),
                    name: OsString::from("f.lock"),
                }
            );
        }
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

    #[test]
    fn a_replay_gets_a_lock_probe_that_never_looks_whichever_scope_it_names() {
        let runner = replay();
        for scope in [FilesystemScope::Elsewhere, FilesystemScope::ThisMachine] {
            let mut ctx = context(&runner);
            ctx.filesystem = scope;
            assert!(lock_probe(&ctx).is_unknowable(), "{scope:?}");
        }
    }

    #[test]
    fn a_live_runner_beside_another_filesystem_gets_a_lock_probe_that_never_looks() {
        // `effective_scope` would call this machine; the probe must not.
        let runner = LiveRunner;
        assert!(lock_probe(&context(&runner)).is_unknowable());
    }

    #[test]
    fn an_unresolvable_portdbpath_is_an_unresolved_lock_site() {
        // Mutant 8: a configuration naming another database, or none readable,
        // must not fall back to some place under the prefix.
        use crate::lock_probe::LockSite;
        let id = ProviderId::new(crate::macports::PROVIDER_ID);
        let prefix = std::env::temp_dir().join(format!("mpb-portdb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&prefix);
        let conf_dir = prefix.join("etc/macports");
        std::fs::create_dir_all(&conf_dir).expect("a scratch prefix");
        let site = DetectedPrefix {
            provider: id.clone(),
            prefix: prefix.clone(),
            home: Some(prefix.clone()),
            derivation: derivation(&id).expect("a known provider"),
            replay: false,
        };
        assert_eq!(lock_site(&site), LockSite::Unresolved, "no configuration");
        let conf = conf_dir.join("macports.conf");
        std::fs::write(&conf, "portdbpath /elsewhere/db\n").expect("a scratch configuration");
        assert_eq!(lock_site(&site), LockSite::Unresolved, "another database");
        std::fs::write(&conf, "").expect("a scratch configuration");
        let LockSite::File { dir, .. } = lock_site(&site) else {
            panic!("the default database resolves to a named lock file");
        };
        assert!(dir.starts_with(prefix.join("var/macports")), "{dir:?}");
        std::fs::remove_dir_all(&prefix).expect("cleanup");
    }

    #[test]
    fn the_lock_probe_asks_the_runner_whether_it_is_a_replay_exactly_once() {
        let runner = Flipping(std::cell::Cell::new(0));
        let probe = lock_probe(&context(&runner));
        assert_eq!(runner.0.get(), 1);
        assert!(probe.is_unknowable(), "the first answer, replay, decides");
    }

    #[test]
    fn a_live_lock_probe_samples_only_detected_providers_under_their_documented_prefixes() {
        // Stats this machine's filesystem through detection and reads its
        // `macports.conf` if it has one, and spawns nothing; the override in
        // the context is ignored against a live runner. Constructing the
        // probe samples nothing. Says nothing where no provider is installed.
        let runner = LiveRunner;
        let mut ctx = context(&runner);
        ctx.filesystem = FilesystemScope::ThisMachine;
        let probe = lock_probe(&ctx);
        assert!(!probe.is_unknowable());
        for (provider, site) in probe.sites() {
            // A provider left undetected by an error is sampled too, unresolved.
            if *site == crate::lock_probe::LockSite::Unresolved {
                continue;
            }
            let detected = detected_prefix(&ctx, provider).expect("a detected provider");
            match site {
                crate::lock_probe::LockSite::Directory(dir) => {
                    assert!(dir.starts_with(detected.prefix()), "{dir:?}");
                }
                crate::lock_probe::LockSite::File { dir, .. } => {
                    assert!(dir.starts_with(detected.prefix()), "{dir:?}");
                }
                crate::lock_probe::LockSite::Unresolved => {}
            }
        }
    }
}
