// SPDX-License-Identifier: Apache-2.0

//! `explain <package>`: the full derivation (`03-policy-and-recommendations.md`,
//! "Explainability").

use crate::analysis::{analyze, Analysis, AnalysisOptions, AnalysisSource};
use crate::facts::{package_facts, PackageInputs};
use crate::report::PackageReport;
use macportsbrew_domain::{DomainError, ErrorId, Snapshot};
use macportsbrew_policy::{explain, Derivation, Facts, PolicyData};
use serde::{Deserialize, Serialize};

/// Everything `explain` prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    /// Output schema version.
    pub schema_version: u32,
    /// The package unit.
    pub package: PackageReport,
    /// The facts the engine consumed.
    pub facts: Facts,
    /// The derivation.
    pub derivation: Derivation,
}

/// Explains one package by name (any provider's spelling).
pub fn explain_package(
    snapshot: &Snapshot,
    policy: &PolicyData,
    options: &AnalysisOptions,
    name: &str,
) -> Result<Explanation, DomainError> {
    let opts = AnalysisOptions {
        advanced: true,
        only: [name.to_string()].into_iter().collect(),
        ..options.clone()
    };
    // `Snapshot` here is not a claim about where the input came from: only
    // the package report is read, and the analysis's own provenance and
    // concurrency flag are never shown. A caller that needs either takes it
    // from its own `AnalysisSource` (spec-guardian, 2026-10-03).
    let analysis: Analysis = analyze(snapshot, policy, &opts, AnalysisSource::Snapshot);
    let unit = analysis
        .packages
        .into_iter()
        .find(|p| p.name == name || p.rows.values().any(|r| r.name == name))
        .ok_or_else(|| {
            DomainError::new(
                ErrorId::PackageNotFound,
                format!("{name} is not installed under any detected provider"),
            )
        })?;
    let source_row = &unit.rows[&unit.source];
    let installed_version = source_row
        .installed
        .iter()
        .find(|r| r.active)
        .or_else(|| source_row.installed.first())
        .map(|r| r.version.clone());
    let facts = package_facts(
        snapshot,
        policy,
        &options.today,
        &PackageInputs {
            name: &unit.name,
            source: &unit.source,
            ownership: source_row.ownership,
            classes: &unit.class,
            installed_version,
            mappings: &unit.mappings,
        },
    );
    let derivation = explain(&facts, policy);
    Ok(Explanation {
        schema_version: crate::ANALYSIS_SCHEMA_VERSION,
        package: unit,
        facts,
        derivation,
    })
}
