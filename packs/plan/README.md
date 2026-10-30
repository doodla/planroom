# The plan pack

The plan pack takes a change from what you want to the next actions that get it done, in seven stages. It requires no other pack, and declares no kinds of its own: a project and its items are the kernel's.

Planroom embeds it in the binary, so every workspace loads it.

## What it declares

| Table | What the plan pack puts there |
|---|---|
| `[[stages]]` | `intent`, `outcome`, `breakdown`, `next_actions`, `priorities`, `premortem` and `lint`, each with a `label` the pane's stage picker shows, its method, the item types it writes and their props, its checklist and the lint rules it runs |
| `[[rules]]` | the nine structural lint rules below |
| `[[prompts]]` | `stage`, the Claude prefix every engine-run stage shares, in `prompts/plan.md` |

A project is a `kernel.project` node, `plan:<slug>:project`. Each plan item is a `kernel.item` node, `plan:<slug>:<key>`, whose `type` prop names an item type. What a next action touches is a `kernel.touches` edge to a node on the map.

## The stages

| Stage | Method | Item types and props | Lint |
|---|---|---|---|
| `intent` | Jobs-to-be-Done, laddering | `intent`: `when`, `want`, `so_that`, `ladder` | `intent_so_that` |
| `outcome` | SMART, non-goals | `outcome`: `metric`, `target`, `deadline`; `non_goal` | `outcome_metric`, `outcome_deadline` |
| `breakdown` | MECE, the WBS 100% rule | `work`: `method`, `scope` | `work_covers_parent_scope`, `work_children_disjoint` |
| `next_actions` | GTD, HTN | `next_action`: `verb`, `done_test`, `context` | `next_action_done_test`, `next_action_target` |
| `priorities` | MoSCoW | `work` and `next_action`: `moscow` (`must`, `should`, `could` or `wont`) | `item_moscow` |
| `premortem` | pre-mortem | `risk`: `story`, `likelihood` (`low`, `medium` or `high`), `mitigation` | `top_risk_mitigation` |
| `lint` | lint | none | all nine |

An item type's props are every stage's props for it together, so a `work` item takes `method`, `scope` and `moscow`.

A stage's `prompt` is its method's checklist, one step per line. `planroom ask stage STAGE` returns those lines, and `planroom intent next` sends them unchanged in the request's tail, after the shared prefix.

The shared prefix is `prompts/plan.md`: what each stage is for, the rules every drafted item follows, the reply's working limits and a worked example, then the reply's schema. It is the same bytes for every stage and every project, so after a plan's first stage each call reads the cache the first one wrote, for an hour. Keep each checklist in `pack.toml`, one step per line, and the prefix free of anything that names one stage's request.

## The lint rules

| Rule | Type | Flags |
|---|---|---|
| `intent_so_that` | `requires_prop` | an intent with no `so_that` |
| `outcome_metric` | `requires_prop` | an outcome with no `metric` |
| `outcome_deadline` | `requires_prop` | an outcome with no `deadline` |
| `work_covers_parent_scope` | `covers` | a work item whose child work items' `scope` tags leave one of its own out |
| `work_children_disjoint` | `disjoint` | a work item two of whose child work items share a `scope` tag |
| `next_action_done_test` | `requires_prop` | a next action with no `done_test` |
| `next_action_target` | `requires_edge` | a next action that touches no node on the map |
| `item_moscow` | `requires_prop` | a work item or next action with no `moscow` |
| `top_risk_mitigation` | `requires_prop` | a high-likelihood risk with no `mitigation` |

`covers` and `disjoint` count only a parent's child work items, so a work item whose children are next actions passes both. Four checks wait for later rule types: a ladder under 2 rungs, no non-goal, no MVP line and a Must below the line.

## Using it

```sh
planroom intent new "Add retries to the worker"
planroom ask stage next_actions
planroom intent next --stage outcome
planroom intent propose --base SEQ --file plan.json
planroom intent lint
planroom intent list
planroom intent version v1
```

An agent's proposal lands as ghosts, and so does every item `intent next` drafts, whoever runs it: Claude wrote it. A person accepts each item with `planroom review`. `intent version` is a person's alone.
