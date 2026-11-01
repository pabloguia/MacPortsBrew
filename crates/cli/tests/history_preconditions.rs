// SPDX-License-Identifier: Apache-2.0

//! Preconditions the history store, the run lock, the interrupted-run marker
//! and the concurrent-provider detector put on the switch that turns mutations
//! on (pass 08 slices 1, 2 and 4; SE-092, SE-015).
//!
//! **Since slice 4 the wiring rules run unconditionally** against the shell:
//! the mutating sequence exists behind a mutation allowance, so its order is
//! checked on every build, not only the one that flips the switch. The **flip
//! tripwires** at the end still hold trivially while
//! `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, and each fails the build that
//! flips it while the item it names is outstanding — so the precondition is a
//! gate, not a checklist item.
//!
//! **The dispatch token is `mutation_dispatch(`** (slice 4, D5, its own SE-092
//! entry). `calls_of` ignores a match glued to an identifier character, so
//! once the read helper became `read_commands` and the mutating one
//! `mutation_dispatch`, the old `dispatch(` named no call at all; the token
//! moved with the names, and the inverse tripwires below make sure no read
//! helper can be named out of the rule's reach.
//!
//! Two exceptions (work order 16 part S): `the_destination_is_reread_before_removal`
//! here is a positive structural check held whatever the switch is, and
//! `the_switch_needs_the_destination_contract_evaluable` lives in
//! `crates/cli/tests/executor.rs`, beside the in-code snapshots it plans from.
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
    // Cut at the test module only: an earlier `#[cfg(test)]` helper must not
    // hide the rest of the file from the scans (slice 4 review).
    let before_tests = text.split("#[cfg(test)]\nmod").next().unwrap_or("");
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
const DISPATCH: &str = "mutation_dispatch(";
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
    // Unconditionally since slice 4: the sequence is wired behind an allowance.
    wiring_violations(true, &executor_files())
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
    let results = mutation_dispatch(step, &site, checked);
    history_store::write_record(&state, &record, uid)?;
    run_marker::clear(&lock, &state, &run, uid)?;
}
fn mutation_dispatch() {}
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
    let v = violations_of(&GOOD.replace(
        "mutation_dispatch(step, &site, checked)",
        "step.run(&site, checked)",
    ));
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
    let defined_only = GOOD.replace("let results = mutation_dispatch(step, &site, checked);", "");
    assert!(violations_of(&defined_only)
        .iter()
        .any(|v| v.contains("never dispatches")));
    let longer = GOOD.replace(
        "mutation_dispatch(step, &site, checked)",
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
        ("run_marker::write", "mutation_dispatch(step"),
        ("mutation_dispatch(step", "history_store::write_record"),
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
        "}\nfn mutation_dispatch",
        "    history_store::write_record(&state, &record, uid)?;\n    run_marker::clear(&lock, &state, &run, uid)?;\n}\nfn mutation_dispatch",
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
            "fn b() { run_marker::read(); run_marker::write(); mutation_dispatch(); history_store::write_record(); run_marker::clear(); }".to_string(),
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
        "    let results = mutation_dispatch(step, &site, checked);",
        "    let results = mutation_dispatch(step, &site, checked);\n    let more = mutation_dispatch(next, &site, checked);",
    );
    let not_handed = GOOD.replace(
        "mutation_dispatch(step, &site, checked)",
        "mutation_dispatch(step, &site)",
    );
    let handed_another = GOOD.replace(
        "mutation_dispatch(step, &site, checked)",
        "mutation_dispatch(step, &site, stale)",
    );
    let shared_token = GOOD.replace(
        "    let results = mutation_dispatch(step, &site, checked);",
        &format!(
            "    let results = mutation_dispatch(step, &site, checked);\n{}\n    let more = mutation_dispatch(next, &site, checked);",
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
        "    let results = mutation_dispatch(step, &site, checked);",
        &format!(
            "    let results = mutation_dispatch(step, &site, checked);\n{}\n    let more = mutation_dispatch(next, &site, checked_again);",
            line.replace("checked", "checked_again")
        ),
    );
    assert_eq!(violations_of(&each), Vec::<String>::new());
}

#[test]
fn a_calls_arguments_end_at_its_matching_parenthesis() {
    let code = "mutation_dispatch(step, f(&site)); then(checked)";
    assert_eq!(call_arguments(code, 0), ["step", "f", "site"]);
    assert!(call_arguments("x", 0).is_empty());
}

// --- slice 4: the inverse tripwires (D5) -------------------------------------

/// The body of the function named `name` in `code`: from its `fn name(` to the
/// brace that closes the first block after it.
fn body_of<'a>(code: &'a str, name: &str) -> Option<&'a str> {
    let start = code.find(&format!("fn {name}("))?;
    let open = start + code[start..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in code[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&code[open..=open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every way a mutating command could reach the mutation site other than
/// through `mutation_dispatch`, or a read helper could be named out of the
/// wiring rules' reach.
fn site_violations(files: &[SourceFile]) -> Vec<String> {
    let mut violations = Vec::new();
    for (name, text) in files {
        let code = code_of(text);
        // Every occurrence: `calls_of` would skip a method call, whose dot is
        // glued to the receiver's name.
        let mut performs: Vec<usize> = code.match_indices(".perform(").map(|(at, _)| at).collect();
        performs.extend(
            code.match_indices("MutationSite::perform(")
                .map(|(at, _)| at),
        );
        let inside = body_of(&code, "mutation_dispatch").map(|body| {
            let at = code.find(body).expect("the body is in the code");
            at..at + body.len()
        });
        for at in performs {
            if !inside.as_ref().is_some_and(|r| r.contains(&at)) {
                violations.push(format!(
                    "[site] {name}: the mutation site is called outside mutation_dispatch"
                ));
            }
        }
        if body_of(&code, "read_commands").is_some_and(|b| b.contains(".perform(")) {
            violations.push(format!(
                "[site] {name}: read_commands calls the mutation site"
            ));
        }
        if code.contains("fn dispatch(") || code.contains("fn dispatch<") {
            violations.push(format!(
                "[site] {name}: a function is named bare `dispatch`, which the wiring rules \
                 cannot see"
            ));
        }
    }
    violations
}

#[test]
fn the_mutation_site_is_reached_only_through_mutation_dispatch() {
    let v = site_violations(&executor_files());
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn the_inverse_tripwires_fire_on_their_mutants() {
    let good = r"
fn read_commands(site: &dyn ReadSite) { site.read(); }
fn mutation_dispatch(site: &dyn MutationSite) { site.perform(); }
";
    assert_eq!(site_violations(&one(good)), Vec::<String>::new());
    for (what, text) in [
        (
            "a perform outside mutation_dispatch",
            format!("{good}\nfn elsewhere(site: &dyn MutationSite) {{ site.perform(); }}"),
        ),
        (
            "read_commands performing",
            good.replace("site.read();", "site.perform();"),
        ),
        (
            "the read helper renamed dispatch",
            good.replace("fn read_commands(", "fn dispatch("),
        ),
        (
            "a fully qualified perform outside mutation_dispatch",
            format!("{good}\nfn elsewhere(site: &S) {{ MutationSite::perform(site); }}"),
        ),
    ] {
        assert!(
            site_violations(&one(&text))
                .iter()
                .any(|v| v.starts_with("[site]")),
            "{what}: {:?}",
            site_violations(&one(&text))
        );
    }
}

// --- slice 4: the flip tripwires (D14) ---------------------------------------
//
// Each holds trivially while the switch is off and fails the build that turns
// it on while its item is outstanding. Each names how it detects the item; an
// item with no structural fact to detect is an unconditional failure while the
// switch is on, removable only in the change that delivers the item with its
// evidence.

fn workspace_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The two brackets, performed (pass 08 slice 5, D6). Scoped to the shell's
/// one file that takes the lock and ordered like the other wiring tripwires:
/// the snapshot step is performed before the marker is written, and the record
/// is read back after it is written and before the marker is cleared. A call
/// inside a comment, a string or a test module is not wiring (`code_of`).
fn bracket_violations(files: &[SourceFile]) -> Vec<String> {
    let codes: Vec<String> = files.iter().map(|(_, text)| code_of(text)).collect();
    let locking: Vec<&String> = codes
        .iter()
        .filter(|code| !calls_of(code, ACQUIRE).is_empty())
        .collect();
    let [code] = locking.as_slice() else {
        return vec!["[brackets] not exactly one file of the executor shell takes the lock".into()];
    };
    // A call, not the definition: `fn take_snapshot(` is not performing it.
    let calls = |token: &str| -> Vec<usize> {
        calls_of(code, token)
            .into_iter()
            .filter(|at| !code[..*at].ends_with("fn "))
            .collect()
    };
    let mut violations = Vec::new();
    let snapshot = calls(SNAPSHOT_STEP).first().copied();
    let marker = calls(MARKER_WRITE).first().copied();
    let written = calls(WRITE_RECORD).first().copied();
    let read_back = calls(READ_BACK).first().copied();
    let clear = written.and_then(|w| calls(MARKER_CLEAR).into_iter().find(|c| *c > w));
    match (snapshot, marker) {
        (None, _) => violations.push(format!("[brackets] the shell never calls {SNAPSHOT_STEP}")),
        (Some(s), Some(m)) if s >= m => violations.push(
            "[brackets] the snapshot is taken after the marker is written, not before".into(),
        ),
        _ => {}
    }
    for token in [
        "snapshot_store::save_snapshot_durable(",
        "snapshot_store::state_snapshot(",
    ] {
        if calls(token).is_empty() {
            violations.push(format!("[brackets] the shell never calls {token}"));
        }
    }
    match (written, read_back, clear) {
        (_, None, _) => violations.push(format!("[brackets] the shell never calls {READ_BACK}")),
        (Some(w), Some(r), _) if r <= w => violations
            .push("[brackets] the record is read back before it is written, not after".into()),
        (_, Some(r), Some(c)) if r >= c => violations
            .push("[brackets] the marker is cleared before the record is read back".into()),
        (_, Some(_), None) => {
            violations.push("[brackets] the marker is never cleared after the read-back".into());
        }
        _ => {}
    }
    violations
}

const SNAPSHOT_STEP: &str = "take_snapshot(";
const READ_BACK: &str = "(io.read_back)(";

/// Detection: the shell performs the brackets, in order, and the application
/// answers them under an allowance. Pass 08 slice 5 delivered both; this stays
/// as a positive check so a regression is still caught (it was a check that the
/// brackets were *not* performed until then).
#[test]
fn the_switch_needs_snapshot_before_and_record_history_performed() {
    let executor = code_of(&workspace_file("application/src/executor.rs"));
    // Compared with all whitespace removed, so a reflow cannot make the
    // detection pass silently (slice 4 review).
    let squeeze = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let executor = squeeze(&executor);
    let application_performs = [
        "StepKind::SnapshotBeforeifallowance.is_some()=>Action::TakeSnapshot",
        "StepKind::RecordHistoryifallowance.is_some()=>Action::RecordHistory",
    ]
    .iter()
    .all(|needle| executor.contains(needle));
    let mut violations = bracket_violations(&executor_files());
    if !application_performs {
        violations.push(
            "[brackets] next_action_guarded no longer answers TakeSnapshot / RecordHistory \
             under an allowance"
                .into(),
        );
    }
    // The detection must still match the source: a stale one is as bad as a
    // missing item, whatever the switch says.
    assert!(
        violations.is_empty(),
        "the detection no longer matches the source (or a bracket regressed): {violations:#?}"
    );
    assert!(
        !THIS_BUILD_DISPATCHES_MUTATIONS || violations.is_empty(),
        "mutations dispatch and SNAPSHOT_BEFORE / RECORD_HISTORY are not performed in order \
         (plan.v1.md obligation 11; SE-092, slice 5 D6)"
    );
}

const BRACKETS_GOOD: &str = r"
fn run_with_mutations() {
    let lock = run_lock::acquire(state, euid);
    let snapshot = take_snapshot(state, &now, euid);
    run_marker::write(&lock, state, &named, euid);
    history_store::write_record(state, &record, euid);
    let read = (io.read_back)(state, id, euid);
    run_marker::clear(&lock, state, &marker, euid);
}
fn take_snapshot() {
    snapshot_store::save_snapshot_durable(dir, snap);
    snapshot_store::state_snapshot(state, &name, euid);
}
";

fn brackets_of(text: &str) -> Vec<String> {
    bracket_violations(&[("sample.rs".to_string(), text.to_string())])
}

#[test]
fn the_bracket_tripwire_accepts_the_right_order_and_names_each_way_it_is_broken() {
    assert_eq!(brackets_of(BRACKETS_GOOD), Vec::<String>::new());
    let swapped = BRACKETS_GOOD
        .replace("let snapshot = take_snapshot(state, &now, euid);\n", "")
        .replace(
            "run_marker::write(&lock, state, &named, euid);",
            "run_marker::write(&lock, state, &named, euid);\n    let snapshot = take_snapshot(state, &now, euid);",
        );
    for (what, text, expect) in [
        (
            "no snapshot step",
            BRACKETS_GOOD.replace("let snapshot = take_snapshot(state, &now, euid);", ""),
            "never calls take_snapshot(",
        ),
        ("snapshot after the marker", swapped, "after the marker"),
        (
            "no durable save",
            BRACKETS_GOOD.replace("save_snapshot_durable(", "save_snapshot("),
            "save_snapshot_durable(",
        ),
        (
            "no read of the snapshot back",
            BRACKETS_GOOD.replace("state_snapshot(", "load_state("),
            "state_snapshot(",
        ),
        (
            "no record read-back",
            BRACKETS_GOOD.replace("(io.read_back)(", "(io.nothing)("),
            "never calls (io.read_back)(",
        ),
        (
            "read-back before the write",
            BRACKETS_GOOD
                .replace("let read = (io.read_back)(state, id, euid);", "")
                .replace(
                    "history_store::write_record(",
                    "let read = (io.read_back)(state, id, euid);\n    history_store::write_record(",
                ),
            "before it is written",
        ),
        (
            "marker cleared before the read-back",
            BRACKETS_GOOD
                .replace("let read = (io.read_back)(state, id, euid);", "")
                .replace(
                    "run_marker::clear(&lock, state, &marker, euid);",
                    "run_marker::clear(&lock, state, &marker, euid);\n    let read = (io.read_back)(state, id, euid);",
                ),
            "before the record is read back",
        ),
        (
            "a call only in a comment",
            BRACKETS_GOOD.replace("let snapshot = take_snapshot(state, &now, euid);", "// take_snapshot(state)"),
            "never calls take_snapshot(",
        ),
        (
            "the definition is not a call",
            BRACKETS_GOOD.replace("let snapshot = take_snapshot(state, &now, euid);", ""),
            "never calls take_snapshot(",
        ),
    ] {
        let found = brackets_of(&text);
        assert!(
            found.iter().any(|v| v.contains(expect)),
            "{what}: expected `{expect}` in {found:?}"
        );
    }
}

/// A positive structural check, held whatever the switch is (work order 16
/// part E-3, §6): consent is decided by `consent_for` after the first
/// recapture and `plan_still_holds`, its result kept with `?`, before the
/// marker is written; a second recapture follows it; a run that is not live
/// is refused before it; and `migrate`'s product seam asks through
/// `consent::ask` with a fresh code and no fallback, behind the live-scope
/// refusal, so `--replay` and `--from-snapshot` never reach it (E-1 review,
/// I1). The caller count it had before E-3 stays as a sub-check.
#[test]
fn the_switch_needs_consent_wired() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let migrate =
        std::fs::read_to_string(root.join("crates/cli/src/app/migrate.rs")).expect("migrate.rs");
    let violations = consent_violations(&executor_files(), &migrate);
    assert!(violations.is_empty(), "{violations:?}");
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut callers = 0;
    let mut stack: Vec<_> = std::fs::read_dir(&crates)
        .expect("crates/")
        .map(|e| e.expect("entry").path().join("src"))
        .filter(|p| p.is_dir())
        .collect();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "consent.rs")
            {
                let code = code_of(&std::fs::read_to_string(&path).expect("source"));
                callers += calls_of(&code, "consent::ask(").len();
            }
        }
    }
    assert_eq!(
        callers, 1,
        "one product caller of consent::ask (INV-06; SE-016, slice 4 D10)"
    );
}

const RECAPTURE: &str = "(io.recapture)(";
const CONSENT_FOR: &str = "consent_for(";

/// Every way the shell and `migrate` fail to wire consent as E-3 orders it.
fn consent_violations(executor: &[SourceFile], migrate: &str) -> Vec<String> {
    let mut violations = Vec::new();
    let locking: Vec<String> = executor
        .iter()
        .map(|(_, t)| code_of(t))
        .filter(|c| !calls_of(c, ACQUIRE).is_empty())
        .collect();
    match locking.as_slice() {
        [code] => {
            let recaptures = calls_of(code, RECAPTURE);
            let holds = calls_of(code, "plan_still_holds(");
            let consent = calls_of(code, CONSENT_FOR);
            let write = calls_of(code, MARKER_WRITE).first().copied();
            let live = code.find("if !io.live");
            match (recaptures.as_slice(), consent.as_slice(), write) {
                ([first, second], [asked], Some(write)) => {
                    let held = |from: usize, to: usize| holds.iter().any(|h| *h > from && *h < to);
                    if !(held(*first, *asked) && asked < second && held(*second, write)) {
                        violations.push(
                            "[order] not: recapture, plan_still_holds, consent_for, recapture, \
                             plan_still_holds, run_marker::write"
                                .to_string(),
                        );
                    }
                    // Each re-plan's verdict is acted on, and the later capture
                    // is the one the run writes (spec review of E-3, 3a-b).
                    let acted = holds
                        .iter()
                        .all(|h| code[..*h].trim_end().ends_with("if let Err(fact) ="));
                    if !acted {
                        violations
                            .push("[holds] a plan_still_holds verdict is not acted on".into());
                    }
                    let replaced = code.find("now = again;");
                    if !replaced.is_some_and(|r| r > *second && r < write) {
                        violations.push(
                            "[replaced] the capture after the answer is not the one written".into(),
                        );
                    }
                    let snapshot = calls_of(code, "take_snapshot(");
                    if snapshot.is_empty() || snapshot.iter().any(|t| t < asked) {
                        violations.push("[order] the snapshot is written before consent".into());
                    }
                    let carried = calls_of(code, DISPATCH).iter().all(|at| {
                        let args = call_arguments(code, *at);
                        args.windows(2).any(|w| w == ["consent", "take"])
                    });
                    if !(code.contains("tokens.take(step.index)") && carried) {
                        violations.push(
                            "[carried] the step's consent token is not taken and handed to the site"
                                .into(),
                        );
                    }
                    if live.is_none_or(|l| l >= *asked) {
                        violations.push("[live] a run that is not live reaches consent".into());
                    }
                    let begin = code[..*asked].rfind([';', '{', '}']).map_or(0, |i| i + 1);
                    let statement = code[begin..*asked].trim();
                    let tail = &code[*asked..];
                    let tail = &tail[..tail.find(';').unwrap_or(tail.len())];
                    if !(statement.starts_with("let ")
                        && !statement.starts_with("let _")
                        && tail.contains('?'))
                    {
                        violations.push(format!(
                            "[kept] consent_for's result is not bound and returned with `?`: {statement:?}"
                        ));
                    }
                }
                _ => violations.push(format!(
                    "[calls] the shell must recapture twice ({}), call consent_for once ({}) and \
                     write the marker",
                    recaptures.len(),
                    consent.len()
                )),
            }
        }
        _ => violations.push(format!("{} files take the lock; one must", locking.len())),
    }
    let migrate = code_of(migrate);
    let body_of = |name: &str| -> String {
        let body = migrate.find(name).map_or("", |i| &migrate[i..]);
        body[..body[1..].find("\nfn ").map_or(body.len(), |i| i + 1)].to_string()
    };
    let ask = body_of("fn ask_for_consent(");
    if calls_of(&ask, "consent::ask(").is_empty() {
        violations.push("[migrate] the consent seam does not call consent::ask".into());
    }
    if !(ask.contains("let mut code = macportsbrew_adapters::entropy::fresh_bytes;")
        && ask.matches("let mut code").count() == 1
        && ask.matches("code =").count() == 1
        && ask.contains("code: &mut code"))
    {
        violations.push("[migrate] the consent seam's code is not fresh_bytes alone".into());
    }
    if !(ask.contains("input_is_terminal: std::io::stdin().is_terminal()")
        && ask.contains("prompt_is_terminal: std::io::stderr().is_terminal()"))
    {
        violations.push("[terminal] the consent seam does not ask the two streams it uses".into());
    }
    let callers = calls_of(&migrate, "ask_for_consent(");
    let execute = body_of("fn execute_mutating(");
    let scope = execute.find("ctx.history != HistoryScope::ThisMachine");
    let seam = calls_of(&execute, "ask_for_consent(");
    match (callers.as_slice(), scope, seam.as_slice()) {
        ([_], Some(scope), [seam]) if scope < *seam => {}
        _ => violations.push(
            "[migrate] the consent seam is not built once, behind the live-scope refusal".into(),
        ),
    }
    violations
}

#[test]
fn consent_violations_fire_on_each_mutant() {
    // The proof the check above would fire, kept as a test (the module's
    // convention): one mutant per rule, made from the real sources.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let migrate =
        std::fs::read_to_string(root.join("crates/cli/src/app/migrate.rs")).expect("migrate.rs");
    let shell = executor_files();
    let edit = |from: &str, to: &str| -> Vec<SourceFile> {
        let mut hit = false;
        let files = shell
            .iter()
            .map(|(n, t)| {
                hit |= t.contains(from);
                (n.clone(), t.replacen(from, to, 1))
            })
            .collect();
        assert!(hit, "the mutant applies: {from}");
        files
    };
    let consent_line =
        "let mut tokens = consent_for(planned, io.consent).map_err(ConsentHalt::error)?;";
    let shell_mutants: [(&str, Vec<SourceFile>); 4] = [
        (
            "kept",
            edit(consent_line, "let _ = consent_for(planned, io.consent);"),
        ),
        (
            "kept",
            edit(
                consent_line,
                "let mut tokens = consent_for(planned, io.consent).unwrap_or_else(|_| todo!());",
            ),
        ),
        (
            "calls",
            edit(
                "let (again, fresh) = (io.recapture)()?;",
                "let (again, fresh) = (now.clone(), fresh.clone());",
            ),
        ),
        ("live", edit("if !io.live {", "if false {")),
    ];
    let shell_mutants = shell_mutants.into_iter().chain([
        (
            "holds",
            edit(
                "if let Err(fact) = plan_still_holds(plan, &fresh, planned.before(), &again) {",
                "let _ = plan_still_holds(plan, &fresh, planned.before(), &again);\n        if false {",
            ),
        ),
        ("replaced", edit("now = again;", "let _ = again;")),
        (
            "carried",
            edit(
                "let mut consent = tokens.take(step.index);",
                "let mut consent = None;",
            ),
        ),
        (
            "carried",
            edit(
                "                        consent.take(),\n",
                "                        None,\n",
            ),
        ),
    ]);
    for (tag, files) in shell_mutants {
        let found = consent_violations(&files, &migrate);
        assert!(
            found.iter().any(|v| v.starts_with(&format!("[{tag}]"))),
            "{tag}: {found:?}"
        );
    }
    for (from, to) in [
        ("crate::consent::ask(", "crate::consent::confirm_plan_never("),
        (
            // A fallback shadowing the draw.
            "let mut prompt = std::io::stderr();\n    let mut code = macportsbrew_adapters::entropy::fresh_bytes;",
            "let mut prompt = std::io::stderr();\n    let mut code = macportsbrew_adapters::entropy::fresh_bytes;\n    let mut code = || code().or(Some([0, 0, 0]));",
        ),
        (
            // The seam's own draw, not the plan question's.
            "let mut prompt = std::io::stderr();\n    let mut code = macportsbrew_adapters::entropy::fresh_bytes;",
            "let mut prompt = std::io::stderr();\n    let mut code = || Some([0u8, 0, 0]);",
        ),
        (
            "let consent = |requests: &[ElevationRequest]| ask_for_consent(ctx, requests);",
            "let consent = |requests: &[ElevationRequest]| ConsentDecision::Given(Vec::new());",
        ),
    ] {
        assert!(migrate.contains(from), "the mutant applies: {from}");
        let found = consent_violations(&shell, &migrate.replacen(from, to, 1));
        assert!(
            found.iter().any(|v| v.starts_with("[migrate]")),
            "{from}: {found:?}"
        );
    }
    // Either stream assumed to be a terminal.
    for stream in ["stdin", "stderr"] {
        let from = format!("std::io::{stream}().is_terminal(),\n            input: &mut input,\n            prompt: &mut prompt,");
        let from = if stream == "stdin" {
            "input_is_terminal: std::io::stdin().is_terminal(),\n            prompt_is_terminal: std::io::stderr().is_terminal(),\n            input: &mut input,\n            prompt: &mut prompt,".to_string()
        } else {
            from
        };
        assert!(migrate.contains(&from), "the mutant applies: {stream}");
        let to = if stream == "stdin" {
            from.replacen("std::io::stdin().is_terminal()", "true", 1)
        } else {
            from.replacen("std::io::stderr().is_terminal()", "true", 1)
        };
        let found = consent_violations(&shell, &migrate.replacen(&from, &to, 1));
        assert!(
            found.iter().any(|v| v.starts_with("[terminal]")),
            "{stream}: {found:?}"
        );
    }
    // And the move of consent past the marker write: the order fires.
    let moved: Vec<SourceFile> = shell
        .iter()
        .map(|(n, t)| {
            let t = t.replacen(consent_line, "let mut tokens = ConsentTokensLater;", 1);
            let t = t.replacen(
                "run_marker::write(&lock, state, &named, euid)?;",
                &format!("run_marker::write(&lock, state, &named, euid)?;\n{consent_line}"),
                1,
            );
            (n.clone(), t)
        })
        .collect();
    assert!(consent_violations(&moved, &migrate)
        .iter()
        .any(|v| v.starts_with("[order]")));
    // The snapshot written before the question: the order fires too.
    let early: Vec<SourceFile> = shell
        .iter()
        .map(|(n, t)| {
            let t = t.replacen(
                consent_line,
                &format!("let _early = take_snapshot(state, &now, euid);\n    {consent_line}"),
                1,
            );
            (n.clone(), t)
        })
        .collect();
    assert!(consent_violations(&early, &migrate)
        .iter()
        .any(|v| v == "[order] the snapshot is written before consent"));
}

/// `doctor` shows a stranded marker (pass 08 slice 5, D12). A positive check,
/// held whatever the switch is: the finding exists, the live `doctor` path asks
/// for the facts, and the facts come from looking at the marker and the lock. A
/// `doctor` that always passed `NOT_CHECKED` would contain the finding's name and
/// satisfy a search for it, so the calls are what is looked for.
#[test]
fn the_switch_needs_doctor_to_show_a_stranded_marker() {
    // Raw text: `code_of` blanks string literals, and the id is one.
    let finding = workspace_file("application/src/doctor.rs");
    let command = code_of(&workspace_file("cli/src/app/mod.rs"));
    let facts = code_of(&workspace_file("cli/src/app/interrupted.rs"));
    let squeeze = |text: &str| text.split_whitespace().collect::<String>();
    let (finding, command, facts) = (squeeze(&finding), squeeze(&command), squeeze(&facts));
    assert!(
        finding.contains("id:\"interrupted_run\".into()"),
        "doctor.rs has no `interrupted_run` finding"
    );
    assert!(
        command.contains(".with_run_finding(&interrupted::gather(&ctx))"),
        "the live doctor command does not add the interrupted-run finding from a look at the state directory"
    );
    assert!(
        facts.contains("run_marker::peek(state,euid)")
            && facts.contains("run_lock::probe(state,euid)"),
        "the facts handed to doctor do not come from the marker and the run lock"
    );
}

/// Still an unconditional failure while the switch is on. Since pass 08 slice 6
/// the planner raises `MANUAL_REVIEW` for a destination recorded as
/// unavailable, unsupported or disabled (tested in
/// `crates/application/tests/invariants.rs`), but the 04 MUST is still unmet in
/// every case a real snapshot reaches: no adapter reports those states, a
/// restore's destination row is `UNKNOWN`, a source build is DP-050's, and
/// `history` reports nothing as partially restorable. Declared in 04, SE-092
/// and SE-107. Turning this into a positive check while those stand would make
/// the gap invisible.
#[test]
fn the_switch_needs_manual_review_for_an_unavailable_artifact() {
    let switch = THIS_BUILD_DISPATCHES_MUTATIONS;
    assert!(
        !switch,
        "04 (Undoing a migration) requires a step whose artifact is unavailable to become \
         MANUAL_REVIEW; the planner raises it only for a destination recorded as unavailable, \
         which no adapter reports and a restore's UNKNOWN row is not. Remove this only when \
         the clauses in 04's declared deviation are met or the owner accepts them \
         (SE-092 slice 5 D8, SE-107)"
    );
}

/// A positive structural check, held whatever the switch is (work order 16
/// part E, §6, E-2): the run loop refuses a plan the confirmation does not
/// cover before it probes the history store or takes the lock; `migrate`
/// confirms the plan after the gate and before the state directory, and hands
/// the witness to the loop; and a `ShownPlan` is built in one file only. Until
/// part E-4 one assertion still fails on a flip: DP-049 ruled both routes in,
/// and the digest route (a) is not built.
#[test]
fn the_switch_needs_the_executed_plan_tied_to_the_one_shown() {
    let violations = tie_violations(&executor_files());
    assert!(violations.is_empty(), "{violations:?}");
    let switch = THIS_BUILD_DISPATCHES_MUTATIONS;
    assert!(
        !switch,
        "DP-049 (a), `migrate --yes --plan-digest <d>`, is not built (work order 16 part E-4); \
         remove this assertion in the change that builds it"
    );
}

fn tie_violations(executor: &[SourceFile]) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut violations = Vec::new();
    let locking: Vec<String> = executor
        .iter()
        .map(|(_, t)| code_of(t))
        .filter(|c| c.contains("history_store::probe("))
        .collect();
    match locking.as_slice() {
        [code] => match (
            code.find("io.shown.covers("),
            code.find("history_store::probe("),
        ) {
            (Some(tie), Some(probe)) if tie < probe => {}
            _ => violations.push("the tie is not checked before the history probe".to_string()),
        },
        _ => violations.push(format!(
            "{} files probe the history store; one must",
            locking.len()
        )),
    }
    let migrate = code_of(
        &std::fs::read_to_string(root.join("crates/cli/src/app/migrate.rs")).expect("migrate.rs"),
    );
    let body = migrate
        .find("fn execute_mutating(")
        .map_or("", |i| &migrate[i..]);
    let body = &body[..body.find("\nfn ").unwrap_or(body.len())];
    let order = ["clear(planned", "confirm_the_plan(", "state_and_user("];
    let at: Vec<Option<usize>> = order.iter().map(|t| body.find(t)).collect();
    if !at
        .windows(2)
        .all(|w| matches!(w, [Some(a), Some(b)] if a < b))
    {
        violations.push(format!(
            "migrate does not run {order:?} in that order: {at:?}"
        ));
    }
    if !migrate.contains("crate::consent::confirm_plan(") {
        violations.push("migrate does not ask through consent::confirm_plan".to_string());
    }
    match body.find("executor::MutatingRun {") {
        Some(i) => {
            let rest = &body[i..];
            let literal = &rest[..rest.find("&ctx.redaction").unwrap_or(rest.len())];
            if !literal.contains("shown") {
                violations.push("migrate's MutatingRun carries no shown plan".to_string());
            }
        }
        None => violations.push("migrate builds no MutatingRun".to_string()),
    }
    let mut builders = Vec::new();
    for krate in ["domain", "policy", "application", "adapters", "cli"] {
        let mut stack = vec![root.join("crates").join(krate).join("src")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("sources") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let code = code_of(&std::fs::read_to_string(&path).expect("source"));
                    if code
                        .replace("pub struct ShownPlan {", "")
                        .contains("ShownPlan {")
                    {
                        builders.push(path.display().to_string());
                    }
                }
            }
        }
    }
    if builders.len() != 1 || !builders[0].ends_with("application/src/plan_tie.rs") {
        violations.push(format!(
            "a ShownPlan is built outside plan_tie.rs: {builders:?}"
        ));
    }
    violations
}

/// A positive structural check, held whatever the switch is (work order 16
/// part S, §8; it replaces `the_switch_needs_the_window_before_removal_closed_or_accepted`):
/// in the shell's `DispatchMutating` handling, the branch taken when the
/// application names a re-read (`reverify: Some(..)`) calls
/// `destination_still_verified(` before `verify_still_current(` and before
/// `mutation_dispatch(`. The shell has no removal arm of its own: which step is
/// re-read is the application's `reverify_for`. And 04 declares the residual
/// DP-048 accepts, after that re-read.
#[test]
fn the_destination_is_reread_before_removal() {
    let violations = reread_violations(&executor_files());
    assert!(violations.is_empty(), "{violations:?}");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let spec = std::fs::read_to_string(root.join("docs/spec/04-migration-and-safety.md"))
        .expect("04 reads");
    assert!(
        spec.contains("**The residual removal window is accepted and declared (DP-048"),
        "04 no longer declares the residual window after the re-read (DP-048)"
    );
}

/// What is wrong with the re-read before removal in `files`.
fn reread_violations(files: &[SourceFile]) -> Vec<String> {
    let mut violations = Vec::new();
    let codes: Vec<(String, String)> = files
        .iter()
        .map(|(name, text)| (name.clone(), code_of(text)))
        .collect();
    let arms: Vec<&(String, String)> = codes
        .iter()
        .filter(|(_, c)| c.contains("Action::DispatchMutating { reverify }"))
        .collect();
    let [(name, code)] = arms.as_slice() else {
        return vec![format!(
            "{} files handle `Action::DispatchMutating {{ reverify }}`; one must",
            arms.len()
        )];
    };
    let arm = code
        .find("Action::DispatchMutating { reverify }")
        .unwrap_or(0);
    let rest = &code[arm..];
    let at = |token: &str| rest.find(token);
    // Per command (mutant 35): the re-read is inside the loop over the step's
    // commands, so a step with two removal commands re-reads twice.
    match (
        at("for command in &step.commands"),
        at("if let Some(reverify) = reverify"),
    ) {
        (Some(each), Some(branch)) if each < branch => {}
        _ => violations.push(format!(
            "{name}: the re-read must be inside the loop over the step's commands"
        )),
    }
    let (Some(branch), Some(reverified), Some(current), Some(dispatch)) = (
        at("if let Some(reverify) = reverify"),
        at("destination_still_verified("),
        at("verify_still_current("),
        at("mutation_dispatch("),
    ) else {
        return vec![format!(
            "{name}: the reverify branch, destination_still_verified(, verify_still_current( \
             and mutation_dispatch( must all follow the DispatchMutating arm"
        )];
    };
    if branch >= reverified {
        violations.push(format!(
            "{name}: the re-read is not in the `reverify` branch"
        ));
    }
    if reverified >= current {
        violations.push(format!(
            "{name}: destination_still_verified( must come before verify_still_current( (M1)"
        ));
    }
    if reverified >= dispatch {
        violations.push(format!(
            "{name}: destination_still_verified( must come before mutation_dispatch("
        ));
    }
    violations
}

#[test]
fn the_reread_tripwire_fires_on_its_mutants() {
    let good = "fn f() { match a { Action::DispatchMutating { reverify } => { \
                for command in &step.commands { \
                if let Some(reverify) = reverify { destination_still_verified(x); } \
                lock.verify_still_current(); mutation_dispatch(y); } } } }";
    assert!(reread_violations(&one(good)).is_empty());
    let dropped = good.replace("destination_still_verified(x);", "");
    assert!(!reread_violations(&one(&dropped)).is_empty(), "dropped");
    let late = "fn f() { match a { Action::DispatchMutating { reverify } => { \
                lock.verify_still_current(); \
                if let Some(reverify) = reverify { destination_still_verified(x); } \
                mutation_dispatch(y); } } }";
    assert!(!reread_violations(&one(late)).is_empty(), "after the lock");
    let after = "fn f() { match a { Action::DispatchMutating { reverify } => { \
                lock.verify_still_current(); mutation_dispatch(y); \
                if let Some(reverify) = reverify { destination_still_verified(x); } } } }";
    assert!(
        !reread_violations(&one(after)).is_empty(),
        "after the dispatch"
    );
    let unconditional = "fn f() { match a { Action::DispatchMutating { reverify } => { \
                destination_still_verified(x); \
                lock.verify_still_current(); mutation_dispatch(y); } } }";
    let once_per_step = "fn f() { match a { Action::DispatchMutating { reverify } => { \
                if let Some(reverify) = reverify { destination_still_verified(x); } \
                for command in &step.commands { \
                lock.verify_still_current(); mutation_dispatch(y); } } } }";
    assert!(
        !reread_violations(&one(once_per_step)).is_empty(),
        "once per step, not per command"
    );
    assert!(
        !reread_violations(&one(unconditional)).is_empty(),
        "no branch"
    );
}

/// The shell never derives a command: which commands the destination is read
/// again with is the plan's `VERIFY_DESTINATION`, named by the application,
/// never the command table (work order 16 part S, V4, L5; mutant 18).
#[test]
fn the_shell_never_derives_a_command() {
    for (name, text) in executor_files() {
        let code = code_of(&text);
        for word in [
            "planned_argv",
            "provider_commands",
            "verify_destination_reads",
        ] {
            assert!(!code.contains(word), "{name} names {word}");
        }
    }
}

/// Fails while the switch is on (work order 16 part S, V3): removal is not yet
/// opt-in. DP-048 (c) bounds the residual window by making removal opt-in
/// (`--remove-source`, with a confirmation stronger than an install's) and by
/// dispatching `REMOVE_SOURCE` only when the re-read ran, DP-030's probe
/// answered and all six destination checks were evaluated. It replaces slice 6
/// part D's planned flip tripwire; work order 16 part K turns it positive.
#[test]
fn the_switch_needs_removal_opt_in() {
    let switch = THIS_BUILD_DISPATCHES_MUTATIONS;
    assert!(
        !switch,
        "removal is not opt-in: DP-048 (c) requires --remove-source with a confirmation \
         stronger than an install's, and REMOVE_SOURCE dispatched only when the re-read ran, \
         DP-030's probe answered and all six destination checks were evaluated; work order 16 \
         part K builds it (SE-107)"
    );
}

/// Detection: a release is built without the `test-support` feature (slice 4
/// review L1), which compiles the test-only mutation allowance in. A positive
/// check, held whatever the switch is. It looks for the check's structure, not
/// its words: `build-release.sh` asks cargo what the release resolves, for the
/// target, over the edges a release build uses, with the exact command line
/// below, and refuses when `test-support` is among it, when cargo cannot answer,
/// and when a compiler wrapper could add the cfg unseen; `check-release-
/// workflow.sh` refuses cargo and a feature flag in the release workflow; and
/// the release workflow builds only through `build-release.sh`. The behaviour is
/// proved by constructed cases in `scripts/test-gates.sh`, which also pin the
/// arguments cargo is given; this test is that the check cannot be deleted or
/// narrowed without a test failing here.
#[test]
fn the_switch_needs_release_builds_free_of_the_test_support_feature() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    };
    // Whitespace-free, so a re-wrapped line does not hide the check.
    let squeeze = |text: &str| text.split_whitespace().collect::<String>();
    let script = squeeze(&read("scripts/build-release.sh"));
    let check = squeeze(&read("scripts/check-release-workflow.sh"));
    let workflow = read(".github/workflows/release.yml");

    for (what, needle) in [
        (
            "it asks cargo the whole question, and nothing narrower",
            r#"graph="$(cargotree--locked-pmacportsbrew--target"$target"\--prefixnone-enormal,build-f'{p}{f}'2>&1)""#,
        ),
        (
            "it matches the feature as a whole word, here-string not pipeline",
            r#"grep-E'(^|[,])test-support([,]|$)'<<<"$graph"||true"#,
        ),
        (
            "refusing when it resolves",
            "test-supportfeatureresolvesintothisbuild",
        ),
        (
            "refusing when cargo cannot answer",
            "cargotreefailed;cannotshowthatthebuildisfree",
        ),
        (
            "refusing a compiler wrapper in the environment",
            "forvarinRUSTCRUSTC_WRAPPERRUSTC_WORKSPACE_WRAPPER",
        ),
        (
            "refusing a compiler wrapper in cargo's configuration",
            "rustc-wrapper|rustc-workspace-wrapper",
        ),
    ] {
        assert!(
            script.contains(needle),
            "scripts/build-release.sh: {what}: `{needle}` is gone (slice 4 review L1)"
        );
    }
    for (what, needle) in [
        (
            "cargo anywhere in the release workflow",
            "release.ymlnamescargo",
        ),
        ("a long feature flag anywhere in it", "--(all-)?features"),
        ("-F on a call to the release build", "build-release"),
    ] {
        assert!(
            check.contains(needle),
            "scripts/check-release-workflow.sh no longer refuses {what}"
        );
    }
    let names_cargo = workflow.lines().any(|l| {
        let l = l.trim_start();
        !l.starts_with('#')
            && l.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                .any(|w| w == "cargo")
    });
    assert!(
        !names_cargo && workflow.contains("scripts/build-release.sh"),
        "release.yml must build only through scripts/build-release.sh, so the check applies"
    );
}

// `the_switch_needs_the_destination_contract_evaluable` lives in
// `crates/cli/tests/executor.rs`, beside the in-code snapshots it plans from
// in each direction (work order 16 part S, §8): the recordings do not record
// whether MacPorts packages define services, so the planner refuses to plan
// from them toward Homebrew.
