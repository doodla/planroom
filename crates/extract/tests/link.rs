//! The built-in linkers, over hints and nodes built as values: no pack file and no repo is read.
//!
//! Each test gives `runtime_wiring` the hints the fixture's code and compose file give, shaped as
//! the extractors emit them, and checks the facts it links (architecture.md, "Hints and
//! linkers"; DR-12).

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{Edge, Evidence, Hint, Kind, Layer, LevelId, Node, NodeId, Provenance, Ref, Tier};
use planroom_extract::link::{Linked, WorkspaceFacts, builtin};
use serde_json::{Value, json};

const COMMIT: &str = "0000000000000000000000000000000000000000";

/// The linker's options, with the kinds and relations the code pack names for it, as the engine
/// fills them in from the loaded vocabulary, and the five languages' test globs as the code pack
/// declares them, which the engine passes from the merged `[[languages]]` table.
fn opts() -> Value {
    let kind = |kind: &str, id_prefix: &str, level: &str| json!({"kind": kind, "id_prefix": id_prefix, "level": level});
    let from = json!(["code.service", "code.library", "code.tooling"]);
    let js = [
        "**/*.test.*",
        "**/*.spec.*",
        "**/*.cy.*",
        "**/__tests__/**",
        "**/__mocks__/**",
        "**/test/**",
        "**/tests/**",
        "**/e2e/**",
        "**/cypress/**",
    ];
    let language = |id: &str, extensions: &[&str], tests: &[&str]| json!({"id": id, "extensions": extensions, "line_comment": "//", "tests": tests});
    json!({
        "languages": [
            language("yaml", &["yml", "yaml"], &[]),
            language(
                "python",
                &["py"],
                &[
                    "**/tests/**",
                    "**/test/**",
                    "**/e2e/**",
                    "**/test_*.py",
                    "**/*_test.py",
                    "**/tests.py",
                    "**/conftest.py",
                ],
            ),
            language("go", &["go"], &["**/*_test.go", "**/testdata/**", "**/e2e/**"]),
            language("typescript", &["ts", "mts", "cts"], &js),
            language("tsx", &["tsx"], &js),
            language("javascript", &["js", "mjs", "cjs", "jsx"], &js),
            language("rust", &["rs"], &["**/tests/**", "**/benches/**", "**/tests.rs"]),
        ],
        "kinds": {"service": kind("code.service", "container", "container"),
                  "repo": kind("code.repo", "repo", "system"),
                  "endpoint": kind("code.endpoint", "endpoint", "code")},
        "relations": {"http": {"kind": "code.http", "from": from, "to": ["code.service", "code.library"]},
                      "queue_publish": {"kind": "code.queue_publish", "from": from, "to": ["code.queue"]},
                      "queue_consume": {"kind": "code.queue_consume", "from": from, "to": ["code.queue"]},
                      "db": {"kind": "code.db", "from": from, "to": ["code.datastore"]}},
    })
}

/// A hint as an extractor emits it: scoped to `<repo>:file:<path>` unless `scope` says otherwise,
/// citing `line` of `<repo>/<path>` with `contains` as its text.
struct H<'a> {
    kind: &'a str,
    repo: &'a str,
    path: &'a str,
    line: u32,
    key: &'a str,
    value: Option<&'a str>,
    attrs: &'a [(&'a str, &'a str)],
}

impl H<'_> {
    fn hint(&self) -> Hint {
        self.scoped(&format!("{}:file:{}", self.repo, self.path))
    }

    fn scoped(&self, scope: &str) -> Hint {
        let evidence = vec![line(&format!("{}/{}", self.repo, self.path), self.line)];
        let id = format!("{:012x}", fnv(&format!("{}|{scope}|{}|{}", self.kind, self.key, self.line)));
        Hint {
            id,
            kind: Kind(self.kind.into()),
            scope: NodeId(scope.into()),
            key: self.key.into(),
            value: self.value.map(str::to_owned),
            attrs: self.attrs.iter().map(|(k, v)| ((*k).to_owned(), Value::from(*v))).collect(),
            evidence,
            provenance: Provenance::Extracted { extractor: "code/test".into(), tier: Tier::Syntactic },
        }
    }
}

/// A stable 48-bit hash, for test hint IDs: 12 hex characters, as an extractor's are.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
        & 0xffff_ffff_ffff
}

fn line(path: &str, line: u32) -> Evidence {
    Evidence::Line {
        path: path.into(),
        line,
        contains: Some(format!("line {line}")),
        commit: Some(COMMIT.into()),
        branch: None,
    }
}

/// A compose service in `platform/docker-compose.yml`: what compose emits for it.
fn service(name: &str, line: u32, attrs: &[(&str, &str)]) -> Hint {
    H {
        kind: "code.compose_service",
        repo: "platform",
        path: "docker-compose.yml",
        line,
        key: name,
        value: None,
        attrs,
    }
    .hint()
}

/// What compose sets on `service`, on `line` of the compose file.
fn env_set(service: &str, name: &str, value: &str, line: u32) -> Hint {
    let attrs = [("service", service), ("name", name), ("value", value)];
    H {
        kind: "code.env_set",
        repo: "platform",
        path: "docker-compose.yml",
        line,
        key: name,
        value: Some(value),
        attrs: &attrs,
    }
    .hint()
}

fn link(repos: &[&str], hints: Vec<Hint>) -> Linked {
    let facts = WorkspaceFacts {
        repos: repos.iter().map(|r| (*r).to_owned()).collect(),
        hints,
        ..WorkspaceFacts::default()
    };
    let linked = builtin("runtime_wiring").unwrap().link(&facts, &opts()).unwrap();
    // A pure function: the same facts in another order link the same.
    let mut shuffled = facts.clone();
    shuffled.hints.reverse();
    let again = builtin("runtime_wiring").unwrap().link(&shuffled, &opts()).unwrap();
    assert_eq!(serde_json::to_string(&linked.batch).unwrap(), serde_json::to_string(&again.batch).unwrap());
    linked
}

/// The one edge of `kind` from `from` to `to` with `via`.
fn edge<'a>(linked: &'a Linked, kind: &str, from: &str, to: &str, via: &str) -> &'a Edge {
    let found: Vec<&Edge> = linked
        .batch
        .edges
        .iter()
        .filter(|e| {
            e.kind.0 == kind
                && e.from == Ref::Id(NodeId(from.into()))
                && e.to == Ref::Id(NodeId(to.into()))
                && e.via.as_deref() == Some(via)
        })
        .collect();
    let [one] = found[..] else { panic!("one {kind} {from} -> {to} via {via}: {:#?}", linked.batch.edges) };
    one
}

/// An edge's evidence as `path:line`, in order.
fn cited(edge: &Edge) -> Vec<String> {
    edge.evidence
        .iter()
        .map(|e| match e {
            Evidence::Line { path, line, .. } => format!("{path}:{line}"),
            Evidence::Ref { commit, .. } => commit.clone(),
        })
        .collect()
}

#[test]
fn runtime_wiring_joins_route_calls_to_endpoints() {
    let hints = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 9, &[("image", "acme/api:1.0"), ("kind", "code.service"), ("role", "service")]),
        env_set("web", "API_URL", "http://api:8080", 6),
        // What the api serves: two handlers, and the spec's operation for one of them.
        H {
            kind: "code.route_serve",
            repo: "api",
            path: "server.go",
            line: 10,
            key: "/orders/{id}",
            value: None,
            attrs: &[("method", "GET"), ("path", "/orders/{id}")],
        }
        .hint(),
        H {
            kind: "code.route_serve",
            repo: "api",
            path: "server.go",
            line: 11,
            key: "/orders",
            value: None,
            attrs: &[("method", "POST"), ("path", "/orders")],
        }
        .hint(),
        H {
            kind: "code.route_serve",
            repo: "api",
            path: "openapi.yaml",
            line: 9,
            key: "/orders",
            value: None,
            attrs: &[("method", "POST"), ("path", "/orders")],
        }
        .scoped("api:spec:openapi.yaml"),
        // A call whose base is a literal URL, with a template segment.
        H {
            kind: "code.route_call",
            repo: "web",
            path: "src/client.ts",
            line: 5,
            key: "'http://api:8080/orders/${id}'",
            value: None,
            attrs: &[("method", "'GET'"), ("path", "'http://api:8080/orders/${id}'")],
        }
        .hint(),
        // A literal call to a path the api doesn't serve.
        H {
            kind: "code.route_call",
            repo: "web",
            path: "src/client.ts",
            line: 8,
            key: "'http://api:8080/health'",
            value: None,
            attrs: &[("method", "get"), ("path", "'http://api:8080/health'")],
        }
        .hint(),
        // A call whose base isn't a literal: its file reads the variable compose sets on web.
        H {
            kind: "code.env_read",
            repo: "web",
            path: "src/orders.ts",
            line: 3,
            key: "API_URL",
            value: None,
            attrs: &[("name", "API_URL")],
        }
        .hint(),
        H {
            kind: "code.route_call",
            repo: "web",
            path: "src/orders.ts",
            line: 7,
            key: "`${base}/orders`",
            value: None,
            attrs: &[("method", "'POST'"), ("path", "`${base}/orders`")],
        }
        .hint(),
        // The same call in a file that reads nothing: its host waits for the wrapper's binding.
        H {
            kind: "code.route_call",
            repo: "web",
            path: "src/later.ts",
            line: 2,
            key: "`${base}/orders/${id}`",
            value: None,
            attrs: &[("method", "'PUT'"), ("path", "`${base}/orders/${id}`")],
        }
        .hint(),
    ];
    let linked = link(&["web", "api", "platform"], hints);
    assert_eq!(linked.unresolved, []);

    // Each served route is one endpoint under the container, citing every line that serves it, by
    // path and line.
    let endpoints: BTreeMap<&str, Vec<String>> = linked
        .batch
        .nodes
        .iter()
        .filter(|n| n.kind.0 == "code.endpoint")
        .map(|n| {
            assert_eq!(n.parent.as_ref().unwrap().0, "api:container:api");
            let lines = n.evidence.iter().map(|e| match e {
                Evidence::Line { path, line, .. } => format!("{path}:{line}"),
                Evidence::Ref { .. } => unreachable!(),
            });
            (n.id.0.as_str(), lines.collect())
        })
        .collect();
    assert_eq!(
        endpoints,
        BTreeMap::from([
            ("api:endpoint:GET /orders/{id}", vec!["api/server.go:10".to_owned()]),
            (
                "api:endpoint:POST /orders",
                vec!["api/openapi.yaml:9".to_owned(), "api/server.go:11".to_owned()]
            ),
        ])
    );

    // The literal call: `via` is the endpoint's served form, and its evidence follows.
    let get = edge(&linked, "code.http", "web:container:web", "api:container:api", "GET /orders/{id}");
    assert_eq!(get.via_ref, Some(Ref::Id(NodeId("api:endpoint:GET /orders/{id}".into()))));
    assert_eq!(cited(get), ["web/src/client.ts:5", "api/server.go:10"]);
    let Provenance::Linked { linker, hints } = &get.provenance else { panic!("{get:?}") };
    assert_eq!(linker, "code/runtime_wiring");
    assert_eq!(hints.len(), 1);

    // No endpoint serves the path: the call's own method and path, and only its evidence.
    let health = edge(&linked, "code.http", "web:container:web", "api:container:api", "GET /health");
    assert_eq!((health.via_ref.as_ref(), cited(health)), (None, vec!["web/src/client.ts:8".to_owned()]));

    // The base isn't a literal: compose's value for the variable its file reads names the host.
    let post = edge(&linked, "code.http", "web:container:web", "api:container:api", "POST /orders");
    assert_eq!(post.via_ref, Some(Ref::Id(NodeId("api:endpoint:POST /orders".into()))));
    assert_eq!(
        cited(post),
        [
            "web/src/orders.ts:7",
            "platform/docker-compose.yml:6",
            "api/openapi.yaml:9",
            "api/server.go:11",
            "web/src/orders.ts:3"
        ]
    );
    assert_eq!(linked.batch.edges.len(), 3, "the call in a file that reads no variable joins nothing");
    let ids: Vec<&str> = linked.batch.edges.iter().map(|e| e.id.0.as_str()).collect();
    assert!(ids.is_sorted(), "sorted by ID");
}

#[test]
fn two_containers_a_call_could_reach_leave_it_unresolved() {
    let hints = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 9, &[("kind", "code.service"), ("role", "service")]),
        H {
            kind: "code.compose_service",
            repo: "platform",
            path: "other/compose.yml",
            line: 3,
            key: "api",
            value: None,
            attrs: &[("kind", "code.service"), ("role", "service"), ("image", "acme/platform:1")],
        }
        .hint(),
        H {
            kind: "code.route_call",
            repo: "web",
            path: "src/client.ts",
            line: 5,
            key: "'http://api/x'",
            value: None,
            attrs: &[("method", "'GET'"), ("path", "'http://api/x'")],
        }
        .hint(),
    ];
    // `api` names two containers: `ws:container:api`, and `platform:container:api` from the second
    // file, whose image names the platform repo.
    let linked = link(&["web", "platform"], hints);
    assert!(linked.batch.edges.is_empty(), "{:?}", linked.batch.edges);
    let [unresolved] = &linked.unresolved[..] else { panic!("{:?}", linked.unresolved) };
    assert_eq!(unresolved.linker, "code/runtime_wiring");
    assert_eq!(unresolved.candidate_count, 2);
    let candidates: Vec<&str> = unresolved.candidates.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(candidates, ["platform:container:api", "ws:container:api"]);
    // `web` now holds two containers, so no join hint of its own can name one.
    let more = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("worker", 7, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("db", 9, &[("kind", "code.datastore"), ("role", "datastore")]),
        H {
            kind: "code.db_open",
            repo: "web",
            path: "db.py",
            line: 4,
            key: "psycopg",
            value: None,
            attrs: &[("driver", "psycopg")],
        }
        .hint(),
        H {
            kind: "code.url",
            repo: "web",
            path: "db.py",
            line: 2,
            key: "db",
            value: None,
            attrs: &[("scheme", "postgres"), ("host", "db")],
        }
        .hint(),
    ];
    let linked = link(&["web", "platform"], more);
    assert!(linked.batch.edges.is_empty());
    // Both join hints, the database open and the URL literal, are left unresolved.
    assert_eq!(linked.unresolved.len(), 2, "{:?}", linked.unresolved);
    for unresolved in &linked.unresolved {
        let candidates: Vec<&str> = unresolved.candidates.iter().map(|c| c.0.as_str()).collect();
        assert_eq!(candidates, ["web:container:web", "web:container:worker"]);
    }
}

#[test]
fn literal_dsn_resolves_by_host() {
    // worker/worker/audit.py at the fixture's `10-audit-rows`, after compose stopped wiring the
    // worker to orders-db.
    let audit = |line: u32, kind: &'static str, key: &'static str, value, attrs: &'static [(&str, &str)]| {
        H { kind, repo: "worker", path: "worker/audit.py", line, key, value, attrs }.hint()
    };
    let mut hints = vec![
        service(
            "worker",
            28,
            &[("image", "acme/worker:0.9.0"), ("kind", "code.service"), ("role", "service")],
        ),
        service(
            "orders-db",
            41,
            &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")],
        ),
        env_set("worker", "REDIS_URL", "redis://redis:6379/0", 30),
        audit(
            15,
            "code.const",
            "DEFAULT_AUDIT_DSN",
            Some("postgresql://orders:orders@orders-db:5432/orders"),
            &[],
        ),
        audit(17, "code.sql", "worker_audit", None, &[("verb", "INSERT"), ("table", "worker_audit")]),
        audit(20, "code.def", "record", Some("(job: Job, outcome: str)"), &[]),
        audit(
            22,
            "code.env_read",
            "AUDIT_DSN",
            None,
            &[("name", "AUDIT_DSN"), ("default", "DEFAULT_AUDIT_DSN")],
        ),
        audit(24, "code.db_open", "psycopg", None, &[("driver", "psycopg")]),
        audit(24, "code.call", "connect", Some("(dsn, autocommit=True)"), &[]),
        H {
            kind: "code.call",
            repo: "worker",
            path: "worker/main.py",
            line: 44,
            key: "audit.record",
            value: Some("(job, outcome)"),
            attrs: &[],
        }
        .hint(),
    ];
    let linked = link(&["worker", "platform"], hints.clone());
    let db = edge_without_via(&linked, "code.db", "worker:container:worker", "ws:container:orders-db");
    assert_eq!(
        cited(db),
        [
            "worker/worker/audit.py:24",
            "worker/worker/audit.py:15",
            "worker/worker/audit.py:17",
            "worker/worker/audit.py:22",
            "worker/worker/main.py:44"
        ]
    );
    assert_eq!(linked.batch.edges.len(), 1);

    // When compose sets the variable, its value wins over the literal default.
    hints.push(service(
        "audit-db",
        45,
        &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")],
    ));
    hints.push(env_set("worker", "AUDIT_DSN", "postgresql://a@audit-db/audit", 33));
    let linked = link(&["worker", "platform"], hints);
    let db = edge_without_via(&linked, "code.db", "worker:container:worker", "ws:container:audit-db");
    assert_eq!(&cited(db)[..2], ["worker/worker/audit.py:24", "platform/docker-compose.yml:33"]);
    assert_eq!(linked.batch.edges.len(), 1, "{:#?}", linked.batch.edges);
}

fn edge_without_via<'a>(linked: &'a Linked, kind: &str, from: &str, to: &str) -> &'a Edge {
    let found: Vec<&Edge> = linked
        .batch
        .edges
        .iter()
        .filter(|e| {
            e.kind.0 == kind && e.from == Ref::Id(NodeId(from.into())) && e.to == Ref::Id(NodeId(to.into()))
        })
        .collect();
    let [one] = found[..] else { panic!("one {kind} {from} -> {to}: {:#?}", linked.batch.edges) };
    assert_eq!(one.via, None);
    one
}

#[test]
fn evidence_follows_callers_two_hops() {
    // orders/internal/http/handlers.go:125 calls enqueueFulfil, which calls Publish, whose LPush
    // pushes onto the jobs list.
    let go = |path: &'static str,
              line: u32,
              kind: &'static str,
              key: &'static str,
              attrs: &'static [(&str, &str)]| {
        H { kind, repo: "orders", path, line, key, value: None, attrs }.hint()
    };
    let publisher = "internal/queue/publisher.go";
    let handlers = "internal/http/handlers.go";
    let hints = vec![
        service(
            "orders",
            16,
            &[("image", "acme/orders:1.4.0"), ("kind", "code.service"), ("role", "service")],
        ),
        service("redis", 38, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
        service(
            "worker",
            28,
            &[("image", "acme/worker:0.9.0"), ("kind", "code.service"), ("role", "service")],
        ),
        env_set("orders", "REDIS_URL", "redis://redis:6379/0", 21),
        go(publisher, 14, "code.const", "JobsKey", &[]),
        go(publisher, 41, "code.def", "Publish", &[]),
        go(publisher, 49, "code.queue_op", "JobsKey", &[("op", "publish"), ("key", "JobsKey")]),
        go(handlers, 54, "code.def", "createOrder", &[]),
        go(handlers, 84, "code.call", "enqueueFulfil", &[]),
        go(handlers, 102, "code.def", "applyEvent", &[]),
        go(handlers, 125, "code.call", "h.enqueueFulfil", &[]),
        go(handlers, 153, "code.def", "enqueueFulfil", &[]),
        go(handlers, 157, "code.call", "h.jobs.Publish", &[]),
        go(handlers, 170, "code.def", "retry", &[]),
        go(handlers, 171, "code.call", "enqueueFulfil", &[]),
        // A call by the same name in another container is no caller.
        H {
            kind: "code.call",
            repo: "worker",
            path: "worker/a.py",
            line: 3,
            key: "Publish",
            value: None,
            attrs: &[],
        }
        .hint(),
    ];
    let mut hints = hints;
    // The constant's value.
    hints.iter_mut().find(|h| h.kind.0 == "code.const").unwrap().value = Some("order-jobs".into());
    let linked = link(&["orders", "worker", "platform"], hints);
    let publish =
        edge(&linked, "code.queue_publish", "orders:container:orders", "ws:container:redis", "order-jobs");
    // The anchor, the constant, compose's wiring, then the call sites: Publish's caller first,
    // then enqueueFulfil's, by line, three in all.
    assert_eq!(
        cited(publish),
        [
            "orders/internal/queue/publisher.go:49",
            "orders/internal/queue/publisher.go:14",
            "platform/docker-compose.yml:21",
            "orders/internal/http/handlers.go:157",
            "orders/internal/http/handlers.go:84",
            "orders/internal/http/handlers.go:125"
        ]
    );
    let kinds: BTreeSet<&str> = linked.batch.edges.iter().map(|e| e.kind.0.as_str()).collect();
    assert_eq!(kinds, BTreeSet::from(["code.queue_publish"]));
}

/// Test code never anchors or cites a runtime join, in any of the five languages: a test file's
/// queue operations, calls, opens, reads and served routes join nothing, and its calls of the
/// runtime's functions are no callers. A file whose name only looks like a test's is runtime code.
#[test]
fn test_files_never_anchor_or_cite_a_join() {
    let cases: [(&str, &[&str], &[&str]); 5] = [
        (
            "worker/consumer.py",
            &[
                "tests/test_consumer.py",
                "worker/test_jobs.py",
                "worker/jobs_test.py",
                "conftest.py",
                "test/x.py",
                // Django's layout, and an end-to-end suite.
                "app/tests.py",
                "e2e/checkout.py",
            ],
            &["worker/contest.py", "worker/testing.py", "worker/attest_test_x.txt", "app/views.py"],
        ),
        (
            "internal/queue.go",
            &["internal/queue_test.go", "internal/testdata/fake.go", "test/e2e/framework.go"],
            &["internal/test_queue.go"],
        ),
        (
            "src/queue.ts",
            &[
                "src/queue.test.ts",
                "src/queue.spec.tsx",
                "src/__tests__/queue.ts",
                "test/queue.mts",
                "tests/e2e.ts",
                // Cypress's and Playwright's layouts.
                "cypress/e2e/checkout.cy.ts",
                "src/Cart.cy.tsx",
                "cypress/support/commands.ts",
                "e2e/checkout.ts",
                "src/__mocks__/queue.ts",
            ],
            &["src/latest.ts", "src/contest.tsx", "src/e2e.ts", "src/policy.ts"],
        ),
        (
            "src/queue.js",
            &[
                "src/queue.test.js",
                "__tests__/queue.mjs",
                "test/queue.cjs",
                "src/a.spec.jsx",
                "cypress/e2e/checkout.cy.js",
                "e2e/checkout.mjs",
            ],
            &["src/testing.js", "src/fancy.js"],
        ),
        (
            "src/queue.rs",
            &[
                "tests/queue.rs",
                "benches/queue.rs",
                "crates/q/tests/it.rs",
                // A unit test module in a file of its own, as `#[cfg(test)] mod tests;` declares it.
                "src/tests.rs",
                "src/queue/tests.rs",
            ],
            &["src/tests_util.rs", "src/test.rs", "src/contests.rs"],
        ),
    ];
    for (runtime, tests, runtimes) in cases {
        let at = |path: &str, line: u32, kind: &str, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo: "worker", path, line, key, value: None, attrs }.hint()
        };
        let mut hints = vec![
            service("worker", 4, &[("build", "worker"), ("kind", "code.service"), ("role", "service")]),
            service("redis", 8, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
            service(
                "db",
                12,
                &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")],
            ),
            service("api", 16, &[("kind", "code.service"), ("role", "service")]),
            env_set("worker", "REDIS_URL", "redis://redis:6379/0", 6),
            env_set("worker", "DATABASE_URL", "postgres://u@db:5432/x", 7),
            at(runtime, 1, "code.def", "consume", &[]),
            at(runtime, 2, "code.queue_op", "jobs", &[("op", "consume"), ("key", "jobs")]),
        ];
        // What a test does: calls the runtime, pushes onto the list under a name it imports,
        // opens the database with a variable compose sets, calls a URL and serves a fake route.
        for path in tests.iter().chain(runtimes) {
            hints.extend([
                at(path, 1, "code.def", "test_it", &[]),
                at(path, 2, "code.call", "consume", &[]),
                at(path, 3, "code.queue_op", "JOBS_KEY", &[("op", "publish"), ("key", "JOBS_KEY")]),
                at(path, 4, "code.env_read", "DATABASE_URL", &[("name", "DATABASE_URL")]),
                at(path, 5, "code.db_open", "pg", &[("driver", "pg")]),
                at(path, 6, "code.url", "api", &[("scheme", "http"), ("host", "api"), ("path", "/x")]),
                at(path, 7, "code.route_serve", "/fake", &[("method", "GET"), ("path", "/fake")]),
            ]);
        }
        let linked = link(&["worker", "platform"], hints);
        let consume =
            edge(&linked, "code.queue_consume", "worker:container:worker", "ws:container:redis", "jobs");
        let mut want = vec![format!("worker/{runtime}:2"), "platform/docker-compose.yml:6".to_owned()];
        // Only a file that is no test calls the runtime: the first by path, up to three.
        let mut callers: Vec<String> = runtimes.iter().map(|p| format!("worker/{p}:2")).collect();
        callers.sort();
        want.extend(callers.into_iter().take(3));
        assert_eq!(cited(consume), want, "{runtime}");
        // A file that is no test joins as runtime code does; a test file joins nothing.
        let in_test = |e: &Evidence| match e {
            Evidence::Line { path, .. } => tests.iter().any(|t| path == &format!("worker/{t}")),
            Evidence::Ref { .. } => false,
        };
        let edges: Vec<&Edge> =
            linked.batch.edges.iter().filter(|e| e.evidence.iter().any(in_test)).collect();
        assert!(edges.is_empty(), "{runtime}: {edges:#?}");
        let nodes: Vec<&Node> =
            linked.batch.nodes.iter().filter(|n| n.evidence.iter().any(in_test)).collect();
        assert!(nodes.is_empty(), "{runtime}: {nodes:#?}");
        edge(&linked, "code.queue_publish", "worker:container:worker", "ws:container:redis", "JOBS_KEY");
        edge_without_via(&linked, "code.db", "worker:container:worker", "ws:container:db");
        edge(&linked, "code.http", "worker:container:worker", "ws:container:api", "/x");
    }
}

/// Rust's inline unit tests: a `#[cfg(test)]` module in a runtime file, a `#[cfg(test)]` function
/// and one nested in a runtime module are test code, as a test file is. Each is a
/// `code.test_scope` hint from its attribute's line to its item's last line: what a hint inside
/// one does joins nothing and is never cited, and the runtime code on the line after it, or in
/// another file, joins as before.
#[test]
fn an_inline_test_item_never_anchors_or_cites_a_join() {
    let at = |path: &str, line: u32, kind: &str, key: &str, attrs: &[(&str, &str)]| {
        H { kind, repo: "worker", path, line, key, value: None, attrs }.hint()
    };
    let scope = |path: &str, line: u32, end: &str| at(path, line, "code.test_scope", "test", &[("end", end)]);
    let publish = |path: &str, line: u32, key: &str| {
        at(path, line, "code.queue_op", key, &[("op", "publish"), ("key", key)])
    };
    let lib = "src/lib.rs";
    // What a test does inside its scope: calls the runtime, pushes onto a list, opens the
    // database with a variable compose sets, calls a URL and serves a fake route.
    let test_code = |first: u32| {
        vec![
            at(lib, first, "code.def", "pushes", &[]),
            at(lib, first + 1, "code.call", "consume", &[]),
            publish(lib, first + 2, "probe-jobs"),
            at(lib, first + 3, "code.env_read", "DATABASE_URL", &[("name", "DATABASE_URL")]),
            at(lib, first + 3, "code.db_open", "pg", &[("driver", "pg")]),
            at(lib, first + 4, "code.url", "api", &[("scheme", "http"), ("host", "api"), ("path", "/x")]),
            at(lib, first + 4, "code.route_serve", "/fake", &[("method", "GET"), ("path", "/fake")]),
        ]
    };
    let mut hints = vec![
        service("worker", 4, &[("build", "worker"), ("kind", "code.service"), ("role", "service")]),
        service("redis", 8, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
        service("db", 12, &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")]),
        service("api", 16, &[("kind", "code.service"), ("role", "service")]),
        env_set("worker", "REDIS_URL", "redis://redis:6379/0", 6),
        env_set("worker", "DATABASE_URL", "postgres://u@db:5432/x", 7),
        at(lib, 1, "code.def", "consume", &[]),
        at(lib, 2, "code.queue_op", "jobs", &[("op", "consume"), ("key", "jobs")]),
        // `#[cfg(test)] mod tests { ... }` on lines 10 to 20.
        scope(lib, 10, "20"),
        // Runtime code on the line after the module ends.
        at(lib, 21, "code.def", "after", &[]),
        at(lib, 21, "code.call", "consume", &[]),
        publish(lib, 22, "after-jobs"),
        // `#[cfg(test)] fn helper() { ... }` on lines 30 to 35, then runtime code.
        scope(lib, 30, "35"),
        at(lib, 36, "code.def", "later", &[]),
        at(lib, 36, "code.call", "consume", &[]),
        // A runtime module from line 40, holding a `#[cfg(test)] mod inner` on lines 42 to 48,
        // and a test scope that ends where it starts: a `#[cfg(test)] use` or `const`.
        at(lib, 40, "code.def", "outer", &[]),
        scope(lib, 42, "48"),
        scope(lib, 50, "50"),
        publish(lib, 50, "const-jobs"),
        publish(lib, 51, "outer-jobs"),
        // A scope in one file never reaches another's lines, and one whose end is no line, or
        // before its start, is no scope.
        publish("src/other.rs", 12, "other-jobs"),
        scope("src/odd.rs", 5, "four"),
        scope("src/odd.rs", 9, "3"),
        publish("src/odd.rs", 6, "odd-jobs"),
        publish("src/odd.rs", 9, "odd-late-jobs"),
    ];
    hints.extend(test_code(11));
    hints.extend(test_code(31));
    hints.extend(test_code(43));
    let linked = link(&["worker", "platform"], hints);

    let consume =
        edge(&linked, "code.queue_consume", "worker:container:worker", "ws:container:redis", "jobs");
    // Called from the runtime after each test scope, never from inside one.
    assert_eq!(
        cited(consume),
        [
            "worker/src/lib.rs:2",
            "platform/docker-compose.yml:6",
            "worker/src/lib.rs:21",
            "worker/src/lib.rs:36"
        ]
    );
    for key in ["after-jobs", "outer-jobs", "other-jobs", "odd-jobs", "odd-late-jobs"] {
        edge(&linked, "code.queue_publish", "worker:container:worker", "ws:container:redis", key);
    }
    let in_scope = |e: &Evidence| match e {
        Evidence::Line { path, line, .. } => {
            path == "worker/src/lib.rs"
                && [(10, 20), (30, 35), (42, 48), (50, 50)].iter().any(|(a, b)| (a..=b).contains(&line))
        }
        Evidence::Ref { .. } => false,
    };
    let edges: Vec<&Edge> = linked.batch.edges.iter().filter(|e| e.evidence.iter().any(in_scope)).collect();
    assert!(edges.is_empty(), "{edges:#?}");
    let nodes: Vec<&Node> = linked.batch.nodes.iter().filter(|n| n.evidence.iter().any(in_scope)).collect();
    assert!(nodes.is_empty(), "{nodes:#?}");
    // No test code joins: no database open, no URL, no endpoint, no other list.
    let kinds: BTreeSet<&str> = linked.batch.edges.iter().map(|e| e.kind.0.as_str()).collect();
    assert_eq!(kinds, BTreeSet::from(["code.queue_consume", "code.queue_publish"]));
    assert_eq!(linked.batch.edges.len(), 6, "{:#?}", linked.batch.edges);
    assert!(linked.unresolved.is_empty(), "{:?}", linked.unresolved);
}

/// A test module a file declares at its top level, `#[cfg(test)] mod test;`, is read from a file
/// of its own, and that file is test code in whole, as is every file below the module's
/// directory: `<name>.rs` beside a `lib.rs`, `main.rs`, `mod.rs` or binary, in the directory named
/// for any other file's stem, or the file its `#[path]` names. Nothing in one joins or is cited,
/// while a runtime file of a like name, a module of the same name another file declares, the
/// file a test module's name gives when its `#[path]` names another, and the file of the same name
/// beside a test module declared inline, which has no file of its own, join as before.
#[test]
fn a_test_module_declared_out_of_line_is_test_code_in_whole() {
    let at = |repo: &str, path: &str, line: u32, kind: &str, key: &str, attrs: &[(&str, &str)]| {
        H { kind, repo, path, line, key, value: None, attrs }.hint()
    };
    let module = |repo: &str, path: &str, line: u32, end: &str, name: &str, attr_path: Option<&str>| {
        let mut attrs = vec![("end", end), ("module", name)];
        attrs.extend(attr_path.map(|p| ("path", p)));
        at(repo, path, line, "code.test_scope", "test", &attrs)
    };
    let inline = |repo: &str, path: &str, line: u32, end: &str, name: &str| {
        at(repo, path, line, "code.test_scope", "test", &[("end", end), ("module", name), ("inline", "true")])
    };
    let publish = |path: &str, key: &str| {
        at("worker", path, 3, "code.queue_op", key, &[("op", "publish"), ("key", key)])
    };
    let lib = "src/lib.rs";
    let mut hints = vec![
        service("worker", 4, &[("build", "worker"), ("kind", "code.service"), ("role", "service")]),
        service("redis", 8, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
        env_set("worker", "REDIS_URL", "redis://redis:6379/0", 6),
        at("worker", lib, 1, "code.def", "consume", &[]),
        at("worker", lib, 2, "code.queue_op", "jobs", &[("op", "consume"), ("key", "jobs")]),
        // `#[cfg(test)] mod test;`
        module("worker", lib, 10, "11", "test", None),
        // `#[cfg(test)] #[path = "unit.rs"] mod checks;`: the scope without the path, and the
        // one with it, which a second pattern gives.
        module("worker", lib, 13, "15", "checks", None),
        module("worker", lib, 13, "15", "checks", Some("unit.rs")),
        // Paths that reach the repo's root, and leave it.
        module("worker", lib, 17, "18", "fake", Some("../shared/./fake.rs")),
        module("worker", lib, 20, "21", "out", Some("../../out.rs")),
        // `#[cfg(test)] mod inline { mod child; }`: no file of its own, only its child's.
        inline("worker", lib, 23, "30", "inline"),
        // `#[cfg(test)] mod mock { ... }` beside `#[cfg(not(test))] mod mock;`, which reads the
        // runtime `src/mock.rs`.
        inline("worker", lib, 32, "35", "mock"),
        // A module of another file: `src/jobs.rs` reads it from `src/jobs/`, and a binary from
        // its own directory.
        module("worker", "src/jobs.rs", 40, "41", "t", None),
        module("worker", "src/bin/tool.rs", 5, "6", "r#fixtures", None),
        // Another repo's test module names none of this repo's files.
        module("kit", lib, 5, "6", "runtime_mod", None),
        at("worker", "src/test.rs", 2, "code.call", "consume", &[]),
    ];
    let test_code = [
        "src/test.rs",
        "src/test/mod.rs",
        "src/test/helpers/deep.rs",
        "src/unit.rs",
        "shared/fake.rs",
        "src/inline/child.rs",
        "src/jobs/t.rs",
        "src/jobs/t/deep.rs",
        "src/bin/fixtures.rs",
    ];
    for path in test_code {
        hints.push(publish(path, &format!("probe:{path}")));
    }
    let runtime = [
        "src/testing.rs",
        "src/test.rs.bak/x.rs",
        "src/checks.rs",
        "src/out.rs",
        "src/t.rs",
        "src/runtime_mod.rs",
        "src/inline.rs",
        "src/mock.rs",
    ];
    for path in runtime {
        hints.push(publish(path, &format!("runtime:{path}")));
    }
    let linked = link(&["worker", "platform", "kit"], hints);

    let consume =
        edge(&linked, "code.queue_consume", "worker:container:worker", "ws:container:redis", "jobs");
    assert_eq!(cited(consume), ["worker/src/lib.rs:2", "platform/docker-compose.yml:6"]);
    let mut vias: Vec<&str> = linked.batch.edges.iter().filter_map(|e| e.via.as_deref()).collect();
    vias.sort_unstable();
    let mut want: Vec<String> = runtime.iter().map(|p| format!("runtime:{p}")).collect();
    want.push("jobs".into());
    want.sort_unstable();
    assert_eq!(vias, want);
    assert!(linked.unresolved.is_empty(), "{:?}", linked.unresolved);
}

#[test]
fn a_service_name_no_id_can_hold_mints_no_container() {
    let hints = vec![
        service("bad\nname", 4, &[("kind", "code.service"), ("role", "service")]),
        service("back\\slash", 6, &[("kind", "code.service"), ("role", "service")]),
        service("good", 8, &[("kind", "code.service"), ("role", "service")]),
    ];
    let linked = link(&["platform"], hints);
    let ids: Vec<&str> = linked.batch.nodes.iter().map(|n| n.id.0.as_str()).collect();
    assert_eq!(ids, ["ws:container:good"]);
}

/// A file's many calls and many reads of variables compose sets link within their own size: an
/// edge cites at most a few wiring lines for its target, however many reach it, so no repo's
/// hints can make the link hold their product.
#[test]
fn many_calls_and_reads_in_one_file_link_within_their_own_size() {
    const N: u32 = 400;
    let mut hints = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 6, &[("kind", "code.service"), ("role", "service")]),
    ];
    let names: Vec<String> = (0..N).map(|i| format!("V{i}")).collect();
    let paths: Vec<String> = (0..N).map(|i| format!("`${{base}}/p{i}`")).collect();
    for i in 0..N as usize {
        let line = u32::try_from(i).unwrap();
        hints.push(env_set("web", &names[i], "http://api:8080", 100 + line));
        let attrs = [("name", names[i].as_str())];
        hints.push(
            H {
                kind: "code.env_read",
                repo: "web",
                path: "src/client.ts",
                line: 1 + line,
                key: &names[i],
                value: None,
                attrs: &attrs,
            }
            .hint(),
        );
        let attrs = [("method", "'GET'"), ("path", paths[i].as_str())];
        hints.push(
            H {
                kind: "code.route_call",
                repo: "web",
                path: "src/client.ts",
                line: 1000 + line,
                key: &paths[i],
                value: None,
                attrs: &attrs,
            }
            .hint(),
        );
    }
    let linked = link(&["web", "platform"], hints);
    assert_eq!(linked.batch.edges.len(), N as usize, "one edge a call");
    for edge in &linked.batch.edges {
        // The call, then at most three compose lines and the three reads they answer.
        assert!(edge.evidence.len() <= 7, "{}", edge.evidence.len());
    }
}

/// What [`one_files_many_opens_and_calls_link_in_linear_time`] may take in a debug build: each
/// shape links in well under a second, and citing a file's SQL for each open, or an endpoint's
/// whole evidence for each call, took over 10 s.
const MANY_JOINS_BUDGET: std::time::Duration = std::time::Duration::from_secs(4);

/// A file's many database opens beside its many SQL statements, and many calls to one endpoint
/// that many handlers serve, link in time that grows with the hints, never with their product:
/// an edge takes a file's SQL lines, or its endpoint's evidence, once.
#[test]
fn one_files_many_opens_and_calls_link_in_linear_time() {
    const N: u32 = 3_000;
    let lines: Vec<String> = (0..N).map(|i| format!("q{i}")).collect();
    let mut opens = vec![
        service("orders", 4, &[("build", "orders"), ("kind", "code.service"), ("role", "service")]),
        service(
            "orders-db",
            9,
            &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")],
        ),
        env_set("orders", "DATABASE_URL", "postgres://o@orders-db:5432/o", 6),
        H {
            kind: "code.env_read",
            repo: "orders",
            path: "store.go",
            line: 1,
            key: "DATABASE_URL",
            value: None,
            attrs: &[("name", "DATABASE_URL")],
        }
        .hint(),
    ];
    let mut calls = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 9, &[("build", "api"), ("kind", "code.service"), ("role", "service")]),
    ];
    for i in 0..N {
        let key = lines[i as usize].as_str();
        let at =
            |line: u32, kind, repo, path, attrs| H { kind, repo, path, line, key, value: None, attrs }.hint();
        opens.push(at(10 + 2 * i, "code.db_open", "orders", "store.go", &[("driver", "pgx")]));
        opens.push(at(11 + 2 * i, "code.sql", "orders", "store.go", &[("verb", "SELECT"), ("table", "t")]));
        calls.push(at(10 + i, "code.route_serve", "api", "server.go", &[("method", "GET"), ("path", "/x")]));
        calls.push(at(
            10 + i,
            "code.route_call",
            "web",
            "client.ts",
            &[("method", "'GET'"), ("path", "'http://api:8080/x'")],
        ));
    }
    for (shape, repos, hints) in [("opens", ["orders", "platform"], opens), ("calls", ["web", "api"], calls)]
    {
        let started = std::time::Instant::now();
        let linked = link(&repos, hints);
        let took = started.elapsed();
        let [edge] = &linked.batch.edges[..] else { panic!("{shape}: {:?}", linked.batch.edges.len()) };
        // Each open and its statement, or each call and the handler it reaches, once; and the wiring.
        let cited = u32::try_from(edge.evidence.len()).unwrap();
        assert!((2 * N..=2 * N + 2).contains(&cited), "{shape}: {cited}");
        assert!(took < MANY_JOINS_BUDGET, "{shape}: {N} took {took:?}");
    }
}

/// Many calls whose method no endpoint serves, against many endpoints at one normalized path,
/// link in time that grows with the hints: a call finds the endpoints of its method at its path
/// by an index, never by walking every endpoint the path holds.
#[test]
fn calls_against_many_endpoints_at_one_path_link_in_linear_time() {
    const N: u32 = 3_000;
    let compose = || {
        vec![
            service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
            service("api", 9, &[("build", "api"), ("kind", "code.service"), ("role", "service")]),
        ]
    };
    let serve = |attrs: &[(&str, &str)], line| {
        let path = attrs.iter().find(|(k, _)| *k == "path").unwrap().1;
        H { kind: "code.route_serve", repo: "api", path: "server.go", line, key: path, value: None, attrs }
            .hint()
    };
    let call = |line| {
        let attrs = [("method", "'GET'"), ("path", "'http://api:8080/x/{q}'")];
        let key = "'http://api:8080/x/{q}'";
        H { kind: "code.route_call", repo: "web", path: "client.ts", line, key, value: None, attrs: &attrs }
            .hint()
    };
    let paths: Vec<String> = (0..N).map(|i| format!("/x/{{p{i}}}")).collect();
    let mut hints = compose();
    for i in 0..N {
        hints.push(serve(&[("method", "POST"), ("path", &paths[i as usize])], 10 + i));
        hints.push(call(10 + i));
    }
    let started = std::time::Instant::now();
    let linked = link(&["web", "api"], hints);
    let took = started.elapsed();
    let [edge] = &linked.batch.edges[..] else { panic!("{:?}", linked.batch.edges.len()) };
    // No endpoint serves GET there: the call's own method and path, and each call's line.
    assert_eq!(edge.via.as_deref(), Some("GET /x/{}"));
    assert_eq!(edge.via_ref, None);
    assert_eq!(edge.evidence.len(), N as usize);
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");

    // One endpoint of the call's method, spelled in any case, among others: it matches. One
    // more served for every method makes two that fit, and no single match.
    let mut hints = compose();
    hints.push(serve(&[("method", "POST"), ("path", "/x/{a}")], 1));
    hints.push(serve(&[("method", "get"), ("path", "/x/{b}")], 2));
    hints.push(call(1));
    let linked = link(&["web", "api"], hints.clone());
    assert_eq!(linked.batch.edges[0].via.as_deref(), Some("GET /x/{b}"));
    hints.push(serve(&[("path", "/x/{c}")], 3));
    let linked = link(&["web", "api"], hints);
    assert_eq!(linked.batch.edges[0].via.as_deref(), Some("GET /x/{}"), "GET and ANY both fit");
}

/// Many containers that share one host alias, each reached from a container of its own, link in
/// time that grows with the hints: each host resolves once per link, and every join that reaches
/// it shares that one set of candidates instead of walking or copying it.
#[test]
fn many_containers_sharing_one_host_link_in_linear_time() {
    const N: usize = 3_000;
    let repos: Vec<String> = (0..N).flat_map(|i| [format!("web{i}"), format!("api{i}")]).collect();
    // Compose file `c<i>.yml` runs `web` from repo `web<i>` and `api` from repo `api<i>`, and
    // sets API_URL on `web` to `http://api:8080`: every `api` carries the alias `host:api`.
    let compose: Vec<(String, String, String)> =
        (0..N).map(|i| (format!("c{i}.yml"), format!("web{i}"), format!("api{i}"))).collect();
    let shape = |literal: bool| {
        let mut hints = Vec::new();
        for (file, web, api) in &compose {
            let at = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
                H { kind, repo: "platform", path: file, line, key, value: None, attrs }.hint()
            };
            hints.push(at(
                "code.compose_service",
                1,
                "web",
                &[("build", web.as_str()), ("kind", "code.service")],
            ));
            hints.push(at(
                "code.compose_service",
                5,
                "api",
                &[("build", api.as_str()), ("kind", "code.service")],
            ));
            let set = [("service", "web"), ("name", "API_URL"), ("value", "http://api:8080")];
            let mut env = at("code.env_set", 3, "API_URL", &set);
            env.value = Some("http://api:8080".into());
            hints.push(env);
            let code = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
                H { kind, repo: web, path: "client.ts", line, key, value: None, attrs }.hint()
            };
            hints.push(code("code.env_read", 1, "API_URL", &[("name", "API_URL")]));
            let path = if literal { "'http://api:8080/x'" } else { "`${base}/x`" };
            hints.push(code("code.route_call", 2, path, &[("method", "'GET'"), ("path", path)]));
        }
        hints
    };
    let repos: Vec<&str> = repos.iter().map(String::as_str).chain(["platform"]).collect();
    for literal in [false, true] {
        let hints = shape(literal);
        let started = std::time::Instant::now();
        let linked = link(&repos, hints);
        let took = started.elapsed();
        // Each call could reach any of the N `api`s: none is guessed.
        assert!(linked.batch.edges.is_empty(), "{:?}", linked.batch.edges.len());
        assert_eq!(linked.unresolved.len(), N);
        let first = &linked.unresolved[0];
        assert_eq!(first.candidate_count, N);
        assert_eq!(first.candidates.len(), 3);
        assert!(first.candidates.windows(2).all(|w| w[0] < w[1]), "the first by ID");
        assert!(took < MANY_JOINS_BUDGET, "literal {literal}: {N} took {took:?}");
    }
}

/// Many containers that each set one variable to a host they share with every other and to a host
/// of their own link in time that grows with the hints: each join counts its candidates without
/// building the union of the shared set and its own, so N containers never build N sets of N.
#[test]
fn many_containers_combining_a_shared_host_with_their_own_link_in_linear_time() {
    const N: usize = 4_000;
    let mut hints = Vec::new();
    let mut repos: Vec<String> = Vec::new();
    // Compose file `c<i>.yml` runs `web` from repo `web<i>`, `api` from repo `api<i>` (alias
    // `api`, shared by all) and `s<i>` from repo `s<i>`, and sets API_URL on `web` twice: to
    // `http://api:8080` and to `http://s<i>:8080`.
    for i in 0..N {
        let (file, web, api, own) =
            (format!("c{i}.yml"), format!("web{i}"), format!("api{i}"), format!("s{i}"));
        let at = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo: "platform", path: &file, line, key, value: None, attrs }.hint()
        };
        hints.push(at(
            "code.compose_service",
            1,
            "web",
            &[("build", web.as_str()), ("kind", "code.service")],
        ));
        hints.push(at(
            "code.compose_service",
            5,
            "api",
            &[("build", api.as_str()), ("kind", "code.service")],
        ));
        hints.push(at("code.compose_service", 7, &own, &[("build", own.as_str()), ("kind", "code.service")]));
        for (line, value) in [(3, "http://api:8080".to_owned()), (4, format!("http://{own}:8080"))] {
            let mut env = at("code.env_set", line, "API_URL", &[("service", "web"), ("name", "API_URL")]);
            env.value = Some(value);
            hints.push(env);
        }
        let code = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo: &web, path: "client.ts", line, key, value: None, attrs }.hint()
        };
        hints.push(code("code.env_read", 1, "API_URL", &[("name", "API_URL")]));
        hints.push(code(
            "code.route_call",
            2,
            "`${base}/x`",
            &[("method", "'GET'"), ("path", "`${base}/x`")],
        ));
        repos.extend([web, api, own]);
    }
    let repos: Vec<&str> = repos.iter().map(String::as_str).chain(["platform"]).collect();
    let started = std::time::Instant::now();
    let linked = link(&repos, hints);
    let took = started.elapsed();
    // Each call could reach any `api` or its own `s<i>`: none is guessed.
    assert!(linked.batch.edges.is_empty(), "{:?}", linked.batch.edges.len());
    assert_eq!(linked.unresolved.len(), N);
    for unresolved in &linked.unresolved {
        assert_eq!(unresolved.candidate_count, N + 1);
        assert_eq!(unresolved.candidates.len(), 3);
        assert!(unresolved.candidates.windows(2).all(|w| w[0] < w[1]), "the first by ID");
    }
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");
}

/// A join whose hosts reach overlapping sets counts each candidate once, and lists the first by ID
/// across all of them, the joining container aside, however its variables combine them.
#[test]
fn a_join_counts_overlapping_host_sets_once() {
    let node = |id: &str, aliases: &[&str]| Node {
        id: NodeId(id.into()),
        kind: Kind("code.service".into()),
        level: LevelId::from("container"),
        parent: None,
        name: id.rsplit(':').next().unwrap().into(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        props: BTreeMap::new(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "code/other".into(), tier: Tier::Config },
        evidence: vec![line("platform/hosts.yml", 1)],
    };
    // `api` reaches a and b, `shared` reaches a, b, c and web itself, `zz` reaches c.
    let nodes = vec![
        node("a:container:api", &["host:shared"]),
        node("b:container:b", &["host:api", "host:shared"]),
        node("c:container:c", &["host:shared", "host:zz"]),
        node("web:container:web", &["host:shared"]),
    ];
    let mut hints = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 6, &[("build", "a"), ("kind", "code.service"), ("role", "service")]),
        service("b", 8, &[("build", "b"), ("kind", "code.service"), ("role", "service")]),
        service("c", 10, &[("build", "c"), ("kind", "code.service"), ("role", "service")]),
        env_set("web", "API_URL", "http://api:8080", 20),
        env_set("web", "API_URL", "http://shared:8080", 21),
        env_set("web", "API_URL", "http://zz:8080", 22),
        env_set("web", "Z_URL", "http://zz:8080", 23),
        env_set("web", "Z_URL", "http://web:8080", 24),
    ];
    let call = |path: &'static str, reads: &[&'static str]| {
        let mut out: Vec<Hint> = reads
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let line = 1 + u32::try_from(i).unwrap();
                H {
                    kind: "code.env_read",
                    repo: "web",
                    path,
                    line,
                    key: name,
                    value: None,
                    attrs: &[("name", name)],
                }
                .hint()
            })
            .collect();
        let attrs = [("method", "'GET'"), ("path", "`${base}/x`")];
        out.push(
            H {
                kind: "code.route_call",
                repo: "web",
                path,
                line: 9,
                key: "`${base}/x`",
                value: None,
                attrs: &attrs,
            }
            .hint(),
        );
        out
    };
    hints.extend(call("one.ts", &["API_URL"]));
    hints.extend(call("two.ts", &["Z_URL"]));
    hints.extend(call("both.ts", &["Z_URL", "API_URL"]));
    let facts = WorkspaceFacts {
        repos: ["web", "a", "b", "c", "platform"].map(str::to_owned).into(),
        nodes,
        hints,
        ..WorkspaceFacts::default()
    };
    let linked = builtin("runtime_wiring").unwrap().link(&facts, &opts()).unwrap();
    let mut shuffled = facts.clone();
    shuffled.hints.reverse();
    shuffled.nodes.reverse();
    assert_eq!(linked, builtin("runtime_wiring").unwrap().link(&shuffled, &opts()).unwrap());
    // `two.ts` reaches only c: its own container aside, `web` names no other.
    edge(&linked, "code.http", "web:container:web", "c:container:c", "GET /x");
    assert_eq!(linked.unresolved.len(), 2, "{:?}", linked.unresolved);
    for unresolved in &linked.unresolved {
        assert_eq!(unresolved.candidate_count, 3, "{unresolved:?}");
        let ids: Vec<&str> = unresolved.candidates.iter().map(|c| c.0.as_str()).collect();
        assert_eq!(ids, ["a:container:api", "b:container:b", "c:container:c"]);
    }
}

/// Many files that each read a variable compose sets many times on their container link in time
/// that grows with the hints: what a variable's sets reach is worked out once per container, and
/// each file pairs its reads with that variable's first three sets only.
#[test]
fn many_files_reading_a_variable_set_many_times_link_in_linear_time() {
    const N: usize = 3_000;
    let files: Vec<String> = (0..N).map(|i| format!("c{i}.yml")).collect();
    let paths: Vec<String> = (0..N).map(|i| format!("src/f{i}.ts")).collect();
    let mut hints = Vec::new();
    // Each compose file runs `web` from repo `web` and sets API_URL on it: one container, N sets.
    for file in &files {
        let at = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo: "platform", path: file, line, key, value: None, attrs }.hint()
        };
        hints.push(at("code.compose_service", 1, "web", &[("build", "web"), ("kind", "code.service")]));
        hints.push(at("code.compose_service", 5, "api", &[("build", "api"), ("kind", "code.service")]));
        let mut env = at("code.env_set", 3, "API_URL", &[("service", "web"), ("name", "API_URL")]);
        env.value = Some("http://api:8080".into());
        hints.push(env);
    }
    for path in &paths {
        let code = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo: "web", path, line, key, value: None, attrs }.hint()
        };
        hints.push(code("code.env_read", 1, "API_URL", &[("name", "API_URL")]));
        hints.push(code(
            "code.route_call",
            2,
            "`${base}/x`",
            &[("method", "'GET'"), ("path", "`${base}/x`")],
        ));
    }
    let started = std::time::Instant::now();
    let linked = link(&["web", "api", "platform"], hints);
    let took = started.elapsed();
    let edge = edge(&linked, "code.http", "web:container:web", "api:container:api", "GET /x");
    // Each call, then the first three sets by path and line, then the reads.
    assert_eq!(edge.evidence.len(), 2 * N + 3, "{:?}", &cited(edge)[..8]);
    let wiring: Vec<String> = cited(edge).into_iter().filter(|c| c.starts_with("platform/")).collect();
    assert_eq!(wiring, ["platform/c0.yml:3", "platform/c1.yml:3", "platform/c10.yml:3"]);
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");
}

/// Many reads of one variable that compose sets many times on the container link in time that
/// grows with the hints: a join cites at most three wiring items, so only a variable's first
/// three reads in a file, or in a repo, can be cited, and the rest are never paired with each set.
#[test]
fn many_reads_of_a_variable_set_many_times_link_in_linear_time() {
    const N: u32 = 2_000;
    let mut hints = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 6, &[("build", "api"), ("kind", "code.service"), ("role", "service")]),
        service("db", 8, &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")]),
    ];
    let read = |name, path, line| {
        let attrs = [("name", name)];
        H { kind: "code.env_read", repo: "web", path, line, key: name, value: None, attrs: &attrs }.hint()
    };
    for i in 0..N {
        hints.push(env_set("web", "API_URL", "http://api:8080", 1000 + 2 * i));
        hints.push(env_set("web", "DB_URL", "postgres://u@db:5432/x", 1001 + 2 * i));
        hints.push(read("API_URL", "a.ts", 10 + 2 * i));
        hints.push(read("DB_URL", "a.ts", 11 + 2 * i));
    }
    let attrs = [("method", "'GET'"), ("path", "`${base}/x`")];
    let at =
        |kind, path, line, key, attrs| H { kind, repo: "web", path, line, key, value: None, attrs }.hint();
    hints.push(at("code.route_call", "a.ts", 1, "`${base}/x`", &attrs));
    // An open in a file that reads nothing: its wiring is every read in the repo.
    hints.push(at("code.db_open", "b.ts", 1, "pg", &[("driver", "pg")]));
    let started = std::time::Instant::now();
    let linked = link(&["web", "api"], hints);
    let took = started.elapsed();
    let http = edge(&linked, "code.http", "web:container:web", "api:container:api", "GET /x");
    let db = edge_without_via(&linked, "code.db", "web:container:web", "ws:container:db");
    // The first three wires by path and line: the first set, with the first three reads it answers.
    for (edge, anchor, set, read) in [(http, "web/a.ts:1", 1000, 10), (db, "web/b.ts:1", 1001, 11)] {
        let mut expected = vec![anchor.to_owned(), format!("platform/docker-compose.yml:{set}")];
        expected.extend((0..3).map(|i| format!("web/a.ts:{}", read + 2 * i)));
        assert_eq!(cited(edge), expected);
    }
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");
}

/// Many calls of one function on one line, as a minified bundle holds them, beside many joins
/// inside that function, link in time that grows with the hints: a line's calls are one caller
/// line, found once, never walked again for each join the function holds.
#[test]
fn many_calls_of_a_function_on_one_line_link_in_linear_time() {
    const N: u32 = 10_000;
    let keys: Vec<String> = (0..N).map(|i| format!("o{i}.f")).collect();
    let at = |kind, line, key, attrs| {
        H { kind, repo: "worker", path: "dist/bundle.js", line, key, value: None, attrs }.hint()
    };
    let mut hints = vec![
        service("worker", 4, &[("build", "worker"), ("kind", "code.service"), ("role", "service")]),
        service("redis", 8, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
        env_set("worker", "REDIS_URL", "redis://redis:6379/0", 6),
        at("code.def", 1, "f", &[]),
        H {
            kind: "code.env_read",
            repo: "worker",
            path: "dist/env.js",
            line: 1,
            key: "REDIS_URL",
            value: None,
            attrs: &[("name", "REDIS_URL")],
        }
        .hint(),
    ];
    for i in 0..N {
        hints.push(at("code.call", 1, &keys[i as usize], &[]));
        hints.push(at("code.queue_op", 2 + i, "jobs", &[("op", "publish"), ("key", "jobs")]));
    }
    let started = std::time::Instant::now();
    let linked = link(&["worker", "platform"], hints);
    let took = started.elapsed();
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");
    let publish =
        edge(&linked, "code.queue_publish", "worker:container:worker", "ws:container:redis", "jobs");
    // Each publish, the compose line, then the one line that calls the function, once.
    let cited = cited(publish);
    assert_eq!(cited.len(), N as usize + 2, "{:?}", &cited[cited.len() - 4..]);
    assert_eq!(cited[cited.len() - 2..], ["platform/docker-compose.yml:6", "worker/dist/bundle.js:1"]);
}

/// A join hint cites at most three wiring items for its target, the first by path and line,
/// whatever order its file reads their variables in; an edge that several hints join cites each
/// one's.
#[test]
fn a_join_cites_its_first_three_wiring_items_by_path_and_line() {
    let mut hints = vec![
        service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
        service("api", 6, &[("build", "api"), ("kind", "code.service"), ("role", "service")]),
    ];
    // The file reads V0 to V3 in that order, and compose sets them on lines 40, 30, 20 and 10.
    let names: Vec<String> = (0..4).map(|i| format!("V{i}")).collect();
    for (i, name) in names.iter().enumerate() {
        let i = u32::try_from(i).unwrap();
        hints.push(env_set("web", name, "http://api:8080", 40 - 10 * i));
        let attrs = [("name", name.as_str())];
        let read = H {
            kind: "code.env_read",
            repo: "web",
            path: "a.ts",
            line: 1 + i,
            key: name,
            value: None,
            attrs: &attrs,
        };
        hints.push(read.hint());
    }
    let call = |path, line| {
        let attrs = [("method", "'GET'"), ("path", "`${base}/x`")];
        H { kind: "code.route_call", repo: "web", path, line, key: "`${base}/x`", value: None, attrs: &attrs }
            .hint()
    };
    hints.push(call("a.ts", 9));
    let linked = link(&["web", "api"], hints.clone());
    let edge = edge(&linked, "code.http", "web:container:web", "api:container:api", "GET /x");
    let wiring: Vec<String> = cited(edge).into_iter().filter(|c| c.starts_with("platform/")).collect();
    assert_eq!(
        wiring,
        [
            "platform/docker-compose.yml:10",
            "platform/docker-compose.yml:20",
            "platform/docker-compose.yml:30"
        ]
    );

    // A second file's call that reads V4, which compose sets on line 50: one edge, citing the
    // wiring each call reached.
    hints.push(env_set("web", "V4", "http://api:8080", 50));
    let attrs = [("name", "V4")];
    hints.push(
        H {
            kind: "code.env_read",
            repo: "web",
            path: "b.ts",
            line: 1,
            key: "V4",
            value: None,
            attrs: &attrs,
        }
        .hint(),
    );
    hints.push(call("b.ts", 2));
    let linked = link(&["web", "api"], hints);
    assert_eq!(linked.batch.edges.len(), 1);
    let wiring: Vec<String> =
        cited(&linked.batch.edges[0]).into_iter().filter(|c| c.starts_with("platform/")).collect();
    assert_eq!(wiring.len(), 4, "{wiring:?}");
    assert_eq!(wiring[3], "platform/docker-compose.yml:50");
}

/// A repo's root manifest gives `<repo>:container:<repo>` at the universal tier, which a compose
/// service of the same name joins. When exactly one service of another name joins the repo, that
/// service's container is the repo's container for joins, and the manifest's yields to it. With
/// two or more joined services, or a container some extractor gives above the universal tier,
/// the repo's join hints name every container and are left unresolved, never guessed.
#[test]
fn a_repos_one_joined_service_is_its_container_over_its_manifests() {
    let library = Node {
        id: NodeId("orders:container:orders".into()),
        kind: Kind("code.library".into()),
        level: LevelId::from("container"),
        parent: Some(NodeId("orders:repo:orders".into())),
        name: "orders".into(),
        aliases: vec![],
        props: BTreeMap::new(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "code/manifest".into(), tier: Tier::Universal },
        evidence: vec![line("orders/go.mod", 1)],
    };
    let at = |kind, line, key, attrs| {
        H { kind, repo: "orders", path: "store.go", line, key, value: None, attrs }.hint()
    };
    let hints = |names: &[&str]| {
        let mut hints = vec![
            service(
                "orders-db",
                9,
                &[("image", "postgres:16.4"), ("kind", "code.datastore"), ("role", "datastore")],
            ),
            at("code.env_read", 1, "DATABASE_URL", &[("name", "DATABASE_URL")]),
            at("code.db_open", 2, "pgxpool", &[("driver", "pgxpool")]),
            at("code.route_serve", 3, "/orders", &[("method", "GET"), ("path", "/orders")]),
        ];
        for (i, name) in names.iter().enumerate() {
            let line = 20 + 4 * u32::try_from(i).unwrap();
            hints.push(service(
                name,
                line,
                &[("build", "orders"), ("kind", "code.service"), ("role", "service")],
            ));
            hints.push(env_set(name, "DATABASE_URL", "postgres://o@orders-db:5432/o", line + 1));
        }
        hints
    };
    let run = |nodes: Vec<Node>, hints| {
        let facts = WorkspaceFacts {
            repos: ["orders", "platform"].map(str::to_owned).into(),
            nodes,
            hints,
            ..WorkspaceFacts::default()
        };
        let linked = builtin("runtime_wiring").unwrap().link(&facts, &opts()).unwrap();
        // Whatever order the facts come in.
        let mut shuffled = facts.clone();
        shuffled.hints.reverse();
        shuffled.nodes.reverse();
        assert_eq!(linked, builtin("runtime_wiring").unwrap().link(&shuffled, &opts()).unwrap());
        linked
    };
    // The service of the same name: one container, and the open joins.
    let same = run(vec![library.clone()], hints(&["orders"]));
    assert_eq!(same.unresolved, []);
    edge_without_via(&same, "code.db", "orders:container:orders", "ws:container:orders-db");
    // Another name: the one service joined to the repo is its container, and the manifest's
    // yields; the repo's routes are that service's endpoints.
    let other = run(vec![library.clone()], hints(&["orders-api"]));
    assert_eq!(other.unresolved, []);
    edge_without_via(&other, "code.db", "orders:container:orders-api", "ws:container:orders-db");
    let endpoint = other.batch.nodes.iter().find(|n| n.kind.0 == "code.endpoint").expect("an endpoint");
    assert_eq!(endpoint.id.0, "orders:endpoint:GET /orders");
    assert_eq!(endpoint.parent, Some(NodeId("orders:container:orders-api".into())));
    // Two services joined to the repo: three containers, and the open is unresolved among them.
    let two = run(vec![library.clone()], hints(&["orders-api", "orders-jobs"]));
    assert!(two.batch.edges.is_empty(), "{:?}", two.batch.edges);
    let [unresolved] = &two.unresolved[..] else { panic!("{:?}", two.unresolved) };
    let candidates: Vec<&str> = unresolved.candidates.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(
        candidates,
        ["orders:container:orders", "orders:container:orders-api", "orders:container:orders-jobs"]
    );
    // A container an extractor gives above the universal tier never yields: two containers.
    let mut config = library.clone();
    config.provenance = Provenance::Extracted { extractor: "code/other".into(), tier: Tier::Config };
    let above = run(vec![library, config], hints(&["orders-api"]));
    assert!(above.batch.edges.is_empty(), "{:?}", above.batch.edges);
    let [unresolved] = &above.unresolved[..] else { panic!("{:?}", above.unresolved) };
    assert_eq!(unresolved.candidate_count, 2);
}

/// What `worker/worker/orders_client.py` gives after the S4 hand-off, at its file lines.
fn orders_client(line: u32, kind: &'static str, key: &'static str, value: Option<&'static str>) -> Hint {
    let attrs: &[(&str, &str)] = match kind {
        "code.env_read" => &[("name", "ORDERS_URL")],
        "code.route_call" => &[("method", "method"), ("path", "self._base + path")],
        _ => &[],
    };
    H { kind, repo: "worker", path: "worker/orders_client.py", line, key, value, attrs }.hint()
}

/// The routes orders serves: each `(method, path, line)` in `internal/http/handlers.go`.
fn served(routes: &[(&'static str, &'static str, u32)]) -> Vec<Hint> {
    routes
        .iter()
        .map(|(method, path, line)| {
            let attrs = [("method", *method), ("path", *path)];
            H {
                kind: "code.route_serve",
                repo: "orders",
                path: "internal/http/handlers.go",
                line: *line,
                key: path,
                value: None,
                attrs: &attrs,
            }
            .hint()
        })
        .collect()
}

/// A wrapper's method and path are its parameters: each call site that passes a literal or a
/// template for them makes one route call, citing the call site, then the wrapped call, then the
/// target's wiring and endpoint. A call site that passes a variable, a parameter that no call site
/// binds, and a method that names no parameter make nothing, and no `via` names a parameter.
#[test]
fn a_wrapper_call_binds_method_and_path() {
    let mut hints = vec![
        service(
            "worker",
            27,
            &[("image", "acme/worker:0.9.0"), ("kind", "code.service"), ("role", "service")],
        ),
        service(
            "orders",
            15,
            &[("image", "acme/orders:1.4.0"), ("kind", "code.service"), ("role", "service")],
        ),
        env_set("worker", "ORDERS_URL", "http://orders:8080", 31),
        orders_client(
            30,
            "code.def",
            "__init__",
            Some("(self, base_url: str | None = None, timeout: float = DEFAULT_TIMEOUT_SECONDS)"),
        ),
        orders_client(31, "code.env_read", "ORDERS_URL", None),
        orders_client(37, "code.def", "get_order", Some("(self, order_id: str)")),
        orders_client(
            39,
            "code.call",
            "self._request",
            Some("(\"GET\", f\"/orders/{urllib.parse.quote(order_id)}\")"),
        ),
        orders_client(
            41,
            "code.def",
            "apply_event",
            Some("(self, order_id: str, event: str, **fields: str)"),
        ),
        orders_client(
            44,
            "code.call",
            "self._request",
            Some("(\"POST\", f\"/orders/{urllib.parse.quote(order_id)}/events\", body)"),
        ),
        orders_client(
            46,
            "code.def",
            "_request",
            Some("(self, method: str, path: str, body: dict[str, Any] | None = None)"),
        ),
        orders_client(52, "code.route_call", "self._base + path", None),
        // A call site that passes variables: it binds nothing.
        orders_client(60, "code.def", "replay", Some("(self, m, p)")),
        orders_client(61, "code.call", "self._request", Some("(m, p)")),
        orders_client(62, "code.call", "self._request", Some("(\"GET\", p)")),
        // A method that is no parameter of its function: it never joins, with or without an
        // endpoint at its path.
        orders_client(70, "code.def", "raw", Some("(self)")),
        H {
            kind: "code.route_call",
            repo: "worker",
            path: "worker/orders_client.py",
            line: 71,
            key: "\"/orders\"",
            value: None,
            attrs: &[("method", "method"), ("path", "\"/orders\"")],
        }
        .hint(),
        // A wrapper whose method its call site leaves out, with a literal default.
        orders_client(80, "code.def", "fetch", Some("(self, path, method=\"GET\")")),
        H {
            kind: "code.route_call",
            repo: "worker",
            path: "worker/orders_client.py",
            line: 82,
            key: "self._base + path",
            value: None,
            attrs: &[("method", "method"), ("path", "self._base + path")],
        }
        .hint(),
        orders_client(90, "code.call", "self.fetch", Some("(\"/orders\")")),
        // A test's call of the wrapper binds nothing: test code is no call site.
        H {
            kind: "code.call",
            repo: "worker",
            path: "tests/test_orders_client.py",
            line: 12,
            key: "client._request",
            value: Some("(\"PUT\", \"/orders/1\")"),
            attrs: &[],
        }
        .hint(),
    ];
    hints.extend(served(&[
        ("GET", "/orders/{id}", 38),
        ("POST", "/orders/{id}/events", 40),
        ("POST", "/orders", 37),
    ]));
    let linked = link(&["worker", "orders", "platform"], hints);
    assert_eq!(linked.unresolved, []);

    let post = edge(
        &linked,
        "code.http",
        "worker:container:worker",
        "orders:container:orders",
        "POST /orders/{id}/events",
    );
    assert_eq!(post.via_ref, Some(Ref::Id(NodeId("orders:endpoint:POST /orders/{id}/events".into()))));
    assert_eq!(
        cited(post),
        [
            "worker/worker/orders_client.py:44",
            "worker/worker/orders_client.py:52",
            "platform/docker-compose.yml:31",
            "orders/internal/http/handlers.go:40",
            "worker/worker/orders_client.py:31"
        ]
    );
    let get =
        edge(&linked, "code.http", "worker:container:worker", "orders:container:orders", "GET /orders/{id}");
    assert_eq!(&cited(get)[..2], ["worker/worker/orders_client.py:39", "worker/worker/orders_client.py:52"]);
    // The omitted method takes its literal default; no endpoint serves GET /orders.
    let fetched =
        edge(&linked, "code.http", "worker:container:worker", "orders:container:orders", "GET /orders");
    assert_eq!(
        &cited(fetched)[..2],
        ["worker/worker/orders_client.py:90", "worker/worker/orders_client.py:82"]
    );
    assert_eq!(linked.batch.edges.len(), 3, "{:#?}", linked.batch.edges);
    for edge in &linked.batch.edges {
        let via = edge.via.as_deref().unwrap_or_default();
        assert!(!via.contains("method") && !via.contains("path") && !via.ends_with("/p"), "{via}");
        let lines = cited(edge);
        for unbound in [61, 62, 71] {
            assert!(!lines.contains(&format!("worker/worker/orders_client.py:{unbound}")), "{lines:?}");
        }
    }
}

/// A route call's text reads as one canonical path and its base: a template's interpolation, a
/// non-literal operand and a segment holding either are `{}`, a query is left out, and an
/// f-string's doubled braces are text. Its method is a verb by its text, its callee's name or its
/// constant's, and none is GET.
#[test]
fn a_template_path_normalizes() {
    use planroom_extract::link::bind::{Method, Target, method, target};
    use planroom_extract::link::normalize_path;
    let path = |text: &str| match target(text, None) {
        Some(Target::Path { base, path, .. }) => (base, path),
        other => panic!("{text}: {other:?}"),
    };
    let some = |s: &str| Some(s.to_owned());
    let table: [(&str, Option<String>, &str); 11] = [
        ("`${this.baseUrl}/orders`", some("this.baseUrl"), "/orders"),
        ("`${this.baseUrl}/orders/${encodeURIComponent(id)}`", some("this.baseUrl"), "/orders/{}"),
        ("f\"/orders/{urllib.parse.quote(order_id)}\"", None, "/orders/{}"),
        ("f\"/orders/{urllib.parse.quote(order_id)}/events\"", None, "/orders/{}/events"),
        ("'/orders/' + id", None, "/orders/{}"),
        ("`/orders?limit=${n}`", None, "/orders"),
        ("`/a/${f({ b: { c: 1 } })}/d`", None, "/a/{}/d"),
        ("f\"/a/{{x}}\"", None, "/a/{x}"),
        ("`/orders/x-${id}.json`", None, "/orders/{}"),
        ("'http://orders:8080/orders/?token=s3cret#top'", some("http://orders:8080"), "/orders"),
        ("self._base + '/orders/' + order_id", some("self._base"), "/orders/{}"),
    ];
    for (text, base, want) in table {
        assert_eq!(path(text), (base, want.to_owned()), "{text}");
    }
    assert_eq!(target("'orders'", None), None, "a path that doesn't start with `/` makes no route call");
    assert_eq!(target("`${base}`", None), None);
    // The served forms compare as the call's.
    assert_eq!(normalize_path("/orders/:id"), "/orders/{}");
    assert_eq!(normalize_path("/orders/{id}"), "/orders/{}");
    assert_eq!(normalize_path("/files/{path...}/"), "/files/{}");
    assert_eq!(normalize_path("{id}"), "{}");
    let verbs = [("'post'", "POST"), ("Get", "GET"), ("PostForm", "POST"), ("http.MethodPost", "POST")];
    for (text, verb) in verbs {
        assert_eq!(method(Some(text)), Method::Verb(verb.into()), "{text}");
    }
    assert_eq!(method(None), Method::Verb("GET".into()));
    assert_eq!(method(Some("method")), Method::Name("method".into()));
    assert_eq!(method(Some("'FETCH'")), Method::Unknown);
}

/// The imports linker's options, as the engine fills them in from the code pack.
fn import_opts() -> Value {
    let mut opts = json!({
        "kinds": {"package": {"kind": "code.package", "id_prefix": "pkg", "level": "component"}},
        "relations": {
            "imports": {"kind": "code.imports", "from": ["code.file"],
                        "to": ["code.file", "code.module", "code.package", "kernel.external"]},
            "depends_on_package": {"kind": "code.depends_on_package",
                                   "from": ["code.repo", "code.package", "code.service", "code.library", "code.tooling"],
                                   "to": ["code.package", "code.library", "kernel.external"]},
        },
    });
    opts["languages"] = opts_languages();
    opts
}

/// The languages `opts` names, with their test globs.
fn opts_languages() -> Value {
    opts()["languages"].clone()
}

/// A package node, as the manifest built-in gives it.
fn package(repo: &str, name: &str, manifest: &str, line_no: u32) -> Node {
    Node {
        id: NodeId(format!("{repo}:pkg:{name}")),
        kind: Kind("code.package".into()),
        level: LevelId::from("component"),
        parent: Some(NodeId(format!("{repo}:container:{repo}"))),
        name: name.into(),
        aliases: vec![format!("pkg:{name}")],
        props: BTreeMap::new(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "code/manifest".into(), tier: Tier::Universal },
        evidence: vec![line(&format!("{repo}/{manifest}"), line_no)],
    }
}

/// A manifest's dependency, as the manifest built-in gives it: to the alias `pkg:<name>`.
fn dependency(id: &str, from: &str, name: &str, uses: &str, manifest: &str, line_no: u32) -> Edge {
    Edge {
        id: planroom_contract::EdgeId(id.into()),
        kind: Kind("code.depends_on_package".into()),
        from: Ref::Id(NodeId(from.into())),
        to: Ref::Alias(format!("pkg:{name}")),
        via: None,
        via_ref: None,
        props: [("uses".to_owned(), Value::from(uses))].into(),
        evidence: vec![line(manifest, line_no)],
        confidence: 1.0,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "code/manifest".into(), tier: Tier::Universal },
    }
}

/// An import hint in `repo`'s file `path`, on its first line.
fn import(
    repo: &'static str,
    path: &'static str,
    spec: &'static str,
    separator: Option<&'static str>,
) -> Hint {
    let attrs: &[(&str, &str)] = match separator {
        Some(".") => &[("separator", ".")],
        Some(_) => &[("separator", "::")],
        None => &[],
    };
    H { kind: "code.import", repo, path, line: 1, key: spec, value: None, attrs }.hint()
}

/// Imports of another workspace package join it: a `code.imports` edge from each importing file,
/// and the importing manifest's dependency on it given the first import lines across the repo.
/// A relative import, an import of a package the workspace doesn't hold, an import of the file's
/// own package and a test file's import join nothing; a package the manifest doesn't declare gets
/// the import edge and no dependency; two repos holding a package of the name leave it unresolved.
#[test]
fn imports_emit_depends_on_package() {
    let nodes = vec![
        package("storefront", "storefront", "package.json", 2),
        package("kit", "@acme/kit", "package.json", 2),
        package("tools", "@acme/tools", "package.json", 2),
        package("pykit", "acme-kit", "pyproject.toml", 2),
        package("worker", "worker", "pyproject.toml", 2),
        package("sock", "socket", "package.json", 2),
        package("dupa", "@acme/dup", "package.json", 2),
        package("dupb", "@acme/dup", "package.json", 2),
    ];
    let edges = vec![
        dependency(
            "e:000000000001",
            "storefront:pkg:storefront",
            "@acme/kit",
            "1.1.0",
            "storefront/package.json",
            18,
        ),
        dependency(
            "e:000000000002",
            "storefront:pkg:storefront",
            "express",
            "4.21.1",
            "storefront/package.json",
            19,
        ),
        dependency("e:000000000003", "worker:pkg:worker", "acme-kit", ">=1.0", "worker/pyproject.toml", 7),
    ];
    let hints = vec![
        import("storefront", "src/cart/cart.ts", "@acme/kit", None),
        import("storefront", "src/catalog/catalog.ts", "@acme/kit", None),
        import("storefront", "src/checkout/checkout.ts", "@acme/kit", None),
        import("storefront", "src/checkout/routes.ts", "@acme/kit", None),
        // A test file sorts first by path: it is never cited.
        import("storefront", "src/cart/cart.test.ts", "@acme/kit", None),
        import("storefront", "src/api/client.ts", "./orders.js", None),
        import("storefront", "src/server.ts", "express", None),
        import("storefront", "src/app/self.ts", "storefront", None),
        import("storefront", "src/app/tools.ts", "@acme/tools/bin", None),
        import("storefront", "src/app/socket.ts", "socket.io", None),
        import("storefront", "src/app/dup.ts", "@acme/dup", None),
        import("worker", "worker/money.py", "acme_kit.money", Some(".")),
        import("worker", "worker/main.py", "worker.db", Some(".")),
        // A Rust import inside `#[cfg(test)] mod tests { ... }` on lines 10 to 20 is test code.
        H {
            kind: "code.test_scope",
            repo: "worker",
            path: "src/lib.rs",
            line: 10,
            key: "test:20",
            value: None,
            attrs: &[("end", "20")],
        }
        .hint(),
        H {
            kind: "code.import",
            repo: "worker",
            path: "src/lib.rs",
            line: 12,
            key: "acme_kit::money",
            value: None,
            attrs: &[("separator", "::")],
        }
        .hint(),
    ];
    let facts = WorkspaceFacts {
        repos: ["storefront", "kit", "tools", "pykit", "worker", "sock", "dupa", "dupb"]
            .map(str::to_owned)
            .into(),
        nodes,
        edges,
        hints,
        ..WorkspaceFacts::default()
    };
    let imports = builtin("imports").unwrap();
    let linked = imports.link(&facts, &import_opts()).unwrap();
    let mut shuffled = facts.clone();
    shuffled.hints.reverse();
    shuffled.nodes.reverse();
    shuffled.edges.reverse();
    assert_eq!(linked, imports.link(&shuffled, &import_opts()).unwrap(), "whatever order the facts come in");

    // The manifest's dependency, under its own ID, with no `via`: the manifest's line, the
    // package's, then the first three import lines by path.
    let kit = linked.batch.edges.iter().find(|e| e.id.0 == "e:000000000001").expect("the dependency");
    assert_eq!(kit.via, None);
    assert_eq!(kit.to, Ref::Alias("pkg:@acme/kit".into()));
    assert_eq!(kit.props.get("uses"), Some(&Value::from("1.1.0")));
    assert_eq!(
        cited(kit),
        [
            "storefront/package.json:18",
            "kit/package.json:2",
            "storefront/src/cart/cart.ts:1",
            "storefront/src/catalog/catalog.ts:1",
            "storefront/src/checkout/checkout.ts:1"
        ]
    );
    let Provenance::Linked { linker, hints } = &kit.provenance else { panic!("{kit:?}") };
    assert_eq!((linker.as_str(), hints.len()), ("code/imports", 3));
    let python = linked.batch.edges.iter().find(|e| e.id.0 == "e:000000000003").expect("acme-kit");
    assert_eq!(
        cited(python),
        ["worker/pyproject.toml:7", "pykit/pyproject.toml:2", "worker/worker/money.py:1"]
    );

    // Each importing file's edge to the package, by ID, with the package's name as its `via`.
    let mut imported: Vec<(String, String, String)> = linked
        .batch
        .edges
        .iter()
        .filter(|e| e.kind.0 == "code.imports")
        .map(|e| {
            let (Ref::Id(from), Ref::Id(to)) = (&e.from, &e.to) else { panic!("{e:?}") };
            (from.0.clone(), to.0.clone(), e.via.clone().unwrap())
        })
        .collect();
    imported.sort();
    let want = |file: &str, to: &str, via: &str| (file.to_owned(), to.to_owned(), via.to_owned());
    assert_eq!(
        imported,
        [
            want("storefront:file:src/app/tools.ts", "tools:pkg:@acme/tools", "@acme/tools"),
            want("storefront:file:src/cart/cart.ts", "kit:pkg:@acme/kit", "@acme/kit"),
            want("storefront:file:src/catalog/catalog.ts", "kit:pkg:@acme/kit", "@acme/kit"),
            want("storefront:file:src/checkout/checkout.ts", "kit:pkg:@acme/kit", "@acme/kit"),
            want("storefront:file:src/checkout/routes.ts", "kit:pkg:@acme/kit", "@acme/kit"),
            want("worker:file:worker/money.py", "pykit:pkg:acme-kit", "acme-kit"),
        ]
    );
    // Only the two dependencies a manifest declares, and no other.
    let depends = linked.batch.edges.iter().filter(|e| e.kind.0 == "code.depends_on_package").count();
    assert_eq!(depends, 2, "{:#?}", linked.batch.edges);
    assert!(linked.batch.edges.iter().all(|e| !cited(e).iter().any(|c| c.contains("cart.test.ts"))));

    let [unresolved] = &linked.unresolved[..] else { panic!("{:?}", linked.unresolved) };
    assert_eq!(unresolved.linker, "code/imports");
    assert_eq!(unresolved.candidate_count, 2);
    let candidates: Vec<&str> = unresolved.candidates.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(candidates, ["dupa:pkg:@acme/dup", "dupb:pkg:@acme/dup"]);
}

/// storefront's shape: a call whose base is a constructor's parameter, in a file that reads no
/// variable, takes its host from a read elsewhere in its container of a variable compose sets on
/// it, among the containers that serve an endpoint the call matches. Another variable that reaches
/// a container serving no such endpoint changes nothing; two that serve one leave it unresolved.
#[test]
fn a_non_literal_base_takes_its_host_from_the_containers_env_reads() {
    let at = |path, line, kind, key, attrs| {
        H { kind, repo: "storefront", path, line, key, value: None, attrs }.hint()
    };
    let mut hints = vec![
        service(
            "storefront",
            5,
            &[("image", "acme/storefront:2.3.0"), ("kind", "code.service"), ("role", "service")],
        ),
        service(
            "orders",
            15,
            &[("image", "acme/orders:1.4.0"), ("kind", "code.service"), ("role", "service")],
        ),
        service(
            "otel-collector",
            49,
            &[("image", "otel/opentelemetry-collector-contrib:0.111.0"), ("kind", "code.service")],
        ),
        env_set("storefront", "ORDERS_URL", "http://orders:8080", 10),
        env_set("storefront", "OTEL_EXPORTER_OTLP_ENDPOINT", "http://otel-collector:4318", 11),
        at("src/app/config.ts", 9, "code.def", "loadConfig", &[]),
        at(
            "src/app/config.ts",
            12,
            "code.env_read",
            "ORDERS_URL",
            &[("name", "ORDERS_URL"), ("default", "'http://localhost:8080'")],
        ),
        at(
            "src/app/telemetry.ts",
            5,
            "code.env_read",
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            &[("name", "OTEL_EXPORTER_OTLP_ENDPOINT")],
        ),
        at("src/api/orders.ts", 25, "code.def", "createOrder", &[]),
        at(
            "src/api/orders.ts",
            26,
            "code.route_call",
            "`${this.baseUrl}/orders`",
            &[("method", "'POST'"), ("path", "`${this.baseUrl}/orders`")],
        ),
        at("src/api/orders.ts", 35, "code.def", "getOrder", &[]),
        at(
            "src/api/orders.ts",
            36,
            "code.route_call",
            "`${this.baseUrl}/orders/${encodeURIComponent(id)}`",
            &[("method", "GET"), ("path", "`${this.baseUrl}/orders/${encodeURIComponent(id)}`")],
        ),
    ];
    hints.extend(served(&[("POST", "/orders", 37), ("GET", "/orders/{id}", 38)]));
    let linked = link(&["storefront", "orders", "platform"], hints.clone());
    assert_eq!(linked.unresolved, []);
    let post = edge(
        &linked,
        "code.http",
        "storefront:container:storefront",
        "orders:container:orders",
        "POST /orders",
    );
    assert_eq!(
        cited(post),
        [
            "storefront/src/api/orders.ts:26",
            "platform/docker-compose.yml:10",
            "orders/internal/http/handlers.go:37",
            "storefront/src/app/config.ts:12"
        ]
    );
    edge(
        &linked,
        "code.http",
        "storefront:container:storefront",
        "orders:container:orders",
        "GET /orders/{id}",
    );
    assert_eq!(linked.batch.edges.len(), 2, "nothing to the collector: {:#?}", linked.batch.edges);

    // A second container that serves POST /orders, which another variable reaches: two fit.
    hints.push(service(
        "orders-next",
        20,
        &[("image", "acme/orders-next:2.0.0"), ("kind", "code.service"), ("role", "service")],
    ));
    hints.push(env_set("storefront", "NEXT_URL", "http://orders-next:8080", 12));
    hints.push(at("src/app/next.ts", 3, "code.env_read", "NEXT_URL", &[("name", "NEXT_URL")]));
    hints.push(
        H {
            kind: "code.route_serve",
            repo: "orders-next",
            path: "server.go",
            line: 9,
            key: "/orders",
            value: None,
            attrs: &[("method", "POST"), ("path", "/orders")],
        }
        .hint(),
    );
    let linked = link(&["storefront", "orders", "orders-next", "platform"], hints);
    let post = linked.unresolved.iter().find(|u| u.candidate_count == 2).expect("POST /orders, unresolved");
    let candidates: Vec<&str> = post.candidates.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(candidates, ["orders-next:container:orders-next", "orders:container:orders"]);
    // GET /orders/{id} still has one container that serves it.
    edge(
        &linked,
        "code.http",
        "storefront:container:storefront",
        "orders:container:orders",
        "GET /orders/{id}",
    );
    assert!(linked.batch.edges.iter().all(|e| e.via.as_deref() != Some("POST /orders")));
}

/// A wrapper binds through at most its first 64 call sites, by path and line: ten thousand calls
/// of its name that pass no path make no route call, and of a hundred that bind, only the first
/// 64 make edges.
#[test]
fn a_wrapper_binds_at_most_its_cap_of_call_sites() {
    let compose = || {
        vec![
            service("web", 4, &[("build", "web"), ("kind", "code.service"), ("role", "service")]),
            service("api", 6, &[("build", "api"), ("kind", "code.service"), ("role", "service")]),
            env_set("web", "API_URL", "http://api:8080", 5),
        ]
    };
    let at =
        |path, line, kind, key, value, attrs| H { kind, repo: "web", path, line, key, value, attrs }.hint();
    let wrapper = || {
        vec![
            at("client.py", 1, "code.def", "get", Some("(self, path)"), &[] as &[(&str, &str)]),
            at("client.py", 2, "code.env_read", "API_URL", None, &[("name", "API_URL")]),
            at(
                "client.py",
                3,
                "code.route_call",
                "self._base + path",
                None,
                &[("method", "get"), ("path", "self._base + path")],
            ),
        ]
    };
    let mut hints = compose();
    hints.extend(wrapper());
    let files: Vec<String> = (0..10_000).map(|i| format!("cache/c{i:05}.py")).collect();
    for file in &files {
        hints.push(at(file, 1, "code.call", "cache.get", Some("(\"key\")"), &[]));
    }
    let linked = link(&["web", "api", "platform"], hints);
    assert!(linked.batch.edges.is_empty(), "{:?}", linked.batch.edges.len());

    let mut hints = compose();
    hints.extend(wrapper());
    let files: Vec<String> = (0..100).map(|i| format!("f{i:03}.py")).collect();
    let args: Vec<String> = (0..100).map(|i| format!("(\"/p{i:03}\")")).collect();
    for (file, args) in files.iter().zip(&args) {
        hints.push(at(file, 1, "code.call", "client.get", Some(args), &[]));
    }
    let linked = link(&["web", "api", "platform"], hints);
    let mut vias: Vec<&str> = linked.batch.edges.iter().filter_map(|e| e.via.as_deref()).collect();
    vias.sort_unstable();
    let want: Vec<String> = (0..64).map(|i| format!("GET /p{i:03}")).collect();
    assert_eq!(vias, want);
}

/// A queue key no constant in its file resolves takes its value from a constant of that name in a
/// module its file imports, by a relative path or a dotted one: an import of a constant is no
/// guess. A test that pushes under the constant it imports still links nothing; a key that names a
/// parameter of its function joins nothing; and two imported modules that give the name different
/// values resolve it to neither.
#[test]
fn a_queue_key_resolves_through_an_imported_constant() {
    fn at(path: &str, line: u32, kind: &str, key: &str, value: Option<&str>, attrs: &[(&str, &str)]) -> Hint {
        H { kind, repo: "worker", path, line, key, value, attrs }.hint()
    }
    let op = |path, line, op, key| at(path, line, "code.queue_op", key, None, &[("op", op), ("key", key)]);
    let hints = vec![
        service("worker", 28, &[("build", "worker"), ("kind", "code.service"), ("role", "service")]),
        service("redis", 38, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
        env_set("worker", "REDIS_URL", "redis://redis:6379/0", 30),
        at("worker/keys.py", 3, "code.const", "JOBS_KEY", Some("order-jobs"), &[]),
        at("worker/consumer.py", 1, "code.import", ".keys", None, &[("separator", ".")]),
        op("worker/consumer.py", 41, "consume", "JOBS_KEY"),
        at("worker/publisher.py", 1, "code.import", "worker.keys", None, &[("separator", ".")]),
        op("worker/publisher.py", 9, "publish", "JOBS_KEY"),
        // The tests' pushes, under the constant they import.
        at("tests/test_consumer.py", 3, "code.import", "worker.keys", None, &[("separator", ".")]),
        op("tests/test_consumer.py", 13, "publish", "JOBS_KEY"),
        // A key that is a parameter of its function.
        at("worker/queue.py", 1, "code.def", "push", Some("(self, key, body)"), &[]),
        op("worker/queue.py", 2, "publish", "key"),
        // Two imported modules that disagree.
        at("worker/a.py", 1, "code.const", "OTHER", Some("a-jobs"), &[]),
        at("worker/b.py", 1, "code.const", "OTHER", Some("b-jobs"), &[]),
        at("worker/both.py", 1, "code.import", ".a", None, &[("separator", ".")]),
        at("worker/both.py", 2, "code.import", ".b", None, &[("separator", ".")]),
        op("worker/both.py", 5, "publish", "OTHER"),
    ];
    let linked = link(&["worker", "platform"], hints);
    let consume =
        edge(&linked, "code.queue_consume", "worker:container:worker", "ws:container:redis", "order-jobs");
    assert_eq!(
        cited(consume),
        ["worker/worker/consumer.py:41", "worker/worker/keys.py:3", "platform/docker-compose.yml:30"]
    );
    let publish =
        edge(&linked, "code.queue_publish", "worker:container:worker", "ws:container:redis", "order-jobs");
    assert_eq!(
        cited(publish),
        ["worker/worker/publisher.py:9", "worker/worker/keys.py:3", "platform/docker-compose.yml:30"]
    );
    // The disagreeing name keeps its own text; the parameter joins nothing.
    edge(&linked, "code.queue_publish", "worker:container:worker", "ws:container:redis", "OTHER");
    let vias: BTreeSet<&str> = linked.batch.edges.iter().filter_map(|e| e.via.as_deref()).collect();
    assert_eq!(vias, BTreeSet::from(["order-jobs", "OTHER"]));
    assert_eq!(linked.batch.edges.len(), 3, "{:#?}", linked.batch.edges);
}

/// Many containers whose calls take their host from a read elsewhere in their repo, against many
/// containers that share the host and serve every path the calls name, link in time that grows
/// with the hints: which targets serve a call is worked out once per set of targets, path and
/// method, however many containers share the set, never once per container.
#[test]
fn many_containers_calling_through_their_repos_reads_link_in_linear_time() {
    const N: usize = 1_000;
    const PATHS: usize = 10;
    let paths: Vec<String> = (0..PATHS).map(|j| format!("/x{j}")).collect();
    let calls: Vec<String> = (0..PATHS).map(|j| format!("`${{base}}/x{j}`")).collect();
    let mut hints = Vec::new();
    let mut repos: Vec<String> = Vec::new();
    for i in 0..N {
        let (web, api) = (format!("web{i}"), format!("api{i}"));
        let file = format!("c{i}.yml");
        let at = |kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo: "platform", path: &file, line, key, value: None, attrs }.hint()
        };
        hints.push(at(
            "code.compose_service",
            1,
            "web",
            &[("build", web.as_str()), ("kind", "code.service")],
        ));
        hints.push(at(
            "code.compose_service",
            5,
            "api",
            &[("build", api.as_str()), ("kind", "code.service")],
        ));
        let mut env = at("code.env_set", 3, "API_URL", &[("service", "web"), ("name", "API_URL")]);
        env.value = Some("http://api:8080".into());
        hints.push(env);
        let code = |repo: &str, path: &str, kind: &str, line: u32, key: &str, attrs: &[(&str, &str)]| {
            H { kind, repo, path, line, key, value: None, attrs }.hint()
        };
        hints.push(code(&web, "config.ts", "code.env_read", 1, "API_URL", &[("name", "API_URL")]));
        for j in 0..PATHS {
            let line = u32::try_from(j).unwrap() + 1;
            let call = [("method", "'GET'"), ("path", calls[j].as_str())];
            hints.push(code(&web, "client.ts", "code.route_call", line, &calls[j], &call));
            let serve = [("method", "GET"), ("path", paths[j].as_str())];
            hints.push(code(&api, "server.go", "code.route_serve", line, &paths[j], &serve));
        }
        repos.extend([web, api]);
    }
    let repos: Vec<&str> = repos.iter().map(String::as_str).chain(["platform"]).collect();
    let started = std::time::Instant::now();
    let linked = link(&repos, hints);
    let took = started.elapsed();
    // Each call could reach any of the N `api`s that serve its path: none is guessed.
    assert!(linked.batch.edges.is_empty(), "{:?}", linked.batch.edges.len());
    assert_eq!(linked.unresolved.len(), N * PATHS);
    assert!(linked.unresolved.iter().all(|u| u.candidate_count == N));
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");
}

/// Many repos that each hold a package of one name, and import it from many files, beside many
/// repos that import that name and hold none, link in time that grows with the hints: which
/// packages a name gives is looked up once, never copied or walked for each import, so an import
/// of a repo's own package and an import two or more repos could satisfy cost a few lookups each.
#[test]
fn many_imports_of_a_name_many_repos_hold_link_in_linear_time() {
    const N: usize = 1_000;
    const FILES: usize = 50;
    let mut nodes = Vec::new();
    let mut hints = Vec::new();
    let mut repos: Vec<String> = Vec::new();
    for i in 0..N {
        let (holder, user) = (format!("lib{i}"), format!("app{i}"));
        nodes.push(package(&holder, "util", "package.json", 2));
        for (repo, j) in [(&holder, 0), (&user, 1)] {
            for f in 0..FILES {
                let path = format!("src/f{f}.ts");
                let attrs: &[(&str, &str)] = &[];
                let mut hint =
                    H { kind: "code.import", repo, path: &path, line: 1, key: "util", value: None, attrs }
                        .hint();
                hint.id = format!("{:012x}", fnv(&format!("{repo}|{path}|{j}")));
                hints.push(hint);
            }
        }
        repos.extend([holder, user]);
    }
    let facts =
        WorkspaceFacts { repos: repos.into_iter().collect(), nodes, hints, ..WorkspaceFacts::default() };
    let started = std::time::Instant::now();
    let linked = builtin("imports").unwrap().link(&facts, &import_opts()).unwrap();
    let took = started.elapsed();
    // An import of a repo's own package waits; each other import could reach any of the N.
    assert!(linked.batch.edges.is_empty(), "{:?}", linked.batch.edges.len());
    assert_eq!(linked.unresolved.len(), N * FILES);
    assert!(linked.unresolved.iter().all(|u| u.candidate_count == N && u.candidates.len() == 3));
    assert!(took < MANY_JOINS_BUDGET, "{N} took {took:?}");
}

/// Many constants in a module deep in its repo, which a file beside it imports, link in time
/// that grows with the constants and the module's path, never with their product: the module's
/// keys are worked out once for its file, and a dotted import's key reads at most its last
/// segments.
#[test]
fn many_constants_in_a_deeply_nested_module_link_in_linear_time() {
    const CONSTS: usize = 300;
    let dir = "a/".repeat(1_500);
    let keys = format!("{dir}keys.py");
    let consumer = format!("{dir}consumer.py");
    let names: Vec<String> = (0..CONSTS).map(|i| format!("KEY_{i}")).collect();
    let values: Vec<String> = (0..CONSTS).map(|i| format!("jobs-{i}")).collect();
    let at = |path: &str, line: u32, kind: &str, key: &str, value: Option<&str>, attrs: &[(&str, &str)]| {
        H { kind, repo: "worker", path, line, key, value, attrs }.hint()
    };
    let mut hints = vec![
        service("worker", 28, &[("build", "worker"), ("kind", "code.service"), ("role", "service")]),
        service("redis", 38, &[("image", "redis:7.4"), ("kind", "code.queue"), ("role", "queue")]),
        env_set("worker", "REDIS_URL", "redis://redis:6379/0", 30),
        at(&consumer, 1, "code.import", ".keys", None, &[("separator", ".")]),
        at(&consumer, 9, "code.queue_op", "KEY_7", None, &[("op", "consume"), ("key", "KEY_7")]),
    ];
    for (i, (name, value)) in names.iter().zip(&values).enumerate() {
        hints.push(at(&keys, u32::try_from(i).unwrap() + 1, "code.const", name, Some(value), &[]));
    }
    let started = std::time::Instant::now();
    let linked = link(&["worker", "platform"], hints);
    let took = started.elapsed();
    edge(&linked, "code.queue_consume", "worker:container:worker", "ws:container:redis", "jobs-7");
    assert!(took < MANY_JOINS_BUDGET, "{CONSTS} took {took:?}");
}
