//! Emit props coerced to the prop schema of the kind or relation they land on (DR-46).
//!
//! A prop written with captures is text until it is coerced: `enabled = "$2"` becomes the boolean
//! `false` when the kind's schema says `boolean`, so a decision kind's `when` can select it. A
//! prop written as a plain TOML value keeps the value as written, and must meet its schema when
//! the extractor loads ([`breaks`]).
//!
//! A prop schema is a JSON Schema fragment, `{ type = "object", properties = { ... } }` with
//! `properties` a table, and each prop's schema is a table of only the keywords coercion enforces:
//! `type`, `enum`, `minimum`, `maximum`, `items` with a `type`, and `description`, each in a form
//! coercion reads: `type` a JSON type name or a list of them, `enum` a non-empty list of values of
//! the prop's type, `minimum` and `maximum` numbers, `items` a table whose `type` a capture fills.
//! The pack loader refuses any other ([`fault`]), so no emitted value breaks its kind's schema.

use serde_json::{Map, Number, Value};

/// The keywords a prop schema takes beside its props.
const SCHEMA_KEYWORDS: &[&str] = &["type", "properties", "description"];
/// The keywords a prop's own schema takes: each one coercion enforces.
const PROP_KEYWORDS: &[&str] = &["type", "enum", "minimum", "maximum", "items", "description"];
/// The keywords an array prop's `items` takes; `type` is required.
const ITEM_KEYWORDS: &[&str] = &["type", "description"];

/// The JSON types a prop schema may name, in the order a list of them is tried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Boolean,
    Integer,
    Number,
    Null,
    Array,
    String,
}

/// What a captured prop must fit: its types, and the keywords that bound its value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Prop {
    /// The declared types, in the order they are tried; none for an untyped prop, which stays
    /// text.
    pub types: Vec<Type>,
    /// The values it may take, when the schema lists them.
    pub one_of: Option<Vec<Value>>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    /// An array's item types.
    pub items: Vec<Type>,
}

/// A keyword of a prop schema that coercion can't enforce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// The prop it sits on; none for a keyword beside `properties`.
    pub prop: Option<String>,
    /// The keyword; one inside an array's `items` reads `items.KEY`. For a prop whose schema
    /// isn't a table, `properties`, the keyword that holds it; for a kind's `props` that isn't a
    /// table, `props` itself.
    pub keyword: String,
    pub form: Form,
}

/// What is wrong with a [`Fault`]'s keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// A keyword coercion never enforces, such as `pattern`.
    Unenforced,
    /// A keyword coercion enforces, written in a form it can't read, such as an empty `enum`, a
    /// `minimum` that isn't a number, a `type` that names no JSON type, `properties` that isn't
    /// a table, or a kind's top-level `type` that isn't `object`.
    Malformed,
    /// A prop whose schema isn't a table, such as `false` or `"integer"`, or a kind's `props`
    /// that isn't one.
    NotATable,
}

/// The JSON Schema type names a `type` may give.
const TYPE_NAMES: &[&str] = &["string", "boolean", "integer", "number", "null", "array", "object"];

/// Whether a `type` keyword is a JSON type name, or a non-empty list of them.
fn typed_well(value: &Value) -> bool {
    let name = |v: &Value| v.as_str().is_some_and(|t| TYPE_NAMES.contains(&t));
    match value {
        Value::Array(names) => !names.is_empty() && names.iter().all(name),
        other => name(other),
    }
}

/// The first keyword of a prop schema that coercion can't enforce: one it never enforces, one it
/// enforces written in a form it can't read, a prop whose schema isn't a table, or a schema that
/// isn't a table whose `type`, when it has one, is `object`. `None` when coercion enforces every
/// one, or the kind declares no props.
pub fn fault(schema: &Value) -> Option<Fault> {
    let schema = match schema {
        // A kind or relation that declares no props.
        Value::Null => return None,
        Value::Object(schema) => schema,
        // Every emit gives an object of props, which only a table describes.
        _ => return Some(Fault { prop: None, keyword: "props".into(), form: Form::NotATable }),
    };
    if let Some(k) = schema.keys().find(|k| !SCHEMA_KEYWORDS.contains(&k.as_str())) {
        return Some(Fault { prop: None, keyword: k.clone(), form: Form::Unenforced });
    }
    if schema.get("type").is_some_and(|t| t != "object") {
        return Some(Fault { prop: None, keyword: "type".into(), form: Form::Malformed });
    }
    let props = match schema.get("properties") {
        None => None,
        Some(Value::Object(props)) => Some(props),
        Some(_) => {
            return Some(Fault { prop: None, keyword: "properties".into(), form: Form::Malformed });
        }
    };
    for (name, prop) in props.into_iter().flatten() {
        let at = |keyword: &str, form: Form| {
            Some(Fault { prop: Some(name.clone()), keyword: keyword.to_owned(), form })
        };
        let Some(prop) = prop.as_object() else { return at("properties", Form::NotATable) };
        let unknown = |m: &Map<String, Value>, allowed: &[&str]| {
            m.keys().find(|k| !allowed.contains(&k.as_str())).cloned()
        };
        if let Some(k) = unknown(prop, PROP_KEYWORDS) {
            return at(&k, Form::Unenforced);
        }
        if prop.get("type").is_some_and(|t| !typed_well(t)) {
            return at("type", Form::Malformed);
        }
        let types = type_names(prop.get("type"));
        if let Some(one_of) = prop.get("enum") {
            let typed = |v: &Value| types.is_empty() || types.iter().any(|t| is_type(v, t));
            if !one_of.as_array().is_some_and(|vs| !vs.is_empty() && vs.iter().all(typed)) {
                return at("enum", Form::Malformed);
            }
        }
        for bound in ["minimum", "maximum"] {
            if prop.get(bound).is_some_and(|b| !b.is_number()) {
                return at(bound, Form::Malformed);
            }
        }
        if let Some(items) = prop.get("items") {
            let Some(items) = items.as_object().filter(|i| i.contains_key("type")) else {
                return at("items", Form::Malformed);
            };
            if let Some(k) = unknown(items, ITEM_KEYWORDS) {
                return at(&format!("items.{k}"), Form::Unenforced);
            }
            let names = type_names(items.get("type"));
            let well = items.get("type").is_some_and(typed_well);
            if !well || !names.iter().all(|t| filled(t).is_some()) {
                return at("items", Form::Malformed);
            }
        }
    }
    None
}

/// The type names `value` gives: a name, or a list of names.
fn type_names(value: Option<&Value>) -> Vec<&str> {
    match value {
        Some(Value::String(t)) => vec![t.as_str()],
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    }
}

/// Whether `value` is of the JSON Schema type `name`; an integer is a number with no fraction.
fn is_type(value: &Value, name: &str) -> bool {
    match name {
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "number" => value.is_number(),
        "integer" => value.as_f64().is_some_and(|n| n.fract() == 0.0),
        _ => false,
    }
}

/// The first keyword of the prop `name`'s schema that the plain `value` breaks: `type`, `enum`,
/// `minimum`, `maximum`, or `items` for an item of none of the items' types. `None` when it meets
/// them all, or the schema doesn't declare the prop.
pub fn breaks(schema: &Value, name: &str, value: &Value) -> Option<&'static str> {
    let declared = schema.get("properties").and_then(|p| p.get(name))?;
    let types = type_names(declared.get("type"));
    if !types.is_empty() && !types.iter().any(|t| is_type(value, t)) {
        return Some("type");
    }
    if let Some(one_of) = declared.get("enum").and_then(Value::as_array)
        && !one_of.iter().any(|v| same(v, value))
    {
        return Some("enum");
    }
    if let Some(n) = value.as_number().and_then(Number::as_f64) {
        let bound = |k: &str| declared.get(k).and_then(Value::as_f64);
        if bound("minimum").is_some_and(|min| n < min) {
            return Some("minimum");
        }
        if bound("maximum").is_some_and(|max| n > max) {
            return Some("maximum");
        }
    }
    if let Some(items) = value.as_array() {
        let types = type_names(declared.get("items").and_then(|i| i.get("type")));
        if !types.is_empty() && !items.iter().all(|i| types.iter().any(|t| is_type(i, t))) {
            return Some("items");
        }
    }
    None
}

/// The type names `value` gives, a name or a list of names, read as [`Type`]s; the error is the
/// names, joined, when they are some and none is a type a capture fills, such as `object`.
fn read_types(value: Option<&Value>) -> Result<Vec<Type>, String> {
    let names = type_names(value);
    let types: Vec<Type> = names.iter().filter_map(|t| filled(t)).collect();
    if types.is_empty() && !names.is_empty() { Err(names.join(", ")) } else { Ok(types) }
}

/// The [`Type`] a capture fills for the type name `name`; `None` for `object` or an unknown name.
fn filled(name: &str) -> Option<Type> {
    match name {
        "boolean" => Some(Type::Boolean),
        "integer" => Some(Type::Integer),
        "number" => Some(Type::Number),
        "null" => Some(Type::Null),
        "array" => Some(Type::Array),
        "string" => Some(Type::String),
        _ => None,
    }
}

/// What the prop schema fragment (`props` of a `[[kinds]]` or `[[relations]]` entry) asks of the
/// prop `name`: its `properties.<name>`. An untyped prop stays text.
///
/// A capture is text, so it fills every type but `object`. When the schema declares types and a
/// capture fills none of them, the error is the declared names, joined: the emit can never give
/// that prop a valid value, so the pack loader refuses it rather than emit a prop that breaks its
/// own schema.
pub fn prop(schema: &Value, name: &str) -> Result<Prop, String> {
    let Some(declared) = schema.get("properties").and_then(|p| p.get(name)) else {
        return Ok(Prop::default());
    };
    let number = |k: &str| declared.get(k).and_then(Value::as_f64);
    Ok(Prop {
        types: read_types(declared.get("type"))?,
        one_of: declared.get("enum").and_then(Value::as_array).cloned(),
        minimum: number("minimum"),
        maximum: number("maximum"),
        items: read_types(declared.get("items").and_then(|i| i.get("type"))).unwrap_or_default(),
    })
}

/// `text` as the first of `types` it reads as: `true` or `false` in any case for a boolean, a
/// decimal integer, a finite number, an empty or `null` text for null, a one-item array of the
/// text read as one of `items` (or as text when they are none), or the text itself. `None` when
/// it reads as none of them; with no types, the text.
fn read(text: &str, t: Type, items: &[Type]) -> Option<Value> {
    match t {
        Type::Boolean => match text.to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        Type::Integer => text.parse::<i64>().ok().map(Value::from),
        Type::Number => {
            text.parse::<f64>().ok().filter(|n| n.is_finite()).and_then(Number::from_f64).map(|n| match text
                .parse::<i64>(
            ) {
                Ok(i) => Value::from(i),
                Err(_) => Value::Number(n),
            })
        }
        Type::Null => (text.is_empty() || text == "null").then_some(Value::Null),
        Type::Array => {
            let item =
                if items.is_empty() { Some(Value::String(text.to_owned())) } else { first(text, items, &[]) };
            item.map(|i| Value::Array(vec![i]))
        }
        Type::String => Some(Value::String(text.to_owned())),
    }
}

fn first(text: &str, types: &[Type], items: &[Type]) -> Option<Value> {
    types.iter().find_map(|t| read(text, *t, items))
}

/// Whether `a` and `b` are the same JSON value, a number compared by its value, so `3` matches
/// `3.0`.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// Whether `value` meets `prop`'s `enum`, `minimum` and `maximum`. A bound applies only to a
/// number, as in JSON Schema.
fn fits(value: &Value, prop: &Prop) -> bool {
    if let Some(one_of) = &prop.one_of
        && !one_of.iter().any(|v| same(v, value))
    {
        return false;
    }
    match value.as_number().and_then(Number::as_f64) {
        Some(n) => prop.minimum.is_none_or(|min| n >= min) && prop.maximum.is_none_or(|max| n <= max),
        None => true,
    }
}

/// `text` as the first of `prop`'s types it reads as and whose value meets every keyword the prop
/// declares; with no types, the text, if it meets them. `None` when nothing fits: the prop is left
/// out, and its file is partial.
pub fn coerce(text: &str, prop: &Prop) -> Option<Value> {
    if prop.types.is_empty() {
        return Some(Value::String(text.to_owned())).filter(|v| fits(v, prop));
    }
    prop.types.iter().filter_map(|t| read(text, *t, &prop.items)).find(|v| fits(v, prop))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_reads_as_the_first_type_it_fits() {
        let schema = json!({"type": "object", "properties": {
            "enabled": {"type": "boolean"}, "count": {"type": "integer"}, "ratio": {"type": "number"},
            "owner": {"type": ["null", "string"]}, "tags": {"type": "array"}, "free": {},
            "settings": {"type": "object"}, "either": {"type": ["object", "string"]}
        }});
        assert_eq!(prop(&schema, "settings"), Err("object".to_owned()));
        assert_eq!(prop(&schema, "either").map(|p| p.types), Ok(vec![Type::String]));
        let coerce = |text: &str, name: &str| coerce(text, &prop(&schema, name).unwrap());
        assert_eq!(coerce("False", "enabled"), Some(json!(false)));
        assert_eq!(coerce("maybe", "enabled"), None);
        assert_eq!(coerce("-12", "count"), Some(json!(-12)));
        assert_eq!(coerce("1.5", "count"), None);
        assert_eq!(coerce("1.5", "ratio"), Some(json!(1.5)));
        assert_eq!(coerce("3", "ratio"), Some(json!(3)));
        assert_eq!(coerce("NaN", "ratio"), None);
        assert_eq!(coerce("", "owner"), Some(Value::Null));
        assert_eq!(coerce("ana", "owner"), Some(json!("ana")));
        assert_eq!(coerce("a", "tags"), Some(json!(["a"])));
        assert_eq!(coerce("7", "free"), Some(json!("7")));
        assert_eq!(coerce("7", "absent"), Some(json!("7")));
    }

    #[test]
    fn a_value_meets_every_keyword_its_prop_declares() {
        let schema = json!({"type": "object", "properties": {
            "color": {"enum": ["red", "blue"]},
            "size": {"type": "number", "enum": [1, 2.5]},
            "pct": {"type": ["integer", "string"], "minimum": 0, "maximum": 100},
            "ids": {"type": "array", "items": {"type": "integer"}, "description": "ticket numbers"},
        }});
        let coerce = |text: &str, name: &str| coerce(text, &prop(&schema, name).unwrap());
        assert_eq!(coerce("red", "color"), Some(json!("red")));
        assert_eq!(coerce("green", "color"), None);
        assert_eq!(coerce("1.0", "size"), Some(json!(1.0)), "a number matches its enum by value");
        assert_eq!(coerce("3", "size"), None);
        assert_eq!(coerce("100", "pct"), Some(json!(100)));
        // Out of bounds as an integer, it is still a string, which no bound applies to.
        assert_eq!(coerce("-1", "pct"), Some(json!("-1")));
        assert_eq!(coerce("7", "ids"), Some(json!([7])));
        assert_eq!(coerce("x", "ids"), None);
    }

    #[test]
    fn only_the_keywords_coercion_enforces_are_accepted() {
        let at = |prop: Option<&str>, k: &str, malformed: bool| {
            let form = if malformed { Form::Malformed } else { Form::Unenforced };
            Some(Fault { prop: prop.map(String::from), keyword: k.to_owned(), form })
        };
        let props = |p: Value| json!({"type": "object", "properties": {"p": p}});
        assert_eq!(fault(&props(json!({"type": "string", "enum": ["a"], "description": "d"}))), None);
        assert_eq!(fault(&props(json!({"minimum": 0, "maximum": 1.5}))), None);
        assert_eq!(fault(&props(json!({"type": "array", "items": {"type": "string"}}))), None);
        assert_eq!(fault(&props(json!({"type": ["integer", "null"], "enum": [1, null]}))), None);
        assert_eq!(fault(&json!({})), None);
        assert_eq!(fault(&props(json!({"anyOf": [{"type": "string"}]}))), at(Some("p"), "anyOf", false));
        assert_eq!(fault(&props(json!({"type": "array", "items": {}}))), at(Some("p"), "items", true));
        let item_kw = props(json!({"type": "array", "items": {"type": "string", "pattern": "x"}}));
        assert_eq!(fault(&item_kw), at(Some("p"), "items.pattern", false));
        assert_eq!(fault(&json!({"type": "object", "required": ["p"]})), at(None, "required", false));
    }

    #[test]
    fn an_enforced_keyword_in_a_form_coercion_cant_read_is_a_fault() {
        let at =
            |k: &str| Some(Fault { prop: Some("p".into()), keyword: k.to_owned(), form: Form::Malformed });
        let props = |p: Value| json!({"type": "object", "properties": {"p": p}});
        for (p, keyword) in [
            (json!({"type": "array", "items": {"type": "object"}}), "items"),
            (json!({"type": "array", "items": {"type": ["string", "object"]}}), "items"),
            (json!({"type": "array", "items": "string"}), "items"),
            (json!({"type": "string", "enum": "red"}), "enum"),
            (json!({"type": "string", "enum": []}), "enum"),
            (json!({"type": "string", "enum": ["red", 1]}), "enum"),
            (json!({"type": "integer", "enum": [1, 1.5]}), "enum"),
            (json!({"type": "integer", "minimum": "5"}), "minimum"),
            (json!({"type": "integer", "maximum": true}), "maximum"),
            (json!({"type": 5}), "type"),
            (json!({"type": "integr"}), "type"),
            (json!({"type": []}), "type"),
            (json!({"type": ["string", 5]}), "type"),
            (json!({"type": "array", "items": {"type": 5}}), "items"),
        ] {
            assert_eq!(fault(&props(p.clone())), at(keyword), "{p}");
        }
    }

    #[test]
    fn a_prop_schema_that_isnt_a_table_or_properties_that_isnt_one_is_a_fault() {
        let props = |p: Value| json!({"type": "object", "properties": {"p": p}});
        let not_a_table =
            Some(Fault { prop: Some("p".into()), keyword: "properties".into(), form: Form::NotATable });
        assert_eq!(fault(&props(json!(false))), not_a_table);
        assert_eq!(fault(&props(json!("integer"))), not_a_table);
        let properties = Some(Fault { prop: None, keyword: "properties".into(), form: Form::Malformed });
        assert_eq!(fault(&json!({"type": "object", "properties": 5})), properties);
        // Every JSON type name loads, object included, which a plain value fills.
        let typed = json!({"type": ["string", "boolean", "integer", "number", "null", "array", "object"]});
        assert_eq!(fault(&props(typed)), None);
    }

    #[test]
    fn props_that_arent_an_object_schema_are_a_fault() {
        let not_a_table = Some(Fault { prop: None, keyword: "props".into(), form: Form::NotATable });
        for props in [json!(false), json!(5), json!("object"), json!([1])] {
            assert_eq!(fault(&props), not_a_table, "{props}");
        }
        let typed = Some(Fault { prop: None, keyword: "type".into(), form: Form::Malformed });
        for t in [json!(5), json!("string"), json!("integr"), json!(["object"])] {
            assert_eq!(fault(&json!({"type": t, "properties": {}})), typed, "{t}");
        }
        // A kind that declares no props, or an object schema with no `type`, has no fault.
        assert_eq!(fault(&Value::Null), None);
        assert_eq!(fault(&json!({"properties": {"p": {"type": "string"}}})), None);
    }

    #[test]
    fn a_plain_value_meets_every_keyword_its_prop_declares() {
        let schema = json!({"type": "object", "properties": {
            "color": {"type": "string", "enum": ["red", "blue"]},
            "pct": {"type": "integer", "minimum": 0, "maximum": 100},
            "ratio": {"type": ["number", "null"]},
            "ids": {"type": "array", "items": {"type": "integer"}},
            "settings": {"type": "object"}, "free": {},
        }});
        let breaks = |name: &str, value: Value| breaks(&schema, name, &value);
        assert_eq!(breaks("color", json!("red")), None);
        assert_eq!(breaks("color", json!("green")), Some("enum"));
        assert_eq!(breaks("color", json!(1)), Some("type"));
        assert_eq!(breaks("pct", json!(100)), None);
        assert_eq!(breaks("pct", json!(100.0)), None, "an integer is a number with no fraction");
        assert_eq!(breaks("pct", json!(101)), Some("maximum"));
        assert_eq!(breaks("pct", json!(-1)), Some("minimum"));
        assert_eq!(breaks("pct", json!(2.5)), Some("type"));
        assert_eq!(breaks("ratio", Value::Null), None);
        assert_eq!(breaks("ids", json!([1, 2])), None);
        assert_eq!(breaks("ids", json!([1, "x"])), Some("items"));
        assert_eq!(breaks("settings", json!({"a": 1})), None);
        assert_eq!(breaks("free", json!([{"a": 1}])), None);
        assert_eq!(breaks("undeclared", json!("any")), None);
    }
}
