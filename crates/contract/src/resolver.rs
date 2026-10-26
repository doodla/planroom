//! The resolver's outputs (contracts.md, "The resolver's environment"; DR-69): `plugin/scripts/planroom
//! status --json` prints a `ResolverStatus`, and its failures are `Envelope`s with the error codes
//! `engine_fetching` and `engine_unavailable`. The resolver is POSIX sh, so it spells none of these
//! itself: `cargo xtask gen` writes `plugin/scripts/contract.sh` from this module and the error
//! registry, and the resolver sources it.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What `scripts/planroom status --json` prints: the step that resolves the engine, and its path
/// when one resolved without a fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolverStatus {
    /// Whether an engine resolved without a fetch.
    pub ok: bool,
    pub step: ResolverStep,
    /// The engine binary; absent when the next run would fetch it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The engine version this plugin names, from `engine.env`.
    pub version: String,
}

/// The step that resolves the engine, in the order the resolver tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolverStep {
    /// `PLANROOM_ENGINE`, used as it is.
    Override,
    /// A build in the dev checkout.
    Checkout,
    /// The data root's engine cache.
    Cache,
    /// A verified download of the release; with `ok: false`, what the next run would do.
    Download,
}

impl ResolverStep {
    pub const ALL: [ResolverStep; 4] =
        [ResolverStep::Override, ResolverStep::Checkout, ResolverStep::Cache, ResolverStep::Download];

    pub const fn as_str(self) -> &'static str {
        match self {
            ResolverStep::Override => "override",
            ResolverStep::Checkout => "checkout",
            ResolverStep::Cache => "cache",
            ResolverStep::Download => "download",
        }
    }
}

/// `engine_unavailable`'s `details.reason` (DR-69, DR-81).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The download failed or didn't verify.
    DownloadFailed,
    /// The version has no release, such as a `-dev` version.
    NoRelease,
    /// The checkout's build serves another command catalogue.
    StaleBuild,
    /// A source build needs a tool that's missing; `details.missing` names it.
    NoToolchain,
    /// `PLANROOM_ENGINE` names no executable file; the fix is `unset PLANROOM_ENGINE`.
    OverrideMissing,
    /// `PLANROOM_RESOLVE=checkout` found no build in the checkout.
    NoBuild,
}

impl UnavailableReason {
    pub const ALL: [UnavailableReason; 6] = [
        UnavailableReason::DownloadFailed,
        UnavailableReason::NoRelease,
        UnavailableReason::StaleBuild,
        UnavailableReason::NoToolchain,
        UnavailableReason::OverrideMissing,
        UnavailableReason::NoBuild,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            UnavailableReason::DownloadFailed => "download_failed",
            UnavailableReason::NoRelease => "no_release",
            UnavailableReason::StaleBuild => "stale_build",
            UnavailableReason::NoToolchain => "no_toolchain",
            UnavailableReason::OverrideMissing => "override_missing",
            UnavailableReason::NoBuild => "no_build",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_reasons_serialize_as_their_spelling() {
        for step in ResolverStep::ALL {
            assert_eq!(serde_json::to_value(step).unwrap(), step.as_str());
        }
        for reason in UnavailableReason::ALL {
            assert_eq!(serde_json::to_value(reason).unwrap(), reason.as_str());
        }
    }
}
