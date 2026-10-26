---
type: llm
focus: trace
weight: 1
---

merge-plan.md runs cargo xtask check and the full cargo xtask test (including the claude plugin test harness) on the merged tree before pushing, rather than relying on the track's report
