// SPDX-License-Identifier: Apache-2.0

//! The Brazilian Portuguese catalogue as a user meets it (pass 12, item A).
//!
//! `tests/i18n.rs` covers the catalogue and the resolution function; these run
//! the real binary, against a recording, to prove that the four selectors reach
//! the translation in the documented order and that machine-readable output
//! does not change language. The per-key fallback to English is covered by the
//! unit tests `catalogue::tests::an_overlay_*`, and key parity by
//! `scripts/check-i18n-parity.sh`.

use macportsbrew::i18n::Catalogue;
use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");
const FIXTURE: &str = "homebrew-only-intel-macos26";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs `doctor` on the recording with exactly the language selectors given.
/// The three environment selectors are cleared first, so the host's own locale
/// cannot decide the outcome.
fn doctor(flag: Option<&str>, env: &[(&str, &str)], json: bool) -> String {
    let state = std::env::temp_dir().join(format!(
        "mpb-pt-br-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let mut cmd = Command::new(BIN);
    cmd.arg("--replay")
        .arg(root().join(format!("fixtures/recordings/{FIXTURE}.json")));
    if let Some(tag) = flag {
        cmd.args(["--language", tag]);
    }
    if json {
        cmd.arg("--json");
    }
    cmd.arg("doctor")
        .current_dir(root())
        .env("MACPORTSBREW_STATE_DIR", &state)
        .env_remove("MACPORTSBREW_LANG")
        .env_remove("LC_ALL")
        .env_remove("LANG");
    for (name, value) in env {
        cmd.env(name, value);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"));
    let _ = std::fs::remove_dir_all(&state);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    // A run that printed nothing would make every equality below vacuous.
    assert!(
        !text.trim().is_empty(),
        "doctor printed nothing: {:?}",
        out.status
    );
    text
}

#[test]
fn every_selector_reaches_the_translation() {
    let english = doctor(Some("en"), &[], false);
    let by_flag = doctor(Some("pt-BR"), &[], false);
    assert_ne!(english, by_flag, "--language pt-BR rendered English");

    for (name, value) in [
        ("MACPORTSBREW_LANG", "pt-BR"),
        ("LC_ALL", "pt_BR.UTF-8"),
        ("LANG", "pt_BR.UTF-8"),
    ] {
        assert_eq!(
            doctor(None, &[(name, value)], false),
            by_flag,
            "{name}={value} did not select pt-BR"
        );
    }
}

#[test]
fn the_selectors_keep_their_documented_order() {
    let english = doctor(Some("en"), &[], false);
    let portuguese = doctor(Some("pt-BR"), &[], false);

    // The flag beats the environment, both ways round.
    assert_eq!(
        doctor(Some("en"), &[("LANG", "pt_BR.UTF-8")], false),
        english
    );
    assert_eq!(
        doctor(Some("pt-BR"), &[("LANG", "en_US.UTF-8")], false),
        portuguese
    );
    // The application's own variable beats LC_ALL, and LC_ALL beats LANG.
    assert_eq!(
        doctor(
            None,
            &[("MACPORTSBREW_LANG", "en"), ("LC_ALL", "pt_BR.UTF-8")],
            false
        ),
        english
    );
    assert_eq!(
        doctor(
            None,
            &[("LC_ALL", "pt_BR.UTF-8"), ("LANG", "en_US.UTF-8")],
            false
        ),
        portuguese
    );
    assert_eq!(
        doctor(
            None,
            &[("LC_ALL", "en_US.UTF-8"), ("LANG", "pt_BR.UTF-8")],
            false
        ),
        english
    );
}

#[test]
fn json_output_does_not_change_language() {
    assert_eq!(
        doctor(Some("pt-BR"), &[], true),
        doctor(Some("en"), &[], true),
        "--json differs between languages; it must always be English and stable"
    );
}

#[test]
fn the_overlay_has_the_english_keys_and_most_of_them_are_translated() {
    let english = Catalogue::english();
    let portuguese = Catalogue::portuguese_brazil();
    assert_eq!(
        english.keys().collect::<Vec<_>>(),
        portuguese.keys().collect::<Vec<_>>()
    );
    // The key sets agree by construction (the overlay starts from English), so
    // this proves only that nothing was added; a *missing* translation is
    // caught by `scripts/check-i18n-parity.sh` and by requiring here that
    // nearly every sentence differs from its English source.
    let same = english
        .keys()
        .filter(|key| english.get(key) == portuguese.get(key))
        .count();
    assert!(
        same * 10 < english.len(),
        "{same} of {} entries are still English",
        english.len()
    );
}

#[test]
fn any_portuguese_tag_resolves_to_the_brazilian_catalogue() {
    // Only one Portuguese catalogue exists, so a European tag gets it rather
    // than English; this is `resolve`'s bare-language rule, pinned here.
    let european = doctor(None, &[("LANG", "pt_PT.UTF-8")], false);
    assert_eq!(european, doctor(Some("pt-BR"), &[], false));
}

/// `--help` with exactly the language selectors given, every subcommand path
/// named in `args`.
fn help(args: &[&str], flag: Option<&str>, env: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(BIN);
    if let Some(tag) = flag {
        cmd.args(["--language", tag]);
    }
    cmd.args(args)
        .arg("--help")
        .env_remove("MACPORTSBREW_LANG")
        .env_remove("LC_ALL")
        .env_remove("LANG");
    for (name, value) in env {
        cmd.env(name, value);
    }
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("spawning {BIN}: {e}"));
    assert!(out.status.success(), "--help failed: {:?}", out.status);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(!text.trim().is_empty(), "--help printed nothing");
    text
}

/// `--help` is compile-time English that does not go through the catalogue,
/// a stated exemption (`docs/spec-evolution.md`, the help-text addendum). So
/// there is no Portuguese help to drift from the English: every selector gives
/// the same bytes. A change that localises part of the help fails here, and has
/// to bring the help into the catalogue, where `check-i18n-parity.sh` keeps the
/// two languages in step, rather than leave a second copy beside it.
#[test]
fn help_is_one_text_whatever_the_language() {
    for path in [&[][..], &["history"][..], &["history", "acknowledge"][..]] {
        let english = help(path, None, &[]);
        assert_eq!(english, help(path, Some("pt-BR"), &[]), "{path:?}");
        assert_eq!(
            english,
            help(path, None, &[("MACPORTSBREW_LANG", "pt-BR")]),
            "{path:?}"
        );
        assert_eq!(
            english,
            help(path, None, &[("LANG", "pt_BR.UTF-8")]),
            "{path:?}"
        );
    }
}

/// The top-level line for `history` once said it "Reads only", while
/// `history acknowledge <id> --yes` writes MacPortsBrew's own state directory
/// (it makes the record durable and removes the interrupted-run marker).
#[test]
fn the_history_help_line_names_the_one_subcommand_that_writes() {
    let top = help(&[], None, &[]);
    let line = top
        .lines()
        .find(|l| l.trim_start().starts_with("history "))
        .unwrap_or_else(|| panic!("no `history` line in --help:\n{top}"));
    assert!(!line.contains("Reads only"), "{line}");
    assert!(
        line.contains("`history acknowledge <id> --yes` writes"),
        "{line}"
    );
    assert!(line.contains("own state directory"), "{line}");
}
