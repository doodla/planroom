#!/usr/bin/env python3
"""Triage one review round from its workflow journals.

Reads each journal's `review:<focus>` and `verify:<focus>` results (older
journals label the verifier `refute:<focus>`), writes one `round<N>-<focus>.json`
per focus into OUT, and prints each finding's outcome: MATERIAL (unrefuted,
material after the verifier), minor (unrefuted, minor), or refuted. With --rows,
also writes the README's outcome table rows to that file; --fixes names a JSON
file mapping finding IDs to the branch that fixes them.

    python3 -I triage.py --round 1 --out docs/contributing/research/<phase>-review \
        JOURNAL [JOURNAL ...] [--rows rows.md] [--fixes fixes.json]
"""

import argparse
import json
import os
import sys


def load(journal):
    labels, results = {}, {}
    with open(journal) as f:
        for line in f:
            d = json.loads(line)
            if d.get("type") == "started":
                labels[d["key"]] = d["label"]
            elif d.get("type") == "result":
                results[d["key"]] = d["result"]
    return {labels.get(k, k): r for k, r in results.items()}


def outcome(finding, verdict):
    if not verdict:
        return "unverified"
    if verdict.get("refuted"):
        return "refuted"
    after = verdict.get("severity_after", finding["severity"])
    return "material" if after == "material" else "minor"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("journals", nargs="+")
    p.add_argument("--round", type=int, required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--rows")
    p.add_argument("--fixes", help="JSON file mapping finding IDs to the branch that fixes them")
    a = p.parse_args()
    fixes = json.load(open(a.fixes)) if a.fixes else {}
    os.makedirs(a.out, exist_ok=True)
    rows, counts = [], {"material": 0, "minor": 0, "refuted": 0, "unverified": 0}
    for journal in a.journals:
        res = load(journal)
        for label in sorted(res):
            if not label.startswith("review:"):
                continue
            focus = label.split(":", 1)[1]
            review = res[label] or {"findings": [], "summary": ""}
            check = res.get("verify:" + focus) or res.get("refute:" + focus) or {}
            verdicts = check.get("verdicts") or []
            with open(os.path.join(a.out, f"round{a.round}-{focus}.json"), "w") as f:
                json.dump({"focus": focus, "review": review, "verdicts": verdicts}, f, indent=1)
                f.write("\n")
            by_id = {v["id"]: v for v in verdicts}
            print(f"## {focus}")
            for finding in review.get("findings", []):
                o = outcome(finding, by_id.get(finding["id"]))
                counts[o] += 1
                tag = {"material": "MATERIAL", "minor": "minor", "refuted": "refuted", "unverified": "UNVERIFIED"}[o]
                print(f"  {tag:10} {finding['id']}: {finding['title'][:140]}")
                text = {
                    "material": "Material: fix" + (f" in {fixes[finding['id']]}" if finding["id"] in fixes else ""),
                    "minor": "Minor: known gap",
                    "refuted": "Refuted",
                    "unverified": "Unverified: no verdict",
                }[o]
                title = finding["title"].replace("|", "\\|")
                rows.append(f"| {finding['id']} | {title} | {text} |")
    print("\n" + ", ".join(f"{n} {k}" for k, n in counts.items()))
    if counts["unverified"]:
        print("Some findings have no verdict: a verifier is missing or still running.", file=sys.stderr)
    if a.rows:
        with open(a.rows, "w") as f:
            f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main()
