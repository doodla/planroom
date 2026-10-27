//! The claims overlay (architecture.md, "The fold"): claims depend on the clock, so they stay out
//! of the fold. The fold keeps every claim as written; [`active`] applies `now`, an explicit input
//! (the command's start time, or a scan event's `at`), so `fold(at = N)` stays reproducible.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use planroom_contract::{Actor, EventId, Seq, Target, Timestamp};

use super::state::{TargetKey, Writer, actor_key};

/// One `claim` event the fold kept: an applied claim no undo reversed.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimRecord {
    pub event: EventId,
    pub seq: Seq,
    pub writer: Writer,
    pub target: Target,
    pub until: Timestamp,
    /// A release ends the writer's claim on the target.
    pub release: bool,
}

/// A claim that holds at `now`: its holder's latest claim on the target, not a release and not
/// expired.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveClaim {
    pub target: Target,
    pub holder: Actor,
    pub until: Timestamp,
    /// The claim event.
    pub event: EventId,
}

/// The claims that hold at `now`, sorted by target and then by the claim's `seq`. Each actor's
/// latest claim on a target is the one that counts.
pub fn active(claims: &[ClaimRecord], now: &Timestamp) -> Vec<ActiveClaim> {
    let mut latest: BTreeMap<(TargetKey, String), &ClaimRecord> = BTreeMap::new();
    for claim in claims {
        let key = (TargetKey::from(&claim.target), actor_key(&claim.writer.actor));
        match latest.get(&key) {
            Some(held) if held.seq > claim.seq => {}
            _ => {
                latest.insert(key, claim);
            }
        }
    }
    let mut out: Vec<(TargetKey, Seq, ActiveClaim)> = latest
        .into_iter()
        .filter(|(_, c)| !c.release && compare(&c.until, now) == Ordering::Greater)
        .map(|((target, _), c)| {
            let claim = ActiveClaim {
                target: c.target.clone(),
                holder: c.writer.actor.clone(),
                until: c.until.clone(),
                event: c.event.clone(),
            };
            (target, c.seq, claim)
        })
        .collect();
    out.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    out.into_iter().map(|(_, _, claim)| claim).collect()
}

/// Two timestamps in time order. The engine writes every timestamp in one fixed UTC form,
/// `YYYY-MM-DDTHH:MM:SS.ffffffZ` (contracts.md, "Event model"), so text order is time order.
pub fn compare(a: &Timestamp, b: &Timestamp) -> Ordering {
    a.0.cmp(&b.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> Timestamp {
        Timestamp(s.into())
    }

    #[test]
    fn timestamps_in_the_engines_form_compare_in_time_order() {
        let ordered = [
            "1970-01-01T00:00:00.000000Z",
            "2026-10-26T15:29:59.999999Z",
            "2026-10-26T15:30:00.000000Z",
            "2026-10-26T15:30:00.500000Z",
            "2026-10-27T00:00:00.000000Z",
            "2027-01-01T00:00:00.000000Z",
        ];
        for pair in ordered.windows(2) {
            assert_eq!(compare(&ts(pair[0]), &ts(pair[1])), Ordering::Less, "{pair:?}");
        }
        assert_eq!(compare(&ts(ordered[2]), &ts(ordered[2])), Ordering::Equal);
    }
}
