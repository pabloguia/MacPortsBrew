// SPDX-License-Identifier: Apache-2.0

//! The command-line crate names no provider (AGENTS.md: provider names appear
//! in `crates/adapters` and nowhere above; `registry.rs` is the one place that
//! knows which providers exist).
//!
//! `scripts/check-layering.sh` scans `domain` and `policy` for identifiers; a
//! replay helper in this crate once parsed `brew --prefix` and `port dir`
//! itself, which a third provider added by the recipe would silently miss
//! (spec-guardian, pre-release review, M1). This holds the presentation crate
//! to the rule: no provider id or program name as a string literal in non-test
//! code, and no path into a provider's adapter module (`adapters::<provider>`,
//! also inside a `{..}` group; a glob import is refused outright).
//!
//! What it does not see, stated so the check claims no more than it does: an
//! alias (`use macportsbrew_adapters as a;`), a `#[cfg(test)]` spelled any other
//! way than exactly that (it is then scanned, which only risks a false alarm),
//! a marker named inside a comment, and a literal after a `//` on the same line.

use std::path::{Path, PathBuf};

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("source directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Index just past the `}` that closes the block opening at `open` (a `{`),
/// reading the source as Rust: braces inside strings, raw strings, character
/// literals and comments do not count.
fn end_of_block(src: &[char], open: usize) -> usize {
    let at = |i: usize| src.get(i).copied().unwrap_or('\0');
    let mut depth = 0usize;
    let mut i = open;
    while i < src.len() {
        let c = src[i];
        if c == '/' && at(i + 1) == '/' {
            while i < src.len() && src[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && at(i + 1) == '*' {
            let mut nesting = 1;
            i += 2;
            while i < src.len() && nesting > 0 {
                if src[i] == '/' && at(i + 1) == '*' {
                    nesting += 1;
                    i += 2;
                } else if src[i] == '*' && at(i + 1) == '/' {
                    nesting -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // A raw string: `r"..."`, `r#"..."#`, with any number of hashes, and
        // not the tail of an identifier such as `for"`.
        if c == 'r' && !(i > 0 && (src[i - 1].is_alphanumeric() || src[i - 1] == '_')) {
            let mut hashes = 0;
            while at(i + 1 + hashes) == '#' {
                hashes += 1;
            }
            if at(i + 1 + hashes) == '"' {
                i += 2 + hashes;
                while i < src.len() {
                    if src[i] == '"' && (0..hashes).all(|h| at(i + 1 + h) == '#') {
                        i += 1 + hashes;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
        }
        if c == '"' {
            i += 1;
            while i < src.len() && src[i] != '"' {
                i += if src[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            continue;
        }
        // A character literal, as opposed to a lifetime: `'x'` or `'\x'`.
        if c == '\'' {
            if at(i + 1) == '\\' {
                i += 2;
                while i < src.len() && src[i] != '\'' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            if at(i + 2) == '\'' {
                i += 3;
                continue;
            }
        }
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    src.len()
}

/// `text` with every `#[cfg(test)] mod ... { ... }` removed, and nothing else.
///
/// An earlier version cut the file at the first `#[cfg(test)]`, so one test-only
/// `use` or helper near the top hid every line below it - production code
/// included. Only the test *module* is test code here; a `#[cfg(test)]` on any
/// other item is scanned like the rest, which errs toward a false alarm.
fn without_test_modules(text: &str) -> String {
    const ATTRIBUTE: &str = "#[cfg(test)]";
    let src: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < src.len() {
        let rest: String = src[i..src.len().min(i + ATTRIBUTE.len())].iter().collect();
        if rest == ATTRIBUTE {
            // The module, if that is what the attribute is on: skip further
            // attributes, then `mod name`, then either `;` or the block.
            let mut j = i + ATTRIBUTE.len();
            loop {
                while j < src.len() && src[j].is_whitespace() {
                    j += 1;
                }
                if src.get(j) == Some(&'#') {
                    while j < src.len() && src[j] != ']' {
                        j += 1;
                    }
                    j += 1;
                } else {
                    break;
                }
            }
            // Optional visibility (`pub`, `pub(crate)`), then `mod`: anything
            // else under the attribute is scanned like production code.
            if src[j..].starts_with(&['p', 'u', 'b']) {
                j += 3;
                if src.get(j) == Some(&'(') {
                    while j < src.len() && src[j] != ')' {
                        j += 1;
                    }
                    j += 1;
                }
                while j < src.len() && src[j].is_whitespace() {
                    j += 1;
                }
            }
            if src[j..].starts_with(&['m', 'o', 'd', ' ']) {
                while j < src.len() && src[j] != '{' && src[j] != ';' {
                    j += 1;
                }
                if src.get(j) == Some(&'{') {
                    let end = end_of_block(&src, j);
                    // Keep the line count, so a reported line is the file's.
                    out.extend(src[i..end].iter().filter(|c| **c == '\n'));
                    i = end;
                    continue;
                }
                if src.get(j) == Some(&';') {
                    out.extend(src[i..=j].iter().filter(|c| **c == '\n'));
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(src[i]);
        i += 1;
    }
    out
}

/// The part of a source file that is not a test module, and nothing of a file
/// that exists only for tests.
fn non_test_code(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with("_tests.rs") {
        return String::new();
    }
    without_test_modules(&std::fs::read_to_string(path).expect("source"))
}

/// Every provider the data names, from the one file that names them.
fn provider_names() -> Vec<String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/policy/provider-prefixes.toml");
    let text = std::fs::read_to_string(path).expect("provider-prefixes.toml");
    let names: Vec<String> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("provider = \""))
        .filter_map(|l| l.split('"').next())
        .map(str::to_string)
        .collect();
    assert!(names.len() >= 2, "the data names the providers: {names:?}");
    names
}

/// Where `code` names a provider: as a quoted literal (an id or a program), or
/// as a path into that provider's adapter module (`adapters::<provider>`,
/// including inside `adapters::{...}`). One `file:line: what` per hit.
fn provider_violations(code: &str, providers: &[String]) -> Vec<String> {
    let mut quoted: Vec<String> = providers.iter().map(|p| format!("\"{p}\"")).collect();
    quoted.extend(["\"brew\"".to_string(), "\"port\"".to_string()]);
    let mut found = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let line_code = line.split("//").next().unwrap_or("");
        for word in &quoted {
            if line_code.contains(word.as_str()) {
                found.push(format!("{}: {word}", n + 1));
            }
        }
    }
    let mut from = 0;
    while let Some(at) = code[from..].find("adapters::") {
        let start = from + at + "adapters::".len();
        let tail = code[start..].trim_start();
        let path = if tail.starts_with('{') {
            // The whole group, by brace depth: rustfmt sorts the items, so a
            // provider can follow a nested `filesystem::{..}`.
            let mut depth = 0;
            let mut close = tail.len();
            for (at, c) in tail.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = at + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            &tail[..close]
        } else if tail.starts_with('*') {
            // A glob imports every module, a provider's included, and then
            // names it unqualified where no scan here can see it.
            let line = code[..start].matches('\n').count() + 1;
            found.push(format!("{line}: adapters::*"));
            ""
        } else {
            let stop = tail
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            &tail[..stop]
        };
        for token in path.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if providers.iter().any(|p| p == token) {
                let line = code[..start].matches('\n').count() + 1;
                found.push(format!("{line}: adapters::{token}"));
            }
        }
        from = start;
    }
    found
}

#[test]
fn the_command_line_crate_names_no_provider_in_its_own_code() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty());
    let providers = provider_names();
    let mut found = Vec::new();
    for file in &files {
        for hit in provider_violations(&non_test_code(file), &providers) {
            found.push(format!("{}:{hit}", file.display()));
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

// --- The scanner is held to what it claims ---------------------------------
//
// Planted violations, in strings: the scan passed over production code after a
// test-only item for as long as it cut the file at the first `#[cfg(test)]`.

fn names() -> Vec<String> {
    vec!["alpha".into(), "beta".into()]
}

#[test]
fn a_literal_in_production_code_is_found() {
    assert_eq!(
        provider_violations("let a = \"alpha\";\n", &names()),
        ["1: \"alpha\""]
    );
    assert_eq!(
        provider_violations("run(\"brew\");\nrun(\"port\");\n", &names()).len(),
        2
    );
}

#[test]
fn production_code_after_a_test_only_item_is_still_scanned() {
    // A `#[cfg(test)]` on a function, not a module: the old cut-at-first-marker
    // scan stopped here and never saw the literal below.
    let code = "#[cfg(test)]\nfn helper() {}\n\nfn real() { name(\"beta\"); }\n";
    let cleaned = without_test_modules(code);
    assert_eq!(provider_violations(&cleaned, &names()), ["4: \"beta\""]);
}

#[test]
fn production_code_after_a_test_module_is_still_scanned() {
    let code = "fn a() {}\n#[cfg(test)]\nmod tests {\n    fn t() { x(\"alpha\"); }\n}\n\nfn b() { y(\"alpha\"); }\n";
    let cleaned = without_test_modules(code);
    // The literal inside the module is test code; the one after it is not, and
    // keeps its own line number.
    assert_eq!(provider_violations(&cleaned, &names()), ["7: \"alpha\""]);
}

#[test]
fn a_test_module_declared_by_semicolon_keeps_the_line_numbers() {
    let code = "#[cfg(test)]\nmod tests;\nfn real() { name(\"alpha\"); }\n";
    assert_eq!(
        provider_violations(&without_test_modules(code), &names()),
        ["3: \"alpha\""]
    );
}

#[test]
fn only_a_test_module_is_skipped_not_any_public_item_under_the_attribute() {
    for item in [
        "pub fn h() { x(\"alpha\"); }",
        "pub(crate) fn h() { x(\"alpha\"); }",
    ] {
        let code = format!("#[cfg(test)]\n{item}\n");
        assert_eq!(
            provider_violations(&without_test_modules(&code), &names()),
            ["2: \"alpha\""],
            "{item}"
        );
    }
    // While `pub mod` and `pub(crate) mod` test modules are skipped.
    for head in ["pub mod tests", "pub(crate) mod tests"] {
        let code = format!("#[cfg(test)]\n{head} {{ fn t() {{ x(\"alpha\"); }} }}\n");
        assert!(provider_violations(&without_test_modules(&code), &names()).is_empty());
    }
}

#[test]
fn braces_in_strings_and_comments_do_not_end_a_test_module_early() {
    let code = concat!(
        "#[cfg(test)]\nmod tests {\n",
        "    const A: &str = \"}\";\n",
        "    const B: &str = r#\"} \"}\"#;\n",
        "    const C: char = '}';\n",
        "    // }\n",
        "    /* } */\n",
        "    fn t() { x(\"alpha\"); }\n",
        "}\nfn real() {}\n"
    );
    assert!(provider_violations(&without_test_modules(code), &names()).is_empty());
}

#[test]
fn a_path_into_a_providers_adapter_module_is_refused() {
    let found = |code: &str| provider_violations(code, &names());
    assert_eq!(
        found("use macportsbrew_adapters::alpha::Thing;\n"),
        ["1: adapters::alpha"]
    );
    assert_eq!(
        found("use macportsbrew_adapters::{process, beta};\n"),
        ["1: adapters::beta"]
    );
    assert_eq!(
        found("let x = adapters::beta::run();\n"),
        ["1: adapters::beta"]
    );
    // The adapters' shared modules are not a provider.
    assert!(found("use macportsbrew_adapters::registry::build;\n").is_empty());
    assert!(found("use macportsbrew_adapters::{process, registry};\n").is_empty());
    // A nested group: the provider follows a group that closes first.
    assert_eq!(
        found("use macportsbrew_adapters::{filesystem::{FilesystemScope}, beta};\n"),
        ["1: adapters::beta"]
    );
    // Whitespace after the path separator, and a glob.
    assert_eq!(
        found("use macportsbrew_adapters::\n    alpha::X;\n"),
        ["1: adapters::alpha"]
    );
    assert_eq!(found("use macportsbrew_adapters::*;\n"), ["1: adapters::*"]);
    // A name that merely starts with a provider's is not that provider.
    assert!(found("use macportsbrew_adapters::alphabet::X;\n").is_empty());
}

#[test]
fn the_providers_are_the_ones_the_data_names() {
    let names = provider_names();
    assert!(names.len() >= 2);
    assert!(names
        .iter()
        .all(|n| n.chars().all(|c| c.is_ascii_lowercase())));
}

#[test]
fn invariant_14_no_network_crate_is_locked_and_no_socket_type_is_named_in_the_source() {
    // What this checks, and only that: no network crate in `Cargo.lock` and no
    // socket type spelled in any crate's own source. It does not prove the build
    // cannot open a connection - a dependency could do it without being on the
    // list, and the child processes (a Homebrew read can fetch Homebrew's
    // catalogue) are outside it. INV-14 itself (no LLM or other service at run
    // time, no telemetry, per the charter) is not proved by it: this is a proxy
    // for the remote case of INV-14 (no network, so no hosted model) and for the
    // charter's no-telemetry line. A local model crate, or a recommendation that
    // is not a pure function, is caught elsewhere
    // (`invariant_14_no_llm_no_io_engine_is_pure` in `crates/policy`).
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("Cargo.lock");
    let network_crates = [
        "reqwest",
        "hyper",
        "ureq",
        "curl",
        "isahc",
        "surf",
        "rustls",
        "native-tls",
        "openssl",
        "tokio",
        "async-std",
        "mio",
        "socket2",
        "h2",
        "attohttpc",
    ];
    let mut found: Vec<String> = lock
        .lines()
        .filter_map(|l| l.strip_prefix("name = \""))
        .map(|n| n.trim_end_matches('"').to_string())
        .filter(|n| network_crates.contains(&n.as_str()))
        .collect();
    let mut files = Vec::new();
    for krate in ["domain", "policy", "application", "adapters", "cli"] {
        sources(&root.join("crates").join(krate).join("src"), &mut files);
    }
    for file in &files {
        let text = std::fs::read_to_string(file).expect("source");
        for word in ["std::net", "TcpStream", "UdpSocket", "TcpListener"] {
            if text.contains(word) {
                found.push(format!("{}: {word}", file.display()));
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}
