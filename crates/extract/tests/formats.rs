//! The format built-ins and the code pack's language queries: `manifest` (packages and their
//! dependencies), `openapi` (served endpoints on their lines), and the imports, definitions and
//! calls the code pack's `queries/*.scm` read in each of its five languages, six grammars.

mod code_pack;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use planroom_contract::{Edge, Evidence, FactBatch, Hint, LevelId, Node, NodeId, PartialReason, Ref};
use planroom_extract::declarative::grammar;
use planroom_extract::walk::Blobs;
use planroom_extract::{RepoPath, RepoView, builtin, manifest, openapi, select, verify};
use serde::Deserialize;
use serde_json::json;

/// A repo named `name` at a made-up commit, holding `files` in memory, and their sorted paths.
fn memory(name: &str, files: &[(&str, &str)]) -> (RepoView, Vec<RepoPath>) {
    let blobs: BTreeMap<String, Vec<u8>> =
        files.iter().map(|(p, t)| ((*p).to_owned(), t.as_bytes().to_vec())).collect();
    let paths = blobs.keys().cloned().map(RepoPath).collect();
    let view = RepoView {
        name: name.into(),
        root: "/nonexistent".into(),
        commit: "c".repeat(40),
        blobs: Blobs::new(blobs),
    };
    (view, paths)
}

fn kind(kind: &str, id_prefix: &str, level: &str) -> serde_json::Value {
    json!({"kind": kind, "id_prefix": id_prefix, "level": level})
}

/// Run the built-in `name` with the kinds the code pack names for it, as the engine fills them in.
fn run(view: &RepoView, name: &str, files: &[RepoPath]) -> FactBatch {
    let kinds = match name {
        "manifest" => json!({
            "package": kind("code.package", "pkg", "component"),
            "library": kind("code.library", "container", "container"),
            "repo": kind("code.repo", "repo", "system"),
            "directory": kind("code.directory", "dir", "component"),
        }),
        "openapi" => json!({
            "spec": kind("code.api_spec", "spec", "component"),
            "repo": kind("code.repo", "repo", "system"),
            "directory": kind("code.directory", "dir", "component"),
        }),
        other => panic!("no kinds for {other}"),
    };
    let extractor = builtin(name).unwrap_or_else(|| panic!("the built-in `{name}`"));
    extractor.run(view, files, &json!({ "kinds": kinds })).unwrap()
}

/// The line one evidence item cites, checked to hold its `contains` in the blob it names.
fn line_of(view: &RepoView, evidence: &Evidence) -> u32 {
    let Evidence::Line { path, line, contains: Some(contains), commit, .. } = evidence else {
        panic!("a line: {evidence:?}")
    };
    assert_eq!(commit.as_deref(), Some(view.commit.as_str()));
    let file = path.strip_prefix(&format!("{}/", view.name)).unwrap();
    let text = std::str::from_utf8(view.read(&RepoPath(file.into())).unwrap()).unwrap();
    assert!(verify::line_holds(text, *line, contains), "{path}:{line} holds {contains:?}");
    *line
}

fn lines(view: &RepoView, evidence: &[Evidence]) -> Vec<u32> {
    evidence.iter().map(|e| line_of(view, e)).collect()
}

fn node<'a>(batch: &'a FactBatch, id: &str) -> &'a Node {
    batch.nodes.iter().find(|n| n.id.0 == id).unwrap_or_else(|| panic!("{id} in {:#?}", batch.nodes))
}

/// Facts come out sorted by ID, each ID once.
fn sorted_once(batch: &FactBatch) {
    let ids = |v: Vec<&str>| assert!(v.windows(2).all(|w| w[0] < w[1]), "sorted, each once: {v:?}");
    ids(batch.nodes.iter().map(|n| n.id.0.as_str()).collect());
    ids(batch.edges.iter().map(|e| e.id.0.as_str()).collect());
    ids(batch.hints.iter().map(|h| h.id.as_str()).collect());
}

const PACKAGE_JSON: &str = r#"{
  "name": "@acme/web",
  "version": "2.3.0",
  "private": true,
  "dependencies": {
    "@acme/kit": "1.1.0",
    "express": "4.21.0"
  },
  "devDependencies": {
    "typescript": "5.6.2",
    "express": "4.21.0"
  }
}
"#;

const GO_MOD: &str = "\
module example.com/shop/api // the API service

go 1.22

require github.com/jackc/pgx/v5 v5.6.0

require (
\tgithub.com/redis/go-redis/v9 v9.6.1
\t// github.com/commented/out v1.0.0
\tgo.opentelemetry.io/otel v1.30.0 // indirect
)

replace example.com/old => ./old
";

const PYPROJECT: &str = r#"[build-system]
requires = ["setuptools>=69"]

[project]
name = "acme-worker"
version = "0.9.0"
# dependencies = ["commented==1.0"]
dependencies = [
    "redis==5.0.8",
    "psycopg[binary]==3.2.2",
]

[project.optional-dependencies]
test = ["pytest==8.3.3"]
"#;

const CARGO_TOML: &str = r#"[package]
name = "acme-cli"
version = "0.1.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
anyhow = "1"
kit = { package = "acme-kit", path = "../kit" }

[dev-dependencies]
tempfile = "3"
"#;

#[test]
fn manifests_emit_packages_and_dependencies() {
    let (view, files) = memory(
        "shop",
        &[
            ("package.json", PACKAGE_JSON),
            ("services/api/go.mod", GO_MOD),
            ("worker/pyproject.toml", PYPROJECT),
            ("tools/cli/Cargo.toml", CARGO_TOML),
            ("broken/package.json", "{ \"name\": \"broken\", \n"),
            ("noname/package.json", "{\n  \"dependencies\": { \"left-pad\": \"1.3.0\" }\n}\n"),
        ],
    );
    let batch = run(&view, "manifest", &files);
    sorted_once(&batch);

    // The root manifest makes the repo's own container, a library until a deployment says it is a
    // service, on the line that names the package.
    let container = node(&batch, "shop:container:shop");
    assert_eq!((container.kind.0.as_str(), container.level.as_str()), ("code.library", "container"));
    assert_eq!(container.parent, Some(NodeId("shop:repo:shop".into())));
    assert_eq!(container.props["role"], "library");
    assert_eq!(lines(&view, &container.evidence), [2]);
    assert_eq!(
        batch.nodes.iter().filter(|n| n.level == LevelId::from("container")).count(),
        1,
        "a nested manifest makes no container"
    );

    // Each manifest's package, named as it names itself, with the alias a dependency joins by.
    let packages = [
        ("shop:pkg:@acme/web", "shop:container:shop", 2),
        ("shop:pkg:example.com/shop/api", "shop:dir:services/api", 1),
        ("shop:pkg:acme-worker", "shop:dir:worker", 5),
        ("shop:pkg:acme-cli", "shop:dir:tools/cli", 2),
    ];
    for (id, parent, line) in packages {
        let package = node(&batch, id);
        assert_eq!((package.kind.0.as_str(), package.level.as_str()), ("code.package", "component"), "{id}");
        assert_eq!(package.parent.as_ref().map(|p| p.0.as_str()), Some(parent), "{id}");
        let name = id.splitn(3, ':').nth(2).unwrap();
        assert_eq!(package.name, name);
        assert_eq!(package.aliases, [format!("pkg:{name}")]);
        assert_eq!(lines(&view, &package.evidence), [line], "{id}");
    }

    // Every dependency, from the package that declares it to the package it names, with the
    // version it asks for and every line that asks for it; no `via` yet.
    let deps: BTreeMap<(String, String), (Option<String>, Vec<u32>)> = batch
        .edges
        .iter()
        .map(|e: &Edge| {
            assert_eq!(e.kind.0, "code.depends_on_package");
            assert_eq!(e.via, None);
            assert_eq!(e.confidence, 1.0);
            let Ref::Id(from) = &e.from else { panic!("from a node: {e:?}") };
            let Ref::Alias(to) = &e.to else { panic!("to an alias: {e:?}") };
            let uses = e.props.get("uses").map(|u| u.as_str().unwrap().to_owned());
            ((from.0.clone(), to.clone()), (uses, lines(&view, &e.evidence)))
        })
        .collect();
    let want = |from: &str, to: &str, uses: &str, lines: &[u32]| {
        let uses = (!uses.is_empty()).then(|| uses.to_owned());
        ((from.to_owned(), format!("pkg:{to}")), (uses, lines.to_vec()))
    };
    let expected: BTreeMap<_, _> = [
        want("shop:pkg:@acme/web", "@acme/kit", "1.1.0", &[6]),
        want("shop:pkg:@acme/web", "express", "4.21.0", &[7, 11]),
        want("shop:pkg:@acme/web", "typescript", "5.6.2", &[10]),
        want("shop:pkg:example.com/shop/api", "github.com/jackc/pgx/v5", "v5.6.0", &[5]),
        want("shop:pkg:example.com/shop/api", "github.com/redis/go-redis/v9", "v9.6.1", &[8]),
        want("shop:pkg:example.com/shop/api", "go.opentelemetry.io/otel", "v1.30.0", &[10]),
        want("shop:pkg:acme-worker", "redis", "==5.0.8", &[9]),
        want("shop:pkg:acme-worker", "psycopg", "==3.2.2", &[10]),
        want("shop:pkg:acme-worker", "pytest", "==8.3.3", &[14]),
        want("shop:pkg:acme-cli", "serde", "1.0", &[6]),
        want("shop:pkg:acme-cli", "anyhow", "1", &[7]),
        // A dependency by path asks for no version.
        want("shop:pkg:acme-cli", "acme-kit", "", &[8]),
        want("shop:pkg:acme-cli", "tempfile", "3", &[11]),
        // A manifest that names no package still says what the repo depends on.
        want("shop:repo:shop", "left-pad", "1.3.0", &[2]),
    ]
    .into_iter()
    .collect();
    assert_eq!(deps, expected);

    // A comment names no dependency, and a manifest that doesn't parse is partial: the rest stand.
    assert!(!deps.keys().any(|(_, to)| to.contains("commented")));
    let partial: Vec<&str> = batch.partial.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(partial, ["shop/broken/package.json"]);

    // Deterministic, and it reads only the files it is given.
    assert_eq!(
        serde_json::to_string(&batch).unwrap(),
        serde_json::to_string(&run(&view, "manifest", &files)).unwrap()
    );
    let some = run(&view, "manifest", &select(&files, &["**/go.mod"]).unwrap());
    assert!(some.nodes.iter().all(|n| n.id.0 == "shop:pkg:example.com/shop/api"), "{:?}", some.nodes);
}

#[test]
fn manifests_that_nest_or_alias_past_their_bounds_are_partial() {
    let deep_json =
        format!("{{\"name\": \"deep\", \"x\": {}1{}}}\n", "[".repeat(100_000), "]".repeat(100_000));
    let deep_toml =
        format!("[package]\nname = \"deep\"\nx = {}1{}\n", "[".repeat(100_000), "]".repeat(100_000));
    // An anchor on a long string, named again and again: each alias would copy it.
    let mut bomb = format!("name: bomb\nbig: &big \"{}\"\nmany: [", "x".repeat(200_000));
    bomb.push_str(&"*big, ".repeat(100_000));
    bomb.push_str("]\n");
    let (view, files) = memory(
        "r",
        &[("a/package.json", &deep_json), ("b/Cargo.toml", &deep_toml), ("c/package.json", &bomb)],
    );
    let batch = run(&view, "manifest", &files);
    let partial: Vec<&str> = batch.partial.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(partial, ["r/a/package.json", "r/b/Cargo.toml", "r/c/package.json"]);
    assert!(batch.nodes.is_empty() && batch.edges.is_empty());
}

const OPENAPI_YAML: &str = "\
openapi: 3.0.3
info:
  title: Orders API
  version: 1.5.0
# /commented:
#   get: {}
paths:
  /orders:
    post:
      operationId: createOrder
    get:
      operationId: listOrders
  /orders/{id}:
    parameters:
      - name: id
        in: path
    get:
      operationId: getOrder
  /shared:
    $ref: '#/paths/~1orders'
components:
  schemas:
    Loop:
      $ref: '#/components/schemas/Loop'
";

const SWAGGER_JSON: &str = r#"{
  "swagger": "2.0",
  "basePath": "/v1",
  "paths": {
    "/healthz": {
      "get": { "operationId": "healthz" }
    }
  }
}
"#;

#[test]
fn openapi_emits_endpoints_on_their_lines() {
    let (view, files) = memory(
        "orders",
        &[
            ("api/openapi.yaml", OPENAPI_YAML),
            ("legacy/swagger.json", SWAGGER_JSON),
            ("bad/openapi.yaml", "paths: [unclosed\n"),
        ],
    );
    let batch = run(&view, "openapi", &files);
    sorted_once(&batch);

    // The spec itself, under the directory that holds it, on its version line.
    let spec = node(&batch, "orders:spec:api/openapi.yaml");
    assert_eq!((spec.kind.0.as_str(), spec.level.as_str()), ("code.api_spec", "component"));
    assert_eq!(spec.parent, Some(NodeId("orders:dir:api".into())));
    assert_eq!(spec.name, "openapi.yaml");
    assert_eq!(lines(&view, &spec.evidence), [1]);
    assert_eq!(
        node(&batch, "orders:spec:legacy/swagger.json").parent,
        Some(NodeId("orders:dir:legacy".into()))
    );

    // Each endpoint a `code.route_serve` hint keyed by its path, citing the path's line, then the
    // method's; a path item that only refers elsewhere serves nothing here, and a $ref loop
    // stalls nothing.
    let served: BTreeSet<(String, String, String, Vec<u32>)> = batch
        .hints
        .iter()
        .map(|h: &Hint| {
            assert_eq!(h.kind.0, "code.route_serve");
            let attr = |k: &str| h.attrs[k].as_str().unwrap().to_owned();
            assert_eq!(attr("path"), h.key);
            (h.scope.0.clone(), attr("method"), h.key.clone(), lines(&view, &h.evidence))
        })
        .collect();
    let want = |scope: &str, method: &str, path: &str, lines: &[u32]| {
        (format!("orders:spec:{scope}"), method.to_owned(), path.to_owned(), lines.to_vec())
    };
    let expected = BTreeSet::from([
        want("api/openapi.yaml", "POST", "/orders", &[8, 9]),
        want("api/openapi.yaml", "GET", "/orders", &[8, 11]),
        want("api/openapi.yaml", "GET", "/orders/{id}", &[13, 17]),
        // Swagger 2's base path comes before each path.
        want("legacy/swagger.json", "GET", "/v1/healthz", &[5, 6]),
    ]);
    assert_eq!(served, expected);
    let partial: Vec<&str> = batch.partial.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(partial, ["orders/bad/openapi.yaml"]);
    assert_eq!(
        serde_json::to_string(&batch).unwrap(),
        serde_json::to_string(&run(&view, "openapi", &files)).unwrap()
    );
}

#[test]
fn every_operation_of_a_path_item_is_its_own_endpoint_whatever_line_it_sits_on() {
    // A spec dumped on one line, as frameworks often write it, and a YAML flow mapping.
    let one_line = r#"{"openapi":"3.0.3","paths":{"/orders":{"get":{},"post":{}},"/orders/{id}":{"get":{},"delete":{}}}}"#;
    let flow = "openapi: 3.0.3\npaths: {/items: {get: {}, put: {}, patch: {}}}\n";
    let (view, files) = memory("o", &[("a/openapi.json", one_line), ("b/openapi.yaml", flow)]);
    let batch = run(&view, "openapi", &files);
    sorted_once(&batch);
    let served: BTreeSet<(String, String, String)> = batch
        .hints
        .iter()
        .map(|h| {
            lines(&view, &h.evidence);
            (h.scope.0.clone(), h.attrs["method"].as_str().unwrap().to_owned(), h.key.clone())
        })
        .collect();
    let want = |scope: &str, method: &str, path: &str| {
        (format!("o:spec:{scope}"), method.to_owned(), path.to_owned())
    };
    let expected = BTreeSet::from([
        want("a/openapi.json", "GET", "/orders"),
        want("a/openapi.json", "POST", "/orders"),
        want("a/openapi.json", "GET", "/orders/{id}"),
        want("a/openapi.json", "DELETE", "/orders/{id}"),
        want("b/openapi.yaml", "GET", "/items"),
        want("b/openapi.yaml", "PUT", "/items"),
        want("b/openapi.yaml", "PATCH", "/items"),
    ]);
    assert_eq!(served, expected);
    assert_eq!(batch.hints.len(), expected.len(), "one hint per operation");
}

#[test]
fn a_byte_order_mark_before_a_manifest_or_a_spec_is_read_past() {
    let bom = |text: &str| format!("\u{feff}{text}");
    let (package, cargo, spec) = (bom(PACKAGE_JSON), bom(CARGO_TOML), bom(SWAGGER_JSON));
    let (view, files) = memory(
        "shop",
        &[("web/package.json", &package), ("kit/Cargo.toml", &cargo), ("api/swagger.json", &spec)],
    );
    let manifests = run(&view, "manifest", &select(&files, manifest::DEFAULT_FILES).unwrap());
    assert!(manifests.partial.is_empty(), "{:?}", manifests.partial);
    let web = node(&manifests, "shop:pkg:@acme/web");
    // The mark sits on line 1 alone: the name's line is the same, and its evidence verifies.
    assert_eq!(lines(&view, &web.evidence), [2]);
    let kit = manifests
        .nodes
        .iter()
        .find(|n| n.kind.0 == "code.package" && n.parent.as_ref().is_some_and(|p| p.0 == "shop:dir:kit"));
    let kit = kit.unwrap_or_else(|| panic!("the crate's package in {:#?}", manifests.nodes));
    assert_eq!(lines(&view, &kit.evidence), [2]);
    assert!(manifests.edges.iter().any(|e| e.from == Ref::Id(web.id.clone())), "web's dependencies");
    let specs = run(&view, "openapi", &select(&files, openapi::DEFAULT_FILES).unwrap());
    assert!(specs.partial.is_empty(), "{:?}", specs.partial);
    assert_eq!(lines(&view, &node(&specs, "shop:spec:api/swagger.json").evidence), [2]);
    assert_eq!(specs.hints.len(), 1);
}

#[test]
fn a_package_name_no_node_id_can_hold_gives_no_package_node() {
    let npm = "{\n  \"name\": \"a\\nb\\\\c\",\n  \"dependencies\": {\"left-pad\": \"1.3.0\"}\n}\n";
    let cargo = "[package]\nname = 'x\\y'\n\n[dependencies]\nserde = \"1\"\n";
    let (view, files) = memory("weird", &[("package.json", npm), ("sub/Cargo.toml", cargo)]);
    let batch = run(&view, "manifest", &files);
    for n in &batch.nodes {
        assert!(!n.id.0.contains('\n') && !n.id.0.contains('\\'), "{:?}", n.id);
    }
    assert!(batch.nodes.is_empty(), "no package, and no container from it: {:#?}", batch.nodes);
    // Each manifest is partial, and its dependencies hang from the repo, as a nameless one's do.
    let partial: Vec<(&str, Option<PartialReason>)> =
        batch.partial.iter().map(|p| (p.path.as_str(), p.reason)).collect();
    assert_eq!(partial, [("weird/package.json", None), ("weird/sub/Cargo.toml", None)]);
    let repo = Ref::Id(NodeId("weird:repo:weird".into()));
    assert!(batch.edges.iter().all(|e| e.from == repo), "{:#?}", batch.edges);
    assert_eq!(batch.edges.len(), 2);
}

/// A `package.json` of `count` dependencies, each on its own short line.
fn dense_package_json(name: &str, count: usize) -> String {
    let deps: Vec<String> = (0..count).map(|i| format!("\"d{i}\":\"1\"")).collect();
    format!("{{\"name\":\"{name}\",\"dependencies\":{{\n{}\n}}}}\n", deps.join(",\n"))
}

#[test]
fn format_facts_past_a_files_capture_budget_are_refused() {
    // A dependency a line of a few bytes: its edge holds far more heap than the line's bytes.
    let dense = dense_package_json("dense", 60_000);
    // A spec of one anchored path item of eight operations, aliased under thousands of paths,
    // within the YAML reader's copy cap: each alias gives eight endpoints.
    let mut aliased = String::from(
        "openapi: 3.0.3\nx-item: &item {get: {}, put: {}, post: {}, delete: {}, options: {}, head: {}, patch: {}, trace: {}}\npaths:\n",
    );
    for i in 0..5_000 {
        aliased.push_str(&format!("  /p{i}: *item\n"));
    }
    let (view, files) = memory(
        "r",
        &[
            ("dense/package.json", &dense),
            ("web/package.json", PACKAGE_JSON),
            ("api/openapi.yaml", &aliased),
            ("ok/openapi.yaml", OPENAPI_YAML),
        ],
    );
    let manifests = run(&view, "manifest", &select(&files, manifest::DEFAULT_FILES).unwrap());
    let specs = run(&view, "openapi", &select(&files, openapi::DEFAULT_FILES).unwrap());
    let reasons = |batch: &FactBatch| -> Vec<(String, Option<PartialReason>)> {
        batch.partial.iter().map(|p| (p.path.clone(), p.reason)).collect()
    };
    assert_eq!(reasons(&manifests), [("r/dense/package.json".to_owned(), Some(PartialReason::WorkLimit))]);
    assert_eq!(reasons(&specs), [("r/api/openapi.yaml".to_owned(), Some(PartialReason::WorkLimit))]);
    // A refused file gives no fact; the everyday files beside it keep theirs.
    assert!(manifests.nodes.iter().all(|n| !n.id.0.contains("dense")), "{:?}", manifests.nodes);
    for edge in &manifests.edges {
        assert!(
            edge.evidence
                .iter()
                .all(|ev| matches!(ev, Evidence::Line { path, .. } if path.starts_with("r/web/")))
        );
    }
    assert_eq!(manifests.edges.len(), 3, "web's three dependencies");
    assert!(specs.nodes.iter().all(|n| n.id.0 != "r:spec:api/openapi.yaml"));
    assert!(specs.hints.iter().all(|h| h.scope.0 == "r:spec:ok/openapi.yaml"));
    assert_eq!(specs.hints.len(), 3);
}

#[test]
fn format_facts_past_the_repo_bound_cut_in_sorted_path_order() {
    use planroom_extract::declarative::{CAPTURE_FLOOR, REPO_CAPTURE_MAX};
    let packages: Vec<(String, String)> =
        (0..6).map(|i| (format!("p{i}/package.json"), PACKAGE_JSON.to_owned())).collect();
    let specs: Vec<(String, String)> =
        (0..6).map(|i| (format!("s{i}/openapi.yaml"), OPENAPI_YAML.to_owned())).collect();
    let all: Vec<(&str, &str)> =
        packages.iter().chain(&specs).map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let (view, files) = memory("r", &all);
    let kinds = |name: &str| -> serde_json::Value {
        match name {
            "manifest" => json!({"kinds": {
                "package": kind("code.package", "pkg", "component"),
                "library": kind("code.library", "container", "container"),
                "repo": kind("code.repo", "repo", "system"),
                "directory": kind("code.directory", "dir", "component"),
            }}),
            _ => json!({"kinds": {
                "spec": kind("code.api_spec", "spec", "component"),
                "repo": kind("code.repo", "repo", "system"),
                "directory": kind("code.directory", "dir", "component"),
            }}),
        }
    };
    // Each file charges less than the floor, so at least the first fits in a bound of one floor,
    // and some later file's charge would pass it: the cut falls there, and every file after it
    // is cut too.
    let max = CAPTURE_FLOOR;
    assert!(max < REPO_CAPTURE_MAX);
    let m = select(&files, manifest::DEFAULT_FILES).unwrap();
    let s = select(&files, openapi::DEFAULT_FILES).unwrap();
    let batches = [
        manifest::Manifest.run_with_repo_capture_max(&view, &m, &kinds("manifest"), max).unwrap(),
        openapi::OpenApi.run_with_repo_capture_max(&view, &s, &kinds("openapi"), max).unwrap(),
    ];
    for (batch, prefix) in batches.iter().zip(["p", "s"]) {
        let cut: Vec<(&str, Option<PartialReason>)> =
            batch.partial.iter().map(|p| (p.path.as_str(), p.reason)).collect();
        let first_cut = cut.first().map(|(p, _)| *p).unwrap_or_else(|| panic!("{prefix}: a cut"));
        assert!(cut.iter().all(|(_, r)| *r == Some(PartialReason::RepoLimit)), "{cut:?}");
        // The cut is a suffix of the sorted paths, after at least one kept file.
        let paths: Vec<String> = (0..6).map(|i| format!("r/{prefix}{i}/")).collect();
        let at = paths.iter().position(|p| first_cut.starts_with(p.as_str())).unwrap();
        assert!(at >= 1, "{cut:?}");
        assert_eq!(cut.len(), 6 - at, "every file from the cut on: {cut:?}");
        // No cut file gave a fact.
        let facts = serde_json::to_string(&(&batch.nodes, &batch.edges, &batch.hints)).unwrap();
        for p in &paths[at..] {
            let evidence = format!("\"path\":\"{p}");
            assert!(!facts.contains(&evidence), "{p} gave a fact");
        }
    }
    // Under the real bound nothing is cut.
    assert!(run(&view, "manifest", &m).partial.is_empty());
    assert!(run(&view, "openapi", &s).partial.is_empty());
}

/// A fixture repo's `main` as `examples/build-fixture` builds it: each commit's overlay on `main`,
/// in name order, its files written and its `delete` list removed.
fn fixture_main(repo: &str) -> BTreeMap<String, String> {
    #[derive(Deserialize)]
    struct Commit {
        branch: Option<String>,
        #[serde(default)]
        delete: Vec<String>,
    }
    fn files(dir: &Path, base: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files(&path, base, out);
            } else {
                let rel = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read_to_string(&path).unwrap());
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/fixture/repos").join(repo);
    let mut steps: Vec<PathBuf> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).collect();
    steps.sort();
    let mut tree = BTreeMap::new();
    for step in steps {
        let commit: Commit =
            toml::from_str(&std::fs::read_to_string(step.join("COMMIT.toml")).unwrap()).unwrap();
        if commit.branch.as_deref().is_some_and(|b| b != "main") {
            continue;
        }
        for gone in &commit.delete {
            tree.remove(gone);
        }
        if step.join("tree").is_dir() {
            files(&step.join("tree"), &step.join("tree"), &mut tree);
        }
    }
    tree
}

/// A hint as the fixture test reads it: its file and line, kind, key, value and attributes.
type Seen = (String, u32, String, String, Option<String>, BTreeMap<String, String>);

#[test]
fn the_fixture_code_gives_the_hints_its_relations_join() {
    let pack = code_pack::load();
    let programs = pack.programs();
    let mut hints: Vec<Seen> = Vec::new();
    for repo in ["storefront", "orders", "worker", "kit"] {
        let tree = fixture_main(repo);
        let files: Vec<(&str, &str)> = tree.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
        let (view, paths) = memory(repo, &files);
        for program in &programs {
            let globs: Vec<&str> = program.files().iter().map(String::as_str).collect();
            let batch = program.run(&view, &select(&paths, &globs).unwrap());
            let refused: Vec<_> = batch.partial.iter().filter(|p| p.reason.is_some()).collect();
            assert!(refused.is_empty(), "{}: no fixture file is refused: {refused:?}", program.name());
            for h in batch.hints {
                let line = line_of(&view, &h.evidence[0]);
                let Evidence::Line { path, .. } = &h.evidence[0] else { unreachable!() };
                let attrs =
                    h.attrs.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned())).collect();
                hints.push((path.clone(), line, h.kind.0.clone(), h.key, h.value, attrs));
            }
        }
    }
    let at = |path: &str, line: u32, kind: &str| -> Vec<(String, Option<String>, BTreeMap<String, String>)> {
        hints
            .iter()
            .filter(|(p, l, k, ..)| p == path && *l == line && k == kind)
            .map(|(.., key, value, attrs)| (key.clone(), value.clone(), attrs.clone()))
            .collect()
    };
    let attrs = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    };
    let one = |path: &str, line: u32, kind: &str, key: &str, value: Option<&str>, want: &[(&str, &str)]| {
        let got = at(path, line, kind);
        let want = (key.to_owned(), value.map(str::to_owned), attrs(want));
        assert!(got.contains(&want), "{path}:{line} {kind}: {got:?} holds {want:?}");
    };

    // storefront to orders: the client's call, and the address compose sets on it.
    let orders_ts = "storefront/src/api/orders.ts";
    one(
        orders_ts,
        26,
        "code.route_call",
        "`${this.baseUrl}/orders`",
        None,
        &[("method", "'POST'"), ("path", "`${this.baseUrl}/orders`")],
    );
    let get = "`${this.baseUrl}/orders/${encodeURIComponent(id)}`";
    one(orders_ts, 36, "code.route_call", get, None, &[("method", "GET"), ("path", get)]);
    one(
        "storefront/src/app/config.ts",
        12,
        "code.env_read",
        "ORDERS_URL",
        None,
        &[("name", "ORDERS_URL"), ("default", "http://localhost:8080")],
    );
    one(
        "storefront/src/app/config.ts",
        12,
        "code.url",
        "localhost",
        None,
        &[("scheme", "http"), ("host", "localhost"), ("port", "8080")],
    );
    // healthcheck to storefront: the route the storefront serves.
    one(
        "storefront/src/server.ts",
        17,
        "code.route_serve",
        "/healthz",
        None,
        &[("method", "get"), ("path", "/healthz")],
    );
    // The orders service: its routes, its queue and its database.
    let handlers = "orders/internal/http/handlers.go";
    one(handlers, 37, "code.route_serve", "/orders", None, &[("method", "POST"), ("path", "/orders")]);
    one(handlers, 125, "code.call", "enqueueFulfil", Some("(r.Context(), o)"), &[]);
    one(
        "orders/internal/queue/publisher.go",
        49,
        "code.queue_op",
        "JobsKey",
        None,
        &[("op", "publish"), ("key", "JobsKey")],
    );
    one("orders/internal/queue/publisher.go", 14, "code.const", "JobsKey", Some("order-jobs"), &[]);
    one("orders/internal/store/postgres.go", 26, "code.db_open", "pgxpool", None, &[("driver", "pgxpool")]);
    one(
        "orders/internal/config/config.go",
        22,
        "code.env_read",
        "DATABASE_URL",
        None,
        &[("name", "DATABASE_URL")],
    );
    // The worker: the queue it consumes and the database it writes.
    one(
        "worker/worker/consumer.py",
        41,
        "code.queue_op",
        "JOBS_KEY",
        None,
        &[("op", "consume"), ("key", "JOBS_KEY")],
    );
    one("worker/worker/consumer.py", 15, "code.const", "JOBS_KEY", Some("order-jobs"), &[]);
    one("worker/worker/db.py", 22, "code.db_open", "psycopg", None, &[("driver", "psycopg")]);
    one("worker/worker/db.py", 46, "code.sql", "orders", None, &[("verb", "UPDATE"), ("table", "orders")]);
    one("worker/worker/config.py", 24, "code.env_read", "REDIS_URL", None, &[("name", "REDIS_URL")]);

    // The comment that names the list the orders service pushes onto, and the docstring that
    // names orders-db, give nothing.
    assert_eq!(at("worker/worker/consumer.py", 14, "code.queue_op"), []);
    let docstring: Vec<_> = hints.iter().filter(|(p, l, ..)| p == "worker/worker/db.py" && *l <= 5).collect();
    assert_eq!(docstring, Vec::<&Seen>::new());
}

/// Code in each language: two imports, a definition, calls, and an import, a call and a URL
/// that only a comment or a docstring names.
const CODE: &[(&str, &str, &str)] = &[
    (
        "typescript",
        "web/src/api.ts",
        "import { formatMoney } from '@acme/kit';\n\
         import type { Order } from './orders.js';\n\
         // import { old } from 'commented-out';\n\
         /** Calls ghost('x') in a doc comment. */\n\
         export function total(order: Order): string {\n  \
         return formatMoney(order.total /* http://ghost:1/x */, 'USD');\n\
         }\n",
    ),
    (
        "tsx",
        "web/src/App.tsx",
        "import React from 'react';\n\
         import { total } from './api';\n\
         // import Old from 'commented-out';\n\
         export function App({ order }) {\n  \
         return <p>{total(order)}</p>; // ghost(order)\n\
         }\n",
    ),
    (
        "javascript",
        "web/lib/server.js",
        "const express = require('express');\n\
         import { route } from './routes.js';\n\
         // const legacy = require('commented-out');\n\
         function start(port) {\n  \
         express().listen(port); /* ghost() */\n\
         }\n",
    ),
    (
        "python",
        "worker/worker/jobs.py",
        "\"\"\"Jobs: import commented_out; ghost().\"\"\"\n\
         import json\n\
         from worker.db import set_order_state\n\
         # from commented_out import thing\n\
         def ship(order_id, tracking=None):\n    \
         \"\"\"Ship it: calls ghost(order_id).\"\"\"\n    \
         set_order_state(order_id, \"shipped\")\n    \
         return json.dumps({})\n",
    ),
    (
        "go",
        "orders/internal/http/handlers.go",
        "package http\n\
         \n\
         import (\n\
         \t\"encoding/json\"\n\
         \t\"github.com/acme-shop/orders/internal/store\"\n\
         \t// \"commented-out\"\n\
         )\n\
         \n\
         // createOrder calls ghost() in a comment.\n\
         func createOrder(st *store.Store) error {\n\
         \treturn json.NewEncoder(nil).Encode(st.Create())\n\
         }\n",
    ),
    (
        "rust",
        "kit/src/lib.rs",
        "use std::collections::BTreeMap;\n\
         use crate::money::format;\n\
         use crate::{\n    \
         tax::{self, rate}, // the rate table\n    \
         ledger as books,\n\
         };\n\
         use serde::*;\n\
         // use commented_out::thing;\n\
         /// Calls `ghost()` in a doc comment.\n\
         pub fn total(items: &BTreeMap<String, u32>) -> String {\n    \
         format(items.values().sum())\n\
         }\n",
    ),
];

/// A language, its imports with their lines, its definitions and its calls.
type Want = (&'static str, &'static [(&'static str, u32)], &'static [&'static str], &'static [&'static str]);

/// What one language's code gives: (imports with their lines, definitions, calls).
const WANT: &[Want] = &[
    ("typescript", &[("@acme/kit", 1), ("./orders.js", 2)], &["total"], &["formatMoney"]),
    ("tsx", &[("react", 1), ("./api", 2)], &["App"], &["total"]),
    ("javascript", &[("express", 1), ("./routes.js", 2)], &["start"], &["express", "listen", "require"]),
    ("python", &[("json", 2), ("worker.db", 3)], &["ship"], &["dumps", "set_order_state"]),
    (
        "go",
        &[("encoding/json", 4), ("github.com/acme-shop/orders/internal/store", 5)],
        &["createOrder"],
        &["Create", "Encode", "NewEncoder"],
    ),
    (
        "rust",
        &[
            ("std::collections::BTreeMap", 1),
            ("crate::money::format", 2),
            // Each path of a group, prefixed, on the line its `use` starts on; never the group.
            ("crate::tax", 3),
            ("crate::tax::rate", 3),
            ("crate::ledger", 3),
            ("serde", 7),
        ],
        &["total"],
        &["format", "sum", "values"],
    ),
];

#[test]
fn imports_and_calls_for_each_language() {
    let pack = code_pack::load();
    let files: Vec<(&str, &str)> = CODE.iter().map(|(_, path, text)| (*path, *text)).collect();
    let (view, paths) = memory("shop", &files);
    for (language, imports, defs, calls) in WANT {
        let programs = pack.queries(language);
        assert!(!programs.is_empty(), "{language}: the code pack queries it");
        let (_, path, _) = CODE.iter().find(|(l, _, _)| l == language).unwrap();
        let file = format!("shop:file:{path}");
        let (mut edges, mut hints) = (Vec::new(), Vec::new());
        for program in &programs {
            let globs: Vec<&str> = program.files().iter().map(String::as_str).collect();
            let mine = select(&paths, &globs).unwrap();
            assert_eq!(mine, [RepoPath((*path).into())], "{language}: its files and no other language's");
            let one = program.run(&view, &mine);
            assert!(one.partial.is_empty(), "{language}: {:?}", one.partial);
            assert_eq!(
                serde_json::to_string(&one).unwrap(),
                serde_json::to_string(&program.run(&view, &mine)).unwrap()
            );
            assert!(one.nodes.is_empty(), "{language}: the files built-in names the file");
            edges.extend(one.edges);
            hints.extend(one.hints);
        }

        // Each import a `code.import` hint scoped to the file, keyed by the specifier as written,
        // on its line, with the language's separator where its first segment names a package: a
        // linker joins it, so no import is an edge of the file's own.
        assert!(edges.iter().all(|e| e.kind.0 != "code.imports"), "{language}: {edges:?}");
        let separator = match *language {
            "python" => Some("."),
            "rust" => Some("::"),
            _ => None,
        };
        let got: BTreeSet<(String, u32)> = hints
            .iter()
            .filter(|h| h.kind.0 == "code.import")
            .map(|h| {
                assert_eq!(h.scope.0, file, "{language}");
                assert_eq!(h.attrs.get("separator").and_then(|s| s.as_str()), separator, "{language}: {h:?}");
                let [evidence] = &h.evidence[..] else { panic!("{language}: one line: {h:?}") };
                (h.key.clone(), line_of(&view, evidence))
            })
            .collect();
        let want: BTreeSet<(String, u32)> = imports.iter().map(|(m, l)| ((*m).to_owned(), *l)).collect();
        assert_eq!(got, want, "{language}: imports");

        // Each definition and each call a hint scoped to the file, keyed by its name, with no map
        // of attributes: a definition's value is its parameters and a call's its arguments, as
        // written, with no comment in them.
        let named = |kind: &str| -> BTreeSet<String> {
            hints
                .iter()
                .filter(|h| h.kind.0 == kind)
                .map(|h| {
                    assert_eq!(h.scope.0, file, "{language}");
                    assert!(h.attrs.is_empty(), "{language}: {h:?}");
                    let value = h.value.as_deref().unwrap_or_else(|| panic!("{language}: a value: {h:?}"));
                    assert!(value.starts_with('(') && !value.contains("ghost"), "{language}: {value}");
                    line_of(&view, &h.evidence[0]);
                    h.key.clone()
                })
                .collect()
        };
        let set = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<BTreeSet<_>>();
        assert_eq!(named("code.def"), set(defs), "{language}: definitions");
        assert_eq!(named("code.call"), set(calls), "{language}: calls");

        // Nothing comes from a comment or a docstring: no fact holds what only they name, the
        // lines its evidence cites aside.
        for edge in &mut edges {
            edge.evidence.clear();
        }
        for hint in &mut hints {
            hint.evidence.clear();
        }
        let text = serde_json::to_string(&(&edges, &hints)).unwrap();
        assert!(!text.contains("commented") && !text.contains("ghost"), "{language}: {text}");
    }
}

/// An `@name` in an emit that names no capture of its query is text, which the runner keeps as
/// written: a misspelt capture would make a fact from every match. So every one the code pack's
/// query extractors write names a capture of their query, and their only text `@`s are references.
#[test]
fn every_capture_an_emit_names_is_one_its_query_sets() {
    let pack = code_pack::load();
    let all = pack.queries_and_templates();
    assert!(all.len() >= 12, "two queries for each of six grammars: {}", all.len());
    for (name, text, templates) in all {
        let language = name.trim_start_matches("code/").trim_end_matches("_runtime");
        let grammar = grammar::language(language).unwrap_or_else(|| panic!("{name}: a grammar"));
        let query = tree_sitter::Query::new(&grammar, &text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let captures: BTreeSet<&str> = query.capture_names().iter().copied().collect();
        for template in &templates {
            let mut rest = template.as_str();
            while let Some(at) = rest.find('@') {
                rest = &rest[at + 1..];
                if ["id:", "alias:", "ext:"].iter().any(|r| rest.starts_with(r)) {
                    continue;
                }
                let end = rest
                    .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
                    .unwrap_or(rest.len());
                let named = &rest[..end];
                assert!(captures.contains(named), "{name}: `{template}` names @{named}, no capture");
            }
        }
    }
}

/// Rust's inline test code: each item `#[cfg(test)]` marks, wherever it sits and whatever it is,
/// is a `code.test_scope` from its attribute's line to the item's last, whatever attributes and
/// comments stand between them; a module whose inner `#![cfg(test)]` marks it is one, and so is a
/// file that `#![cfg(test)]` opens, and a match arm or a statement it marks. An item a `cfg`
/// other than `test` marks is runtime code, and the runtime line after a scope stays outside it.
#[test]
fn rust_test_scopes_span_each_item_cfg_test_marks() {
    let lib = "\
pub fn consume(conn: &mut Conn) {
    conn.brpop(\"jobs\", 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes() {
        conn.lpush(\"probe-jobs\", 1);
    }
}
pub fn after(conn: &mut Conn) { conn.lpush(\"after-jobs\", 1); }

#[cfg(test)]
#[allow(dead_code)]
// A helper only the tests call.
fn helper(conn: &mut Conn) {
    conn.lpush(\"fn-jobs\", 1);
}

mod outer {
    #[cfg(test)]
    use std::fmt;
    #[cfg(test)]
    const KEY: &str = \"const-jobs\";
    #[cfg(test)]
    impl Probe {
        fn go(&self) {}
    }
    mod inner {
        #![cfg(test)]
        fn go() {}
    }
    pub fn runtime(conn: &mut Conn) { conn.lpush(\"outer-jobs\", 1); }
}

#[cfg(not(test))]
fn not_test() {}
#[cfg(feature = \"test\")]
fn feature() {}
#[cfg(test)] fn one_line() {}
fn arms(x: Option<u8>, conn: &mut Conn) {
    match x {
        #[cfg(test)]
        Some(_) => {
            conn.lpush(\"arm-jobs\", 1);
        }
        None => {}
    }
    #[cfg(test)]
    conn.lpush(\"statement-jobs\", 1);
}
";
    let whole = "#![cfg(test)]\n\nfn a() {}\n\nfn b() {}\n";
    let files = [("src/lib.rs", lib), ("src/probe.rs", whole)];
    let (view, paths) = memory("worker", &files);
    let pack = code_pack::load();
    let mut scopes: BTreeSet<(String, u32, String)> = BTreeSet::new();
    for program in pack.queries("rust") {
        let batch = program.run(&view, &paths);
        assert!(batch.partial.is_empty(), "{:?}", batch.partial);
        // A module's declaration names its module too: rust_test_modules_name_their_files reads
        // those.
        for hint in
            batch.hints.iter().filter(|h| h.kind.0 == "code.test_scope" && !h.attrs.contains_key("module"))
        {
            let Evidence::Line { path, .. } = &hint.evidence[0] else { unreachable!() };
            assert_eq!(hint.scope.0, format!("worker:file:{}", path.trim_start_matches("worker/")));
            let end = hint.attrs["end"].as_str().unwrap().to_owned();
            assert_eq!(hint.key, format!("test:{end}"));
            scopes.insert((path.clone(), line_of(&view, &hint.evidence[0]), end));
        }
    }
    let want: BTreeSet<(String, u32, String)> = [
        ("worker/src/lib.rs", 5, "13"),
        // The `#[test]` function inside it.
        ("worker/src/lib.rs", 9, "12"),
        // The function, and the shorter span the attribute after `#[cfg(test)]` gives inside it.
        ("worker/src/lib.rs", 16, "17"),
        ("worker/src/lib.rs", 16, "21"),
        ("worker/src/lib.rs", 24, "25"),
        ("worker/src/lib.rs", 26, "27"),
        ("worker/src/lib.rs", 28, "31"),
        ("worker/src/lib.rs", 32, "35"),
        ("worker/src/lib.rs", 43, "43"),
        // A match arm, and a statement in a function's body.
        ("worker/src/lib.rs", 46, "47"),
        ("worker/src/lib.rs", 46, "49"),
        ("worker/src/lib.rs", 52, "53"),
        ("worker/src/probe.rs", 1, "5"),
    ]
    .iter()
    .map(|(p, l, e)| ((*p).to_owned(), *l, (*e).to_owned()))
    .collect();
    assert_eq!(scopes, want);
}

/// Rust's test functions and test modules: an item `#[test]` marks, or an attribute whose path
/// has `test` for its last segment does, is a test scope as a `#[cfg(test)]` item is, while
/// `#[test_case]` isn't. A raw string names a module's path as a string does.
/// A module `#[cfg(test)]` marks at a file's top level also gives a scope that names it, keyed
/// `module:<name>:<end>`, and one keyed `path:<name>:<end>:<path>` with the file its `#[path]` names,
/// whichever side of `#[cfg(test)]` that stands on. A module only a `#[path]` or another `cfg`
/// marks, and one nested in a module, names none.
#[test]
fn rust_test_modules_name_their_files() {
    let lib = "\
pub fn run(c: &mut C) { c.lpush(\"rt\", 1); }

#[test]
fn bare(c: &mut C) {
    c.lpush(\"bare\", 1);
}

#[tokio::test(flavor = \"multi_thread\")]
async fn tokio_test() {
    probe();
}
#[test_case(1)]
fn not_a_test_attr() {}
#[cfg(test)]
mod test;
#[cfg(test)]
#[path = \"lib_tests.rs\"]
mod lib_tests;
#[path = \"other.rs\"]
/// Docs.
#[cfg(test)]
mod other_tests;
#[cfg(test)]
mod inline {
    #[cfg(test)]
    mod nested;
}
#[path = \"runtime.rs\"]
mod runtime;
mod plain;
#[cfg(not(test))]
mod not_test;
#[cfg(test)]
#[path = r\"raw_tests.rs\"]
mod raw;
#[cfg(test)]
use std::fmt;
mod after_use;
#[cfg(test)]
fn helper() {}
#[path = \"p.rs\"]
mod after_fn;
";
    let (view, paths) = memory("worker", &[("src/lib.rs", lib)]);
    let pack = code_pack::load();
    let mut spans: BTreeSet<(u32, String)> = BTreeSet::new();
    let mut modules: BTreeSet<(String, u32, String, Option<String>, bool)> = BTreeSet::new();
    for program in pack.queries("rust") {
        let batch = program.run(&view, &paths);
        assert!(batch.partial.is_empty(), "{:?}", batch.partial);
        for hint in batch.hints.iter().filter(|h| h.kind.0 == "code.test_scope") {
            let line = line_of(&view, &hint.evidence[0]);
            let end = hint.attrs["end"].as_str().unwrap().to_owned();
            match hint.attrs.get("module") {
                None => {
                    spans.insert((line, end));
                }
                Some(module) => {
                    let path = hint.attrs.get("path").map(|p| p.as_str().unwrap().to_owned());
                    let inline = hint.attrs.contains_key("inline");
                    let module = module.as_str().unwrap();
                    let key = match &path {
                        Some(path) => format!("path:{module}:{end}:{path}"),
                        None if inline => format!("inline:{module}:{end}"),
                        None => format!("module:{module}:{end}"),
                    };
                    assert_eq!(hint.key, key);
                    modules.insert((module.to_owned(), line, end, path, inline));
                }
            }
        }
    }
    for want in [(3, "6"), (8, "11")] {
        assert!(spans.contains(&(want.0, want.1.to_owned())), "{want:?} in {spans:?}");
    }
    assert!(!spans.iter().any(|(line, _)| (12..=13).contains(line)), "{spans:?}");
    // A module declared inline is `inline`: it has no file of its own.
    let want: BTreeSet<(String, u32, String, Option<String>, bool)> = [
        ("test", 14, "15", None, false),
        ("lib_tests", 16, "18", None, false),
        ("lib_tests", 16, "18", Some("lib_tests.rs"), false),
        ("other_tests", 21, "22", None, false),
        // The match that starts on `#[path]` gives the module's scope without its path as well.
        ("other_tests", 19, "22", None, false),
        ("other_tests", 19, "22", Some("other.rs"), false),
        ("inline", 23, "24", None, true),
        ("raw", 33, "35", None, false),
        ("raw", 33, "35", Some("raw_tests.rs"), false),
    ]
    .iter()
    .map(|(m, l, e, p, i)| ((*m).to_owned(), *l, (*e).to_owned(), p.map(str::to_owned), *i))
    .collect();
    assert_eq!(modules, want);
}
