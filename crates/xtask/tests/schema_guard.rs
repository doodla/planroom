//! The breaking-change guard (contracts.md, Versioning rules; DR-59): the generated schema against
//! the committed `schema/baseline/` and, once a release exists, the schema at its `v*` tag. It runs
//! here on temporary roots with planted baselines, and on this repository as it stands.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use planroom_contract::SCHEMA_VERSION;
use planroom_contract::schema::{View, generate};
use serde_json::{Value, json};
use xtask::schema_guard::{self, BASELINE, CHANGELOG};

/// A temporary root, removed on drop.
struct Root(PathBuf);

impl Root {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("planroom-schema-guard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Write both views as the baseline, `planroom.v{version}.…`, each changed by `edit`.
    fn baseline(&self, version: u32, edit: impl Fn(View, &mut Value)) {
        let dir = self.0.join(BASELINE);
        fs::create_dir_all(&dir).unwrap();
        for view in View::ALL {
            let mut schema = generate(view);
            edit(view, &mut schema);
            let name = view.file_name().replace("v1", &format!("v{version}"));
            fs::write(dir.join(name), serde_json::to_string_pretty(&schema).unwrap()).unwrap();
        }
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(["-c", "user.name=guard-test", "-c", "user.email=guard-test@example.invalid"])
            .args(args)
            .current_dir(&self.0)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn current() -> Vec<(View, Value)> {
    View::ALL.iter().map(|&v| (v, generate(v))).collect()
}

/// The guard's problems for the current schema at `version` against `root`.
fn problems(root: &Path, version: u32) -> Vec<String> {
    schema_guard::guard_against(root, &current(), version).unwrap().problems
}

fn defs(schema: &mut Value) -> &mut serde_json::Map<String, Value> {
    schema["$defs"].as_object_mut().unwrap()
}

#[test]
fn breaking_change_guard() {
    // This repository's schema against its committed baseline: nothing broken.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let found = schema_guard::guard(&repo).unwrap();
    assert!(found.problems.is_empty(), "{:?}", found.problems);

    // No baseline fails, naming the command that writes one.
    let root = Root::new("none");
    assert!(problems(&root.0, SCHEMA_VERSION).iter().any(|p| p.contains("gen --schema-baseline")));

    // The same schema passes.
    let root = Root::new("same");
    root.baseline(SCHEMA_VERSION, |_, _| {});
    assert_eq!(problems(&root.0, SCHEMA_VERSION), Vec::<String>::new());

    // A property, an enum value, a type and an open set's value the baseline has and the schema
    // lacks: each a removal, in both views.
    let root = Root::new("removed");
    root.baseline(SCHEMA_VERSION, |_, s| {
        let d = defs(s);
        d["MapReq"]["properties"]["radius"] = json!({"type": "integer", "format": "uint32", "minimum": 0});
        d["Layer"]["enum"].as_array_mut().unwrap().push(json!("galaxy"));
        d.insert("Retired".into(), json!({"type": "string"}));
        d["ErrorCode"]["anyOf"][0]["enum"].as_array_mut().unwrap().push(json!("gone_code"));
    });
    let p = problems(&root.0, SCHEMA_VERSION);
    for want in [
        "$defs/MapReq: property `radius` was removed",
        "$defs/Layer: enum value \"galaxy\" was removed",
        "$defs/Retired: the type was removed",
        "enum value \"gone_code\" was removed",
    ] {
        assert_eq!(p.iter().filter(|l| l.contains(want)).count(), 2, "{want}: {p:#?}");
    }

    // A field that became required in an existing input type breaks writers that leave it out;
    // the same in the output view, where every always-sent field is required, is additive.
    let root = Root::new("required");
    root.baseline(SCHEMA_VERSION, |view, s| {
        if view == View::Input {
            defs(s)["ReviewReq"]["required"].as_array_mut().unwrap().retain(|n| n != "verdict");
        }
    });
    let p = problems(&root.0, SCHEMA_VERSION);
    assert!(
        p.iter().any(|l| l.contains("planroom.v1.input.schema.json")
            && l.contains("ReviewReq: input field `verdict` became required")),
        "{p:#?}"
    );
    let root = Root::new("output-required");
    root.baseline(SCHEMA_VERSION, |view, s| {
        if view == View::Output {
            defs(s)["Meta"]["required"].as_array_mut().unwrap().retain(|n| n != "seq");
        }
    });
    assert_eq!(problems(&root.0, SCHEMA_VERSION), Vec::<String>::new());

    // Additive changes pass: the schema adds a type, a property, an enum value and a variant the
    // baseline lacks.
    let root = Root::new("additive");
    root.baseline(SCHEMA_VERSION, |_, s| {
        let d = defs(s);
        d.remove("LastSeq");
        d["Meta"]["properties"].as_object_mut().unwrap().remove("replayed_from");
        d["Layer"]["enum"].as_array_mut().unwrap().retain(|v| v != "intended");
        d["ErrorCode"]["anyOf"][0]["enum"].as_array_mut().unwrap().retain(|v| v != "store_busy");
        d["EventBody"]["anyOf"].as_array_mut().unwrap().remove(0);
    });
    assert_eq!(problems(&root.0, SCHEMA_VERSION), Vec::<String>::new());

    // A variant of an untagged union is known by its properties, so a field added to it, which
    // the output view also lists as required, is additive; a property it lost is a removal.
    let root = Root::new("untagged-added");
    root.baseline(SCHEMA_VERSION, |_, s| {
        let variant = &mut defs(s)["EmitTemplate"]["anyOf"][0];
        variant["properties"].as_object_mut().unwrap().remove("aliases");
        variant["required"].as_array_mut().unwrap().retain(|n| n != "aliases");
    });
    assert_eq!(problems(&root.0, SCHEMA_VERSION), Vec::<String>::new());
    // An added untagged variant is additive; an externally tagged one the schema lost is removed.
    let root = Root::new("untagged-variants");
    root.baseline(SCHEMA_VERSION, |_, s| {
        let d = defs(s);
        d["EmitTemplate"]["anyOf"].as_array_mut().unwrap().remove(1);
        d["Cause"]["oneOf"].as_array_mut().unwrap().push(json!({
            "type": "object", "properties": {"legacy": {"type": "string"}}, "required": ["legacy"]
        }));
    });
    let p = problems(&root.0, SCHEMA_VERSION);
    assert_eq!(p.len(), 2, "{p:#?}");
    assert!(p.iter().all(|l| l.contains("$defs/Cause/oneOf: variant fields legacy was removed")), "{p:#?}");
    let root = Root::new("untagged-lost");
    root.baseline(SCHEMA_VERSION, |_, s| {
        defs(s)["EmitTemplate"]["anyOf"][0]["properties"]["legacy"] = json!({"type": "string"});
    });
    let p = problems(&root.0, SCHEMA_VERSION);
    assert_eq!(
        p.iter()
            .filter(|l| l.contains("$defs/EmitTemplate/anyOf: variant fields") && l.contains("legacy"))
            .count(),
        2,
        "{p:#?}"
    );

    // A bump of schema_version lets the removal through; a baseline newer than the tree fails.
    let root = Root::new("bumped");
    root.baseline(SCHEMA_VERSION, |_, s| {
        defs(s)["MapReq"]["properties"]["radius"] = json!({"type": "integer"});
    });
    assert_eq!(problems(&root.0, SCHEMA_VERSION + 1), Vec::<String>::new());
    let root = Root::new("newer");
    root.baseline(SCHEMA_VERSION + 1, |_, _| {});
    assert!(problems(&root.0, SCHEMA_VERSION).iter().any(|p| p.contains("newer than this tree's")));

    // Once a release exists, its tag's schema counts too, and a tag the guard needs and can't
    // find fails the check, never skips it.
    let root = Root::new("release");
    root.baseline(SCHEMA_VERSION, |_, _| {});
    fs::write(root.0.join(CHANGELOG), "# Changelog\n\n## [Unreleased]\n\n## [0.1.0] - 2026-11-30\n").unwrap();
    root.git(&["init", "--quiet"]);
    let p = problems(&root.0, SCHEMA_VERSION);
    assert!(p.iter().any(|l| l.contains("its tag v0.1.0 isn't here")), "{p:#?}");
    // The tag's schema had a property today's lacks: the guard names the tag's side.
    fs::create_dir_all(root.0.join("schema")).unwrap();
    for view in View::ALL {
        let mut s = generate(view);
        defs(&mut s)["Meta"]["properties"]["legacy"] = json!({"type": "string"});
        fs::write(root.0.join("schema").join(view.file_name()), serde_json::to_string(&s).unwrap()).unwrap();
    }
    root.git(&["add", "-A"]);
    root.git(&["commit", "--quiet", "-m", "release 0.1.0"]);
    root.git(&["tag", "v0.1.0"]);
    let p = problems(&root.0, SCHEMA_VERSION);
    assert!(
        p.iter()
            .any(|l| l.starts_with("the schema at v0.1.0") && l.contains("property `legacy` was removed")),
        "{p:#?}"
    );
    assert!(!p.iter().any(|l| l.starts_with(BASELINE)), "{p:#?}");

    // A release commit sets the tree's version and dates its CHANGELOG section before the owner
    // tags it: the guard compares with the release before it, whose tag is here.
    fs::write(root.0.join("Cargo.toml"), "[workspace.package]\nversion = \"0.2.0\"\n").unwrap();
    fs::write(
        root.0.join(CHANGELOG),
        "# Changelog\n\n## [Unreleased]\n\n## [0.2.0] - 2026-12-30\n\n## [0.1.0] - 2026-11-30\n",
    )
    .unwrap();
    let p = problems(&root.0, SCHEMA_VERSION);
    assert!(!p.iter().any(|l| l.contains("isn't here")), "{p:#?}");
    assert!(p.iter().any(|l| l.starts_with("the schema at v0.1.0")), "{p:#?}");
    // The next -dev commit, with v0.2.0 still untagged, needs that tag.
    fs::write(root.0.join("Cargo.toml"), "[workspace.package]\nversion = \"0.2.1-dev\"\n").unwrap();
    let p = problems(&root.0, SCHEMA_VERSION);
    assert!(p.iter().any(|l| l.contains("its tag v0.2.0 isn't here")), "{p:#?}");
}

/// The values the generated schema's open `Level` set opened from: the closed enum it was.
const CLOSED_LEVELS: [&str; 5] = ["intent", "system", "container", "component", "code"];

#[test]
fn an_enum_opened_to_an_open_set_that_keeps_its_values_passes() {
    // The baseline's `Level` is the closed enum it was; the schema's is the kernel's `intent`
    // or any ID a pack declares, so every old value is still accepted.
    let root = Root::new("opened");
    root.baseline(SCHEMA_VERSION, |_, s| {
        defs(s)["Level"] = json!({"type": "string", "enum": CLOSED_LEVELS});
    });
    assert_eq!(problems(&root.0, SCHEMA_VERSION), Vec::<String>::new());

    // A value is kept by the new top-level enum, a `oneOf` branch's enum, or a pattern at the
    // top or in a branch.
    let old = json!({"$defs": {"L": {"type": "string", "enum": ["a", "b", "c1", "d_2"]}}});
    for new in [
        json!({"type": "string", "enum": ["a", "b", "c1", "d_2", "e"]}),
        json!({"type": "string", "pattern": "^[a-z][a-z0-9_]*$"}),
        json!({"oneOf": [{"enum": ["a"]}, {"enum": ["b"]}, {"pattern": "^[cd]"}]}),
        json!({"anyOf": [{"type": "string", "enum": ["a", "b"]}, {"type": "string", "pattern": "^(c|d)"}]}),
    ] {
        let new = json!({"$defs": {"L": new}});
        for view in View::ALL {
            assert_eq!(schema_guard::breaking_changes(&old, &new, view), Vec::<String>::new(), "{new}");
        }
    }
}

#[test]
fn a_value_the_new_pattern_refuses_is_still_removed() {
    // A baseline value the open set's pattern refuses is a removal, in both views.
    let root = Root::new("refused");
    root.baseline(SCHEMA_VERSION, |_, s| {
        let mut closed: Vec<Value> = CLOSED_LEVELS.iter().map(|l| json!(l)).collect();
        closed.push(json!("Galaxy-9"));
        defs(s)["Level"] = json!({"type": "string", "enum": closed});
    });
    let p = problems(&root.0, SCHEMA_VERSION);
    assert_eq!(
        p.iter().filter(|l| l.contains("$defs/Level: enum value \"Galaxy-9\" was removed")).count(),
        2,
        "{p:#?}"
    );
    assert_eq!(p.len(), 2, "{p:#?}");

    // Neither a branch's enum nor its pattern keeps it, and a pattern in a branch keeps only what
    // it matches.
    let old = json!({"$defs": {"L": {"type": "string", "enum": ["a", "Zed", "b"]}}});
    let new =
        json!({"$defs": {"L": {"anyOf": [{"enum": ["a"]}, {"type": "string", "pattern": "^[a-z]+$"}]}}});
    assert_eq!(
        schema_guard::breaking_changes(&old, &new, View::Output),
        vec!["$defs/L: enum value \"Zed\" was removed".to_owned()]
    );
}

#[test]
fn the_baseline_moves_only_with_a_schema_version_bump() {
    let root = Root::new("write");
    let written = schema_guard::write_baseline(&root.0).unwrap();
    assert_eq!(written.len(), 2);
    let err = schema_guard::write_baseline(&root.0).unwrap_err();
    assert!(err.contains("moves only with a schema_version bump"), "{err}");
    let changelog = "## [Unreleased]\n## [0.2.0] - x\n## [0.1.0] - y\n";
    assert_eq!(schema_guard::last_release(changelog, Some("0.2.1-dev")).as_deref(), Some("0.2.0"));
    assert_eq!(schema_guard::last_release(changelog, Some("0.2.0")).as_deref(), Some("0.1.0"));
    assert_eq!(schema_guard::last_release(changelog, None).as_deref(), Some("0.2.0"));
    assert_eq!(schema_guard::last_release("## [Unreleased]\n", None), None);
}
