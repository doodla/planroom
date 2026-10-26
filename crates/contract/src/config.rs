//! The configuration's types: every setting Planroom reads from its two optional `config.toml`
//! files and the `PLANROOM_*` variables (contracts.md, Configuration).
//!
//! These are the types alone. Every key's default, place and meaning live in
//! `crates/planroom/src/config/defaults.toml`, which deserializes into [`Config`] with no key left
//! over and none missing, and the engine reads and merges the files.
//!
//! [`Config`] is the resolved configuration, every key present. In the input schema it describes
//! one file: every key may be left out, since the defaults fill it, and a key the schema doesn't
//! know is ignored rather than refused, so a newer version's file still loads.

use std::fmt;

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Deserializer};

/// Every setting, resolved; in the input schema, one `config.toml` file.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = one_file)]
pub struct Config {
    /// Extra pack directories, after the built-ins: workspace state, set only in a workspace's
    /// file (DR-62).
    pub packs: Vec<String>,
    /// `auto`, or `off` to turn every model call off.
    pub models: String,
    pub scan: Scan,
    pub claude: Claude,
    pub jev: Jev,
    pub budget: Budget,
}

/// `[scan]`.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ConfigScan", transform = one_file)]
pub struct Scan {
    /// Directories a scan skips, as globs relative to each repo's root (DR-89).
    pub exclude: Vec<String>,
}

/// `[claude]`. Its credential settings come only from the user-level file or the environment.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ConfigClaude", transform = one_file)]
pub struct Claude {
    pub backend: String,
    /// The only host a credential goes to; never set by a variable.
    pub base_url: String,
    /// The name of the variable that holds the gateway key.
    pub api_key_env: String,
    /// The gateway key itself, honoured only from a user-level file of mode 0600.
    #[schemars(with = "String")]
    pub api_key: Secret,
    pub model_cheap: String,
    pub model_capable: String,
    pub providers: Vec<String>,
}

/// `[jev]`.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ConfigJev", transform = one_file)]
pub struct Jev {
    pub backend: String,
    pub model: String,
}

/// `[budget]`, in dollars (DR-65).
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ConfigBudget", transform = one_file)]
pub struct Budget {
    pub jev_per_scan_usd: f64,
    pub claude_per_day_usd: f64,
    pub live_tests_total_usd: f64,
}

/// A credential: never printed, logged or put in an error. `Debug` says only whether it's set.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Secret(Option<String>);

impl Secret {
    /// The value, for the one call that sends it.
    pub fn expose(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub fn is_set(&self) -> bool {
        self.0.is_some()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.is_set() { "Secret(set)" } else { "Secret(none)" })
    }
}

impl<'de> Deserialize<'de> for Secret {
    /// An empty string is no value.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Secret, D::Error> {
        let value = String::deserialize(d)?;
        Ok(Secret(Some(value).filter(|v| !v.is_empty())))
    }
}

/// A section as one file holds it: every key optional, since the defaults fill the rest, and an
/// unknown key allowed, since the engine ignores it rather than refusing the file.
fn one_file(schema: &mut Schema) {
    schema.remove("required");
    schema.remove("additionalProperties");
}
