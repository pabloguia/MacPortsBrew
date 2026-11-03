// SPDX-License-Identifier: Apache-2.0

//! The workspace's `unsafe` surface, held to ADR-0007's single site (work
//! order 16 part G, `docs/passes/16-plans/G.md` items 4 and 20).
//!
//! The compiler does most of it: the workspace lint is `deny`, and every crate
//! root but `adapters` carries an inner `forbid`, under which an `allow` does
//! not compile. What the compiler cannot say is *where* in `adapters` an
//! `allow` may sit, so this scan reads every `.rs` file under `crates/`, with
//! comments and string contents removed, and admits:
//!
//! - the keyword `unsafe` as a whole token at most once, and only in
//!   `crates/adapters/src/lock_probe/getlk.rs`;
//! - the lint name `unsafe_code` only as the roots' inner `forbid`, and inside
//!   `allow(..)`, `expect(..)` or `warn(..)` at most once, in that same file;
//! - the identifier `libc` only under `crates/adapters/src/lock_probe/`;
//! - no `include!`, and every `#[path = ..]` resolving inside `crates/`.
//!
//! The roots and the build graph are read from `cargo metadata`: no
//! `proc-macro` target and no build script anywhere in the workspace, and
//! every target under `crates/`, so the scan sees all the Rust the workspace
//! itself compiles. It does not see what a dependency's macro expands to
//! (rustc does not report `unsafe_code` there either), so the direct
//! dependencies and the graph's proc-macro crates are pinned below and a new
//! one is a deliberate edit. A planted sample of each
//! forbidden form is caught (the self-tests below), as strings, never as files
//! in the tree. Only the owner relaxes this (ADR-0007; work order 16 §3).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The one file allowed an `unsafe` block and its `allow`.
const SITE: &str = "crates/adapters/src/lock_probe/getlk.rs";
/// The one directory allowed to name `libc`.
const LIBC_DIR: &str = "crates/adapters/src/lock_probe/";
/// The crate whose root is not `forbid`, because the site is in it.
const SITE_CRATE: &str = "macportsbrew-adapters";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/cli sits two levels below the workspace root")
        .to_path_buf()
}

/// What `strip` leaves.
struct Stripped {
    /// The code, comments removed and every literal emptied.
    code: String,
    /// Each emptied string's content, in order.
    strings: Vec<String>,
    /// Whether every comment and literal was closed. An unclosed one is how a
    /// hand lexer falls out of step with rustc, so the scan refuses it rather
    /// than read the rest of the file as a comment or a string.
    closed: bool,
}

/// The code of a file with every comment (line, block — nested — and doc)
/// removed and every string, byte string, C string, raw string and character
/// literal emptied (`""`, `' '`); lifetimes stay as code. What is left is what
/// the compiler sees as tokens, so a name in a comment or a string is not a
/// use. Each emptied string's content is kept, in order, so an attribute's
/// string can be read back.
fn strip(text: &str) -> Stripped {
    let chars: Vec<char> = text.chars().collect();
    let mut strings = Vec::new();
    let mut closed = true;
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let after_ident = out.chars().next_back().is_some_and(is_ident);
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 1;
            i += 2;
            while i < chars.len() && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            closed &= depth == 0;
            out.push(' ');
        } else if !after_ident
            && (c == 'r' || (matches!(c, 'b' | 'c') && next == Some('r')))
            && raw_string_end(&chars, i).is_some()
        {
            let (end, terminated) = raw_string_end(&chars, i).unwrap_or((chars.len(), false));
            closed &= terminated;
            let open = (i..end).find(|&k| chars[k] == '"').unwrap_or(end);
            let close = (open + 1..end)
                .rev()
                .find(|&k| chars[k] == '"')
                .unwrap_or(end);
            strings.push(chars[(open + 1).min(close)..close].iter().collect());
            i = end;
            out.push_str("\"\"");
        } else if c == '"' || (matches!(c, 'b' | 'c') && next == Some('"') && !after_ident) {
            if c != '"' {
                i += 1;
            }
            i += 1;
            let open = i;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            closed &= i < chars.len();
            strings.push(
                chars[open.min(chars.len())..i.min(chars.len())]
                    .iter()
                    .collect(),
            );
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' || (c == 'b' && next == Some('\'') && !after_ident) {
            let start = if c == 'b' { i + 1 } else { i };
            if let Some(end) = char_literal_end(&chars, start) {
                out.push_str("' '");
                i = end;
            } else {
                // A lifetime or a label: code.
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    Stripped {
        code: out,
        strings,
        closed,
    }
}

/// Where a raw string starting at `at` (`r"`, `r#"`, `br#"`, `cr#"` …) ends
/// and whether it was closed, or `None` if what starts there is not one.
fn raw_string_end(chars: &[char], at: usize) -> Option<(usize, bool)> {
    let mut k = at + if matches!(chars[at], 'b' | 'c') { 2 } else { 1 };
    let mut hashes = 0;
    while chars.get(k) == Some(&'#') {
        hashes += 1;
        k += 1;
    }
    if chars.get(k) != Some(&'"') {
        return None;
    }
    k += 1;
    while k < chars.len() {
        if chars[k] == '"' && (0..hashes).all(|h| chars.get(k + 1 + h) == Some(&'#')) {
            return Some((k + 1 + hashes, true));
        }
        k += 1;
    }
    Some((chars.len(), false))
}

/// Where a character literal starting at the quote `at` ends, or `None` for a
/// lifetime or a label (`'a`, `'static`, `'outer:`).
fn char_literal_end(chars: &[char], at: usize) -> Option<usize> {
    match chars.get(at + 1) {
        Some('\\') => {
            // `\x41` is four characters, `\u{..}` runs to its brace (any
            // length: `'\u{4_____1}'` is valid), any other escape is two; the
            // escaped character itself is skipped, so `'\''` ends at its
            // second quote.
            let k = match chars.get(at + 2) {
                Some('x') => at + 5,
                Some('u') => (at + 3..chars.len()).find(|&k| chars[k] == '}')? + 1,
                _ => at + 3,
            };
            (chars.get(k) == Some(&'\'')).then_some(k + 1)
        }
        Some(_) if chars.get(at + 2) == Some(&'\'') => Some(at + 3),
        _ => None,
    }
}

/// Every identifier-shaped token, with its byte offset.
fn tokens(code: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, c) in code.char_indices() {
        let part = c.is_alphanumeric() || c == '_';
        match (part, start) {
            (true, None) => start = Some(at),
            (false, Some(s)) => {
                found.push((s, &code[s..at]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        found.push((s, &code[s..]));
    }
    found
}

/// The identifier just before `(` that opens the group around `at`, and
/// whether the group is an inner attribute (`#![`).
fn lint_level_around(code: &str, at: usize) -> (String, bool) {
    let before = code[..at].trim_end();
    let Some(before) = before.strip_suffix('(') else {
        return (String::new(), false);
    };
    let before = before.trim_end();
    let level: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let rest = before[..before.len() - level.len()].trim_end();
    (level, rest.ends_with("#!["))
}

/// The violations in one file, `relative` being its path from the workspace
/// root, and `roots` the crate roots that must open with the `forbid`.
fn file_violations(relative: &str, text: &str, roots: &[String]) -> Vec<String> {
    let stripped = strip(text);
    let code = stripped.code;
    let toks = tokens(&code);
    let mut v = Vec::new();
    if !stripped.closed {
        v.push(format!(
            "{relative}: an unclosed comment or literal; the scan cannot read past it"
        ));
    }
    let is_site = relative == SITE;
    let unsafe_count = toks.iter().filter(|(_, t)| *t == "unsafe").count();
    if unsafe_count > 0 && !is_site {
        v.push(format!("{relative}: the keyword `unsafe` outside {SITE}"));
    }
    if unsafe_count > 1 {
        v.push(format!(
            "{relative}: `unsafe` {unsafe_count} times; one is admitted"
        ));
    }
    let mut allows = 0;
    for &(at, t) in &toks {
        match t {
            "unsafe_code" => {
                let (level, inner) = lint_level_around(&code, at);
                match level.as_str() {
                    "forbid" if inner && roots.iter().any(|r| r == relative) => {}
                    "allow" | "expect" | "warn" if is_site && !inner => {
                        allows += 1;
                        if !on_a_fn_item(&code, at) {
                            v.push(format!(
                                "{relative}: the admitted `allow` is not on a `fn` item"
                            ));
                        }
                    }
                    _ => v.push(format!(
                        "{relative}: the lint `unsafe_code` under `{level}` here is not admitted"
                    )),
                }
            }
            "libc" if !relative.starts_with(LIBC_DIR) => {
                v.push(format!("{relative}: names `libc` outside {LIBC_DIR}"));
            }
            // Bare, not only as `include!`: `use std::include as inc;` would
            // otherwise reach the macro under another name.
            "include" => {
                v.push(format!("{relative}: `include` hides code from this scan"));
            }
            // A macro at the site would carry its one `unsafe` to every caller.
            "macro_rules" | "macro" if is_site => {
                v.push(format!("{relative}: a macro at the site multiplies it"));
            }
            _ => {}
        }
    }
    if allows > 1 {
        v.push(format!(
            "{relative}: {allows} `allow`s of `unsafe_code`; one is admitted"
        ));
    }
    if roots.iter().any(|r| r == relative) {
        let first = code
            .find("#!")
            .map(|at| code[at..].split(']').next().unwrap_or(""));
        let first: Option<String> =
            first.map(|a| a.chars().filter(|c| !c.is_whitespace()).collect());
        if first.as_deref() != Some("#![forbid(unsafe_code)") {
            v.push(format!(
                "{relative}: a crate root whose first inner attribute is not `forbid(unsafe_code)`"
            ));
        }
    }
    v
}

/// Whether the attribute holding the lint name at `at` sits on a `fn` item:
/// after its closing `]`, any further attributes, then an optional `pub` or
/// `pub(..)`, then `fn`.
fn on_a_fn_item(code: &str, at: usize) -> bool {
    let Some(close) = code[at..].find(']') else {
        return false;
    };
    let mut rest = code[at + close + 1..].trim_start();
    while let Some(attribute) = rest.strip_prefix("#[") {
        let Some(end) = attribute.find(']') else {
            return false;
        };
        rest = attribute[end + 1..].trim_start();
    }
    if let Some(after) = rest.strip_prefix("pub") {
        rest = after.trim_start();
        if let Some(group) = rest.strip_prefix('(') {
            let Some(end) = group.find(')') else {
                return false;
            };
            rest = group[end + 1..].trim_start();
        }
    }
    rest.strip_prefix("fn")
        .is_some_and(|after| after.starts_with(|c: char| c.is_whitespace()))
}

/// Every `path = "…"` attribute target, whether written `#[path = ..]` or
/// inside `#[cfg_attr(.., path = ..)]`, read from the stripped tokens so a
/// comment or a string is not one. A `path =` in attribute position whose
/// value is not a plain string literal comes back as `None`: unreadable, and
/// so refused.
fn path_attributes(text: &str) -> Vec<Option<String>> {
    let Stripped { code, strings, .. } = strip(text);
    let mut found = Vec::new();
    for (at, t) in tokens(&code) {
        if t != "path" {
            continue;
        }
        let before = code[..at].trim_end();
        let statement = before.rfind([';', '{', '}']).map_or(0, |k| k + 1);
        let in_attribute = before.ends_with("#[") || before[statement..].contains("cfg_attr");
        let Some(rest) = code[at + t.len()..].trim_start().strip_prefix('=') else {
            continue;
        };
        if !in_attribute {
            continue;
        }
        let rest = rest.trim_start();
        let value_at = code.len() - rest.len();
        found.push(
            rest.starts_with("\"\"")
                .then(|| {
                    strings
                        .get(code[..value_at].matches("\"\"").count())
                        .cloned()
                })
                .flatten(),
        );
    }
    found
}

/// Every `.rs` under `dir`, skipping only a crate's own build directory
/// (`crates/<crate>/target`), never a module directory named `target`.
fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let build_dir = name == "target"
            && path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                == Some("crates".as_ref());
        if path.is_dir() && !build_dir {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// `cargo metadata` for the workspace members, offline.
fn metadata() -> serde_json::Value {
    metadata_with(&["--format-version", "1", "--no-deps", "--offline"])
}

/// `cargo metadata` with these arguments.
fn metadata_with(args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .arg("metadata")
        .args(args)
        .current_dir(workspace_root())
        .output()
        .expect("cargo metadata runs");
    assert!(output.status.success(), "cargo metadata succeeds");
    serde_json::from_slice(&output.stdout).expect("cargo metadata is JSON")
}

/// The crate roots that must carry the `forbid`, relative to the workspace,
/// and the violations the build graph itself shows.
fn roots_and_graph_violations(meta: &serde_json::Value) -> (Vec<String>, Vec<String>) {
    let root = workspace_root();
    let mut roots = Vec::new();
    let mut v = Vec::new();
    let packages = meta["packages"].as_array().expect("packages");
    assert!(packages.len() >= 5, "every member was seen");
    for package in packages {
        let name = package["name"].as_str().unwrap_or("");
        for target in package["targets"].as_array().expect("targets") {
            let kinds: Vec<&str> = target["kind"]
                .as_array()
                .expect("kind")
                .iter()
                .filter_map(|k| k.as_str())
                .collect();
            let src = Path::new(target["src_path"].as_str().unwrap_or(""));
            let relative = src
                .strip_prefix(&root)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !relative.starts_with("crates/") {
                v.push(format!(
                    "{name}: a target outside crates/ ({}) the scan does not read",
                    src.display()
                ));
            }
            if kinds.contains(&"proc-macro") {
                v.push(format!(
                    "{name}: a proc-macro target hides code from the scan"
                ));
            }
            if kinds.contains(&"custom-build") {
                v.push(format!(
                    "{name}: a build script ({relative}) can generate unscanned code"
                ));
            }
            // Every compiled crate root: `lib`, `bin`, `rlib`, `cdylib`,
            // `staticlib`, `dylib`. Tests, benches and examples are separate
            // crates under the workspace `deny` and the scan, not roots.
            let auxiliary = kinds
                .iter()
                .all(|k| matches!(*k, "test" | "bench" | "example" | "custom-build"));
            if name != SITE_CRATE && !auxiliary && !roots.contains(&relative) {
                roots.push(relative);
            }
        }
    }
    roots.sort();
    (roots, v)
}

/// Every member's direct external dependency, as `member dependency`, any
/// kind. A macro from a dependency expands `unsafe` that neither the lint
/// (rustc does not report it inside an external macro) nor this scan sees, so
/// a new direct dependency is a deliberate edit here, on top of ADR-0004's
/// gate.
const DIRECT_DEPENDENCIES: &[&str] = &[
    "macportsbrew clap",
    "macportsbrew ratatui",
    "macportsbrew serde",
    "macportsbrew serde_json",
    "macportsbrew toml",
    "macportsbrew unicode-width",
    "macportsbrew-adapters serde",
    "macportsbrew-adapters serde_json",
    "macportsbrew-adapters toml",
    "macportsbrew-application serde",
    "macportsbrew-application serde_json",
    "macportsbrew-application sha2",
    "macportsbrew-application toml",
    "macportsbrew-domain serde",
    "macportsbrew-domain serde_json",
    "macportsbrew-policy serde",
    "macportsbrew-policy serde_json",
    "macportsbrew-policy toml",
];

/// The proc-macro crates in the whole graph (`cargo metadata` without
/// `--no-deps`), pinned for the same reason.
const PROC_MACROS: &[&str] = &[
    "clap_derive",
    "darling_macro",
    "indoc",
    "instability",
    "rustversion",
    "serde_derive",
    "strum_macros",
    "thiserror-impl",
];

/// Drift from the two pins above.
fn dependency_violations(meta: &serde_json::Value) -> Vec<String> {
    dependency_drift(
        meta,
        &metadata_with(&["--format-version", "1", "--offline"]),
    )
}

/// The comparison itself, over `meta` (no dependencies) and `full` (the
/// graph), so a drift can be planted.
fn dependency_drift(meta: &serde_json::Value, full: &serde_json::Value) -> Vec<String> {
    let packages = meta["packages"].as_array().expect("packages");
    let members: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut direct: Vec<String> = packages
        .iter()
        .flat_map(|p| {
            let name = p["name"].as_str().unwrap_or("").to_string();
            p["dependencies"]
                .as_array()
                .expect("dependencies")
                .iter()
                .filter_map(|d| d["name"].as_str())
                .filter(|d| !members.contains(d))
                .map(move |d| format!("{name} {d}"))
                .collect::<Vec<_>>()
        })
        .collect();
    direct.sort();
    direct.dedup();
    let mut macros: Vec<String> = full["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter(|p| {
            p["targets"].as_array().is_some_and(|ts| {
                ts.iter().any(|t| {
                    t["kind"]
                        .as_array()
                        .is_some_and(|k| k.iter().any(|k| k == "proc-macro"))
                })
            })
        })
        .filter_map(|p| p["name"].as_str().map(str::to_string))
        .collect();
    macros.sort();
    macros.dedup();
    let mut v = Vec::new();
    if direct != DIRECT_DEPENDENCIES {
        v.push(format!("the direct dependencies changed: {direct:?}"));
    }
    if macros != PROC_MACROS {
        v.push(format!("the proc-macro crates changed: {macros:?}"));
    }
    v
}

/// Every value `unsafe_code` is given under `[workspace.lints.rust]`.
fn workspace_unsafe_level(manifest: &str) -> Vec<&str> {
    let section = manifest
        .split("[workspace.lints.rust]")
        .nth(1)
        .and_then(|s| s.split("\n[").next())
        .unwrap_or("");
    section
        .lines()
        .filter_map(|l| l.trim().strip_prefix("unsafe_code"))
        .filter_map(|l| l.trim().strip_prefix('='))
        .map(str::trim)
        .collect()
}

/// A workspace `unsafe_code` level other than exactly `"deny"`: `forbid`
/// would refuse the site's one `allow`, anything lower admits more.
fn workspace_level_violation(manifest: &str) -> Option<String> {
    let level = workspace_unsafe_level(manifest);
    (level != ["\"deny\""])
        .then(|| format!("the workspace's `unsafe_code` is {level:?}, not exactly \"deny\""))
}

/// What the workspace's lint configuration shows.
fn configuration_violations(root: &Path, members: &[PathBuf]) -> Vec<String> {
    let mut v = Vec::new();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
    v.extend(workspace_level_violation(&manifest));
    for member in members {
        let text = fs::read_to_string(member).unwrap_or_default();
        let opted_in = text.split("\n[lints]").nth(1).is_some_and(|s| {
            s.lines()
                .take(3)
                .any(|l| l.replace(' ', "") == "workspace=true")
        });
        if !opted_in {
            v.push(format!(
                "{}: not `[lints] workspace = true`",
                member.display()
            ));
        }
    }
    // Any flag or compiler substitution, not only a lowering one: a wrapper
    // can pass any flag, so the keys are refused outright.
    for config in ["config", "config.toml"] {
        let text = fs::read_to_string(root.join(".cargo").join(config)).unwrap_or_default();
        if substitutes_the_compiler(&text) || capping(&text) {
            v.push(format!(".cargo/{config}: flags or a compiler wrapper"));
        }
    }
    // Cargo configuration outside the repository (a parent directory,
    // `$CARGO_HOME`) is not read: it is the developer's machine, not the tree,
    // and CI has none (recorded in SE-110).
    for (var, value) in std::env::vars() {
        let flags = var == "RUSTFLAGS"
            || var == "CARGO_ENCODED_RUSTFLAGS"
            || var == "CARGO_BUILD_RUSTFLAGS"
            || (var.starts_with("CARGO_TARGET_") && var.ends_with("_RUSTFLAGS"));
        if flags && capping(&value) {
            v.push(format!("{var}: flags that lower lints"));
        }
        if matches!(
            var.as_str(),
            "RUSTC" | "RUSTC_WRAPPER" | "RUSTC_WORKSPACE_WRAPPER"
        ) && !value.is_empty()
            && !coverage_wrapper(&var, &value, &|k| std::env::var(k).ok())
        {
            v.push(format!("{var}: a substituted compiler can pass any flag"));
        }
    }
    v
}

/// The one wrapper admitted: CI's coverage job runs the tests under
/// `cargo-llvm-cov`, which sets itself as `RUSTC_WRAPPER` and passes rustc
/// the flags it publishes in `__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS`. It
/// is admitted only under that name, only as `RUSTC_WRAPPER`, and only while
/// those flags lower no lint.
fn coverage_wrapper(var: &str, value: &str, env: &dyn Fn(&str) -> Option<String>) -> bool {
    var == "RUSTC_WRAPPER"
        && Path::new(value)
            .file_name()
            .is_some_and(|n| n == "cargo-llvm-cov")
        && env("__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS").is_some_and(|f| !capping(&f))
}

/// Whether Cargo configuration sets flags or substitutes the compiler.
fn substitutes_the_compiler(config: &str) -> bool {
    config.lines().any(|line| {
        let key = line.split('=').next().unwrap_or("").trim();
        key.ends_with("rustflags")
            || matches!(key, "rustc" | "rustc-wrapper" | "rustc-workspace-wrapper")
    })
}

/// Whether flags lower a lint level: `-A`/`--allow`, `-W`/`--warn`,
/// `--force-warn` (which overrides even `forbid`), `--cap-lints`.
fn capping(flags: &str) -> bool {
    ["--cap-lints", "--allow", "--warn", "--force-warn"]
        .iter()
        .any(|f| flags.contains(f))
        || flags
            .split(|c: char| {
                c.is_whitespace() || matches!(c, '\u{1f}' | '"' | '\'' | ',' | '[' | ']' | '=')
            })
            .any(|f| f.starts_with("-A") || f.starts_with("-W"))
}

#[test]
fn the_unsafe_surface_is_adr_0007s_single_site_and_nothing_else() {
    let root = workspace_root();
    let meta = metadata();
    let (roots, mut v) = roots_and_graph_violations(&meta);
    let members: Vec<PathBuf> = meta["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|p| p["manifest_path"].as_str().map(PathBuf::from))
        .collect();
    v.extend(configuration_violations(&root, &members));
    v.extend(dependency_violations(&meta));
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    files.sort();
    assert!(files.len() > 50, "the crates were read");
    let mut seen_roots: BTreeMap<String, bool> = roots.iter().map(|r| (r.clone(), false)).collect();
    for file in &files {
        let relative = file
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .into_owned();
        let text = fs::read_to_string(file).expect("readable");
        if let Some(seen) = seen_roots.get_mut(&relative) {
            *seen = true;
        }
        v.extend(file_violations(&relative, &text, &roots));
        if file.file_name().is_some_and(|n| n == "build.rs") {
            v.push(format!("{relative}: a build script, compiled or not"));
        }
        for target in path_attributes(&text) {
            let Some(target) = target else {
                v.push(format!(
                    "{relative}: a `path` attribute the scan cannot read"
                ));
                continue;
            };
            if Path::new(&target).extension().is_none_or(|e| e != "rs") {
                v.push(format!(
                    "{relative}: `#[path = \"{target}\"]` names a file the scan does not read"
                ));
            }
            let resolved = file.parent().expect("a parent").join(&target);
            let inside = resolved
                .canonicalize()
                .is_ok_and(|p| p.starts_with(root.join("crates").canonicalize().expect("crates")));
            if !inside {
                v.push(format!(
                    "{relative}: `#[path = \"{target}\"]` leaves crates/"
                ));
            }
        }
    }
    for (r, seen) in seen_roots {
        if !seen {
            v.push(format!("{r}: a crate root the scan did not read"));
        }
    }
    assert!(v.is_empty(), "the unsafe surface grew: {v:#?}");
}

// --- The scan's own proof: each forbidden form, planted as a string, is caught.

const ROOT: &str = "crates/domain/src/lib.rs";
const OTHER: &str = "crates/adapters/src/registry.rs";

fn planted(relative: &str, text: &str) -> Vec<String> {
    file_violations(relative, text, &[ROOT.to_string()])
}

#[test]
fn a_planted_unsafe_block_or_fn_outside_the_site_is_caught() {
    let block = format!("fn f() {{ {} {{ }} }}", "unsafe");
    assert!(!planted(OTHER, &block).is_empty());
    let attribute = format!("#[{}(no_mangle)] fn f() {{}}", "unsafe");
    assert!(!planted(OTHER, &attribute).is_empty());
    let external = format!("{} extern \"C\" {{ }}", "unsafe");
    assert!(!planted(OTHER, &external).is_empty());
}

#[test]
fn a_second_unsafe_at_the_site_is_caught() {
    let one = format!(
        "#[allow({c})]\nfn held() {{ {u} {{ }} }}",
        c = "unsafe_code",
        u = "unsafe"
    );
    assert!(planted(SITE, &one).is_empty(), "{:?}", planted(SITE, &one));
    let two = format!("{one}\nfn more() {{ {} {{ }} }}", "unsafe");
    assert!(!planted(SITE, &two).is_empty());
}

#[test]
fn an_allow_of_the_lint_anywhere_but_on_the_site_is_caught() {
    for level in ["allow", "expect", "warn"] {
        let outer = format!("#[{level}({})] fn f() {{}}", "unsafe_code");
        assert!(!planted(OTHER, &outer).is_empty(), "{level}");
        let inner = format!("#![{level}({})]", "unsafe_code");
        assert!(
            !planted(SITE, &inner).is_empty(),
            "a module-wide {level} at the site"
        );
    }
    let conditional = format!("#[cfg_attr(test, allow({}))] fn f() {{}}", "unsafe_code");
    assert!(!planted(OTHER, &conditional).is_empty());
    let two = format!(
        "#[allow({c})] fn a() {{}}\n#[allow({c})] fn b() {{}}",
        c = "unsafe_code"
    );
    assert!(!planted(SITE, &two).is_empty(), "two allows at the site");
}

#[test]
fn the_forbid_is_admitted_only_as_a_roots_first_inner_attribute() {
    let forbid = format!("//! Docs.\n#![{}({})]\npub mod a;", "forbid", "unsafe_code");
    assert!(
        planted(ROOT, &forbid).is_empty(),
        "{:?}",
        planted(ROOT, &forbid)
    );
    assert!(
        !planted(OTHER, &forbid).is_empty(),
        "a forbid outside a root"
    );
    assert!(
        !planted(ROOT, "//! Docs.\npub mod a;").is_empty(),
        "a root without it"
    );
    let late = format!(
        "#![allow(missing_docs)]\n#![{}({})]",
        "forbid", "unsafe_code"
    );
    assert!(
        !planted(ROOT, &late).is_empty(),
        "not the first inner attribute"
    );
}

#[test]
fn libc_and_include_are_caught_outside_their_place() {
    assert!(!planted(OTHER, &format!("use {} as c;", "libc")).is_empty());
    assert!(!planted(OTHER, &format!("extern crate {};", "libc")).is_empty());
    assert!(planted(
        "crates/adapters/src/lock_probe/mod.rs",
        &format!("use {};", "libc")
    )
    .is_empty());
    assert!(!planted(
        OTHER,
        &format!("{}!(concat!(env!(\"OUT_DIR\"), \"/x.rs\"));", "include")
    )
    .is_empty());
    assert!(planted(OTHER, "const A: &str = include_str!(\"a\");").is_empty());
}

#[test]
fn comments_strings_and_longer_names_are_not_uses() {
    let hidden = [
        format!("// {}", "unsafe"),
        format!(
            "/* outer /* {} */ still a comment {} */",
            "unsafe", "unsafe"
        ),
        format!("/// {} {{ }}", "unsafe"),
        format!("let s = \"{}\";", "unsafe"),
        format!("let s = r#\"{} \"quoted\" \"#;", "unsafe"),
        format!("let s = br##\"{}\"##;", "unsafe"),
        format!("let s = b\"{}\";", "unsafe"),
        format!("let c = '\"'; let s = \"{}\";", "unsafe"),
        format!("fn f<'a>(x: &'a str) -> &'a str {{ x }} // {}", "unsafe"),
        format!("#![deny({}_op_in_{}_fn)]", "unsafe", "unsafe"),
        format!("let not_{} = 1;", "unsafe"),
    ];
    for text in &hidden {
        let found = planted(OTHER, text);
        assert!(found.is_empty(), "{text:?} was read as a use: {found:?}");
    }
    let after = format!("/* a */ fn f() {{ {} {{ }} }}", "unsafe");
    assert!(
        !planted(OTHER, &after).is_empty(),
        "code after a comment is still read"
    );
    let after_string = format!("let s = \"a\\\"b\"; fn f() {{ {} {{ }} }}", "unsafe");
    assert!(
        !planted(OTHER, &after_string).is_empty(),
        "code after an escaped quote"
    );
}

#[test]
fn lowered_lint_flags_are_recognised() {
    assert!(capping("-A unsafe_code"));
    assert!(capping("-Aunsafe-code"));
    assert!(capping("--cap-lints warn"));
    assert!(capping("-C\u{1f}--cap-lints=allow"));
    assert!(capping("--allow unsafe_code"));
    assert!(capping("-W unsafe_code"));
    assert!(capping("--warn=unsafe_code"));
    assert!(capping("--force-warn unsafe_code"));
    assert!(capping("rustflags = ['-Aunsafe_code']"));
    assert!(capping("rustflags = [\"-Wunsafe_code\"]"));
    assert!(!capping("-C target-cpu=native"));
    assert!(!capping("-D warnings"));
    assert!(!capping(""));
}

#[test]
fn a_path_attribute_is_read_in_every_form() {
    let one = |t: &str| path_attributes(t);
    assert_eq!(
        one("#[path = \"a_tests.rs\"]\nmod t;"),
        [Some("a_tests.rs".into())]
    );
    assert_eq!(
        one("  #[path=\"../x.rs\"] mod t;"),
        [Some("../x.rs".into())]
    );
    assert_eq!(
        one("#[cfg(test)] #[path = r#\"../y.rs\"#] mod t;"),
        [Some("../y.rs".into())]
    );
    assert_eq!(
        one("#[cfg_attr(test,\n  path = \"../../z.rs\")]\nmod t;"),
        [Some("../../z.rs".into())]
    );
    assert_eq!(one("#[path = concat!(\"a\", \"b\")] mod t;"), [None]);
    assert!(one("let path = \"../x\"; // #[path = \"q\"]").is_empty());
    assert!(one("let s = \"#[path = \\\"q\\\"]\";").is_empty());
}

#[test]
fn include_under_any_name_is_caught() {
    let renamed = format!("use std::{} as inc;\ninc!(\"../../x.rs\");", "include");
    assert!(!planted(OTHER, &renamed).is_empty());
}

#[test]
fn the_lexer_stays_in_step_with_rustc() {
    // Security review G-1, B1: a raw C string holding a quote.
    let c_raw = format!(
        "const C: &core::ffi::CStr = cr#\"\"\"#;\n#[allow({c})] pub fn f() {{ {u} {{ }} }}\n// \"",
        c = "unsafe_code",
        u = "unsafe"
    );
    assert!(!planted(OTHER, &c_raw).is_empty());
    let c_plain = format!(
        "const C: &core::ffi::CStr = c\"\\\"\"; fn f() {{ {} {{ }} }}",
        "unsafe"
    );
    assert!(!planted(OTHER, &c_plain).is_empty());
    let fake_forbid = format!(
        "const C: &core::ffi::CStr = cr##\"x\"#![{}({})]\"##;\npub mod a;",
        "forbid", "unsafe_code"
    );
    assert!(
        !planted(ROOT, &fake_forbid).is_empty(),
        "a forbid inside a string"
    );
    // B2: a long `\u{..}` character literal.
    let long_char = format!(
        "pub const C: [char; 2] = ['\\u{{4_________1}}','\"'];\nfn f() {{ {} {{ }} }}\n// \"",
        "unsafe"
    );
    assert!(!planted(OTHER, &long_char).is_empty());
    assert!(planted(
        OTHER,
        "const C: [char; 3] = ['\\x41', '\\'', '\\u{1F600}'];"
    )
    .is_empty());
    // An unclosed comment or literal is refused, not read as the rest of the file.
    for open in [
        "/* never closed",
        "let s = \"never closed",
        "let s = r#\"never closed",
    ] {
        assert!(!planted(OTHER, open).is_empty(), "{open:?}");
    }
}

#[test]
fn the_site_admits_one_allow_on_a_fn_and_no_macro() {
    let on_fn = format!(
        "#[allow({})]\n#[inline]\npub(crate) fn held() {{}}",
        "unsafe_code"
    );
    assert!(
        planted(SITE, &on_fn).is_empty(),
        "{:?}",
        planted(SITE, &on_fn)
    );
    let on_let = format!("fn g() {{ #[allow({})] let r = 1; }}", "unsafe_code");
    assert!(
        !planted(SITE, &on_let).is_empty(),
        "an allow on a statement"
    );
    let exported = format!(
        "#[macro_export]\nmacro_rules! raw {{ ($e:expr) => {{{{ #[allow({c})] let r = {u} {{ $e }}; r }}}} }}",
        c = "unsafe_code",
        u = "unsafe"
    );
    assert!(!planted(SITE, &exported).is_empty(), "a macro at the site");
    // Mutation proof, mutant 6: a macro otherwise admitted (one `unsafe`, one
    // `allow`, on a `fn`) is refused by the macro rule alone.
    let admitted_but_a_macro = format!(
        "macro_rules! raw {{ () => {{ #[allow({c})] pub fn f() {{ {u} {{}} }} }} }}",
        c = "unsafe_code",
        u = "unsafe"
    );
    assert!(!planted(SITE, &admitted_but_a_macro).is_empty());
}

#[test]
fn cargo_configuration_that_substitutes_the_compiler_is_recognised() {
    assert!(substitutes_the_compiler(
        "[build]\nrustflags = [\"-Dwarnings\"]"
    ));
    assert!(substitutes_the_compiler(
        "[target.x86_64-apple-darwin]\nrustflags = []"
    ));
    assert!(substitutes_the_compiler(
        "[build]\nrustc-wrapper = \"w.sh\""
    ));
    assert!(substitutes_the_compiler("rustc = \"/x/rustc\""));
    assert!(!substitutes_the_compiler("[net]\noffline = true"));
}

#[test]
fn only_a_crates_own_build_directory_is_skipped() {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("unsafe_scan_walk");
    let _ = fs::remove_dir_all(&base);
    let module = base.join("crates/a/src/target");
    let build = base.join("crates/a/target");
    fs::create_dir_all(&module).expect("a module directory");
    fs::create_dir_all(&build).expect("a build directory");
    fs::write(module.join("m.rs"), "").expect("written");
    fs::write(build.join("b.rs"), "").expect("written");
    let mut found = Vec::new();
    rust_files(&base.join("crates"), &mut found);
    fs::remove_dir_all(&base).expect("removed");
    assert_eq!(found, [module.join("m.rs")]);
}

#[test]
fn a_drift_in_the_pins_or_the_level_is_caught() {
    let meta = |extra: &str| {
        let mut packages: Vec<serde_json::Value> = Vec::new();
        let mut by_member: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for pin in DIRECT_DEPENDENCIES
            .iter()
            .copied()
            .chain((!extra.is_empty()).then_some(extra))
        {
            let (member, dep) = pin.split_once(' ').expect("member dependency");
            by_member.entry(member).or_default().push(dep);
        }
        for (member, deps) in by_member {
            let deps: Vec<serde_json::Value> = deps
                .iter()
                .map(|d| serde_json::json!({ "name": d }))
                .collect();
            packages.push(serde_json::json!({ "name": member, "dependencies": deps }));
        }
        serde_json::json!({ "packages": packages })
    };
    let full = |extra: Option<&str>| {
        let packages: Vec<serde_json::Value> = PROC_MACROS
            .iter()
            .copied()
            .chain(extra)
            .map(|n| serde_json::json!({ "name": n, "targets": [{ "kind": ["proc-macro"] }] }))
            .collect();
        serde_json::json!({ "packages": packages })
    };
    assert!(dependency_drift(&meta(""), &full(None)).is_empty());
    assert!(!dependency_drift(&meta("macportsbrew-domain foo"), &full(None)).is_empty());
    assert!(!dependency_drift(&meta(""), &full(Some("paste"))).is_empty());
    let lints = |level: &str| format!("[workspace.lints.rust]\nunsafe_code = \"{level}\"\n");
    assert!(workspace_level_violation(&lints("deny")).is_none());
    for level in ["forbid", "warn", "allow"] {
        assert!(
            workspace_level_violation(&lints(level)).is_some(),
            "{level}"
        );
    }
    assert!(
        workspace_level_violation("[workspace]\n").is_some(),
        "no level at all"
    );
}

#[test]
fn only_the_coverage_wrapper_with_coverage_flags_is_admitted() {
    let flags = |f: &'static str| {
        move |k: &str| (k == "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS").then(|| f.to_string())
    };
    let coverage = flags("-Cinstrument-coverage\u{1f}--cfg=coverage");
    assert!(coverage_wrapper(
        "RUSTC_WRAPPER",
        "/x/bin/cargo-llvm-cov",
        &coverage
    ));
    assert!(!coverage_wrapper(
        "RUSTC_WRAPPER",
        "/x/bin/sccache",
        &coverage
    ));
    assert!(!coverage_wrapper(
        "RUSTC_WORKSPACE_WRAPPER",
        "/x/bin/cargo-llvm-cov",
        &coverage
    ));
    assert!(!coverage_wrapper(
        "RUSTC_WRAPPER",
        "/x/bin/cargo-llvm-cov",
        &|_: &str| None
    ));
    let lowering = flags("-Cinstrument-coverage\u{1f}--cap-lints=allow");
    assert!(!coverage_wrapper(
        "RUSTC_WRAPPER",
        "/x/bin/cargo-llvm-cov",
        &lowering
    ));
}
