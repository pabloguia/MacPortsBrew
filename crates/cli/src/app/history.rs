// SPDX-License-Identifier: Apache-2.0

//! `history` and `history show <id>`: what MacPortsBrew's own state directory
//! says about the runs it made (pass 08 slice 5, Part B; D7, D8).
//!
//! **Reads, and creates nothing.** Neither command builds the capture the
//! analysing commands build: there is no provider to ask, so there is no
//! `Context`; the catalogue, the home and the state directory are resolved here
//! from the same process facts. Refused first - before the state directory is
//! resolved - under `--replay` and `--from-snapshot` (INV-15: history is this
//! machine's, now; a variable naming a store must not make a recording read the
//! real one). The root refusal (INV-06) is `run_inner`'s, before this runs.

use super::{json, Cli};
use crate::executor::escape_controls;
use crate::i18n::{resolve, Messages};
use macportsbrew_adapters::persistence::history_store::{self, RecordList};
use macportsbrew_adapters::persistence::run_marker;
use macportsbrew_adapters::persistence::snapshot_store::{
    state_snapshot, state_snapshot_file, SnapshotFile, StateSnapshot,
};
use macportsbrew_adapters::persistence::state_dir::StateDir;
use macportsbrew_adapters::system::machine::{
    effective_uid, from_process_environment, state_dir_override,
};
use macportsbrew_application::{restorability, SnapshotLook};
use macportsbrew_domain::{
    is_run_id, redact_home, DomainError, ErrorId, HomeRedaction, MarkerState, NotRestorable,
    RecordStanding, Restorability, RunRecord, RunStatus, StoredEnvelope,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;

/// How many runs `history` shows when `--limit` is not given.
pub(super) const DEFAULT_LIMIT: u16 = 20;
/// The most `--limit` accepts: a larger page is a usage error, not a long read.
pub(super) const MAX_LIMIT: u16 = 200;

/// What `history` was asked.
pub(super) enum Request<'a> {
    /// The newest `limit` runs.
    List { limit: usize },
    /// One run, by id.
    Show { id: &'a str },
    /// Deal with the interrupted-run marker of one run; with `execute`, remove
    /// it (`history acknowledge <id> --yes`).
    Acknowledge { id: &'a str, execute: bool },
}

/// The one entry point for both commands.
pub(super) fn run(
    cli: &Cli,
    request: &Request<'_>,
    stdout: &mut dyn Write,
) -> Result<i32, DomainError> {
    // First, before the state directory is resolved (INV-15).
    if cli.replay.is_some() || cli.from_snapshot.is_some() {
        return Err(DomainError::new(
            ErrorId::SafetyBlocked,
            "history is this machine's own record of its own runs, so it is not read for a \
             recording or a snapshot file; nothing was read",
        ));
    }
    // A run id that is not one never becomes a path.
    if let Request::Show { id } | Request::Acknowledge { id, .. } = request {
        if !is_run_id(id) {
            return Err(DomainError::new(
                ErrorId::Usage,
                "that is not a run id: they look like 20260101T000000Z-0123456789abcdef, and \
                 `macportsbrew history` lists them",
            ));
        }
    }
    let messages = Messages::for_locale(&resolve(
        cli.language.as_deref(),
        super::context::env_language().as_deref(),
    ));
    let (_, _, home) = from_process_environment();
    let euid = effective_uid().ok_or_else(|| {
        DomainError::new(
            ErrorId::SafetyBlocked,
            "who this process runs as could not be established, so its history cannot be \
             read and nothing was read",
        )
    })?;
    let state = StateDir::resolve(home.as_deref(), state_dir_override().as_deref())?;
    let shown = Shown {
        messages: &messages,
        redaction: HomeRedaction::live(home),
    };
    let io = |e: std::io::Error| DomainError::new(ErrorId::Io, e.to_string());
    match request {
        Request::List { limit } => {
            let list = history_store::list_records(&state, euid, *limit)?;
            let marker = run_marker::marker_state(&state, euid);
            let document = list_document(&state, euid, &list, &marker, *limit);
            if cli.json {
                writeln!(stdout, "{}", json(&document)?).map_err(io)?;
            } else {
                write!(stdout, "{}", shown.list(&state, &document)).map_err(io)?;
            }
        }
        Request::Acknowledge { id, execute } => {
            return super::interrupted::acknowledge(
                &state, euid, id, *execute, cli.json, &shown, stdout,
            );
        }
        Request::Show { id } => {
            let record = history_store::read_one(&state, id, euid)?.ok_or_else(|| {
                DomainError::new(ErrorId::RunNotFound, format!("run {id} is not recorded"))
            })?;
            let marker = run_marker::marker_state(&state, euid);
            let document = show_document(&state, euid, &record, &marker);
            if cli.json {
                writeln!(stdout, "{}", json(&document)?).map_err(io)?;
            } else {
                write!(stdout, "{}", shown.show(&document)).map_err(io)?;
            }
        }
    }
    Ok(0)
}

/// Whether the snapshot a row names was only looked at or was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SnapshotChecked {
    /// File details only: a regular file this user owns, closed to others,
    /// within the size cap. Used by the list, which will not parse up to 64 MiB
    /// a row.
    Metadata,
    /// Read and parsed as a snapshot, and tied to the run by its capture stamp.
    Parsed,
}

/// What the look at the snapshot found, as the documents state it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SnapshotState {
    /// The record names none.
    NotTaken,
    /// It is there (see [`SnapshotChecked`] for how far it was looked at).
    Present,
    /// It is not there.
    Missing,
    /// Something is there that cannot be trusted or read.
    Unreadable,
}

/// The list's `--json` document: `history-list.v1`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListDocument {
    schema_version: u32,
    /// How many runs the store holds.
    total: usize,
    /// The page size asked for.
    limit: usize,
    /// Newest first, by the stamp in the id.
    runs: Vec<ListedRun>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedRun {
    run_id: String,
    started_at: String,
    package: String,
    source: String,
    destination: String,
    status: RunStatus,
    #[serde(flatten)]
    restorability: Restorability,
    standing: RecordStanding,
    snapshot_file: Option<String>,
    snapshot: SnapshotState,
    snapshot_checked: SnapshotChecked,
}

/// The show's `--json` document: `history-show.v1`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowDocument<'a> {
    schema_version: u32,
    /// The record exactly as `history.v2` stores it.
    record: StoredEnvelope<'a>,
    restorability: Restorability,
    standing: RecordStanding,
    snapshot: SnapshotState,
    snapshot_checked: SnapshotChecked,
    /// Kept out of the document: what the look said about an unreadable
    /// snapshot is for the human text, never the contract.
    #[serde(skip)]
    snapshot_problem: Option<String>,
    #[serde(skip)]
    record_ref: &'a RunRecord,
}

fn list_document(
    state: &StateDir,
    euid: u32,
    list: &RecordList,
    marker: &MarkerState,
    limit: usize,
) -> ListDocument {
    let runs = list
        .records
        .iter()
        .map(|record| {
            let standing = marker.standing_of(record.id());
            let (state_of, look) = match record.snapshot_path() {
                None => (SnapshotState::NotTaken, SnapshotLook::Missing),
                Some(name) => match state_snapshot_file(state, name, euid) {
                    // Metadata only: the capture stamp is not read, so the tie
                    // to the run is assumed here and proved by `show`.
                    SnapshotFile::Present => (
                        SnapshotState::Present,
                        SnapshotLook::Present {
                            captured_at: record.started_at().to_owned(),
                        },
                    ),
                    SnapshotFile::Missing => (SnapshotState::Missing, SnapshotLook::Missing),
                    SnapshotFile::Unreadable(_) => {
                        (SnapshotState::Unreadable, SnapshotLook::Unreadable)
                    }
                },
            };
            let subject = record.subject();
            ListedRun {
                run_id: record.id().to_owned(),
                started_at: record.started_at().to_owned(),
                package: subject.package.name.clone(),
                source: subject.direction.source.as_str().to_owned(),
                destination: subject.direction.destination.as_str().to_owned(),
                status: record.status(),
                restorability: restorability(record, &look, standing),
                standing,
                snapshot_file: record.snapshot_path().map(str::to_owned),
                snapshot: state_of,
                snapshot_checked: SnapshotChecked::Metadata,
            }
        })
        .collect();
    ListDocument {
        schema_version: 1,
        total: list.total,
        limit,
        runs,
    }
}

fn show_document<'a>(
    state: &StateDir,
    euid: u32,
    record: &'a RunRecord,
    marker: &MarkerState,
) -> ShowDocument<'a> {
    let standing = marker.standing_of(record.id());
    let (snapshot, look, problem) = match record.snapshot_path() {
        None => (SnapshotState::NotTaken, SnapshotLook::Missing, None),
        Some(name) => match state_snapshot(state, name, euid) {
            StateSnapshot::Present(snapshot) => (
                SnapshotState::Present,
                SnapshotLook::Present {
                    captured_at: snapshot.provenance.captured_at.clone(),
                },
                None,
            ),
            StateSnapshot::Missing => (SnapshotState::Missing, SnapshotLook::Missing, None),
            StateSnapshot::Unreadable(why) => (
                SnapshotState::Unreadable,
                SnapshotLook::Unreadable,
                Some(why),
            ),
        },
    };
    ShowDocument {
        schema_version: 1,
        record: record.stored(),
        restorability: restorability(record, &look, standing),
        standing,
        snapshot,
        snapshot_checked: SnapshotChecked::Parsed,
        snapshot_problem: problem,
        record_ref: record,
    }
}

/// The text of both commands: every sentence through the catalogue, every value
/// that came from a file through `escape_controls`, the home as `~`.
pub(super) struct Shown<'a> {
    pub(super) messages: &'a Messages,
    pub(super) redaction: HomeRedaction,
}

impl Shown<'_> {
    pub(super) fn say(&self, id: &str, pairs: &[(&str, String)]) -> String {
        let params: BTreeMap<String, String> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect();
        self.messages.render(id, &params)
    }

    /// A value read from a file or from the command line, made safe to print.
    pub(super) fn plain(&self, value: &str) -> String {
        escape_controls(&redact_home(value, &self.redaction))
    }

    fn list(&self, state: &StateDir, document: &ListDocument) -> String {
        let mut out = String::new();
        if document.runs.is_empty() {
            out.push_str(&self.say(
                "history.list_empty",
                &[("state", self.plain(&state.root().display().to_string()))],
            ));
            out.push('\n');
            return out;
        }
        out.push_str(&self.say(
            "history.list_header",
            &[
                ("shown", document.runs.len().to_string()),
                ("total", document.total.to_string()),
            ],
        ));
        out.push('\n');
        for run in &document.runs {
            out.push_str(&self.say(
                "history.list_row",
                &[
                    ("id", run.run_id.clone()),
                    ("package", self.plain(&run.package)),
                    ("source", self.plain(&run.source)),
                    ("destination", self.plain(&run.destination)),
                    ("status", self.status(run.status)),
                    ("restorability", self.restorability(run.restorability)),
                    ("standing", self.standing_suffix(run.standing)),
                ],
            ));
            out.push('\n');
        }
        out.push_str(&self.messages.render_id("history.list_snapshot_note"));
        out.push('\n');
        out
    }

    fn show(&self, document: &ShowDocument<'_>) -> String {
        let record = document.record_ref;
        let subject = record.subject();
        let mut out = String::new();
        let mut line = |text: String| {
            out.push_str(&text);
            out.push('\n');
        };
        line(self.say("history.show_run", &[("id", record.id().to_owned())]));
        line(self.say(
            "history.show_started",
            &[
                ("startedAt", record.started_at().to_owned()),
                ("toolVersion", self.plain(record.tool_version())),
            ],
        ));
        line(self.say(
            "history.show_subject",
            &[
                ("package", self.plain(&subject.package.name)),
                ("source", self.plain(subject.direction.source.as_str())),
                (
                    "destination",
                    self.plain(subject.direction.destination.as_str()),
                ),
            ],
        ));
        if subject.retain_source {
            line(self.messages.render_id("history.show_retain_source"));
        }
        line(self.say(
            "history.show_status",
            &[("status", self.status(record.status()))],
        ));
        line(self.say(
            "history.show_restorability",
            &[("restorability", self.restorability(document.restorability))],
        ));
        match (record.snapshot_path(), document.snapshot) {
            (None, _) => line(self.messages.render_id("history.show_no_snapshot")),
            (Some(name), state) => {
                let mut pairs = vec![("file", self.plain(name)), ("state", self.snapshot(state))];
                if let Some(problem) = &document.snapshot_problem {
                    pairs.push(("problem", self.plain(problem)));
                    line(self.say("history.show_snapshot_problem", &pairs));
                } else {
                    line(self.say("history.show_snapshot", &pairs));
                }
            }
        }
        line(self.say(
            "history.show_standing",
            &[("standing", self.standing(document.standing))],
        ));
        if document.standing == RecordStanding::Unconfirmed {
            // The same description `doctor` and `acknowledge` give of this state.
            line(super::interrupted::describe(
                self,
                &macportsbrew_application::RunFacts::Stranded {
                    run_id: record.id().to_owned(),
                    snapshot_file: record.snapshot_path().map(str::to_owned),
                    record: macportsbrew_application::StrandedRecord::Unconfirmed {
                        changed: super::interrupted::changed_steps(record),
                    },
                },
            ));
        }
        line(self.messages.render_id("history.show_steps"));
        for step in record.steps() {
            // The step kind and its result are protocol identifiers, the same
            // words `--json` carries; they are not translated prose.
            line(self.say(
                "history.show_step",
                &[
                    ("index", step.index().to_string()),
                    ("kind", identifier(&step.kind())),
                    ("result", identifier(step.result())),
                ],
            ));
        }
        out
    }

    fn status(&self, status: RunStatus) -> String {
        let id = match status {
            RunStatus::Success => "history.status.success",
            RunStatus::Partial => "history.status.partial",
            RunStatus::Failure => "history.status.failure",
            RunStatus::NoChange => "history.status.no_change",
        };
        self.messages.render_id(id)
    }

    pub(super) fn restorability(&self, restorability: Restorability) -> String {
        match restorability {
            Restorability::NothingToRestore => {
                self.messages.render_id("history.restorability.nothing")
            }
            Restorability::FromSnapshot => self
                .messages
                .render_id("history.restorability.from_snapshot"),
            Restorability::Partially { reason } => self.say(
                "history.restorability.partially",
                &[("reason", self.reason(reason))],
            ),
            Restorability::Not { reason } => self.say(
                "history.restorability.not",
                &[("reason", self.reason(reason))],
            ),
        }
    }

    pub(super) fn reason(&self, reason: NotRestorable) -> String {
        let id = match reason {
            NotRestorable::NoSnapshotTaken => "history.reason.no_snapshot_taken",
            NotRestorable::SnapshotMissing => "history.reason.snapshot_missing",
            NotRestorable::SnapshotUnreadable => "history.reason.snapshot_unreadable",
            NotRestorable::SnapshotNotThisRuns => "history.reason.snapshot_not_this_runs",
            NotRestorable::RecordNotConfirmed => "history.reason.record_not_confirmed",
            NotRestorable::StandingUnknown => "history.reason.standing_unknown",
            NotRestorable::OutcomeUncertain => "history.reason.outcome_uncertain",
        };
        self.messages.render_id(id)
    }

    pub(super) fn standing(&self, standing: RecordStanding) -> String {
        let id = match standing {
            RecordStanding::Confirmed => "history.standing.confirmed",
            RecordStanding::Unconfirmed => "history.standing.unconfirmed",
            RecordStanding::Unknown => "history.standing.unknown",
        };
        self.messages.render_id(id)
    }

    /// Nothing for a confirmed record; a flag on the row for any other.
    fn standing_suffix(&self, standing: RecordStanding) -> String {
        match standing {
            RecordStanding::Confirmed => String::new(),
            RecordStanding::Unconfirmed => self
                .messages
                .render_id("history.standing_suffix.unconfirmed"),
            RecordStanding::Unknown => self.messages.render_id("history.standing_suffix.unknown"),
        }
    }

    fn snapshot(&self, state: SnapshotState) -> String {
        let id = match state {
            SnapshotState::NotTaken => "history.snapshot.not_taken",
            SnapshotState::Present => "history.snapshot.present",
            SnapshotState::Missing => "history.snapshot.missing",
            SnapshotState::Unreadable => "history.snapshot.unreadable",
        };
        self.messages.render_id(id)
    }
}

/// The SCREAMING_SNAKE spelling a value serialises to, for the identifiers the
/// text shows as they are.
pub(super) fn identifier<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => text,
        Ok(serde_json::Value::Object(map)) => map
            .get("kind")
            .or_else(|| map.get("result"))
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_owned(),
        _ => "?".to_owned(),
    }
}
