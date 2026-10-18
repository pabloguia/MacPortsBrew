// SPDX-License-Identifier: Apache-2.0

//! The interrupted-run marker, as the reading commands see it (pass 08
//! slice 5, Part D; D12, D13, D14): what `doctor` reports, and the one explicit
//! way to deal with a marker, `history acknowledge <id> --yes`.
//!
//! **Reads without the lock and creates nothing**, except `acknowledge --yes`,
//! which takes the run lock because it removes a marker. The marker is how a
//! record is known *not* to be confirmed, so it is never removed for a record
//! that cannot be made durable and read back, and never for a record that cannot
//! be read: that fails closed with the marker in place.
//!
//! The three states of a stranded run are described by one catalogue text each
//! (`doctor.interrupted_run_*`), used by `doctor`, `history show` and
//! `acknowledge` alike.

use super::context::{Context, HistoryScope};
use super::history::{identifier, Shown};
use super::json;
use super::migrate::state_and_user;
use macportsbrew_adapters::persistence::history_store;
use macportsbrew_adapters::persistence::run_lock::{self, LockProbe};
use macportsbrew_adapters::persistence::run_marker::{self, ClearOutcome};
use macportsbrew_adapters::persistence::snapshot_store::{state_snapshot_file, SnapshotFile};
use macportsbrew_adapters::persistence::state_dir::StateDir;
use macportsbrew_application::{interrupted_run_finding, RunFacts, StrandedRecord};
use macportsbrew_domain::{DomainError, ErrorId, InterruptedRun, RunRecord};
use serde::Serialize;
use std::io::Write;

/// What `doctor` is told about interrupted runs. Looks at the state directory
/// without taking the lock and without creating anything; a run that is not
/// this machine's (a recording, a snapshot file) is not looked for.
pub(super) fn gather(ctx: &Context) -> RunFacts {
    if ctx.history != HistoryScope::ThisMachine {
        return RunFacts::NotChecked;
    }
    let Ok((state, euid)) = state_and_user(ctx) else {
        return RunFacts::NotChecked;
    };
    facts_of(&state, euid)
}

/// [`gather`] over a state directory already resolved.
pub(super) fn facts_of(state: &StateDir, euid: u32) -> RunFacts {
    match run_marker::peek(state, euid) {
        Err(_) => RunFacts::MarkerUnreadable,
        Ok(None) => RunFacts::NoMarker,
        Ok(Some(marker)) => match run_lock::probe(state, euid) {
            LockProbe::Held => RunFacts::InProgress,
            LockProbe::Untestable => RunFacts::PossiblyInProgress,
            LockProbe::Free => stranded(state, euid, &marker),
        },
    }
}

/// A marker nobody holds the lock for, with what is known of its record.
pub(super) fn stranded(state: &StateDir, euid: u32, marker: &InterruptedRun) -> RunFacts {
    let record = match history_store::read_one(state, marker.run_id(), euid) {
        Ok(None) => StrandedRecord::None,
        Ok(Some(record)) => StrandedRecord::Unconfirmed {
            changed: changed_steps(&record),
        },
        Err(_) => StrandedRecord::Unreadable,
    };
    RunFacts::Stranded {
        run_id: marker.run_id().to_owned(),
        snapshot_file: marker.snapshot_file().map(str::to_owned),
        record,
    }
}

/// The mutating steps the run dispatched, from its record, by identifier.
pub(super) fn changed_steps(record: &RunRecord) -> Vec<String> {
    record
        .steps()
        .iter()
        .filter(|step| step.kind().is_mutating() && step.dispatched())
        .map(|step| identifier(&step.kind()))
        .collect()
}

/// The description of a stranded run's state: the `doctor` finding's own text,
/// so every command says the same thing.
pub(super) fn describe(text: &Shown<'_>, facts: &RunFacts) -> String {
    let finding = interrupted_run_finding(facts);
    let params = finding
        .reason
        .params
        .iter()
        .map(|(name, value)| (name.clone(), text.plain(value)))
        .collect();
    text.messages.render(finding.reason.id.as_str(), &params)
}

/// What the marker's record is, as `history-acknowledge.v1` names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RecordState {
    /// The run began and left no record.
    None,
    /// A record exists, reads, and the marker still names it.
    Unconfirmed,
    /// Something is at the record's id that cannot be read as a record.
    Unreadable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Outcome {
    /// No marker is there.
    NothingToAcknowledge,
    /// Without `--yes`: what acknowledging would do. Nothing was changed.
    WouldAcknowledge,
    /// The marker was removed.
    Acknowledged,
}

/// `history-acknowledge.v1`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    schema_version: u32,
    run_id: &'a str,
    outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<RecordState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_file: Option<&'a str>,
}

fn record_state(facts: &RunFacts) -> Option<RecordState> {
    match facts {
        RunFacts::Stranded { record, .. } => Some(match record {
            StrandedRecord::None => RecordState::None,
            StrandedRecord::Unconfirmed { .. } => RecordState::Unconfirmed,
            StrandedRecord::Unreadable => RecordState::Unreadable,
        }),
        _ => None,
    }
}

/// `history acknowledge <id>`: with `execute` false, what it would do; with it
/// true, the one deliberate removal of an interrupted-run marker.
///
/// Order: nothing is created before the marker is seen; no marker is "nothing to
/// acknowledge" (exit 0, INV-16); a marker naming another run is refused (exit 4)
/// naming both; the lock is taken before anything is removed (held: exit 7); the
/// marker is read again under it; the record is made durable and read back
/// **before** the marker goes, and a record that cannot be read keeps the marker.
pub(super) fn acknowledge(
    state: &StateDir,
    euid: u32,
    id: &str,
    execute: bool,
    json_out: bool,
    text: &Shown<'_>,
    stdout: &mut dyn Write,
) -> Result<i32, DomainError> {
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    let Some(marker) = run_marker::peek(state, euid)? else {
        return nothing(id, json_out, text, stdout);
    };
    refuse_other_run(&marker, id)?;
    let facts = match run_lock::probe(state, euid) {
        // A run holding the lock is not stranded; leave it alone.
        LockProbe::Held => {
            return Err(DomainError::new(
                ErrorId::RunLockHeld,
                "the run lock is held, so a run is in progress and its marker is not stranded",
            ))
        }
        _ => stranded(state, euid, &marker),
    };
    let record = record_state(&facts);
    if !execute {
        if json_out {
            let document = Document {
                schema_version: 1,
                run_id: id,
                outcome: Outcome::WouldAcknowledge,
                record,
                snapshot_file: marker.snapshot_file(),
            };
            writeln!(stdout, "{}", json(&document)?).map_err(io)?;
        } else {
            writeln!(stdout, "{}", describe(text, &facts)).map_err(io)?;
            let would = match record {
                Some(RecordState::Unconfirmed) => "history.ack_would_recorded",
                Some(RecordState::Unreadable) => "history.ack_would_unreadable",
                _ => "history.ack_would_not_recorded",
            };
            writeln!(stdout, "{}", text.say(would, &[("run_id", id.to_owned())])).map_err(io)?;
            // The snapshot the marker names is part of the only trace of a run
            // that left no record: say whether it is still there.
            let look = match marker.snapshot_file() {
                None => text.messages.render_id("history.ack_snapshot_none"),
                Some(name) => {
                    let key = match state_snapshot_file(state, name, euid) {
                        SnapshotFile::Present => "history.ack_snapshot_present",
                        SnapshotFile::Missing => "history.ack_snapshot_missing",
                        SnapshotFile::Unreadable(_) => "history.ack_snapshot_unreadable",
                    };
                    text.say(key, &[("snapshot", text.plain(name))])
                }
            };
            writeln!(stdout, "{look}").map_err(io)?;
        }
        return Ok(0);
    }
    let lock = run_lock::acquire(state, euid)?;
    // Read again under the lock: what was seen without it may have changed.
    let Some(held) = run_marker::read(&lock, state, euid)? else {
        return nothing(id, json_out, text, stdout);
    };
    refuse_other_run(&held, id)?;
    match history_store::read_one(state, id, euid) {
        // A record that cannot be read is not promoted, and the marker that says
        // so stays (fail closed). The way out is named: slice 5 review, M2.
        Err(e) => {
            return Err(DomainError::new(
                e.id,
                format!(
                    "{}; the marker was left in place. If the record cannot be repaired, move \
                     that file out of the history directory (this tool deletes nothing) and \
                     acknowledge again",
                    e.detail
                ),
            ))
        }
        Ok(Some(_)) => {
            history_store::make_durable(state, id, euid)?;
        }
        Ok(None) => {}
    }
    match run_marker::clear(&lock, state, &held, euid)? {
        ClearOutcome::Cleared | ClearOutcome::AlreadyAbsent => {}
        ClearOutcome::NotOurs => {
            return Err(DomainError::new(
                ErrorId::SafetyBlocked,
                "the marker changed to another run's while it was being acknowledged and was left alone",
            ))
        }
    }
    if json_out {
        let document = Document {
            schema_version: 1,
            run_id: id,
            outcome: Outcome::Acknowledged,
            record,
            snapshot_file: held.snapshot_file(),
        };
        writeln!(stdout, "{}", json(&document)?).map_err(io)?;
    } else {
        let done = if record == Some(RecordState::None) {
            "history.ack_done_not_recorded"
        } else {
            "history.ack_done_recorded"
        };
        writeln!(
            stdout,
            "{}",
            text.say(
                done,
                &[
                    ("run_id", id.to_owned()),
                    (
                        "snapshot",
                        held.snapshot_file().unwrap_or("none").to_owned()
                    ),
                ],
            )
        )
        .map_err(io)?;
    }
    Ok(0)
}

fn refuse_other_run(marker: &InterruptedRun, id: &str) -> Result<(), DomainError> {
    if marker.run_id() == id {
        return Ok(());
    }
    Err(DomainError::new(
        ErrorId::SafetyBlocked,
        format!(
            "the interrupted-run marker names run {}, not {id}; `macportsbrew history` shows \
             the runs, and only the run the marker names can be acknowledged",
            marker.run_id()
        ),
    ))
}

fn nothing(
    id: &str,
    json_out: bool,
    text: &Shown<'_>,
    stdout: &mut dyn Write,
) -> Result<i32, DomainError> {
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    if json_out {
        let document = Document {
            schema_version: 1,
            run_id: id,
            outcome: Outcome::NothingToAcknowledge,
            record: None,
            snapshot_file: None,
        };
        writeln!(stdout, "{}", json(&document)?).map_err(io)?;
    } else {
        writeln!(
            stdout,
            "{}",
            text.say("history.ack_nothing", &[("run_id", id.to_owned())])
        )
        .map_err(io)?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use macportsbrew_adapters::persistence::history_store::{record_file_name, write_record};
    use macportsbrew_adapters::persistence::run_lock::acquire;
    use macportsbrew_domain::{
        CommandEnd, Direction, Outcome, PackageIdentity, ProviderId, RecordedStep,
        RecordedVersions, RunEnding, RunStart, RunSubject, StepKind, StepResult, Version,
    };

    const ID: &str = "20261003T120000Z-00000000000000aa";
    const SNAPSHOT: &str = "snapshot-20261003T120000Z.json";

    fn scratch(tag: &str) -> (std::path::PathBuf, StateDir, u32) {
        let base =
            std::env::temp_dir().join(format!("mpb-interrupted-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let state = StateDir::resolve(None, Some(base.join("state").to_str().unwrap())).unwrap();
        let euid = macportsbrew_adapters::system::machine::effective_uid().unwrap();
        (base, state, euid)
    }

    fn record() -> RunRecord {
        let step = |index, kind, result| RecordedStep::new(index, kind, None, result, false);
        RunRecord::new(
            RunStart {
                id: ID.into(),
                started_at: "2026-10-03T12:00:00Z".into(),
                tool_version: "0.1.0".into(),
            },
            RunSubject {
                plan_schema_version: 1,
                policy_version: "test".into(),
                package: PackageIdentity::new(ProviderId::new("a"), "pkg"),
                direction: Direction {
                    source: ProviderId::new("a"),
                    destination: ProviderId::new("b"),
                },
                destination_package: None,
                versions: RecordedVersions {
                    source: Some(Version::new("1.0")),
                    destination: None,
                },
                action: Outcome::MigrateTo {
                    provider: ProviderId::new("b"),
                },
                overrode: None,
                retain_source: true,
                primary_after: ProviderId::new("a"),
            },
            vec![
                step(0, StepKind::SnapshotBefore, StepResult::SnapshotTaken),
                step(
                    1,
                    StepKind::AddDestination,
                    StepResult::Performed {
                        ends: vec![CommandEnd::Exited { code: 0 }],
                    },
                ),
                step(
                    2,
                    StepKind::RecordHistory,
                    StepResult::RecordPendingReadBack,
                ),
            ],
            RunEnding::Completed,
            Some(SNAPSHOT.into()),
        )
    }

    /// Leaves a marker for `ID` and releases the lock, as a dead run does.
    fn strand(state: &StateDir, euid: u32) {
        let lock = acquire(state, euid).unwrap();
        let marker = InterruptedRun::new(ID)
            .unwrap()
            .with_snapshot(SNAPSHOT)
            .unwrap();
        run_marker::write(&lock, state, &marker, euid).unwrap();
    }

    #[test]
    fn with_no_marker_nothing_is_found_and_nothing_is_created() {
        let (base, state, euid) = scratch("none");
        assert_eq!(facts_of(&state, euid), RunFacts::NoMarker);
        assert!(!state.root().exists());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_nobody_holds_the_lock_for_is_stranded_in_one_of_three_ways() {
        let (base, state, euid) = scratch("stranded");
        strand(&state, euid);
        let expect = |record| RunFacts::Stranded {
            run_id: ID.into(),
            snapshot_file: Some(SNAPSHOT.into()),
            record,
        };
        assert_eq!(facts_of(&state, euid), expect(StrandedRecord::None));
        write_record(&state, &record(), euid).unwrap();
        assert_eq!(
            facts_of(&state, euid),
            expect(StrandedRecord::Unconfirmed {
                changed: vec!["ADD_DESTINATION".into()],
            })
        );
        let file = state.history().join(record_file_name(ID).unwrap());
        std::fs::write(&file, "{ not a record").unwrap();
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();
        assert_eq!(facts_of(&state, euid), expect(StrandedRecord::Unreadable));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_held_lock_is_a_run_in_progress_and_an_untestable_one_is_said_to_be_possible() {
        let (base, state, euid) = scratch("progress");
        let lock = acquire(&state, euid).unwrap();
        let marker = InterruptedRun::new(ID).unwrap();
        run_marker::write(&lock, &state, &marker, euid).unwrap();
        assert_eq!(facts_of(&state, euid), RunFacts::InProgress);
        drop(lock);
        std::fs::remove_file(state.root().join(run_lock::LOCK_FILE)).unwrap();
        assert_eq!(facts_of(&state, euid), RunFacts::PossiblyInProgress);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_marker_that_cannot_be_read_is_not_taken_for_none() {
        let (base, state, euid) = scratch("unreadable");
        state.ensure().unwrap();
        let file = state.root().join(run_marker::MARKER_FILE);
        std::fs::write(&file, "not a marker").unwrap();
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();
        assert_eq!(facts_of(&state, euid), RunFacts::MarkerUnreadable);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn making_a_record_durable_returns_it_unchanged_and_refuses_what_is_not_one() {
        let (base, state, euid) = scratch("durable");
        // Not there.
        assert_eq!(
            history_store::make_durable(&state, ID, euid)
                .unwrap_err()
                .id,
            ErrorId::HistoryUnusable
        );
        write_record(&state, &record(), euid).unwrap();
        let made = history_store::make_durable(&state, ID, euid).unwrap();
        assert_eq!(made, record());
        // Something else at the id is never promoted.
        let file = state.history().join(record_file_name(ID).unwrap());
        std::fs::write(&file, "{ not a record").unwrap();
        std::fs::set_permissions(&file, std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .unwrap();
        assert_eq!(
            history_store::make_durable(&state, ID, euid)
                .unwrap_err()
                .id,
            ErrorId::HistoryUnusable
        );
        std::fs::remove_dir_all(&base).ok();
    }
}
