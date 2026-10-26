//! Open sets: a stable list of values plus a pattern that later values follow, so an older reader
//! still validates a newer value.

use schemars::{Schema, json_schema};

pub(crate) fn schema<'a>(
    description: &str,
    values: impl IntoIterator<Item = &'a str>,
    pattern: &str,
) -> Schema {
    let values: Vec<&str> = values.into_iter().collect();
    json_schema!({
        "description": description,
        "anyOf": [
            { "type": "string", "enum": values },
            { "type": "string", "pattern": pattern },
        ],
    })
}

/// The form of every error, warning and other snake_case code.
pub(crate) const CODE_PATTERN: &str = "^[a-z][a-z0-9_]*$";
