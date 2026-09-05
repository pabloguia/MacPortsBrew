// SPDX-License-Identifier: Apache-2.0

//! Where each provider expects to be installed, read from dated data.
//!
//! INV-20 names **prefixes** alongside support tiers and deadlines: they "live
//! in versioned data files carrying a source and a read date, and are
//! re-checkable". They were the one category still held in a `match`, with the
//! provenance in a doc comment beside it — provenance without the property the
//! invariant asks for, and the INV-20 test carried an explicit exemption saying
//! so (DP-014).
//!
//! The file is embedded with `include_str!`, exactly as `crates/policy` embeds
//! its own data. That does not make a prefix editable without a rebuild, and
//! nothing here claims it does: no policy file in this project is. What
//! "re-checkable" buys is a value stated once, beside the source it came from
//! and the date it was read, where a person can compare the two — instead of a
//! literal in control flow whose provenance lives in a comment that no check
//! can hold to it.
//!
//! This layer cannot reach `crates/application`, which loads the rest of
//! `data/policy/`; the Cargo edges are the layering rule and
//! `scripts/check-layering.sh` asserts them. So the loader lives here and reads
//! its own file, which is also what keeps detection working without anything
//! above the adapters being present.

use serde::Deserialize;
use std::path::PathBuf;

const SOURCE: &str = include_str!("../../../data/policy/provider-prefixes.toml");

#[derive(Debug, Deserialize)]
struct Document {
    entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    provider: String,
    prefixes: Vec<PathBuf>,
    #[allow(dead_code, reason = "provenance is the point of the file; INV-20")]
    source: String,
    #[allow(dead_code, reason = "provenance is the point of the file; INV-20")]
    read_on: String,
}

/// The candidate prefixes for `provider`, in detection order.
///
/// An unknown provider has none, which is how a registry entry without data is
/// reported absent rather than probed against a guess.
pub fn candidates(provider: &str) -> Vec<PathBuf> {
    document()
        .entry
        .iter()
        .find(|e| e.provider == provider)
        .map(|e| e.prefixes.clone())
        .unwrap_or_default()
}

/// Parses the embedded document.
///
/// A malformed file is a build-time mistake in this repository's own data, not
/// a user input, and there is no honest fallback: guessing a prefix is
/// guessing which binary to run. So it panics, with the parse error, at the
/// first call — which is a test failure long before it is anything else.
fn document() -> &'static Document {
    use std::sync::OnceLock;
    static PARSED: OnceLock<Document> = OnceLock::new();
    PARSED.get_or_init(|| {
        toml::from_str(SOURCE)
            .unwrap_or_else(|e| panic!("data/policy/provider-prefixes.toml is malformed: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_carries_its_provenance_and_at_least_one_absolute_prefix() {
        let doc = document();
        assert!(!doc.entry.is_empty(), "the file describes no provider");
        for e in &doc.entry {
            assert!(!e.source.trim().is_empty(), "{}: no source", e.provider);
            assert!(!e.read_on.trim().is_empty(), "{}: no read date", e.provider);
            assert!(
                e.read_on.len() == 10 && e.read_on.chars().filter(|c| *c == '-').count() == 2,
                "{}: read_on is not YYYY-MM-DD: {:?}",
                e.provider,
                e.read_on
            );
            assert!(
                !e.prefixes.is_empty(),
                "{}: a provider with no candidate can never be detected",
                e.provider
            );
            for p in &e.prefixes {
                assert!(
                    p.is_absolute(),
                    "{}: {} is not absolute",
                    e.provider,
                    p.display()
                );
                assert!(
                    !p.components().any(|c| c == std::path::Component::ParentDir),
                    "{}: {} traverses",
                    e.provider,
                    p.display()
                );
            }
        }
    }

    #[test]
    fn an_unknown_provider_has_no_candidates() {
        assert!(candidates("pkgsrc").is_empty());
    }
}
