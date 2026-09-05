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

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_macportsbrew");

/// Every documented sample must be replayed, so a sample can never depend on
/// the machine the suite happens to run on (INV-15, INV-05).
const REQUIRED_FLAG: &str = "--replay";

/// A floor, not a count. A scan that finds nothing passes every assertion
/// below while proving nothing, which is the failure mode F-19 catalogued.
const AT_LEAST: usize = 2;

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
        let args: Vec<String> = rest
            .trim_end_matches("-->")
            .split_whitespace()
            .map(ToString::to_string)
            .collect();
        assert!(
            args.iter().any(|a| a == REQUIRED_FLAG),
            "{}:{}: a documented sample must be replayed, not taken from this machine",
            path.display(),
            i + 1
        );
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
        // A leading `$ ...` prompt line is presentation, not output.
        let start = if lines[open + 1].starts_with("$ ") {
            open + 2
        } else {
            open + 1
        };
        found.push(Sample {
            file: path.to_path_buf(),
            line: open + 1,
            args,
            body: lines[start..close].join("\n"),
        });
    }
    found
}

fn documented_files() -> Vec<PathBuf> {
    let root = root();
    let mut files = vec![root.join("README.md")];
    let mut wiki: Vec<PathBuf> = std::fs::read_dir(root.join("wiki"))
        .expect("the wiki directory exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    wiki.sort();
    files.append(&mut wiki);
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
            assert!(
                out.status.success(),
                "{}:{}: `{}` exited {:?}",
                sample.file.display(),
                sample.line,
                sample.args.join(" "),
                out.status.code()
            );
            let printed = String::from_utf8_lossy(&out.stdout);
            assert_eq!(
                sample.body.trim_end(),
                printed.trim_end(),
                "{}:{}: the documented sample is not what `{}` prints.\n\
                 Copy the output above into that fenced block (F-27).",
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
