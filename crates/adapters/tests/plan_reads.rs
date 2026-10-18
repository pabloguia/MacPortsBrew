// SPDX-License-Identifier: Apache-2.0

//! How a plan's provider command becomes something that may run (DP-023).
//!
//! The executor never adopts a program or an environment from a plan: the
//! program is re-derived from the detected prefix and the adapter's executable,
//! the environment from the adapter's own construction, and the plan supplies
//! argv only. These tests pin that at `registry::plan_read_spec`, the one door,
//! against the captured `both-providers-intel-macos26` recording and never a
//! live installation (INV-05).
//!
//! Verbs are read from `data/policy/provider-commands.toml` rather than
//! written here, so the tests exercise the verbs a plan would actually carry,
//! and no mutating verb is spelled out as a literal.

use macportsbrew_adapters::filesystem::FilesystemScope;
use macportsbrew_adapters::process::{
    CommandSpec, ExitStatus, Invocation, PlanReadRefusal, ProcessRunner, RecordedInvocation,
    Recording, ReplayRunner, SystemRunner,
};
use macportsbrew_adapters::registry::{
    self, detected_prefix, plan_read_sites, plan_read_spec, DetectedPrefix, PlanReadError,
    RegistryContext,
};
use macportsbrew_domain::{CommandTarget, Elevation, PlanCommand, ProviderId, SuccessSemantic};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FIXTURE: &str = "both-providers-intel-macos26";

fn repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The provider command table's entry for `provider`.
fn entry(provider: &ProviderId) -> toml::Value {
    let table: toml::Value = toml::from_str(&repo_file("data/policy/provider-commands.toml"))
        .expect("the provider command table parses");
    table["entry"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["provider"].as_str() == Some(provider.as_str()))
        .unwrap_or_else(|| panic!("no command table entry for {provider:?}"))
        .clone()
}

fn verb(provider: &ProviderId, operation: &str) -> Vec<String> {
    entry(provider)["verbs"][operation]
        .as_array()
        .unwrap_or_else(|| panic!("no {operation} verb for {provider:?}"))
        .iter()
        .map(|v| v.as_str().expect("a verb element is a string").to_string())
        .collect()
}

fn executable(provider: &ProviderId) -> String {
    entry(provider)["executable"]
        .as_str()
        .expect("executable is a string")
        .to_string()
}

/// The captured recording, plus one synthesised invocation per provider for the
/// inventory read a `VERIFY_REMOVAL` carries, which no fixture records (F13).
/// Its output is irrelevant here: these tests observe the spec, and the replay
/// runner copies the spec's environment into what it returns.
fn recording() -> Recording {
    let mut rec = Recording::from_json(&repo_file(&format!("fixtures/recordings/{FIXTURE}.json")))
        .expect("fixture parses");
    for id in registry::known_provider_ids() {
        let program = leaf(&executable(&id));
        let argv = verb(&id, "verify_absent");
        synthesise(&mut rec, &program, &argv);
    }
    rec
}

fn synthesise(rec: &mut Recording, program: &str, argv: &[String]) {
    let present = rec
        .invocations
        .iter()
        .any(|r| r.invocation.program == program && r.invocation.argv == argv);
    if !present {
        rec.invocations.push(RecordedInvocation {
            invocation: Invocation {
                program: program.to_string(),
                argv: argv.to_vec(),
                environment: BTreeMap::new(),
                status: ExitStatus::Exited { code: 0 },
                stdout: String::new(),
                stderr: String::new(),
                truncated: false,
                duration_ms: 1,
            },
        });
    }
}

fn leaf(executable: &str) -> String {
    Path::new(executable)
        .file_name()
        .expect("an executable has a file name")
        .to_string_lossy()
        .into_owned()
}

/// Each provider's prefix, as the recording itself answers it.
fn prefix(id: &ProviderId) -> PathBuf {
    let rec = recording();
    let program = leaf(&executable(id));
    // Homebrew states its prefix; MacPorts has no such read, and the recording
    // is of the documented default.
    rec.invocations
        .iter()
        .find(|r| r.invocation.program == program && r.invocation.argv == ["--prefix"])
        .map_or_else(
            || registry::candidate_prefixes(id)[0].clone(),
            |r| PathBuf::from(r.invocation.stdout.trim()),
        )
}

/// A provider read exactly as the planner would write it for `id` at `prefix`.
fn read_command(id: &ProviderId, prefix: &Path) -> PlanCommand {
    PlanCommand {
        target: CommandTarget::Provider,
        program: prefix.join(executable(id)).to_string_lossy().into_owned(),
        argv: verb(id, "verify_absent"),
        environment: BTreeMap::new(),
        elevation: Elevation::None,
        success: SuccessSemantic::ExitStatus,
        placeholder: false,
    }
}

const HOME: &str = "/Users/ada";

/// The registry context a run would hold: a replay runner, the recording's own
/// prefix for every provider as the replay-only override, and `home`.
fn context(
    runner: &ReplayRunner,
    home: Option<PathBuf>,
    overrides: Vec<(ProviderId, PathBuf)>,
) -> RegistryContext<'_> {
    RegistryContext {
        runner,
        home,
        platform_tags: Box::new(|_| Vec::new()),
        filesystem: FilesystemScope::Elsewhere,
        prefix_override: Some(overrides),
    }
}

fn recorded_prefixes() -> Vec<(ProviderId, PathBuf)> {
    registry::known_provider_ids()
        .into_iter()
        .map(|id| {
            let p = prefix(&id);
            (id, p)
        })
        .collect()
}

/// The detected prefix of `id`, the only thing a plan read derives from.
fn site(runner: &ReplayRunner, id: &ProviderId, home: Option<PathBuf>) -> DetectedPrefix {
    detected_prefix(&context(runner, home, recorded_prefixes()), id)
        .unwrap_or_else(|| panic!("{id:?} is detected from the recording"))
}

#[test]
fn invariant_10_a_plan_read_never_adopts_the_environment_it_carries() {
    // Every kind of tampering a plan document can carry: a build flag, a loader
    // injection, a PATH that puts another directory first, a different HOME, a
    // provider variable that switches off its own safety defaults, and a value
    // of a key the derivation does set.
    let tampered: BTreeMap<String, String> = [
        ("CPATH", "/opt/local/include"),
        ("DYLD_INSERT_LIBRARIES", "/tmp/evil.dylib"),
        ("PATH", "/tmp/evil/bin:/usr/bin"),
        ("HOME", "/tmp/evil-home"),
        ("HOMEBREW_DEVELOPER", "1"),
        ("LC_ALL", "en_US.UTF-8"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let runner = ReplayRunner::new(recording());
    for id in registry::known_provider_ids() {
        let site = site(&runner, &id, Some(PathBuf::from(HOME)));
        let honest = read_command(&id, site.prefix());
        let mut forged = honest.clone();
        forged.environment = tampered.clone();

        let spawned = runner
            .run(&plan_read_spec(&site, &forged).expect("a permitted read"))
            .expect("the recording answers the read");
        let reference = runner
            .run(&plan_read_spec(&site, &honest).expect("a permitted read"))
            .expect("the recording answers the read");

        // The plan's environment has no influence at all.
        assert_eq!(
            spawned.environment, reference.environment,
            "{id:?}: a plan's environment reached the spawned command"
        );
        for (key, value) in &tampered {
            assert_ne!(
                spawned.environment.get(key),
                Some(value),
                "{id:?}: the plan's {key} reached the spawned command"
            );
        }
        assert!(!spawned.environment.contains_key("CPATH"));
        assert!(!spawned.environment.contains_key("DYLD_INSERT_LIBRARIES"));
        assert!(!spawned.environment.contains_key("HOMEBREW_DEVELOPER"));
        // HOME is the registry context's own capture (F17), never the plan's
        // copy — which on a live run would be capture's redacted `~`.
        assert_eq!(
            spawned.environment.get("HOME").map(String::as_str),
            Some(HOME)
        );
        assert!(spawned.environment["PATH"]
            .starts_with(&site.prefix().join("bin").to_string_lossy().into_owned()));
    }
}

#[test]
fn the_two_providers_are_derived_by_the_same_code() {
    // The environment a plan read gets is the one each adapter builds for its
    // own invocations: observed on the adapter side through the snapshot it
    // captures from this recording, and on the plan side through the replay
    // runner. Both are built from one registry context, so the prefix and the
    // home are the same capture.
    let runner = ReplayRunner::new(recording());
    let ctx = context(&runner, None, recorded_prefixes());
    let adapters = registry::build(&ctx);
    assert_eq!(adapters.len(), registry::known_provider_ids().len());
    for adapter in &adapters {
        let id = adapter.id().clone();
        let site = detected_prefix(&ctx, &id).expect("detected from the recording");
        assert_eq!(site.provider(), &id);
        let snapshot = adapter.snapshot().expect("the recording captures");
        assert_eq!(
            snapshot.detection.prefix.as_deref(),
            Some(site.prefix()),
            "{id:?}: the adapter and the plan read resolved different prefixes"
        );
        let spec = plan_read_spec(&site, &read_command(&id, site.prefix()))
            .unwrap_or_else(|r| panic!("{id:?}: {r:?}"));
        assert_eq!(spec.program_name(), leaf(&executable(&id)));
        let spawned = runner.run(&spec).expect("the recording answers the read");
        assert_eq!(
            spawned.environment, snapshot.invocation_environment,
            "{id:?}: a plan read is not derived the way the adapter runs"
        );
    }
}

#[test]
fn a_plan_whose_program_disagrees_with_the_derived_one_is_refused() {
    let runner = ReplayRunner::new(recording());
    let ids = registry::known_provider_ids();
    for id in &ids {
        let site = site(&runner, id, None);
        let honest = read_command(id, site.prefix());

        // Same leaf name, somewhere the plan chose.
        let mut elsewhere = honest.clone();
        elsewhere.program = format!("/tmp/evil/bin/{}", leaf(&executable(id)));
        // The other provider's program, under this one's id.
        let other = ids.iter().find(|o| *o != id).expect("two providers");
        let mut swapped = honest.clone();
        swapped.program = read_command(other, site.prefix()).program;
        // A placeholder, which the planner writes when nothing was detected.
        let mut placeholder = honest.clone();
        placeholder.program = format!("<{} prefix>/bin/{}", id.as_str(), id.as_str());

        for forged in [elsewhere, swapped, placeholder] {
            let refusal = plan_read_spec(&site, &forged)
                .expect_err("a program the prefix does not give is refused");
            assert_eq!(
                refusal,
                PlanReadRefusal::ProgramDisagrees {
                    planned: forged.program.clone(),
                    derived: honest.program.clone(),
                },
                "{id:?}"
            );
        }
        assert!(plan_read_spec(&site, &honest).is_ok());
    }
}

#[test]
fn a_plan_read_is_checked_against_the_read_list_at_construction() {
    // The add verb, as the command table spells it, with a package after it.
    // The replay runner asks no allowlist, and this recording is given an
    // answer for exactly that argv: so if construction did not refuse, the
    // runner would happily "run" it. Nothing here touches the live runner.
    let mut rec = recording();
    let ids = registry::known_provider_ids();
    for id in &ids {
        let mut argv = verb(id, "add");
        argv.push("htop".to_string());
        synthesise(&mut rec, &leaf(&executable(id)), &argv);
    }
    let runner = ReplayRunner::new(rec);
    for id in &ids {
        let site = site(&runner, id, None);
        let mut forged = read_command(id, site.prefix());
        forged.argv = verb(id, "add");
        forged.argv.push("htop".to_string());

        let refusal = plan_read_spec(&site, &forged)
            .map(|spec| {
                let answered = runner.run(&spec).is_ok();
                format!("constructed a spec the replay runner answered: {answered}")
            })
            .expect_err("a mutating verb is not a read");
        assert_eq!(
            refusal,
            PlanReadRefusal::NotAPermittedRead {
                argv: forged.argv.clone()
            },
            "{id:?}"
        );
    }
}

#[test]
fn every_argv_injection_form_is_refused_through_the_plan_path() {
    // The forms the spawn-site allowlist exists for (SE-050, F-53), sent the way
    // a forged plan would send them: with the honest program, so the argv is
    // the only thing wrong. Each provider gets its own read as the base.
    let runner = ReplayRunner::new(recording());
    let s = |v: &[&str]| v.iter().map(|a| (*a).to_string()).collect::<Vec<String>>();
    for id in registry::known_provider_ids() {
        let site = site(&runner, &id, None);
        let read = verb(&id, "verify_absent");
        let with = |extra: &[&str]| {
            let mut argv = read.clone();
            argv.extend(s(extra));
            argv
        };
        let mut joined = read.clone();
        let last = joined.pop().expect("a read has a subcommand");
        joined.push(format!("{last};version"));
        let mut cases: Vec<(&str, Vec<String>)> = vec![
            (
                "a command file after the subcommand",
                with(&["-F", "/tmp/f"]),
            ),
            ("a short option after the subcommand", with(&["-v"])),
            ("an element holding a slash", with(&["/tmp/p"])),
            ("an element holding a colon", with(&["x:y"])),
            ("a URL", with(&["file:///tmp/p"])),
            ("a command file before the subcommand", {
                let mut argv = s(&["-F", "/tmp/f"]);
                argv.extend(read.clone());
                argv
            }),
            ("an empty argv", Vec::new()),
        ];
        // `;` is an action separator only for `port` (`process_cmd`, MacPorts
        // 2.12.6, F-53), which is why only `port` has a separator list. `brew`
        // is a bash script handing its arguments on as `"$@"`, so a `;` is one
        // more argument: observed on this machine 2026-09-30, Homebrew 7.0.4,
        // `brew list --versions ';' version` exits 1 with no output (two names
        // that are not installed formulae), and `brew list '--versions;version'`
        // exits 1 with "invalid option". Neither runs a second action, so the
        // two forms are asserted where they mean something (SE-050).
        if leaf(&executable(&id)) != "brew" {
            cases.push(("a separator element", with(&[";", "version"])));
            cases.push(("a separator joined into the subcommand", joined));
        }
        for (form, argv) in cases {
            let mut forged = read_command(&id, site.prefix());
            forged.argv.clone_from(&argv);
            assert_eq!(
                plan_read_spec(&site, &forged),
                Err(PlanReadRefusal::NotAPermittedRead { argv }),
                "{id:?}: {form}"
            );
        }
    }
}

#[test]
fn a_tool_command_is_not_a_provider_read() {
    let runner = ReplayRunner::new(recording());
    for id in registry::known_provider_ids() {
        let site = site(&runner, &id, None);
        // As the planner writes `SET_PRIMARY`, and forged to carry this
        // provider's real program and a real read: the target alone decides.
        let as_written = PlanCommand {
            target: CommandTarget::Tool,
            program: "macportsbrew".to_string(),
            argv: vec!["primary".into(), "htop".into(), id.as_str().to_string()],
            ..read_command(&id, site.prefix())
        };
        let dressed_as_a_read = PlanCommand {
            target: CommandTarget::Tool,
            ..read_command(&id, site.prefix())
        };
        for command in [as_written, dressed_as_a_read] {
            assert_eq!(
                plan_read_spec(&site, &command),
                Err(PlanReadRefusal::NotAProviderCommand),
                "{id:?}"
            );
        }
    }
}

#[test]
fn a_prefix_that_is_not_safe_is_refused() {
    // Under replay the recording's own `--prefix` answer is the only prefix,
    // and it arrives through the override; a recording can say anything, so
    // what it says is checked before a program is built from it.
    let runner = ReplayRunner::new(recording());
    for id in registry::known_provider_ids() {
        for bad in [
            "relative/prefix",
            "",
            "/opt/../tmp",
            "/opt/lo\ncal",
            &format!(
                "/{}",
                "a".repeat(macportsbrew_adapters::process::MAX_PATH_LEN)
            ),
        ] {
            let prefix = PathBuf::from(bad);
            let ctx = context(&runner, None, vec![(id.clone(), prefix.clone())]);
            let site = detected_prefix(&ctx, &id).expect("the override is the detection");
            // The plan agrees with the derivation, so the prefix is the only
            // thing wrong with this request.
            let command = read_command(&id, &prefix);
            assert_eq!(
                plan_read_spec(&site, &command),
                Err(PlanReadRefusal::UnsafePrefix {
                    prefix: bad.to_string()
                }),
                "{id:?} at {bad:?}"
            );
        }
    }
}

#[test]
fn only_detection_yields_a_prefix_and_a_provider_it_does_not_know_yields_none() {
    let runner = ReplayRunner::new(recording());
    // An unknown provider has no derivation, so no site, whatever the context
    // claims about it.
    let unknown = ProviderId::new("pkgsrc");
    let ctx = context(
        &runner,
        None,
        vec![(unknown.clone(), PathBuf::from("/opt/pkg"))],
    );
    assert!(detected_prefix(&ctx, &unknown).is_none());
    // A known provider that was not detected has no site either: under replay
    // with no override nothing is probed (`FilesystemScope::Elsewhere`), and
    // unlike `build` there is no fallback to a first candidate.
    for id in registry::known_provider_ids() {
        let ctx = context(&runner, None, Vec::new());
        assert!(
            detected_prefix(&ctx, &id).is_none(),
            "{id:?}: a site was produced for a provider nobody detected"
        );
    }
    // And the site carries the context's home, not one a caller chose: the
    // same context with and without a home derives different environments.
    let id = &registry::known_provider_ids()[0];
    let with = site(&runner, id, Some(PathBuf::from(HOME)));
    let without = site(&runner, id, None);
    let command = read_command(id, with.prefix());
    let env = |site: &DetectedPrefix| {
        runner
            .run(&plan_read_spec(site, &command).expect("a permitted read"))
            .expect("answered")
            .environment
    };
    assert_eq!(env(&with).get("HOME").map(String::as_str), Some(HOME));
    assert!(!env(&without).contains_key("HOME"));
}

/// A prefix resolved through a replay runner is a recording's answer, not a
/// probe of this machine: a spec built from it is answered by the recording and
/// refused by the live runner, whoever hands it over (SE-050, "Not closed" 7).
/// The prefix does not exist, so a spec that slipped past the mark would end
/// `NotStarted` too — from the allowlist or the kernel; the assertion is on
/// which refusal, so it cannot pass by that route.
#[test]
fn a_prefix_resolved_under_replay_never_reaches_a_live_spawn() {
    let runner = ReplayRunner::new(recording());
    for id in registry::known_provider_ids() {
        let nowhere = PathBuf::from("/nonexistent-mpb-replay-prefix");
        let ctx = context(&runner, None, vec![(id.clone(), nowhere.clone())]);
        let site = detected_prefix(&ctx, &id).expect("the override is the detection");
        let spec = plan_read_spec(&site, &read_command(&id, &nowhere)).expect("a permitted read");
        let inv = SystemRunner::new().run(&spec).expect("recorded");
        let ExitStatus::NotStarted { error } = &inv.status else {
            panic!(
                "{id:?}: a replay-resolved spec was spawned: {:?}",
                inv.status
            );
        };
        assert!(error.contains("under replay"), "{id:?}: {error}");
    }
}

/// A read site runs through the runner its prefix was detected through, and
/// nothing else: a site detected from a recording is answered by that
/// recording. The sites are built from the context the adapters are, so their
/// prefix and home cannot disagree.
#[test]
fn a_read_site_is_bound_to_the_runner_it_was_detected_through() {
    let runner = ReplayRunner::new(recording());
    let ctx = context(&runner, Some(PathBuf::from(HOME)), recorded_prefixes());
    let (adapters, sites) = (registry::build(&ctx), plan_read_sites(&ctx));
    assert_eq!(adapters.len(), registry::known_provider_ids().len());
    assert_eq!(
        sites.providers().len(),
        registry::known_provider_ids().len()
    );
    for id in registry::known_provider_ids() {
        let site = detected_prefix(&ctx, &id).expect("detected from the recording");
        let command = read_command(&id, site.prefix());
        let inv = sites.run(&id, &command).expect("answered by the recording");
        assert_eq!(inv.argv, command.argv, "{id:?}");
        assert_eq!(inv.environment.get("HOME").map(String::as_str), Some(HOME));
        let key = format!("{} {}", leaf(&executable(&id)), command.argv.join(" "));
        assert!(runner.consulted().contains(&key), "{id:?}: {key}");
    }
    // A provider the context did not detect has no site, and is not run.
    let undetected = context(&runner, None, Vec::new());
    for id in registry::known_provider_ids() {
        let command = read_command(&id, &prefix(&id));
        assert!(matches!(
            plan_read_sites(&undetected).run(&id, &command),
            Err(PlanReadError::NotDetected)
        ));
    }
    // Nor is it borrowed from a provider that was detected.
    let ids = registry::known_provider_ids();
    let (first, second) = (&ids[0], &ids[1]);
    let one = context(&runner, None, vec![(first.clone(), prefix(first))]);
    assert_eq!(plan_read_sites(&one).providers(), vec![first]);
    assert!(matches!(
        plan_read_sites(&one).run(second, &read_command(second, &prefix(second))),
        Err(PlanReadError::NotDetected)
    ));
    // A refusal at construction is reported as one, and nothing is run.
    let id = &registry::known_provider_ids()[0];
    let mut forged = read_command(id, &prefix(id));
    forged.program = "/tmp/elsewhere/bin/x".into();
    assert!(matches!(
        sites.run(id, &forged),
        Err(PlanReadError::Refused(
            PlanReadRefusal::ProgramDisagrees { .. }
        ))
    ));
}

/// Long options no read of this build passes, after a subcommand that is on
/// the read list (SE-050): each refused at construction and at the spawn site.
/// Keyed by the program leaf the command table names, so both providers are
/// covered without either name being an argument a test can drift from.
///
/// `brew list --cask` is here on purpose: F-22's cask verb will need it, and
/// adding it then is a deliberate, reviewed widening of the read list.
const OFF_LIST: &[(&str, &[&str])] = &[
    // Opens a browser.
    ("brew", &["info", "--github", "wget"]),
    // Fetches over the network.
    ("brew", &["info", "--fetch-HEAD", "wget"]),
    // `--json=v2` is on the list; nothing else spelled like it is.
    ("brew", &["info", "--json=v1", "--installed"]),
    ("brew", &["info", "--json=v2x", "--installed"]),
    ("brew", &["info", "--json", "--installed"]),
    ("brew", &["info", "--json=v2,", "--installed"]),
    // An abbreviation of `--versions`, which the provider would resolve.
    ("brew", &["list", "--vers"]),
    ("brew", &["list", "--cask"]),
    // A bare `--` ends option parsing; nothing this build runs passes one.
    ("brew", &["list", "--versions", "--"]),
    ("brew", &["config", "--verbose"]),
    // An abbreviation of `--name`, which `port` would resolve.
    ("port", &["-q", "info", "--nam", "libpng"]),
    ("port", &["-q", "info", "--fullname", "libpng"]),
    ("port", &["-q", "installed", "--all"]),
    ("port", &["-q", "provides", "--foo"]),
    ("port", &["-q", "echo", "--bogus"]),
];

/// A prefix that does not exist. A spec that got past the spawn-site guard
/// would end `NotStarted` from the kernel instead, never reaching a provider:
/// no row of this file ever goes through a runner at a real prefix.
const NOWHERE: &str = "/nonexistent-mpb-prefix";

/// The provider `program` names in the command table, and `row` as an argv.
fn off_list_row(program: &str, row: &[&str]) -> (ProviderId, Vec<String>) {
    let id = registry::known_provider_ids()
        .into_iter()
        .find(|id| leaf(&executable(id)) == program)
        .unwrap_or_else(|| panic!("no provider runs `{program}`"));
    (id, row.iter().map(|a| (*a).to_string()).collect())
}

/// `argv` through the live runner at [`NOWHERE`], for `id`'s executable.
///
/// The executable handed to `CommandSpec::provider` is a literal of this file,
/// chosen by the leaf the command table names and asserted equal to the
/// table's value - never the table's string itself, which would let a data
/// file decide what the live runner is asked to start.
fn spawned_at_nowhere(id: &ProviderId, argv: Vec<String>) -> ExitStatus {
    let relative = executable(id);
    let as_path = Path::new(&relative);
    assert!(
        as_path.is_relative()
            && !as_path
                .components()
                .any(|c| c == std::path::Component::ParentDir),
        "{id:?}: the table's executable {relative:?} must be relative and stay under the prefix"
    );
    let literal: &'static str = match leaf(&relative).as_str() {
        "brew" => "bin/brew",
        "port" => "bin/port",
        other => panic!("{id:?}: no literal for `{other}`; add one deliberately"),
    };
    assert_eq!(
        literal, relative,
        "{id:?}: the table and the literal disagree"
    );
    assert!(
        !Path::new(NOWHERE).exists(),
        "{NOWHERE} exists; this test must never reach a real program"
    );
    let spec = CommandSpec::provider(Path::new(NOWHERE), literal, argv, BTreeMap::new());
    SystemRunner::new()
        .run(&spec)
        .expect("a refusal is recorded")
        .status
}

#[test]
fn an_off_list_long_option_is_refused_at_construction() {
    // Through the one door a plan's command goes through, with the honest
    // program, so the option is the only thing wrong.
    let runner = ReplayRunner::new(recording());
    let mut covered = std::collections::BTreeSet::new();
    for (program, row) in OFF_LIST {
        let (id, argv) = off_list_row(program, row);
        let site = site(&runner, &id, None);
        let mut forged = read_command(&id, site.prefix());
        forged.argv.clone_from(&argv);
        assert_eq!(
            plan_read_spec(&site, &forged),
            Err(PlanReadRefusal::NotAPermittedRead { argv: argv.clone() }),
            "{program} {argv:?}"
        );
        covered.insert(id);
    }
    assert_eq!(
        covered.len(),
        registry::known_provider_ids().len(),
        "every provider has a row"
    );
}

#[test]
fn an_off_list_long_option_is_refused_at_the_spawn_site() {
    // Through the live runner, at a prefix that does not exist: a spec that got
    // past the guard would end `NotStarted` from the kernel, so the assertion
    // is on *which* refusal.
    for (program, row) in OFF_LIST {
        let (id, argv) = off_list_row(program, row);
        let status = spawned_at_nowhere(&id, argv.clone());
        let ExitStatus::NotStarted { error } = &status else {
            panic!("{program} {argv:?} reached the kernel: {status:?}");
        };
        assert!(
            error.contains("read allowlist"),
            "{program} {argv:?}: {error}"
        );
    }
    // The control: the planner's own read, at the same prefix, is let through
    // by the guard and refused by the kernel - so the rows above are refused
    // for their options and not for where they point.
    for id in registry::known_provider_ids() {
        let status = spawned_at_nowhere(&id, verb(&id, "verify_absent"));
        let ExitStatus::NotStarted { error } = &status else {
            panic!("{id:?}: a program at a nonexistent prefix started: {status:?}");
        };
        assert!(!error.contains("read allowlist"), "{id:?}: {error}");
    }
}

/// The configuration re-read before a mutating step (pass 08 slice 4, D7):
/// refused unless the prefix detected now derives the step's planned program,
/// so the configuration re-checked and the command that would run share one
/// prefix; and under replay the answer is `UNKNOWABLE`, which the re-check
/// refuses (INV-15) — no file of this host is read for a recording.
#[test]
fn the_configuration_re_read_shares_the_steps_prefix_and_never_reads_for_a_recording() {
    let runner = ReplayRunner::new(recording());
    let ctx = context(&runner, Some(PathBuf::from(HOME)), recorded_prefixes());
    for id in registry::known_provider_ids() {
        let site = site(&runner, &id, Some(PathBuf::from(HOME)));
        let planned = site
            .prefix()
            .join(executable(&id))
            .to_string_lossy()
            .into_owned();
        assert_eq!(
            registry::reread_configuration(&ctx, &id, &planned).expect("same prefix"),
            macportsbrew_domain::ProviderConfiguration::Unknowable,
            "{id:?}"
        );
        for other in ["/tmp/evil/bin/tool", "", planned.trim_end_matches('w')] {
            if other == planned {
                continue;
            }
            assert_eq!(
                registry::reread_configuration(&ctx, &id, other)
                    .expect_err("another program")
                    .id,
                macportsbrew_domain::ErrorId::SafetyBlocked,
                "{id:?} {other:?}"
            );
        }
    }
    assert_eq!(
        registry::reread_configuration(&ctx, &ProviderId::new("nonesuch"), "/x")
            .expect_err("not a provider")
            .id,
        macportsbrew_domain::ErrorId::SafetyBlocked
    );
}
