//! `cargo xtask bench fold` compares each fold case's median with its `budgets.json` budget
//! (architecture.md, Performance budgets; DR-47, DR-89).

use serde_json::json;
use xtask::bench::{FOLD_CASES, compare};

const UNDER: &str =
    "{\"bench\": \"fold.20k\", \"median_ms\": 12.5}\n{\"bench\": \"fold.50k\", \"median_ms\": 80}\n";

fn failures(output: &str, budgets: &serde_json::Value) -> Vec<String> {
    compare(output, budgets, &FOLD_CASES).into_iter().filter_map(|o| o.failure).collect()
}

#[test]
fn a_fold_over_its_budget_fails_the_bench() {
    let budgets = json!({"fold.20k": {"ms": 50}, "fold.50k": {"ms": 200}, "room_text": null});
    assert!(failures(UNDER, &budgets).is_empty(), "{:?}", failures(UNDER, &budgets));

    // Over one budget: that case fails, and only it.
    let over =
        "{\"bench\": \"fold.20k\", \"median_ms\": 50.01}\n{\"bench\": \"fold.50k\", \"median_ms\": 80}\n";
    let outcomes = compare(over, &budgets, &FOLD_CASES);
    let failed: Vec<&str> =
        outcomes.iter().filter(|o| o.failure.is_some()).map(|o| o.case.as_str()).collect();
    assert_eq!(failed, ["fold.20k"], "{outcomes:?}");
    assert!(outcomes[0].failure.as_deref().unwrap().contains("over its 50 ms budget"), "{outcomes:?}");

    // Exactly at the budget passes.
    let at = "{\"bench\": \"fold.20k\", \"median_ms\": 50}\n{\"bench\": \"fold.50k\", \"median_ms\": 200}\n";
    assert!(failures(at, &budgets).is_empty());

    // A budget with no number yet, or none at all, fails rather than passing unchecked.
    let empty = json!({"fold.20k": {}, "fold.50k": {"ms": 200}});
    assert_eq!(failures(UNDER, &empty), ["budgets.json holds no number at fold.20k.ms"]);
    assert_eq!(failures(UNDER, &json!({})).len(), 2);

    // A case the benchmark didn't report fails too, so a renamed case can't pass by vanishing.
    let missing = "cargo's noise\n{\"bench\": \"fold.50k\", \"median_ms\": 80}\n";
    assert_eq!(failures(missing, &budgets), ["the benchmark printed no result for fold.20k"]);
}
