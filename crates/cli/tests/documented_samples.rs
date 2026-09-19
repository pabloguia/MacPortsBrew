// SPDX-License-Identifier: Apache-2.0

//! Every documented sample of the program's output is the program's output.
//!
//! F-27: `README.md` and `wiki/Getting-Started.md` both carried a `status`
//! block that the binary had not produced for two commits — wrong casing, a
//! missing word in the package count, a stale policy version. Each of the three
//! was a change this project made deliberately and got right, and each left a
//! documented sample asserting the behaviour it replaced. The drift then
//! happened a **fourth** time, inside the session that filed the finding,
//! between fixing the blocks and re-reading Homebrew's support page.
//!
//! Nothing could see it. `check-i18n-parity.sh` passes because every key is
//! present and rendered, `check-links.sh` has nothing to resolve, and no test
//! reads a markdown file. This is the instrument.
//!
//! A sample declares its own invocation, so adding one is enough to have it
//! checked:
//!
//! ```markdown
//! <!-- sample: --replay fixtures/recordings/<name>.json status -->
//! ```prose
//! $ macportsbrew status        <- optional; a prompt line is ignored
//! ...the output...
//! ```
//! ```
//!
//! F-50: the scan reached `README.md` and `wiki/*.md` and never `docs/schemas/`,
//! which is the directory the executor pass is told to build against. Two of
//! `plan.v1.md`'s samples were wrong when that was found — one carrying an
//! elevation F-24's observation had changed a fortnight earlier, and neither
//! carrying `placeholder`, which the row above them calls always present. Both
//! were fields an executor must read *before* running anything.
//!
//! Widening the scan needed two forms the convention did not have, because a
//! schema document shows a **piece** of an output and sometimes a **failing**
//! one. Both are optional tokens in the marker, and both default to what every
//! existing marker already means:
//!
//! - `at=<RFC 6901 JSON Pointer>` — parse stdout as JSON, resolve the pointer,
//!   and compare the block against that sub-value re-serialised. Without it the
//!   comparison is raw stdout, exactly as before. A pointer is used rather than
//!   a dotted path because it is a standard with a defined escaping rule and
//!   `serde_json::Value::pointer` implements it, so there is no parser here to
//!   get subtly wrong.
//! - `exit=<n>` — the invocation must exit exactly `n`. The default is `0`,
//!   which is stricter than the `status.success()` this used to assert, and it
//!   is what lets a document show a refusal honestly: `plan.v1.md`'s whole-plan
//!   sample exits 4 because every step is `MANUAL_REVIEW`, and under the old
//!   assertion it could not have been checked at all.
//!
//! Both tokens are stripped before the remainder becomes argv. They are safe to
//! strip positionally because the marker is this project's own syntax and no
//! subcommand takes an `at=` or `exit=` argument; a future one that did would
//! need a separator here.
//!
//! **Two blocks are deliberately still unchecked**, and they are named rather
//! than left to be noticed: `analysis.v1.md`'s and `explain.v1.md`'s, both of
//! which say in their own prose that they are abridged — one to the shape
//! rather than the whole package list, the other to three fired rules out of a
//! roster of fourteen. No marker form makes an abridged block byte-equal to an
//! output. Closing them means either regenerating them at full fidelity from a
//! small recording or adding a key-structure-only comparison, and that is its
//! own change. What is covered is every block in `plan.v1.md` and
//! `doctor.v1.md` — the executor's document, which is where F-50's defects were.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

/// Every documented sample must be replayed, so a sample can never depend on
/// the machine the suite happens to run on (INV-15, INV-05).
const REQUIRED_FLAG: &str = "--replay";

/// The only subcommands and flags a marker may name.
///
/// Found by `security-reviewer` on 2026-09-19, and older than the change that
/// found it: the marker's tokens became argv with **no constraint** but
/// "`--replay` appears somewhere". Two verbs on this binary write files —
/// `doctor --report <FILE>` and `snapshot --out <FILE>` — so one line added to
/// any scanned document, the kind of change reviewed fastest, made `cargo test`
/// replace a regular file at any path the test user can write, locally and in
/// CI. `write_atomic` refuses to write *through* a symlink, so it could not
/// follow a link; it could still overwrite. The exit assertion runs after the
/// write, so a failing test does not undo it.
///
/// Widening the scan to a third directory is what made this worth closing now.
/// The rule is the project's own and is stated in `.claude/rules/adapters.md`:
/// input never becomes a flag, and flags come from a list in code.
const ALLOWED: &[&str] = &[
    "--replay",
    "--json",
    "--to",
    "--retain-source",
    "--today",
    "status",
    "analyze",
    "explain",
    "plan",
    "doctor",
];

/// Refuses any marker argv that is not a read-only replay of a committed
/// recording. Every token must be `--replay`, that recording, an allowlisted
/// flag or subcommand, or a bare name — never a path and never an unlisted
/// flag, which is what kept `--report` and `--out` reachable.
fn check_argv(args: &[String], path: &Path, line: usize) {
    let where_ = format!("{}:{line}", path.display());
    assert_eq!(
        args.first().map(String::as_str),
        Some(REQUIRED_FLAG),
        "{where_}: a documented sample must be replayed, not taken from this machine, \
         and `{REQUIRED_FLAG}` must lead so the recording is the token after it"
    );
    let recording = args.get(1).map_or("", String::as_str);
    assert!(
        recording.starts_with("fixtures/recordings/")
            && Path::new(recording)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
            && !recording.contains(".."),
        "{where_}: {recording:?} is not a committed recording under fixtures/recordings/"
    );
    for token in &args[2..] {
        let bare_name = !token.is_empty()
            && !token.starts_with('-')
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c));
        assert!(
            ALLOWED.contains(&token.as_str()) || bare_name,
            "{where_}: {token:?} is neither an allowlisted flag or subcommand nor a bare \
             package or provider name. A marker may not name a path: `doctor --report` and \
             `snapshot --out` both write one, and this argv is read out of a markdown file"
        );
    }
}

/// A floor, not a count. A scan that finds nothing passes every assertion
/// below while proving nothing, which is the failure mode F-19 catalogued.
///
/// Six today: two prose `status` blocks (`README.md`, `wiki/Getting-Started.md`)
/// and four JSON blocks under `docs/schemas/`. The number is the current count
/// and is meant to be raised when a sample is added — left at 2 while four new
/// ones existed, a whole directory could drop out of scope silently, which is
/// the failure this constant exists for and not a different one.
const AT_LEAST: usize = 6;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root resolves")
}

struct Sample {
    file: PathBuf,
    line: usize,
    args: Vec<String>,
    body: String,
    /// `at=<pointer>`: compare against this sub-value of the parsed stdout
    /// rather than against the whole of it.
    pointer: Option<String>,
    /// `exit=<n>`: the exact code the invocation must exit with.
    expect_exit: i32,
}

/// Finds `<!-- sample: ... -->` markers and the fenced block that follows.
fn samples_in(path: &Path) -> Vec<Sample> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim().strip_prefix("<!-- sample:") else {
            continue;
        };
        let tokens: Vec<String> = rest
            .trim_end_matches("-->")
            .split_whitespace()
            .map(ToString::to_string)
            .collect();
        let mut pointer = None;
        let mut expect_exit = 0;
        let mut args = tokens;
        // Trailing only, and one of each. Stripping these wherever they appear
        // lets one be the *value* of a preceding flag — `--replay at=/x rec.json`
        // consumes `at=/x` as a pointer and runs an argv the marker does not
        // show, which is drift between a document and the command that produced
        // it: the failure F-27 and F-50 both exist to stop, arriving through the
        // instrument built to catch it (`security-reviewer`, 2026-09-19).
        while let Some(last) = args.last().cloned() {
            let seen_twice = |what: &str, seen: bool| {
                assert!(
                    !seen,
                    "{}:{}: {what} appears twice in one marker",
                    path.display(),
                    i + 1
                );
            };
            if let Some(p) = last.strip_prefix("at=") {
                seen_twice("at=", pointer.is_some());
                assert!(
                    p.starts_with('/'),
                    "{}:{}: at={p:?} is not a JSON Pointer; one starts with `/`",
                    path.display(),
                    i + 1
                );
                pointer = Some(p.to_string());
            } else if let Some(code) = last.strip_prefix("exit=") {
                seen_twice("exit=", expect_exit != 0);
                expect_exit = code.parse().unwrap_or_else(|_| {
                    panic!(
                        "{}:{}: exit={code:?} is not a number",
                        path.display(),
                        i + 1
                    )
                });
            } else {
                break;
            }
            args.pop();
        }
        check_argv(&args, path, i + 1);
        let open = i + 1;
        assert!(
            lines.get(open).is_some_and(|l| l.starts_with("```")),
            "{}:{}: a sample marker must be immediately followed by a fenced block",
            path.display(),
            i + 1
        );
        let close = (open + 1..lines.len())
            .find(|&n| lines[n].starts_with("```"))
            .unwrap_or_else(|| {
                panic!("{}:{}: unterminated fenced block", path.display(), open + 1)
            });
        // A leading `$ ...` prompt line is presentation, not output. Indexed
        // through `get`, because an empty fenced block puts the closing fence
        // at `open + 1` and the direct index panicked on it — reachable now
        // that the scan reads files nobody wrote with this scan in mind.
        let start = if lines.get(open + 1).is_some_and(|l| l.starts_with("$ ")) {
            open + 2
        } else {
            open + 1
        };
        found.push(Sample {
            file: path.to_path_buf(),
            line: open + 1,
            args,
            body: lines[start..close].join("\n"),
            pointer,
            expect_exit,
        });
    }
    found
}

/// Every `*.md` directly under `dir`, sorted so a run is deterministic.
fn markdown_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    found.sort();
    found
}

fn documented_files() -> Vec<PathBuf> {
    let root = root();
    let mut files = vec![root.join("README.md")];
    files.append(&mut markdown_in(&root.join("wiki")));
    // F-50. `docs/schemas/` is the contract an executor is told to build
    // against, and it was the one place a sample had never been compared with
    // the binary — which is where two wrong ones were found.
    files.append(&mut markdown_in(&root.join("docs").join("schemas")));
    files
}

#[test]
fn every_documented_sample_is_what_the_program_prints() {
    let root = root();
    let mut checked = 0;
    for file in documented_files() {
        for sample in samples_in(&file) {
            let out = Command::new(BIN)
                .args(&sample.args)
                .current_dir(&root)
                .env_remove("MACPORTSBREW_LANG")
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .output()
                .unwrap_or_else(|e| panic!("spawning the binary: {e}"));
            assert_eq!(
                out.status.code(),
                Some(sample.expect_exit),
                "{}:{}: `{}` exited {:?}, and the marker says {}",
                sample.file.display(),
                sample.line,
                sample.args.join(" "),
                out.status.code(),
                sample.expect_exit
            );
            let printed = String::from_utf8_lossy(&out.stdout);
            // Without `at=`, the block is the whole of stdout and this is the
            // byte comparison it always was. With it, the block is one
            // sub-value, and the pointer is resolved against the parsed
            // document rather than matched textually — a schema document shows
            // a `PlanCommand`, not a plan.
            let expected = match &sample.pointer {
                None => printed.trim_end().to_string(),
                Some(pointer) => {
                    let document: serde_json::Value = serde_json::from_str(&printed)
                        .unwrap_or_else(|e| {
                            panic!(
                                "{}:{}: `{}` did not print JSON, so at={pointer} cannot resolve: {e}",
                                sample.file.display(),
                                sample.line,
                                sample.args.join(" ")
                            )
                        });
                    let selected = document.pointer(pointer).unwrap_or_else(|| {
                        panic!(
                            "{}:{}: at={pointer} resolves to nothing in the output of `{}`",
                            sample.file.display(),
                            sample.line,
                            sample.args.join(" ")
                        )
                    });
                    serde_json::to_string_pretty(selected).expect("re-serialising a parsed value")
                }
            };
            assert_eq!(
                sample.body.trim_end(),
                expected.trim_end(),
                "{}:{}: the documented sample is not what `{}` prints.\n\
                 Copy the output above into that fenced block (F-27, F-50).",
                sample.file.display(),
                sample.line,
                sample.args.join(" ")
            );
            checked += 1;
        }
    }
    assert!(
        checked >= AT_LEAST,
        "found {checked} documented samples, expected at least {AT_LEAST} - \
         a scan that finds nothing passes every assertion above"
    );
}
