// SPDX-License-Identifier: Apache-2.0

//! Whether this host's filesystem describes the machine being read.
//!
//! INV-15 says a replayed reader never consults the live filesystem. Getting
//! that right needs a signal the adapters did not have, because the obvious
//! one is a different question wearing the same clothes.
//!
//! [`crate::process::ProcessRunner::is_replay`] answers **are subprocess
//! results recorded**. What a filesystem read raises is **does this filesystem
//! describe the machine being described**. The two coincide under `--replay`
//! from the CLI and come apart in a contract test, which replays subprocesses
//! so as not to spawn `brew`, while genuinely reading a scratch tree it just
//! created. Guarding the reads on `is_replay` was built and measured before
//! this type existed: three correct contract tests failed, because the
//! adapter stopped reading a filesystem that really was its subject (F-25).
//!
//! So the two signals stay separate, and each guard states which question it
//! is asking. Two places deliberately keep asking the *runner*:
//! `registry::override_for`, where the question genuinely is whether a real
//! executable could be spawned, and answering it with this type would let a
//! caller point the tool at an arbitrary binary on a live machine
//! (`docs/spec/04-migration-and-safety.md`, "Process execution"); and
//! `system::machine`'s `which`, which is the same conflation this type exists
//! to undo and is safe only because it errs toward not reading. Moving `which`
//! across is the remaining half of F-25 and is not done here.
//!
//! `registry::build` collapses the one pair that cannot mean anything: a live
//! runner spawns commands on this host, so this host is the machine, and
//! `Elsewhere` beside it is refused rather than honoured.

/// Whether the filesystem under this process describes the machine being read.
///
/// An adapter holds one. Where it is [`Elsewhere`](Self::Elsewhere) every
/// filesystem answer an adapter *reports* is the unknown one — a capability is
/// `Unknown`, a resolution carries no owner and is not authoritative — because
/// an unknown is never a no any more than it is a yes
/// (`docs/spec/02-domain-model.md`).
///
/// Two answers are shaped by their callers rather than by this rule, and
/// saying so here is cheaper than the next reader discovering it.
/// `detect_prefix` returns `None`, which `registry::build` turns into the
/// first candidate path — a *default*, reported as the prefix, not an unknown.
/// And `service::scan` returns `Unreadable`, which `defines_service` renders
/// as `false`; the capability beside it is `Unknown` and nothing reads that
/// yet, so a replayed analysis classifies fewer packages as services than a
/// live one. That gap is F-29, and it is not closed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilesystemScope {
    /// The filesystem is the machine being described. Read it.
    ThisMachine,
    /// The machine being described is a recording or a snapshot, and this
    /// host's disk says nothing about it. Do not read it.
    Elsewhere,
}

impl FilesystemScope {
    /// Whether a read of this host's filesystem answers a question about the
    /// machine being described.
    #[must_use]
    pub fn is_this_machine(self) -> bool {
        matches!(self, Self::ThisMachine)
    }
}
