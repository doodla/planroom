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
        summary: Some("9 at the container level: cart (4 component); orders-db.".into()),
        result: Some(MapView {
            level: Level::Container,
            lens: "code".into(),
            focus: None,
            nodes: vec![],
            edges: vec![],
            hidden: 0,
            empty: None,
            layout: None,
        }),
        error: None,
        meta: Meta {
            seq: 412,
            took_ms: 38,
            size: Size { bytes: 2210, tokens_est: 553 },
            models: Some(ModelUse::default()),
            ..Meta::default()
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
    let line = &schema::generate(View::Output)["$defs"]["Evidence"]["oneOf"][0];
    assert_eq!(line["properties"]["type"]["const"], json!("line"));
    let line_required = line["required"].as_array().unwrap();
    assert!(line_required.contains(&json!("path")));
    assert!(!line_required.contains(&json!("contains")));
    // Meta sends only seq and size when everything else is empty (DR-66).
    let meta = required(View::Output, "Meta");
    assert_eq!(meta, vec![json!("seq"), json!("size")]);
}

#[test]
fn an_empty_meta_serializes_as_seq_and_size_only() {
    let meta = Meta { seq: 7, size: Size { bytes: 10, tokens_est: 3 }, ..Meta::default() };
    assert_eq!(
        serde_json::to_value(&meta).unwrap(),
        json!({"seq": 7, "size": {"bytes": 10, "tokens_est": 3}})
    );
    let back: Meta =
        serde_json::from_value(json!({"seq": 7, "size": {"bytes": 10, "tokens_est": 3}})).unwrap();
    assert_eq!(back, meta);
}

#[test]
fn evidence_is_a_line_or_a_ref() {
    let line = planroom_contract::Evidence::Line {
        path: "platform/docker-compose.yml".into(),
        line: 6,
        contains: Some("    image: acme/storefront:2.3.0".into()),
        commit: Some("4a8b57c".into()),
        branch: Some("main".into()),
    };
    let reference = planroom_contract::Evidence::Ref {
        commit: "4a8b57c".into(),
        ref_name: Some("refs/heads/main".into()),
    };
    assert_eq!(serde_json::to_value(&line).unwrap()["type"], "line");
    assert_eq!(
        serde_json::to_value(&reference).unwrap(),
        json!({"type": "ref", "commit": "4a8b57c", "ref": "refs/heads/main"})
    );
    for (value, def) in [
        (serde_json::to_value(&line).unwrap(), "Evidence"),
        (serde_json::to_value(&reference).unwrap(), "Evidence"),
    ] {
        assert!(validator_for(View::Output, def).is_valid(&value), "{value}");
    }
}
