// SPDX-License-Identifier: Apache-2.0

//! Capturing a snapshot through the domain's read contracts. The only
//! place the application layer talks to providers; everything after it is
//! a function of the snapshot.

use crate::TOOL_VERSION;
use macportsbrew_domain::{
    DomainError, MachineRead, Provenance, ProviderRead, ProviderSnapshot, Snapshot,
    SNAPSHOT_SCHEMA_VERSION,
};
use std::collections::BTreeMap;

/// What a capture needs.
pub struct CaptureContext<'a> {
    /// The machine reader.
    pub machine: &'a dyn MachineRead,
    /// Every provider the tool knows, present or not.
    pub providers: Vec<&'a dyn ProviderRead>,
    /// Policy version in effect.
    pub policy_version: String,
    /// Capture time, RFC 3339 UTC, passed in.
    pub captured_at: String,
    /// The command that captured it.
    pub captured_by: String,
}

/// Captures a snapshot. A provider that fails to answer is recorded as
/// broken with the error identifier; the capture itself only fails when the
/// machine cannot be read at all.
pub fn capture(ctx: &CaptureContext<'_>) -> Result<Snapshot, DomainError> {
    let machine = ctx.machine.machine()?;
    let environment = ctx.machine.environment()?;
    let mut providers = BTreeMap::new();
    for provider in &ctx.providers {
        let snap = match provider.snapshot() {
            Ok(s) => s,
            Err(e) => broken(*provider, &e),
        };
        providers.insert(provider.id().clone(), snap);
    }
    // Resolutions: for every installed name, ask PATH which file would run
    // and every provider who owns it. Keyed by executable name.
    let mut names: Vec<String> = providers
        .values()
        .flat_map(|p| p.inventory.installed_names())
        .collect();
    names.sort();
    names.dedup();
    for name in names {
        let Some(path) = ctx.machine.which(&name) else {
            continue;
        };
        let mut best = None;
        for provider in &ctx.providers {
            if let Ok(r) = provider.resolution(&path) {
                let is_owner = matches!(
                    r.owner,
                    macportsbrew_domain::ExecutableOwner::Provider { .. }
                );
                if is_owner || best.is_none() {
                    let owner_id = provider.id().clone();
                    best = Some((owner_id, r));
                    if is_owner {
                        break;
                    }
                }
            }
        }
        if let Some((owner_id, mut r)) = best {
            r.executable.clone_from(&name);
            if let Some(p) = providers.get_mut(&owner_id) {
                p.resolutions.insert(name, r);
            }
        }
    }
    Ok(Snapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        tool_version: TOOL_VERSION.to_string(),
        policy_version: ctx.policy_version.clone(),
        provenance: Provenance {
            captured_at: ctx.captured_at.clone(),
            captured_by: ctx.captured_by.clone(),
            origin: "captured".into(),
            note: None,
        },
        machine,
        environment,
        providers,
    })
}

fn broken(provider: &dyn ProviderRead, error: &DomainError) -> ProviderSnapshot {
    ProviderSnapshot {
        detection: macportsbrew_domain::ProviderDetection {
            provider: provider.id().clone(),
            presence: macportsbrew_domain::ProviderPresence::Broken {
                reason: error.id.message_id().to_string(),
            },
            version: None,
            prefix: None,
            executable: None,
            architecture: None,
        },
        capabilities: macportsbrew_domain::ProviderCapabilities::default(),
        inventory: macportsbrew_domain::ProviderInventory::default(),
        environment: BTreeMap::new(),
        invocation_environment: BTreeMap::new(),
        resolutions: BTreeMap::new(),
    }
}
