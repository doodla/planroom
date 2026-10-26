# Architecture review round 3: known gaps

Findings each fix cluster left open, with the reason. One row per finding.

| ID | Title | Why it stays a gap |
|---|---|---|
| correctness-5 | The reconciler misclassifies when a hand-off merge and a regressing commit land in one window | Evaluating change units one by one in topological order needs a fact state per unit and a rule for a fact several units touch, which changes the reconciler's core. S4's CI path scans between the merges, so it passes as specified, as round 2's correctness-10 recorded. |
| extensibility-6 | No pack field carries visual encoding, so surfaces must hard-code kind and lens IDs | Applied in part: `KindDecl.tone` styles blocks and a literal scan keeps kind IDs out of the surfaces (DR-46). A contrast-tested yarn palette for `LensDecl` to index is design work for design-language.md, best done with S10's evidence screenshots. |
| extensibility-8 | Kernel roles add a mechanism that buys no extensibility in v0.1 | A judgment call on complexity, not a defect: kernel decision kinds would move prompts and evals into the kernel, and rebinding roles in load order adds scope no scenario needs. |
