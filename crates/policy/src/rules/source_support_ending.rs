// SPDX-License-Identifier: Apache-2.0

//! `SOURCE_SUPPORT_ENDING`: the side holding the package has published an end.

use super::{labels, PRIORITY_SUPPORT_ENDING, SUPPORT_END_HORIZON_DAYS};
use crate::data::{days_between, PolicyData};
use crate::facts::Facts;
use crate::reasons;
use crate::rules::{Rule, RuleVerdict};
use macportsbrew_domain::{Outcome, ProviderId, Reason, SupportTier};

/// Fires when the provider currently holding the package says its support for
/// this machine is degraded or gone, has published a date for the end of it,
/// that date is inside the horizon, and some other provider offers a strong
/// equivalent with a prebuilt artifact ready
/// (`03-policy-and-recommendations.md`, Starting policy: "stated ... removal
/// date is within a configurable horizon ... Weight increases as the date
/// approaches. The date is data.").
///
/// Three things this rule does not do, each deliberate:
///
/// * It names no provider. The tier, the date and the machine come from the
///   facts; whichever side carries them is the side that triggers this
///   (INV-17). Run it on a machine where the other provider is the one with
///   the deadline and it points the other way.
/// * It reads no clock. `days_remaining` is measured from `facts.today`,
///   which the caller supplies, so the same snapshot gives the same weight
///   forever (INV-14, INV-15).
/// * It holds no date. The date lives in `provider-support.toml` with its
///   source and read date, which is what INV-20 is for: the published end has
///   moved before and will move again, and moving it must be a data edit.
///
/// The weight rises linearly from the far edge of the horizon to the date
/// itself and stays at its maximum after it. It is a reason parameter rather
/// than a priority: how loudly to say this is a presentation decision, and a
/// deadline does not become a different rule as it approaches.
pub struct SourceSupportEnding;

impl Rule for SourceSupportEnding {
    fn id(&self) -> &'static str {
        "SOURCE_SUPPORT_ENDING"
    }

    fn since(&self) -> &'static str {
        "2026.09.02-1"
    }

    fn source(&self) -> &'static str {
        "INV-20; docs/spec/03-policy-and-recommendations.md, Starting policy; docs/research/F-1-homebrew-support-tiers.md findings 3 and 5"
    }

    fn reviewed_on(&self) -> &'static str {
        "2026-09-02"
    }

    fn priority(&self) -> u32 {
        PRIORITY_SUPPORT_ENDING
    }

    fn evaluate(&self, facts: &Facts, _data: &PolicyData) -> Option<RuleVerdict> {
        let support = facts.support(&facts.package.source);
        if !matches!(
            support.tier,
            SupportTier::Degraded | SupportTier::Unsupported
        ) {
            return None;
        }
        let end_date = support.end_date.as_deref()?;
        let days_remaining = days_between(&facts.today, end_date)?;
        if days_remaining > SUPPORT_END_HORIZON_DAYS {
            return None;
        }

        let destination = facts.mapped_destinations().into_iter().find(|id| {
            let usable = facts.at(id).is_some_and(|per| {
                per.mapping_confidence.is_strong() && per.availability.is_prebuilt()
            });
            usable && welcoming(facts, id)
        })?;

        let weight = weight_for(days_remaining);

        Some(RuleVerdict::new(
            Outcome::MigrateTo {
                provider: destination.clone(),
            },
            Reason::new(reasons::SOURCE_SUPPORT_ENDING)
                .with("provider", &facts.package.source)
                .with("tier", &labels::tier(support.tier))
                .with("endDate", &end_date)
                .with("daysRemaining", &days_remaining)
                .with("weight", &weight)
                .with("destination", &destination),
        ))
    }
}

/// Whether a provider is somewhere a package can be sent: it has to answer,
/// and it has to have said something positive about this machine. `UNKNOWN`
/// is not a yes.
fn welcoming(facts: &Facts, id: &ProviderId) -> bool {
    facts.is_usable(id)
        && matches!(
            facts.support(id).tier,
            SupportTier::Supported | SupportTier::Degraded
        )
}

/// 0 at the far edge of the horizon, 100 on the day itself and after it.
fn weight_for(days_remaining: i64) -> i64 {
    if days_remaining <= 0 {
        return 100;
    }
    (100 - (days_remaining * 100) / SUPPORT_END_HORIZON_DAYS).clamp(0, 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::shift_date;
    use crate::rules::testing;
    use macportsbrew_domain::{ArtifactAvailability, Confidence};

    fn data() -> PolicyData {
        PolicyData::load_embedded().unwrap()
    }

    fn ending_in(days: i64) -> Facts {
        let mut facts = testing::baseline();
        let end = shift_date(testing::TODAY, days).unwrap();
        facts.providers.get_mut(&testing::alpha()).unwrap().support =
            testing::support_ending(SupportTier::Degraded, &end, true);
        facts
    }

    #[test]
    fn a_fully_supported_side_does_not_fire_it() {
        assert!(SourceSupportEnding
            .evaluate(&testing::baseline(), &data())
            .is_none());
    }

    #[test]
    fn a_degraded_side_with_no_published_date_does_not_fire_it() {
        let mut facts = testing::baseline();
        facts.providers.get_mut(&testing::alpha()).unwrap().support =
            testing::support(SupportTier::Degraded, true);
        assert!(SourceSupportEnding.evaluate(&facts, &data()).is_none());
    }

    #[test]
    fn a_date_beyond_the_horizon_does_not_fire_it_yet() {
        let facts = ending_in(SUPPORT_END_HORIZON_DAYS + 1);
        assert!(SourceSupportEnding.evaluate(&facts, &data()).is_none());
    }

    #[test]
    fn a_date_inside_the_horizon_points_at_the_other_side() {
        let facts = ending_in(SUPPORT_END_HORIZON_DAYS - 1);
        let verdict = SourceSupportEnding.evaluate(&facts, &data()).unwrap();
        assert_eq!(
            verdict.outcome,
            Outcome::MigrateTo {
                provider: testing::beta()
            }
        );
        assert_eq!(
            verdict.reason.params.get("provider").map(String::as_str),
            Some("alpha")
        );
    }

    #[test]
    fn the_weight_rises_as_the_date_nears() {
        let far = weight_for(SUPPORT_END_HORIZON_DAYS);
        let near = weight_for(30);
        let past = weight_for(-1);
        assert_eq!(far, 0);
        assert!(near > far, "{near} should exceed {far}");
        assert_eq!(past, 100);
        assert!(weight_for(SUPPORT_END_HORIZON_DAYS / 2) > far);
        assert!(weight_for(SUPPORT_END_HORIZON_DAYS / 2) < past);
    }

    #[test]
    fn it_needs_somewhere_to_send_the_package() {
        let mut facts = ending_in(90);
        let per = facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap();
        per.availability = testing::availability(ArtifactAvailability::SourceBuildRequired {
            reason: macportsbrew_domain::availability::SourceBuildReason::NoArtifactForThisPlatform,
        });
        assert!(SourceSupportEnding.evaluate(&facts, &data()).is_none());

        let mut facts = ending_in(90);
        facts
            .package
            .per_provider
            .get_mut(&testing::beta())
            .unwrap()
            .mapping_confidence = Confidence::Probable;
        assert!(SourceSupportEnding.evaluate(&facts, &data()).is_none());

        let mut facts = ending_in(90);
        facts.providers.get_mut(&testing::beta()).unwrap().support =
            testing::support(SupportTier::Unsupported, false);
        assert!(SourceSupportEnding.evaluate(&facts, &data()).is_none());
    }

    #[test]
    fn the_same_facts_with_the_sides_exchanged_point_the_other_way() {
        let facts = ending_in(90);
        let first = SourceSupportEnding.evaluate(&facts, &data()).unwrap();
        let swapped = testing::swap_providers(&facts, &testing::alpha(), &testing::beta());
        let second = SourceSupportEnding.evaluate(&swapped, &data()).unwrap();
        assert_eq!(
            second.outcome,
            testing::swap_outcome(&first.outcome, &testing::alpha(), &testing::beta())
        );
    }
}
