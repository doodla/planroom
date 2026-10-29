//! The truth comparator's rules, on small observed maps: branch evidence through the scan event
//! that read it (DR-86), the out-of-scope list (DR-89) and full evidence recall (DR-12).

use std::collections::BTreeSet;

use planroom_contract::{Evidence, LevelId};
use planroom_testkit::truth;
use planroom_testkit::truth::{
    DEFERRED, Observed, ObservedEdge, ObservedNode, Report, ScanRead, Scope, compare,
};
use serde_json::{Value, json};

fn node(id: &str, name: &str, level: LevelId, parent: Option<&str>) -> ObservedNode {
    ObservedNode {
        id: id.into(),
        kind: if level.as_str() == "system" { "code.repo" } else { "code.service" }.into(),
        name: name.into(),
        level,
        parent: parent.map(Into::into),
        evidence: vec![],
    }
}

/// Two repos, each with its service container.
fn two_services() -> Vec<ObservedNode> {
    vec![
        node("storefront:repo:storefront", "storefront", LevelId::from("system"), None),
        node(
            "storefront:container:storefront",
            "storefront",
            LevelId::from("container"),
            Some("storefront:repo:storefront"),
        ),
        node("orders:repo:orders", "orders", LevelId::from("system"), None),
        node("orders:container:orders", "orders", LevelId::from("container"), Some("orders:repo:orders")),
    ]
}

fn line(path: &str, line: u32, contains: &str, commit: &str) -> Evidence {
    Evidence::Line {
        path: path.into(),
        line,
        contains: Some(contains.into()),
        commit: Some(commit.into()),
        branch: None,
    }
}

fn http(evidence: Vec<Evidence>) -> ObservedEdge {
    ObservedEdge {
        kind: "code.http".into(),
        from: "storefront:container:storefront".into(),
        to: "orders:container:orders".into(),
        evidence,
    }
}

/// A truth that holds one relation, storefront to orders over `http`, with these evidence items.
fn one_relation(evidence: Value) -> Value {
    json!({
        "containers": [],
        "components": [],
        "relations": [{"from": "storefront", "to": "orders", "kind": "http", "evidence": evidence}],
    })
}

fn code_scope() -> Scope {
    Scope::new(["code.repo", "code.service"], ["code.http"], DEFERRED)
}

const RELATION: &str = "relation storefront -> orders http";

#[test]
fn a_branch_evidence_item_matches_through_its_scan_event() {
    let truth = one_relation(json!([
        {"path": "storefront/src/gift.ts", "line": 3, "contains": "giftWrap", "branch": "teammates"},
        {"path": "storefront/src/api/orders.ts", "line": 26, "contains": "fetch("}
    ]));
    let scans = vec![
        ScanRead { repo: "storefront".into(), branch: "main".into(), commit: "c-main".into() },
        ScanRead { repo: "storefront".into(), branch: "teammates".into(), commit: "c-teammates".into() },
    ];
    let compared = |gift_commit: &str, orders_commit: &str| -> Report {
        let observed = Observed {
            nodes: two_services(),
            edges: vec![http(vec![
                line("storefront/src/gift.ts", 3, "  giftWrap: true,", gift_commit),
                line("storefront/src/api/orders.ts", 26, "  return fetch(url)", orders_commit),
            ])],
            scans: scans.clone(),
            ..Observed::default()
        };
        compare(&truth, &observed, &code_scope())
    };

    // The branch item read by the scan event of `teammates`, the other by the one of `main`.
    let report = compared("c-teammates", "c-main");
    assert!(report.item(RELATION).unwrap().passes(), "{}", report.describe());

    // The same line read on `main` isn't the branch's evidence, and a line a branch scan read
    // isn't evidence on `main`.
    let report = compared("c-main", "c-teammates");
    assert_eq!(
        report.item(RELATION).unwrap().missing(),
        [
            "relation storefront -> orders http evidence storefront/src/gift.ts:3@teammates",
            "relation storefront -> orders http evidence storefront/src/api/orders.ts:26",
        ],
        "{}",
        report.describe()
    );

    // A commit no scan event read names no branch, so it can't stand for one.
    let report = compared("c-unknown", "c-main");
    assert_eq!(
        report.item(RELATION).unwrap().missing(),
        ["relation storefront -> orders http evidence storefront/src/gift.ts:3@teammates"]
    );

    // An item that carries a branch of its own still counts on the branch its commit's scan
    // event read it on: `main`'s commit, whatever it says, isn't evidence on `teammates`.
    let mut on_main = line("storefront/src/gift.ts", 3, "  giftWrap: true,", "c-main");
    if let Evidence::Line { branch, .. } = &mut on_main {
        *branch = Some("teammates".into());
    }
    let observed = Observed {
        nodes: two_services(),
        edges: vec![http(vec![
            on_main,
            line("storefront/src/api/orders.ts", 26, "  return fetch(url)", "c-main"),
        ])],
        scans: scans.clone(),
        ..Observed::default()
    };
    let report = compare(&truth, &observed, &code_scope());
    assert_eq!(
        report.item(RELATION).unwrap().missing(),
        ["relation storefront -> orders http evidence storefront/src/gift.ts:3@teammates"],
        "{}",
        report.describe()
    );
}

#[test]
fn a_missing_evidence_item_fails_the_relation() {
    let truth = one_relation(json!([
        {"path": "storefront/src/api/orders.ts", "line": 26, "contains": "fetch("},
        {"path": "platform/docker-compose.yml", "line": 10, "contains": "ORDERS_URL=http://orders:8080"}
    ]));
    let scans = vec![
        ScanRead { repo: "storefront".into(), branch: "main".into(), commit: "s1".into() },
        ScanRead { repo: "platform".into(), branch: "main".into(), commit: "p1".into() },
    ];
    let orders = line("storefront/src/api/orders.ts", 26, "  return fetch(url)", "s1");
    let compose = line("platform/docker-compose.yml", 10, "      - ORDERS_URL=http://orders:8080", "p1");

    let observed = Observed {
        nodes: two_services(),
        edges: vec![http(vec![orders.clone()])],
        scans: scans.clone(),
        ..Observed::default()
    };
    let report = compare(&truth, &observed, &code_scope());
    let relation = report.item(RELATION).unwrap();
    assert!(!relation.passes(), "one evidence item short fails the relation");
    assert_eq!(
        relation.missing(),
        ["relation storefront -> orders http evidence platform/docker-compose.yml:10"]
    );
    assert_eq!(report.failures().len(), 1);
    assert!(report.recall().contains(RELATION), "the relation itself is found");

    // The wrong line, or a line that doesn't hold the truth's text, is no evidence either.
    let off_by_one = line("platform/docker-compose.yml", 11, "      - ORDERS_URL=http://orders:8080", "p1");
    let other_text = line("platform/docker-compose.yml", 10, "      - REDIS_URL=redis://redis:6379", "p1");
    for wrong in [off_by_one, other_text] {
        let observed = Observed {
            nodes: two_services(),
            edges: vec![http(vec![orders.clone(), wrong])],
            scans: scans.clone(),
            ..Observed::default()
        };
        assert!(!compare(&truth, &observed, &code_scope()).item(RELATION).unwrap().passes());
    }

    let observed = Observed {
        nodes: two_services(),
        edges: vec![http(vec![orders, compose])],
        scans,
        ..Observed::default()
    };
    let report = compare(&truth, &observed, &code_scope());
    assert!(report.item(RELATION).unwrap().passes(), "{}", report.describe());
    assert!(report.failures().is_empty());
}

/// Every check the truth's containers, components and relations carry, counted from the truth
/// alone: one for each item, its kind, a component's container, and each evidence item.
fn every_check(truth: &Value) -> usize {
    let list = |key: &str| truth[key].as_array().unwrap().clone();
    let evidence = |item: &Value| item["evidence"].as_array().map_or(0, Vec::len);
    list("containers").iter().map(|c| 2 + evidence(c)).sum::<usize>()
        + list("components").len() * 3
        + list("relations").iter().map(|r| 1 + evidence(r)).sum::<usize>()
}

#[test]
fn out_of_scope_items_are_listed_never_dropped() {
    let truth = truth();
    // The code pack's kinds and relations, as M1 loads them.
    let kinds = [
        "code.workspace",
        "code.repo",
        "code.service",
        "code.datastore",
        "code.queue",
        "code.library",
        "code.tooling",
        "code.package",
        "code.module",
        "code.directory",
        "code.file",
        "code.api_spec",
        "code.endpoint",
        "code.person",
        "code.commit",
    ];
    let relations = [
        "code.http",
        "code.queue_publish",
        "code.queue_consume",
        "code.db",
        "code.imports",
        "code.depends_on_package",
        "code.co_change",
        "code.owned_by",
    ];
    let report = compare(&truth, &Observed::default(), &Scope::new(kinds, relations, DEFERRED));

    // Nothing is found, and every check is either compared or listed with its reason, once.
    assert!(report.recall().is_empty());
    assert_eq!(report.total(), every_check(&truth), "{}", report.describe());
    let compared: BTreeSet<&str> =
        report.items.iter().flat_map(|i| &i.checks).map(|c| c.key.as_str()).collect();
    let listed: BTreeSet<&str> = report.out_of_scope.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(listed.len(), report.out_of_scope.len(), "each out-of-scope check is listed once");
    assert!(compared.is_disjoint(&listed));
    assert!(report.out_of_scope.iter().all(|o| !o.why.is_empty()));

    // DR-89's list, exactly: 16 relations of five kinds with their evidence, three containers'
    // observability kind, and two containers' evidence in the files only that pack reads.
    let deferred_relations: Vec<&Value> = truth["relations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| DEFERRED.relations.contains(&r["kind"].as_str().unwrap()))
        .collect();
    assert_eq!(deferred_relations.len(), 16);
    let mut want: BTreeSet<String> = BTreeSet::new();
    for r in &deferred_relations {
        let key = format!(
            "relation {} -> {} {}",
            r["from"].as_str().unwrap(),
            r["to"].as_str().unwrap(),
            r["kind"].as_str().unwrap()
        );
        for e in r["evidence"].as_array().unwrap() {
            want.insert(format!("{key} evidence {}:{}", e["path"].as_str().unwrap(), e["line"]));
        }
        want.insert(key);
    }
    for name in ["otel-collector", "prometheus", "grafana"] {
        want.insert(format!("container {name} kind observability"));
    }
    want.insert("container otel-collector evidence platform/otel/collector.yaml:2".into());
    want.insert("container prometheus evidence platform/prometheus/prometheus.yml:8".into());
    let listed: BTreeSet<String> = listed.into_iter().map(str::to_owned).collect();
    assert_eq!(listed, want, "{}", report.describe());
    assert_eq!(
        report.items.iter().filter(|i| i.key.starts_with("relation ")).count(),
        7,
        "7 of 23 relations compared"
    );

    // A relation kind no loaded pack declares is listed too, with that reason.
    let without_db = relations.into_iter().filter(|r| *r != "code.db");
    let report = compare(&truth, &Observed::default(), &Scope::new(kinds, without_db, DEFERRED));
    assert_eq!(report.total(), every_check(&truth));
    let db = report.out_of_scope.iter().find(|o| o.key == "relation worker -> orders-db db").unwrap();
    assert!(db.why.contains("no loaded pack declares"), "{}", db.why);
    assert!(report.item("relation worker -> orders-db db").is_none());
}
