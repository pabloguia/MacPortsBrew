// SPDX-License-Identifier: Apache-2.0

//! Preconditions the history store, the run lock, the interrupted-run marker
//! and the concurrent-provider detector put on the switch that turns mutations
//! on (pass 08 slices 1 and 2; SE-092, SE-015). Each is a tripwire: it holds
//! trivially while `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, and fails the
//! build that flips it without wiring what a mutating run needs first — so the
//! precondition is a gate, not a checklist item.
//!
//! The text checks live in [`wiring_violations`], a pure function of the flag
//! and the shell's files, which is itself tested **with the flag true** on a
//! good stub and on one mutant per rule. So the proof that a tripwire would
//! fire is a permanent test that runs on every build, not a one-off scratch run
//! that nobody can repeat.
//!
//! **Necessary, not sufficient.** The text scan sees that the calls exist, are
//! kept and come in order; it cannot see reachability (a call under `if false`
//! or in a closure that is never run). The real check is the review of the
//! wiring pass and, there, a behavioural test of the executor with a recording
//! fake — owed with the wiring (SE-092).

use macportsbrew_application::AnalysisSource;
use macportsbrew_domain::plan::THIS_BUILD_DISPATCHES_MUTATIONS;
use std::path::Path;

/// One source file: its path, as a label, and its text.
type SourceFile = (String, String);

/// Every Rust source under the executor shell.
fn executor_files() -> Vec<SourceFile> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/executor");
    let mut files = Vec::new();
    let mut stack = vec![dir];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).expect("the executor shell") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push((
                    path.display().to_string(),
                    std::fs::read_to_string(&path).expect("source"),
                ));
            }
        }
    }
    files.sort();
    assert!(!files.is_empty(), "the executor shell was read");
    files
}

/// The executable code of a file: no test module (the convention is that it is
/// last), no comment of any kind, no string literal's content. A token inside
/// any of those is not wiring.
fn code_of(text: &str) -> String {
    let before_tests = text.split("#[cfg(test)]").next().unwrap_or("");
    let chars: Vec<char> = before_tests.chars().collect();
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            // A line comment, doc comments included: up to the newline.
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '\'' && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'\'') {
            // A character literal holding a quote must not open a string.
            out.push_str("' '");
            i += 3;
        } else if c == 'r'
            && matches!(next, Some('"' | '#'))
            && !out.chars().next_back().is_some_and(is_ident)
        {
            // A raw string: r"..." or r#"..."#, any number of hashes.
            let mut hashes = 0;
            let mut k = i + 1;
            while chars.get(k) == Some(&'#') {
                hashes += 1;
                k += 1;
            }
            if chars.get(k) == Some(&'"') {
                k += 1;
                loop {
                    if k >= chars.len() {
                        break;
                    }
                    if chars[k] == '"' && (0..hashes).all(|h| chars.get(k + 1 + h) == Some(&'#')) {
                        k += 1 + hashes;
                        break;
                    }
                    k += 1;
                }
                out.push_str("\"\"");
                i = k;
            } else {
                out.push(c);
                i += 1;
            }
        } else if c == '"' {
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            out.push('"');
            i += 1;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Byte offsets of `token` as a call: not part of a longer identifier, and not
/// a function definition.
fn calls_of(code: &str, token: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find(token) {
        let start = from + at;
        let before = &code[..start];
        let glued = before
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let defined = before.ends_with("fn ");
        if !glued && !defined {
            found.push(start);
        }
        from = start + token.len();
    }
    found
}

/// The name `run_lock::acquire(` is bound to, when it is `let [mut] name =`
/// and the name is not the discarding `_`.
fn lock_binding(code: &str, at: Option<usize>) -> Option<String> {
    let before = code[..at?].trim_end();
    let before = before.strip_suffix('=')?.trim_end();
    let name: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let head = before[..before.len() - name.len()].trim_end();
    let bound = head.ends_with("let") || head.ends_with("let mut");
    (bound && !name.is_empty() && name != "_").then_some(name)
}

/// The identifiers in the argument list of the call starting at `at`, up to
/// its matching parenthesis.
fn call_arguments(code: &str, at: usize) -> Vec<String> {
    let Some(open) = code[at..].find('(') else {
        return Vec::new();
    };
    let mut depth = 0usize;
    let mut end = code.len();
    for (i, c) in code[at + open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = at + open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    code[at + open + 1..end]
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

const PROBE: &str = "history_store::probe(";
const ACQUIRE: &str = "run_lock::acquire(";
const MARKER_READ: &str = "run_marker::read(";
const MARKER_WRITE: &str = "run_marker::write(";
const DISPATCH: &str = "dispatch(";
const WRITE_RECORD: &str = "history_store::write_record(";
const MARKER_CLEAR: &str = "run_marker::clear(";
const RECHECK: &str = "recheck_provider_configuration(";

/// Every way the executor shell, with mutations on, fails to take the lock,
/// write the marker and publish the record in the order a mutating run needs,
/// each tagged `[lock]`, `[marker]`, `[record]` or `[order]`. Empty while the
/// flag is `false`.
///
/// The order: probe, lock, marker read, marker write, the first dispatch, the
/// record, then the marker's clear. The probe, the lock and the marker write
/// each appear **once** — two sites is two ways to get the order wrong — while
/// the record and the clear may appear on every ending. All of them must be in
/// the one file that takes the lock.
fn wiring_violations(flag: bool, files: &[SourceFile]) -> Vec<String> {
    if !flag {
        return Vec::new();
    }
    let codes: Vec<(&str, String)> = files
        .iter()
        .map(|(name, text)| (name.as_str(), code_of(text)))
        .collect();
    let locking: Vec<&(&str, String)> = codes
        .iter()
        .filter(|(_, code)| !calls_of(code, ACQUIRE).is_empty())
        .collect();
    let mut violations = Vec::new();
    // With exactly one file taking the lock, everything is looked for in it and
    // the order is checked; otherwise the lock is the violation, and the rest is
    // still looked for across the whole shell so that each tripwire fires on its
    // own — a shell with no lock must not silence the probe's and the record's.
    let (name, code, ordered) = if let [(name, code)] = locking.as_slice() {
        ((*name).to_string(), code.clone(), true)
    } else {
        violations.push(if locking.is_empty() {
            format!("[lock] no file of the executor shell calls {ACQUIRE}")
        } else {
            format!(
                "[lock] {} files call {ACQUIRE}; one site decides the order",
                locking.len()
            )
        });
        let all: Vec<&str> = codes.iter().map(|(_, c)| c.as_str()).collect();
        ("the executor shell".to_string(), all.join("\n"), false)
    };
    let code = code.as_str();
    let once = |violations: &mut Vec<String>, tag: &str, token: &str| -> Option<usize> {
        let at = calls_of(code, token);
        match at.as_slice() {
            [one] => Some(*one),
            [] => {
                violations.push(format!("[{tag}] {name} never calls {token}"));
                None
            }
            _ => {
                violations.push(format!("[{tag}] {name} calls {token} more than once"));
                None
            }
        }
    };
    // The lock must be bound to a name that stays alive: a bare statement or
    // `let _ =` drops it at once, and a `drop` before the first dispatch lets go
    // of it. The marker calls must be handed that name.
    let bound = if ordered {
        lock_binding(code, calls_of(code, ACQUIRE).first().copied())
    } else {
        None
    };
    if ordered {
        match &bound {
            None => violations.push(format!(
                "[lock] {name}: the result of {ACQUIRE} is not bound to a name that is kept"
            )),
            Some(lock) => {
                for token in [MARKER_READ, MARKER_WRITE, MARKER_CLEAR] {
                    let handed = calls_of(code, token)
                        .iter()
                        .all(|at| code[at + token.len()..].starts_with(&format!("&{lock}")));
                    if !handed {
                        violations.push(format!(
                            "[lock] {name}: {token} is not handed the lock `{lock}`"
                        ));
                    }
                }
                let dispatch_at = calls_of(code, DISPATCH).first().copied();
                let dropped = calls_of(code, "drop(")
                    .into_iter()
                    .find(|at| code[at + 5..].starts_with(&format!("{lock})")));
                if let (Some(d), Some(x)) = (dispatch_at, dropped) {
                    if d > x {
                        violations.push(format!(
                            "[lock] {name}: the lock `{lock}` is dropped before the first dispatch"
                        ));
                    }
                }
            }
        }
    }
    let probe = once(&mut violations, "record", PROBE);
    let acquire = if ordered {
        once(&mut violations, "lock", ACQUIRE)
    } else {
        None
    };
    let read = once(&mut violations, "marker", MARKER_READ);
    let write = once(&mut violations, "marker", MARKER_WRITE);
    let dispatch = calls_of(code, DISPATCH).first().copied();
    if dispatch.is_none() {
        violations.push(format!("[order] {name} never dispatches a step"));
    }
    let record = calls_of(code, WRITE_RECORD);
    if record.is_empty() {
        violations.push(format!("[record] {name} never calls {WRITE_RECORD}"));
    }
    let clear = calls_of(code, MARKER_CLEAR);
    if clear.is_empty() {
        violations.push(format!("[marker] {name} never calls {MARKER_CLEAR}"));
    }
    let before = |violations: &mut Vec<String>, a: Option<usize>, b: Option<usize>, what: &str| {
        if let (true, Some(a), Some(b)) = (ordered, a, b) {
            if a >= b {
                violations.push(format!("[order] {name}: {what}"));
            }
        }
    };
    before(
        &mut violations,
        probe,
        acquire,
        "the history probe must come before the lock",
    );
    before(
        &mut violations,
        acquire,
        read,
        "the lock must come before the marker is read",
    );
    before(
        &mut violations,
        read,
        write,
        "the marker must be read before it is written",
    );
    before(
        &mut violations,
        write,
        dispatch,
        "the marker must be written before the first dispatch",
    );
    before(
        &mut violations,
        dispatch,
        record.first().copied(),
        "a record cannot be published before the first dispatch",
    );
    // The provider's configuration is re-checked before *each* dispatch: the
    // last re-check between a dispatch call and the one before it (or the
    // marker write, for the first) is bound to a name, and that name is an
    // argument of the dispatch - the token the wiring hands over, so `let _ =`,
    // a bare statement, or a check made and then not passed throws it away
    // (SE-016, DP-027 (b); pass 08 slice 3, D-R1'; security review, L1). The
    // textual rule cannot see a loop; the token, neither `Clone` nor `Copy`,
    // cannot be moved into one, which is why the dispatch must take it.
    if ordered {
        let rechecks = calls_of(code, RECHECK);
        if rechecks.is_empty() {
            violations.push(format!("[config] {name} never calls {RECHECK}"));
        }
        for r in &rechecks {
            if lock_binding(code, Some(*r)).is_none() {
                violations.push(format!(
                    "[config] {name}: a re-check's result is not bound to a name that is kept"
                ));
            }
        }
        let mut boundary = write.unwrap_or(0);
        for d in calls_of(code, DISPATCH) {
            let guard = rechecks
                .iter()
                .filter(|r| **r > boundary && **r < d)
                .max()
                .and_then(|r| lock_binding(code, Some(*r)));
            match guard {
                None => violations.push(format!(
                    "[config] {name}: a dispatch is not preceded by its own re-check"
                )),
                Some(token) if !call_arguments(code, d).contains(&token) => {
                    violations.push(format!(
                        "[config] {name}: a dispatch is not handed its re-check `{token}`"
                    ));
                }
                Some(_) => {}
            }
            boundary = d;
        }
    }
    // Some clear must follow a record: the marker outlives the record's write.
    if let (true, Some(first_record), false) = (ordered, record.first(), clear.is_empty()) {
        if !clear.iter().any(|c| c > first_record) {
            violations.push(format!(
                "[order] {name}: the marker is never cleared after the record"
            ));
        }
    }
    violations
}

fn violations_with(tag: &str) -> Vec<String> {
    wiring_violations(THIS_BUILD_DISPATCHES_MUTATIONS, &executor_files())
        .into_iter()
        .filter(|v| v.starts_with(&format!("[{tag}]")))
        .collect()
}

#[test]
fn mutations_cannot_dispatch_before_the_history_store_is_probed() {
    // A mutating run must prove it can record itself before it changes
    // anything (04, History; security review of slice 1, M1).
    let missing: Vec<String> = violations_with("record")
        .into_iter()
        .filter(|v| v.contains("probe"))
        .collect();
    assert!(missing.is_empty(), "mutations dispatch: {missing:?}");
}

#[test]
fn mutations_cannot_dispatch_before_a_record_is_written() {
    // Obligation 12: a mutating run writes its record on every way it ends.
    let missing: Vec<String> = violations_with("record")
        .into_iter()
        .filter(|v| v.contains("write_record"))
        .collect();
    assert!(missing.is_empty(), "mutations dispatch: {missing:?}");
}

#[test]
fn mutations_cannot_dispatch_before_the_run_lock_is_taken() {
    // 04, Concurrency and locking: a single-instance lock before any mutation.
    let v = violations_with("lock");
    assert!(
        v.is_empty(),
        "mutations dispatch without the run lock: {v:?}"
    );
}

#[test]
fn mutations_cannot_dispatch_without_the_interrupted_run_marker_read_written_and_cleared() {
    // 06, exit 4: an interrupted run is named and refused, which needs the
    // marker to be looked for, written before the change and cleared after.
    let v = violations_with("marker");
    assert!(v.is_empty(), "mutations dispatch without the marker: {v:?}");
}

#[test]
fn mutations_cannot_dispatch_without_rechecking_the_providers_configuration() {
    // SE-016's check-then-use window: a `brew.env` or `macports.conf` that
    // appeared after the capture refuses the step (DP-027 (b)).
    let v = violations_with("config");
    assert!(
        v.is_empty(),
        "mutations dispatch without re-checking the configuration: {v:?}"
    );
}

#[test]
fn the_shells_order_is_probe_lock_marker_dispatch_record_then_clear() {
    let v = violations_with("order");
    assert!(v.is_empty(), "mutations dispatch out of order: {v:?}");
}

#[test]
fn mutations_cannot_dispatch_while_a_live_run_still_says_not_checked() {
    // SE-015 asks for the detection of a concurrently running `brew` or `port`
    // before mutating, and refusing. No detector exists (R-17, DP-030), so a
    // live run answers NOT_CHECKED and every live run is possibly
    // inconsistent. A build that turns mutations on must have a live check, not
    // a different label: `Unknowable` is not a check either. Only the owner
    // relaxes this (DP-030); an agent that weakens this test breaks the work
    // order's rule against weakening a gate.
    if THIS_BUILD_DISPATCHES_MUTATIONS {
        assert!(
            AnalysisSource::Live
                .concurrent_provider_activity()
                .was_checked(),
            "mutations dispatch and a live run still does not check for a concurrently \
             running provider; see DP-030 in docs/decisions-pending.md"
        );
    }
}

// --- the tripwires, tested with the flag true --------------------------------

/// A shell that wires everything in the right order.
const GOOD: &str = r"
fn run_with() {
    history_store::probe(&state, uid)?;
    let lock = run_lock::acquire(&state, uid)?;
    let earlier = run_marker::read(&lock, &state, uid)?;
    run_marker::write(&lock, &state, &run, uid)?;
    let checked = recheck_provider_configuration(&step.provider, &snapshot, &fresh)?;
    let results = dispatch(step, &site, checked);
    history_store::write_record(&state, &record, uid)?;
    run_marker::clear(&lock, &state, &run, uid)?;
}
fn dispatch() {}
";

fn one(text: &str) -> Vec<SourceFile> {
    vec![("shell.rs".to_string(), text.to_string())]
}

fn violations_of(text: &str) -> Vec<String> {
    wiring_violations(true, &one(text))
}

fn without(token: &str) -> String {
    GOOD.replace(&format!("{token}("), "forgotten(")
}

#[test]
fn a_correct_wiring_has_no_violation_and_nothing_is_checked_while_the_flag_is_false() {
    assert_eq!(violations_of(GOOD), Vec::<String>::new());
    assert!(wiring_violations(false, &one("fn nothing() {}")).is_empty());
}

#[test]
fn removing_any_one_call_is_a_violation_of_its_own_kind() {
    for (call, tag) in [
        ("history_store::probe", "[record]"),
        ("run_lock::acquire", "[lock]"),
        ("run_marker::read", "[marker]"),
        ("run_marker::write", "[marker]"),
        ("history_store::write_record", "[record]"),
        ("run_marker::clear", "[marker]"),
        ("recheck_provider_configuration", "[config]"),
    ] {
        let v = violations_of(&without(call));
        assert!(
            v.iter().any(|x| x.starts_with(tag)),
            "removing {call} gave {v:?}"
        );
    }
    let v =
        violations_of(&GOOD.replace("dispatch(step, &site, checked)", "step.run(&site, checked)"));
    assert!(v.iter().any(|x| x.contains("never dispatches")), "{v:?}");
}

#[test]
fn a_call_in_a_comment_a_string_or_a_test_module_is_not_wiring() {
    let in_comment = GOOD.replace(
        "    let lock = run_lock::acquire(&state, uid)?;",
        "    // run_lock::acquire(&state, uid)\n    let lock = ();",
    );
    assert!(violations_of(&in_comment)
        .iter()
        .any(|v| v.starts_with("[lock]")));
    let in_string = GOOD.replace(
        "    let lock = run_lock::acquire(&state, uid)?;",
        "    let lock = \"run_lock::acquire(\";",
    );
    assert!(violations_of(&in_string)
        .iter()
        .any(|v| v.starts_with("[lock]")));
    let in_tests = format!(
        "{}\n#[cfg(test)]\nmod tests {{ fn t() {{ run_marker::clear(); }} }}",
        GOOD.replace("    run_marker::clear(&lock, &state, &run, uid)?;\n", "")
    );
    assert!(violations_of(&in_tests)
        .iter()
        .any(|v| v.starts_with("[marker]")));
    let in_char_then_string = GOOD.replace(
        "    let lock = run_lock::acquire(&state, uid)?;",
        "    let q = '\"'; let lock = run_lock::acquire(&state, uid)?;",
    );
    assert_eq!(violations_of(&in_char_then_string), Vec::<String>::new());
}

#[test]
fn a_lock_that_is_dropped_discarded_or_not_handed_on_is_not_a_lock() {
    let bare = GOOD.replace(
        "    let lock = run_lock::acquire(&state, uid)?;",
        "    run_lock::acquire(&state, uid)?;",
    );
    let discarded = GOOD.replace(
        "    let lock = run_lock::acquire(&state, uid)?;",
        "    let _ = run_lock::acquire(&state, uid)?;",
    );
    let dropped = GOOD.replace("    let results", "    drop(lock);\n    let results");
    let not_handed = GOOD.replace("run_marker::write(&lock,", "run_marker::write(&other,");
    for (what, text) in [
        ("a bare statement", bare),
        ("let _ =", discarded),
        ("a drop before the dispatch", dropped),
        ("a marker call without the lock", not_handed),
    ] {
        assert!(
            violations_of(&text).iter().any(|v| v.starts_with("[lock]")),
            "{what}: {:?}",
            violations_of(&text)
        );
    }
    // A named, kept binding of either spelling is fine.
    let kept = GOOD.replace("let lock =", "let mut lock =");
    assert_eq!(violations_of(&kept), Vec::<String>::new());
    let underscored = GOOD
        .replace("let lock =", "let _lock =")
        .replace("&lock,", "&_lock,");
    assert_eq!(violations_of(&underscored), Vec::<String>::new());
}

#[test]
fn a_call_named_in_a_comment_or_a_string_is_not_counted_beside_the_real_one() {
    // The real wiring is there; each decoy repeats a token the rules count
    // once. Only an uncommented, unquoted call counts, so none is a second site.
    for decoy in [
        "    // run_lock::acquire(&state, uid); history_store::probe(); run_marker::write()",
        "    /* run_lock::acquire( history_store::probe( run_marker::write( */",
        "    /// run_lock::acquire( history_store::probe( run_marker::write(",
        "    let _text = r#\"run_lock::acquire( history_store::probe( run_marker::write(\"#;",
        "    let _text = r\"run_lock::acquire( history_store::probe( run_marker::write(\";",
        "    let _text = \"run_lock::acquire( history_store::probe( run_marker::write(\";",
        "    let _text = r##\"a\"b run_lock::acquire( c\"# d\"##;",
        "    let _ok = 1; // run_lock::acquire(",
    ] {
        let text = GOOD.replace("    let results", &format!("{decoy}\n    let results"));
        assert_eq!(violations_of(&text), Vec::<String>::new(), "{decoy}");
    }
}

#[test]
fn every_kind_of_comment_and_string_hides_a_call() {
    let hide = |replacement: &str| {
        GOOD.replace(
            "    let lock = run_lock::acquire(&state, uid)?;",
            replacement,
        )
    };
    for (what, text) in [
        (
            "a trailing comment",
            hide("    let lock = (); // run_lock::acquire("),
        ),
        (
            "a block comment",
            hide("    let lock = (); /* run_lock::acquire( */"),
        ),
        (
            "a doc comment",
            hide("    /// run_lock::acquire(\n    let lock = ();"),
        ),
        (
            "a raw string",
            hide("    let lock = r#\"run_lock::acquire(\"#;"),
        ),
        (
            "a plain raw string",
            hide("    let lock = r\"run_lock::acquire(\";"),
        ),
    ] {
        assert!(
            violations_of(&text).iter().any(|v| v.starts_with("[lock]")),
            "{what}: {:?}",
            violations_of(&text)
        );
    }
}

#[test]
fn a_function_definition_or_a_longer_name_is_not_a_call() {
    let defined_only = GOOD.replace("let results = dispatch(step, &site, checked);", "");
    assert!(violations_of(&defined_only)
        .iter()
        .any(|v| v.contains("never dispatches")));
    let longer = GOOD.replace(
        "dispatch(step, &site, checked)",
        "redispatch(step, &site, checked)",
    );
    assert!(violations_of(&longer)
        .iter()
        .any(|v| v.contains("never dispatches")));
}

#[test]
fn each_reordering_is_a_violation_of_the_order() {
    let swap = |a: &str, b: &str| {
        let (la, lb) = (
            GOOD.lines().find(|l| l.contains(a)).unwrap(),
            GOOD.lines().find(|l| l.contains(b)).unwrap(),
        );
        GOOD.replace(la, "@@A").replace(lb, la).replace("@@A", lb)
    };
    for (a, b) in [
        ("history_store::probe", "run_lock::acquire"),
        ("run_lock::acquire", "run_marker::read"),
        ("run_marker::read", "run_marker::write"),
        ("run_marker::write", "dispatch(step"),
        ("dispatch(step", "history_store::write_record"),
        ("history_store::write_record", "run_marker::clear"),
    ] {
        let v = violations_of(&swap(a, b));
        assert!(
            v.iter().any(|x| x.starts_with("[order]")),
            "swapping {a} and {b} gave {v:?}"
        );
    }
}

#[test]
fn a_second_site_for_the_lock_the_marker_write_or_the_probe_is_a_violation() {
    for line in [
        "    let again = run_lock::acquire(&state, uid)?;",
        "    run_marker::write(&lock, &state, &run, uid)?;",
        "    history_store::probe(&state, uid)?;",
    ] {
        let doubled = GOOD.replace("    let results", &format!("{line}\n    let results"));
        assert!(
            violations_of(&doubled)
                .iter()
                .any(|v| v.contains("more than once")),
            "{line}"
        );
    }
    // The record and the clear may be on every ending.
    let many = GOOD.replace(
        "}\nfn dispatch",
        "    history_store::write_record(&state, &record, uid)?;\n    run_marker::clear(&lock, &state, &run, uid)?;\n}\nfn dispatch",
    );
    assert_eq!(violations_of(&many), Vec::<String>::new());
}

#[test]
fn the_wiring_must_be_in_one_file() {
    let split = vec![
        (
            "a.rs".to_string(),
            "fn a() { history_store::probe(); run_lock::acquire(); }".to_string(),
        ),
        (
            "b.rs".to_string(),
            "fn b() { run_marker::read(); run_marker::write(); dispatch(); history_store::write_record(); run_marker::clear(); }".to_string(),
        ),
    ];
    assert!(!wiring_violations(true, &split).is_empty());
    let two_lockers = vec![
        ("a.rs".to_string(), GOOD.to_string()),
        ("b.rs".to_string(), GOOD.to_string()),
    ];
    assert!(wiring_violations(true, &two_lockers)
        .iter()
        .any(|v| v.contains("files call")));
    assert!(wiring_violations(true, &[])
        .iter()
        .any(|v| v.contains("no file")));
}

#[test]
fn the_marker_must_be_cleared_after_a_record_and_not_only_before_one() {
    let early = GOOD
        .replace("    run_marker::clear(&lock, &state, &run, uid)?;\n", "")
        .replace(
            "    let results",
            "    run_marker::clear(&lock, &state, &run, uid)?;\n    let results",
        );
    assert!(violations_of(&early)
        .iter()
        .any(|v| v.contains("never cleared after")));
}

#[test]
fn a_re_check_that_is_discarded_misplaced_or_shared_by_two_dispatches_is_a_violation() {
    let line =
        "    let checked = recheck_provider_configuration(&step.provider, &snapshot, &fresh)?;";
    let discarded = GOOD.replace(
        line,
        "    let _ = recheck_provider_configuration(&step.provider, &snapshot, &fresh)?;",
    );
    let bare = GOOD.replace(
        line,
        "    recheck_provider_configuration(&step.provider, &snapshot, &fresh)?;",
    );
    let before_marker = GOOD.replace(&format!("{line}\n"), "").replace(
        "    run_marker::write(&lock, &state, &run, uid)?;",
        &format!("{line}\n    run_marker::write(&lock, &state, &run, uid)?;"),
    );
    let shared = GOOD.replace(
        "    let results = dispatch(step, &site, checked);",
        "    let results = dispatch(step, &site, checked);\n    let more = dispatch(next, &site, checked);",
    );
    let not_handed = GOOD.replace("dispatch(step, &site, checked)", "dispatch(step, &site)");
    let handed_another = GOOD.replace(
        "dispatch(step, &site, checked)",
        "dispatch(step, &site, stale)",
    );
    let shared_token = GOOD.replace(
        "    let results = dispatch(step, &site, checked);",
        &format!(
            "    let results = dispatch(step, &site, checked);\n{}\n    let more = dispatch(next, &site, checked);",
            line.replace("checked", "checked_again")
        ),
    );
    for (what, text) in [
        ("a re-check not handed to the dispatch", not_handed),
        ("a different name handed to the dispatch", handed_another),
        (
            "the first token handed to the second dispatch",
            shared_token,
        ),
        ("let _ =", discarded),
        ("a bare statement", bare),
        ("a re-check before the marker", before_marker),
        ("one re-check for two dispatches", shared),
    ] {
        assert!(
            violations_of(&text)
                .iter()
                .any(|v| v.starts_with("[config]")),
            "{what}: {:?}",
            violations_of(&text)
        );
    }
    // One re-check per dispatch is fine.
    let each = GOOD.replace(
        "    let results = dispatch(step, &site, checked);",
        &format!(
            "    let results = dispatch(step, &site, checked);\n{}\n    let more = dispatch(next, &site, checked_again);",
            line.replace("checked", "checked_again")
        ),
    );
    assert_eq!(violations_of(&each), Vec::<String>::new());
}

#[test]
fn a_calls_arguments_end_at_its_matching_parenthesis() {
    let code = "dispatch(step, f(&site)); then(checked)";
    assert_eq!(call_arguments(code, 0), ["step", "f", "site"]);
    assert!(call_arguments("x", 0).is_empty());
}
