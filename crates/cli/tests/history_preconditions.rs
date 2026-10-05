// SPDX-License-Identifier: Apache-2.0

//! Preconditions the history store puts on the switch that turns mutations
//! on (pass 08 slice 1; SE-092). Each is a tripwire: it holds trivially while
//! `THIS_BUILD_DISPATCHES_MUTATIONS` is `false`, and fails the build that
//! flips it without wiring what a mutating run needs first — so the
//! precondition is a gate, not a checklist item.

use macportsbrew_domain::plan::THIS_BUILD_DISPATCHES_MUTATIONS;
use std::path::Path;

/// Every Rust source under the executor shell, as one text.
fn executor_shell() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/executor");
    let mut text = String::new();
    let mut stack = vec![dir];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).expect("the executor shell") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                text.push_str(&std::fs::read_to_string(&path).expect("source"));
            }
        }
    }
    assert!(!text.is_empty(), "the executor shell was read");
    text
}

#[test]
fn mutations_cannot_dispatch_before_the_history_store_is_probed() {
    // A mutating run must prove it can record itself before it changes
    // anything (04, History; security review of the slice, M1).
    if THIS_BUILD_DISPATCHES_MUTATIONS {
        assert!(
            executor_shell().contains("history_store::probe("),
            "mutations dispatch, and the executor shell never probes the history store"
        );
    }
}

#[test]
fn mutations_cannot_dispatch_before_a_record_is_written() {
    // Obligation 12: a mutating run writes its record on every way it ends.
    if THIS_BUILD_DISPATCHES_MUTATIONS {
        assert!(
            executor_shell().contains("history_store::write_record("),
            "mutations dispatch, and the executor shell never writes a record"
        );
    }
}
