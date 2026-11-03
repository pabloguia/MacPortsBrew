// SPDX-License-Identifier: Apache-2.0

//! A provider narrowed to the one question the run loop's resolution site may
//! ask it: who owns this file (spec-guardian review of F-3; security review,
//! fix round 2). The provider is a private field of this module, so nothing
//! else in `executor` can reach any other `ProviderRead` method through it.

use macportsbrew_application::ResolutionAnswer;
use macportsbrew_domain::{DomainError, ProviderId, ProviderRead, Resolution};
use std::path::Path;

/// A provider that answers [`ResolutionAnswer`] and nothing else.
pub struct OnlyResolution<'r>(Box<dyn ProviderRead + 'r>);

impl<'r> OnlyResolution<'r> {
    /// Narrows `provider`.
    pub(super) fn new(provider: Box<dyn ProviderRead + 'r>) -> Self {
        Self(provider)
    }
}

impl ResolutionAnswer for OnlyResolution<'_> {
    fn id(&self) -> &ProviderId {
        self.0.id()
    }
    fn resolution(&self, path: &Path) -> Result<Resolution, DomainError> {
        self.0.resolution(path)
    }
}
