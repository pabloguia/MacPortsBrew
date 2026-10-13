// SPDX-License-Identifier: Apache-2.0

//! The system adapter against the captured fixture.
//!
//! `07-quality-and-testing.md` says fixtures are captured, not hand-written,
//! and `AGENTS.md` says a provider fact is observed or cited, never recalled.
//! This test closes that loop for the machine facts: it replays
//! `fixtures/recordings/homebrew-only-intel-macos26.json` — captured by
//! `scripts/capture-fixture.sh` on a real Intel Mac running macOS 26.6.2 on
//! 2026-09-02 — through the same code that reads a live machine, and asserts
//! what that machine actually is.
//!
//! It also pins the argv forms. The capture script records exactly seven
//! system invocations; if the adapter starts asking a question the script does
//! not record, this test fails rather than a fixture silently going stale.

use macportsbrew_adapters::persistence::load_recording;
use macportsbrew_adapters::process::ReplayRunner;
use macportsbrew_adapters::system::SystemMachine;
use macportsbrew_domain::{
    Architecture, CommandLineTools, HomeRedaction, MachineRead, MacosVersion, Translation,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn reader() -> SystemMachine {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/homebrew-only-intel-macos26.json");
    let recording = load_recording(&path).expect("the captured fixture must load");
    SystemMachine::new(
        Box::new(ReplayRunner::new(recording)),
        Some("/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_string()),
        BTreeMap::new(),
        HomeRedaction::Known(PathBuf::from("/Users/ada")),
    )
}

#[test]
fn the_captured_intel_machine_reads_back_exactly() {
    let machine = reader().machine().unwrap();
    assert_eq!(machine.architecture, Architecture::X86_64);
    assert_eq!(machine.macos, MacosVersion::new(26, 6, 2));
    // Both optional sysctls are absent on this machine; Apple's documented
    // `ENOENT` case means native, not unknown.
    assert_eq!(machine.translation, Translation::Native);
    assert_eq!(
        machine.command_line_tools,
        CommandLineTools::Installed {
            path: PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
        }
    );
}

#[test]
fn the_adapter_needs_only_what_the_capture_script_records() {
    // The system forms in `scripts/capture-fixture.sh`. An adapter that needs
    // another one needs the script changed and the fixture recaptured, and
    // this test is what says so.
    let system_forms = [
        ("sw_vers", vec!["-productVersion"]),
        ("sw_vers", vec!["-buildVersion"]),
        ("uname", vec!["-m"]),
        ("sysctl", vec!["-n", "hw.machine"]),
        ("sysctl", vec!["-n", "hw.optional.arm64"]),
        ("sysctl", vec!["-n", "sysctl.proc_translated"]),
        ("xcode-select", vec!["-p"]),
    ];
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/recordings/homebrew-only-intel-macos26.json");
    let mut recording = load_recording(&path).unwrap();

    // Everything the fixture records about the system, and nothing else.
    let present: Vec<(String, Vec<String>)> = recording
        .invocations
        .iter()
        .map(|r| (r.invocation.program.clone(), r.invocation.argv.clone()))
        .filter(|(program, _)| {
            matches!(
                program.as_str(),
                "sw_vers" | "uname" | "sysctl" | "xcode-select"
            )
        })
        .collect();
    for (program, argv) in &system_forms {
        let argv: Vec<String> = argv.iter().map(|a| (*a).to_string()).collect();
        assert!(
            present.contains(&((*program).to_string(), argv.clone())),
            "the fixture no longer records {program} {argv:?}"
        );
    }
    assert_eq!(present.len(), system_forms.len());

    // Reading the machine from a recording that holds only those forms is the
    // proof that the adapter asks for nothing else: `ReplayRunner` errors on
    // any form it does not hold, and a probe that errors cannot produce the
    // exact answer asserted below.
    recording.invocations.retain(|r| {
        matches!(
            r.invocation.program.as_str(),
            "sw_vers" | "uname" | "sysctl" | "xcode-select"
        )
    });
    let reader = SystemMachine::new(
        Box::new(ReplayRunner::new(recording)),
        None,
        BTreeMap::new(),
        HomeRedaction::Recorded,
    );
    let machine = reader.machine().unwrap();
    assert_eq!(machine.architecture, Architecture::X86_64);
    assert_eq!(machine.translation, Translation::Native);
    assert_eq!(
        machine.command_line_tools,
        CommandLineTools::Installed {
            path: PathBuf::from("/Applications/Xcode.app/Contents/Developer"),
        }
    );
}

#[test]
fn the_environment_is_redacted_before_it_is_stored() {
    let reader = reader();
    let environment = reader.environment().unwrap();
    assert_eq!(
        environment.path,
        vec!["/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"]
    );
    assert!(environment.reported.is_empty());
    assert!(
        !environment.path.iter().any(|e| e.contains("/Users/")),
        "no home path may reach a snapshot (06-cli-and-ux.md, privacy)"
    );
}

#[test]
fn a_replayed_reader_says_it_is_a_replay() {
    assert!(reader().is_replay());
}
