//! Both schema views are valid JSON Schema 2020-12 and say what they are.

use planroom_contract::schema::{self, DIALECT, GENERATED_HEADER, View};
use planroom_contract::{
    Command, CommandName, Envelope, ErrorCode, Level, MapView, Meta, ModelUse, SCHEMA_VERSION, Size,
    WarningCode,
};
use serde_json::{Value, json};

/// A validator for one `$defs` entry of a view.
fn validator_for(view: View, def: &str) -> jsonschema::Validator {
    let mut root = schema::generate(view);
    root["$ref"] = json!(format!("#/$defs/{def}"));
    jsonschema::draft202012::new(&root).unwrap_or_else(|e| panic!("{view:?} {def}: {e}"))
}

fn open_set_values(view: View, def: &str) -> Vec<String> {
    let root = schema::generate(view);
    let values = &root["$defs"][def]["anyOf"][0]["enum"];
    values
        .as_array()
        .unwrap_or_else(|| panic!("{def} has no enum"))
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn every_view_is_valid_json_schema_2020_12() {
    for view in View::ALL {
        let root = schema::generate(view);
        assert_eq!(root["$schema"], DIALECT, "{view:?}");
        jsonschema::draft202012::meta::validate(&root).unwrap_or_else(|e| panic!("{view:?}: {e}"));
        // Every `$ref` resolves: compiling each definition fails on a dangling one.
        for def in root["$defs"].as_object().unwrap().keys() {
            validator_for(view, def);
        }
    }
}

#[test]
fn every_view_has_its_id_header_and_the_schema_version_const() {
    for view in View::ALL {
        let root = schema::generate(view);
        assert_eq!(
            root["$id"],
            format!("https://raw.githubusercontent.com/doodla/planroom/main/schema/{}", view.file_name())
        );
        assert_eq!(root["$comment"], GENERATED_HEADER);
        assert_eq!(root["$defs"]["Envelope"]["properties"]["schema_version"]["const"], json!(SCHEMA_VERSION));
    }
}

#[test]
fn the_schema_enumerates_the_stable_codes_and_commands() {
    for view in View::ALL {
        let errors: Vec<&str> = ErrorCode::ALL.iter().map(|c| c.as_str()).collect();
        let warnings: Vec<&str> = WarningCode::ALL.iter().map(|c| c.as_str()).collect();
        let commands: Vec<String> = Command::SPECS.iter().map(|s| s.name()).collect();
        assert_eq!(open_set_values(view, "ErrorCode"), errors);
        assert_eq!(open_set_values(view, "WarningCode"), warnings);
        assert_eq!(open_set_values(view, "CommandName"), commands);
    }
}

#[test]
fn open_sets_accept_later_codes_of_the_documented_form_only() {
    let codes = validator_for(View::Output, "ErrorCode");
    assert!(codes.is_valid(&json!("not_found")));
    assert!(codes.is_valid(&json!("a_code_from_a_later_version")));
    assert!(!codes.is_valid(&json!("Not Found")));
    let envelope = validator_for(View::Output, "Envelope");
    let mut reply = serde_json::to_value(sample_envelope()).unwrap();
    assert!(envelope.is_valid(&reply));
    reply["schema_version"] = json!(2);
    assert!(!envelope.is_valid(&reply), "another schema_version must not validate");
}

fn sample_envelope() -> Envelope<MapView> {
    Envelope {
        schema_version: SCHEMA_VERSION,
        ok: true,
        command: CommandName("map".into()),
        result: Some(MapView {
            level: Level::Container,
            lens: "code".into(),
            focus: None,
            nodes: vec![],
            edges: vec![],
            hidden: 0,
            empty: None,
        }),
        error: None,
        meta: Meta {
            seq: 412,
            took_ms: 38,
            size: Size { bytes: 2210, tokens_est: 553 },
            next: None,
            truncated: false,
            replayed: false,
            warnings: vec![],
            models: ModelUse::default(),
            model_requests: vec![],
        },
    }
}

#[test]
fn an_envelope_round_trips_and_validates_against_both_views() {
    let env = sample_envelope();
    let value = serde_json::to_value(&env).unwrap();
    let back: Envelope<MapView> = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(back, env);
    for view in View::ALL {
        let errors: Vec<String> =
            validator_for(view, "Envelope").iter_errors(&value).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{view:?}: {errors:?}");
        let result: Vec<String> =
            validator_for(view, "MapView").iter_errors(&value["result"]).map(|e| e.to_string()).collect();
        assert!(result.is_empty(), "{view:?}: {result:?}");
    }
}

#[test]
fn the_output_view_requires_always_sent_fields_and_the_input_view_does_not() {
    let required = |view: View, def: &str| -> Vec<Value> {
        schema::generate(view)["$defs"][def]["required"].as_array().cloned().unwrap_or_default()
    };
    // `detail` has a default and `level` is optional when parsed, but both are always sent.
    assert!(required(View::Output, "MapReq").contains(&json!("detail")));
    assert!(required(View::Output, "MapReq").contains(&json!("level")));
    assert!(!required(View::Input, "MapReq").contains(&json!("detail")));
    assert!(!required(View::Input, "MapReq").contains(&json!("level")));
    // Skipped when absent, so optional in both views.
    assert!(!required(View::Output, "Envelope").contains(&json!("result")));
    assert!(!required(View::Output, "Evidence").contains(&json!("contains")));
}
