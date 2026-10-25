# Research: Jev (TypeSafe AI) for Planroom's cheap decisions

Phase 1 research, topic `jev`. Live spike run 2026-10-25 from this container (gateway region `iad1`), against `typesafe-ai/jev-1.14.0` unless noted.

## Evidence key

- **[V]** verified here. Each claim cites a fixture in `jev-fixtures/` (sanitized request and response, no auth headers) or a command. The raw call log and scripts were throwaway, in the scratchpad (`.../scratchpad/jev/`: `jevlib.py`, `latency.py`, `kinds.py`, `calibrate.py`, `fanout.py`, `errors.py`, `tspath.py`, `runs/calls.jsonl`). They are not in the repo. The fixtures carry everything load-bearing.
- **[D]** read in the docs, with a URL. Not exercised.
- **[A]** my assumption or inference.

Spend: 282 calls. 276 of them went through the gateway (one curl, then 275 logged), and the 6 direct-API calls were unauthenticated and not billed. Total **$0.00517** (sum of `providerMetadata.gateway.cost`). The cap was $0.05 and 400 calls.

---

## 1. Summary

- Jev is fast and cheap. The provider-side time is p50 65 ms and p95 90–110 ms, and Planroom sees about 300 ms (p50) at the client through this proxy. A one-question call costs about $0.000016–0.000023. [V]
- On the easy cases Jev is confident and right, 60 of 60. Booleans landed at ≥0.88 or ≤0.12, and choice confidence was ≥0.86. [V]
- On ambiguous cases it is **not** reliably in the middle. Only 2 of the 20 boolean ambiguous cases fell in [0.3, 0.7]. Choice answers stayed at confidence ≥0.95 in 8 of 10 ambiguous cases, 3 of them confidently against my label. Score questions behaved best: ambiguous items landed on the middle levels, and confidence dropped to 0.54–0.66 on the two genuinely borderline ones. [V]
- Answers are **not deterministic**. Repeat calls drift by up to 0.05 on a boolean, 0.10 on a score and 0.07 on a confidence. No choice flipped. [V] Cache answers, and keep thresholds away from knife edges.
- **Several questions about one item** in one request is safe and cheap: about 41 extra input tokens per question, the same latency, and stable answers. **Several items** in one state with `items[i]` questions is **not** safe. Answers moved by up to 0.84 (boolean) and 2.03 score levels. [V] (`anti-pattern-fanout-items.json`, `anti-pattern-fanout-items-boolean.json`)
- Pin the version. The unpinned `typesafe-ai/jev` alias echoes the alias and reports no version. It also lists a second provider (`digitalocean`) as a fallback, and its catalogue entry says `zdr: "some"` where the pinned IDs say `"all"`. [V]
- The proxy-injected gateway credential works on both gateway APIs: `/v1/evaluate` and the TypeSafe-compatible `/typesafe/v1/systemone`. [V] The direct API needs `TYPESAFE_API_KEY`, which is not set here. Without a key it returns **403** (the docs say 401), and with a bad key it returns 401. [V]

## 2. API facts

### 2.1 Gateway decision API (the primary backend)

`POST https://ai-gateway.vercel.sh/v1/evaluate`. The body is `{ model, state, questions, providerOptions? }`. [V] [D https://vercel.com/docs/ai-gateway/modalities/decision]

| | Request | Answer |
|---|---|---|
| boolean | `{type:"boolean", instructions, criteria?: {true, false}}` | `{type:"boolean", probability}` [V `gateway-boolean.json`] |
| choice | `{type:"choice", instructions, criteria: {name: description}}`, 1–255 options | `{type:"choice", choice, probabilities:{name:p}, confidence}` [V `gateway-choice.json`] |
| score | `{type:"score", instructions, criteria: [low … high]}`, **2–10** levels | `{type:"score", score, probabilities:{"0":p,…}, confidence}`, with **no `legend`** [V `gateway-score.json`, `gateway-error-score-*.json`] |

The response's top level is `{answers, model, usage:{inputTokens, outputTokens}, providerMetadata:{typesafe:{confidence:{q:c}}, gateway:{routing, cost, marketCost, surchargeCost, gatewayCost, inferenceCost, inputInferenceCost, outputInferenceCost, generationId}}}`. [V]

- Score answers *do* carry `confidence` on the gateway [V], although the Vercel doc's example omits it.
- `model` echoes the ID that was requested, such as `typesafe-ai/jev-1.14.0` or `typesafe-ai/jev`. [V] It does not name a resolved version.
- `routing.modelAttempts[].providerAttempts[]` has `startTime` and `endTime` in ms, and the difference is the provider-side latency. [V]
- Response headers: `x-vercel-id` (per request), `x-matched-path`, and `Access-Control-Expose-Headers: retry-after, x-ai-gateway-token-count, x-should-retry`. There is **no** `x-typesafe-request-id` on the gateway. Use `generationId` plus `x-vercel-id` as the request IDs. [V]
- Auth is `Authorization: Bearer $AI_GATEWAY_API_KEY` or a Vercel OIDC token. [D] Here the proxy injects it, and it even overrode a bogus `Authorization` header I sent. That call returned 200. [V]
- Optional `providerOptions.gateway`: `zeroDataRetention`, `only`, and `models` for execution-error fallbacks and **one** conditional "decision fallback" (`when: {question, confidenceBelow | probabilityBetween}`). The fallback reruns the whole request on another model, and both stages are billed. [D https://vercel.com/docs/ai-gateway/models-and-providers/decision-fallbacks] I did not exercise it.

### 2.2 TypeSafe-compatible path on the gateway

`POST https://ai-gateway.vercel.sh/typesafe/v1/systemone` and `GET /typesafe/v1/models`. [D https://vercel.com/docs/ai-gateway/sdks-and-apis/typesafe] [V]

The gateway credential works here. [V `typesafe-compat-systemone.json`] It differs from `/v1/evaluate` in these ways:

| | `/v1/evaluate` | `/typesafe/v1/systemone` |
|---|---|---|
| Boolean type | `boolean` → `probability` | `noul` → `noul`. Sending `boolean` is a 400. [V `typesafe-compat-error-boolean-type.json`] |
| Usage | `inputTokens`, `outputTokens` | `input_tokens`, `output_tokens` |
| Metadata key | `providerMetadata` | `provider_metadata` (same content) |
| Score | no legend | adds `legend: {"0": label, …}` |
| Model IDs | `typesafe-ai/jev-1.14.0`, `typesafe-ai/jev` | the same, plus bare `jev-1.14.0` and `jev-latest`. `jev-latest` routes to the unpinned `typesafe-ai/jev`. [V `typesafe-compat-bare-version.json`, `typesafe-compat-jev-latest.json`] |
| Validation error | `{error:{message, type:"invalid_request_error", param}}` | `{message, error_type:"invalid_request"}` |
| Unknown model | 404 `{error:{type:"model_not_found"}}` | the **same gateway envelope**, not the TypeSafe shape [V `typesafe-compat-error-bad-model.json`] |

The answers were identical across the two paths for the same request (p 0.86 against 0.86). Both reported 417 input tokens at the same cost. [V]

### 2.3 Direct TypeSafe API (the second backend)

`POST https://api.typesafe.ai/v1/systemone` with `Authorization: Bearer $TYPESAFE_API_KEY`. [D https://docs.typesafe.ai/api/system-one.md, https://api.typesafe.ai/openapi.json (v0.2.0)]

- Request: `{model, state, questions, assets?}`. The question types are `noul`, `choice`, `score` and `box`. `box` and assets need the `TypeSafe-Danger-Zone-Images: true` header. A Noul's `criteria` is `{true, false}`. `instructions` is optional on Noul and Box, and can be a string, an object or an array. Choice takes 1–255 options. The OpenAPI says Score takes **1**–10 levels, while the gateway enforces ≥2. [D] [V gateway side]
- Response: `{model, answers, usage:{input_tokens, output_tokens}}`. `model` is "Name of the model that answered … May differ from the alias", so `jev-latest` comes back as the versioned ID. [D https://docs.typesafe.ai/models.md] Score answers include `legend`.
- The header `x-typesafe-request-id: req_…` is present, even on auth errors. [V `direct-error-no-key.json`]
- `GET /v1/models` lists the aliases. Versioned IDs such as `jev-1.14.0` are accepted whether or not they are listed. [D] **So the direct backend can pin `jev-1.14.0`.** The docs list it as a model ID, so the `jev-latest` fallback in KICKOFF is not needed. [D https://docs.typesafe.ai/models.md]
- Long context: the `TypeSafe-Danger-Zone-Long-Context: true` header raises both limits to 262k. It is "slower and less accurate". [D] Planroom should not use it.
- Compression: gzip or zstd request bodies are accepted. [D https://docs.typesafe.ai/api/errors.md]
- Success fixtures for the direct API are built from the OpenAPI examples (`direct-docs-{noul,choice,score,422}.json`). No key was available for a live capture.

### 2.4 Models, prices, limits

| | Value | Evidence |
|---|---|---|
| Versions | `jev-1.14.0` (released 2026-10-14/15) and `jev-1.13.0`. On the direct API, `jev-latest` and `jev-preview` both point to `jev-1.14.0` today. | [D models.md] [V gateway catalogue `GET /v1/models`] |
| Price | $0.042 per Mtok of input. Output is free. | [D] [V catalogue `pricing.input "0.000000042"`, `outputInferenceCost "0"`. Cost = inputTokens × 4.2e-8 exactly, e.g. 384 tok → $0.000016128] |
| Context | 32k tokens for state plus one question, 64k per request. No cap on the number of questions. | [D] [V oversize state → 400] |
| Body | ≤32 MiB, UTF-8 without BOM, ≤64 nesting levels, finite numbers only | [D errors.md] |
| Rate limits | Per organisation, in tokens/s and requests/s. Exceeding them returns 429. 529 means overloaded. Honour `retry-after`. | [D models.md, errors.md]. Not observed in 282 sequential calls. [V] |
| Data | Pinned gateway IDs: `zdr: "all"`, `no_training: "all"`. The alias `typesafe-ai/jev` has `zdr: "some"`. | [V catalogue] |

## 3. Measured latency [V `latency-summary.json`]

There were 30 sequential single-question calls and 10 calls with 5 questions each (3 boolean, 1 choice, 1 score), all on the same small plan-item state.

| | n | client wall p50 | wall p95 | provider p50 | provider p95 | input tokens | cost/call |
|---|---|---|---|---|---|---|---|
| 1 question | 30 | 298 ms | 518 ms | 64.5 ms | 110 ms | 384 | $0.0000161 |
| 5 questions | 10 | 294 ms | 329 ms | 65.0 ms | 90 ms | 550 | $0.0000231 |

- The client wall time includes the agent proxy and TLS. The gateway answered with `Connection: close`, so every call paid a new handshake. [V] Production on a user's machine will differ. Budget about 300–600 ms per call, and keep calls off the UI's critical path.
- Batching five questions about one item costs no extra latency. The fan-out calls with 15 questions also stayed at 60–91 ms provider-side. [V `fanout.json` in the scratchpad, `anti-pattern-fanout-items.json`]

## 4. Token and cost model [V]

- Fixed overhead is about **260 input tokens per request**: state `"x"` with one question `"x?"` cost 269 tokens. Each minimal extra question adds about 9 tokens, and a realistic Planroom question adds about 40.
- The real Planroom decision kinds took 363–550 input tokens for one question (mean 444, which is $0.0000187).
- **S7 arithmetic:** $0.001 buys about 23,800 input tokens. That is about 53 uncached single-question requests of the typical size, or about 40 requests carrying several questions each over a 300-token slice. A routine commit rescan should touch a handful of changed nodes, and cache hits are free, so the $0.001 target is comfortable *if* the cache works and requests are grouped per item. [A]
- Errors are not billed. Validation errors, unknown models and the oversize `max_tokens_exceeded` all came back with no cost. [V `gateway-error-*.json`]

## 5. Calibration on Planroom decision kinds [V `calibration-table.json`, `calibration-cases.json`]

I wrote six kinds with 15 cases each: 5 easy-positive, 5 easy-negative and 5 ambiguous. For the choice kinds, "easy" means 10 clear cases spread across the options. **I wrote the labels myself, before any call. These are development cases, not S7's independent held-out set.** Each case ran twice (r1, r2), and the choice kinds ran a third time with the option order reversed. Every request carried one question.

| Kind | Type | Easy correct | Easy spread | Ambiguous correct | Ambiguous outputs | Max run-to-run drift |
|---|---|---|---|---|---|---|
| next action is concrete (verb, target, done-test) | boolean | 10/10 | pos 0.90–0.97, neg 0.02–0.04 | 5/5 | p 0.08, 0.10, 0.16, 0.75, 0.89 (0 in [0.3, 0.7]) | 0.02 |
| file → component (6 options) | choice | 10/10 | conf 1.00 | 4/5 | conf 0.96–1.00 (am1 wrong at 0.96) | conf 0.02, no flips |
| inferred dependency A→B is real | boolean | 10/10 | pos 0.88–0.99, neg 0.02–0.12 | 3/5 | p 0.11, 0.19 (wrong), 0.58, 0.59 (wrong), 0.92 | 0.05 |
| intent-vs-actual mismatch (5 classes) | choice | 10/10 | conf 0.87–1.00 | 3/5 | conf 0.41, 0.95, 0.96, 0.97 (wrong), 1.00 (wrong) | conf 0.06, no flips |
| commit addresses review thread | boolean | 10/10 | pos 0.89–0.98, neg 0.02–0.03 | 5/5 | p 0.10, 0.16, 0.75, 0.79, 0.87 (0 in [0.3, 0.7]) | 0.03 |
| how vague is this plan item (4 levels) | score | 10/10 within 0.5 of the label | conf 0.69–1.00 | 5/5 nearest level | scores 1.03, 1.21, 1.20, 1.99, 2.12; conf 0.54–0.95 | score 0.10 |

What this shows:

1. **The easy cases come out confident.** All 60 were right, with booleans at ≥0.88 or ≤0.12 and choice confidence ≥0.86.
2. **The ambiguous cases mostly do not come out in the middle.** Jev commits to an answer. The middle band [0.3, 0.7] caught only 2 of 20 ambiguous boolean cases, both dependency edges: `docker-compose depends_on` with no import, and a codegen script. Uncertainty showed up as softer extremes (0.75–0.89) more often than as values near 0.5. A three-band policy needs a wide card band, such as 0.15–0.85, not 0.3–0.7. [A from V]
3. **Every confident mistake was literal reading.** Each of the three confident choice errors followed the instruction text over my intent, which is jaggedness item 1 in the docs [D https://docs.typesafe.ai/model-jaggedness/jev-1.13.md]:
   - The file `crates/planroom-server/src/db_pool.rs` imports rusqlite. Jev said `planroom-server` at 0.96, and the instruction said "use the path first".
   - A cache that exists but whose done-test "was never run". Jev said `in_progress` at 1.00, which is defensible.
   - A spike file that nothing imports. Jev said `in_progress` at 0.97 against my "expected".

   An independent labeller will produce exactly these disagreements. So precise instructions and criteria, written from the labeller's rationales, are what reaches 0.95, not threshold tuning.
4. **Score is the best-calibrated type for degree judgements.** Precise items scored 0.00–0.01 with confidence 1.0, and wishes scored 2.69–2.96. Borderline items landed between levels with lower confidence.
5. **No choice flipped when I reversed the option order** (0/30). Confidence moved by up to 0.14 (mismatch am4: 0.41 → 0.55). The docs warn about first-option bias. [D] It did not show here, but the eval should still shuffle.
6. **Answers are nondeterministic**, though only slightly. Thirty identical calls gave 0.95 fifteen times and 0.96 fifteen times. A score's confidence ranged 0.41–0.61 over 10 identical calls. [V `latency.json`]
7. **Do not put several items into one state.** Fifteen next-actions in `{items:[…]}` with 15 `items[i]` questions saved 2.8× on tokens. But a concrete action that had scored 0.08 came back 0.92, and precise plan items scored 0.21–2.03 instead of 0. [V `anti-pattern-fanout-items.json`, `anti-pattern-fanout-items-boolean.json`] This is the indirection and distractor failure the docs describe. [D jaggedness items 4–5] Batch questions **about one slice**. Do not batch slices.

## 6. Error codes

| Case | Gateway `/v1/evaluate` | TypeSafe path | Direct (docs unless marked) | Retry? |
|---|---|---|---|---|
| Unknown model | 404 `model_not_found` [V] | 404, gateway envelope [V] | 422 or 404 [A] | no |
| Missing or invalid field | 400 `invalid_request_error`, `message` names the field (`state: Invalid input`, `questions.q.type: Invalid discriminator value…`) [V] | 400 `{message, error_type:"invalid_request"}` [V] | 422 `{detail:[{loc, msg, type, input?, ctx?}]}` [D] | no |
| Invalid JSON, or NaN | 400 "Invalid JSON in request body" [V] | presumably the same [A] | 400 [D] | no |
| Over the context limit | 400 `AI_APICallError`, message `{"error_type":"max_tokens_exceeded"}`, `isRetryable:false`, not billed [V `gateway-error-oversize-state.json`] | not tested | 400 `max_tokens_exceeded` [D] | no (shrink the slice) |
| Body over 32 MiB | not tested | | 413 [D] | no |
| Bad `Content-Encoding` | not tested | | 415 [D] | no |
| No key | not testable (proxy injects) | | **403** `{detail:{error_type:"authentication_error", message}}` [V] (docs say 401) | no |
| Bad key | not testable | | 401 `authentication_error` [V] | no |
| Rate limited | 429 + `retry-after` [D] | | 429 [D] | yes, with backoff |
| Overloaded | | | 529 [D] | yes |
| Upstream 5xx | 5xx; `x-should-retry` is exposed [D/V header list] | | the SDK retries 500, 502, 503, 504 [D https://docs.typesafe.ai/sdk/python/api/retries.md] | yes |

## 7. Recommended Rust adapter (`planroom-decide::jev`)

One trait and one normalised data model, with two wire codecs. Backend choice is configuration, not code.

```rust
// Normalised types (Planroom's own; serialised in the decision log and fixtures)
pub enum Question {
    Boolean { instructions: String, criteria: Option<BoolCriteria> },   // wire: "boolean" | "noul"
    Choice  { instructions: String, options: IndexMap<String, String> }, // order is preserved: it reaches the model
    Score   { instructions: String, levels: Vec<String> },               // 2..=10, low → high
}
pub enum Answer {
    Boolean { p: f64 },
    Choice  { choice: String, probabilities: IndexMap<String, f64>, confidence: f64 },
    Score   { score: f64, probabilities: Vec<f64>, confidence: f64 },
}
pub struct Evaluation {
    pub answers: BTreeMap<String, Answer>,
    pub model_requested: String,          // what we sent
    pub model_reported: String,           // response `model`; gateway echoes the request
    pub input_tokens: u32, pub output_tokens: u32,
    pub cost_usd: Option<f64>,            // gateway `cost`; direct: tokens × price table
    pub request_id: Option<String>,       // gateway generationId (+ x-vercel-id); direct x-typesafe-request-id
    pub provider_ms: Option<u32>, pub wall_ms: u32,
}
#[async_trait] pub trait JevBackend: Send + Sync {
    async fn evaluate(&self, state: &serde_json::Value, qs: &BTreeMap<String, Question>) -> Result<Evaluation, JevError>;
}
pub enum Backend {
    Gateway  { base: Url /* https://ai-gateway.vercel.sh */, model: String /* typesafe-ai/jev-1.14.0 */ }, // /v1/evaluate
    TypeSafe { base: Url /* https://api.typesafe.ai | https://ai-gateway.vercel.sh/typesafe */, model: String /* jev-1.14.0 */ }, // /v1/systemone
}
pub enum JevError {
    Invalid { status: u16, message: String, field: Option<String> }, // 400/422; never retried
    ContextTooLong,                                                   // max_tokens_exceeded
    ModelNotFound(String), Auth { status: u16 },                      // 404; 401/403
    RateLimited { retry_after: Option<Duration> }, Overloaded,        // 429; 529
    Upstream { status: u16 }, Transport(String), Decode(String),
    BudgetExceeded,                                                   // local spend cap hit
}
```

- **Codecs.** The `Gateway` codec covers `/v1/evaluate`: `boolean`/`probability`, camelCase usage and `providerMetadata`. The `TypeSafe` codec covers both the direct API and the gateway's `/typesafe` path: `noul`/`noul`, snake_case, and a `legend` that is dropped. Three error parsers: the gateway envelope `{error:{…}}`, `{message, error_type}`, and the direct API's `{detail: …}`. Even the TypeSafe path returns the gateway envelope for an unknown model, so try both. [V]
- **Pinning.** The gateway pins `typesafe-ai/jev-1.14.0`, and the direct API pins `jev-1.14.0`. Log `model_reported`. On the direct API it should come back as the versioned ID [D]. On the gateway it is only an echo [V], so the pin *is* the record of the version.
- **Contract tests.** Both codecs are fed every fixture in `jev-fixtures/` and must produce the same `Evaluation` and `JevError` (S7: "Both adapters pass the same contract tests against fixtures built from the docs"). The `typesafe-compat-*` fixtures exercise the TypeSafe codec with live data, which partly makes up for the missing direct key.
- **Record and replay.** A `Recorded<B: JevBackend>` wrapper has three modes: `Live`, `Record(dir)` and `Replay(dir)`. Fixtures are keyed by the request hash. CI runs `Replay` only. Live tests run only when `PLANROOM_LIVE_JEV=1`, and the key-gated direct smoke test runs only when `TYPESAFE_API_KEY` is set. The smoke test records `model_reported`.
- **Cache.** The key is `blake3(backend_model ‖ kind_id ‖ kind_version ‖ serialized(questions) ‖ serialized(state))`. Serialise with serde_json `preserve_order`, because option order is semantically live. Hits cost $0 and make answers deterministic, which matters given the ±0.05 drift.
- **Retries.** Up to 3 attempts with exponential backoff and jitter. Honour `retry-after` and `retry-after-ms`. Retry only on 429, 529, 500, 502, 503, 504 and transport errors. Never retry 4xx validation errors or `max_tokens_exceeded`. Use a 10 s per-call timeout. [D SDK defaults: 3 retries, 10 s]
- **Pre-flight checks** in code, so no billable call is wasted:
  - estimate tokens at chars/4 [A] and reject slices over about 8k tokens, well under 32k, because accuracy falls with state size [D];
  - score levels 2–10 and choice options 1–255;
  - finite numbers only.
- **Spend guard.** A per-scope budget, such as one rescan at $0.001, is charged from the reported `cost`. When the next call would exceed it, the adapter returns `BudgetExceeded` and the decision is queued.
- **No gateway decision fallbacks.** Do not use `providerOptions.gateway.models` for escalation to Claude. Escalation should go through Planroom's own Claude adapter, so it is recorded, budgeted, and counted in S7's "0 Claude calls on the routine path". [A]
- **Crates** (exact versions, each at least two weeks old on 2026-10-25, checked on crates.io):
  - `reqwest = "=0.13.5"`, with `json` and `rustls` features [A on the feature names];
  - `serde = "=1.0.229"`;
  - `serde_json = "=1.0.154"`, with `preserve_order`;
  - `tokio = "=1.53.2"`;
  - `thiserror = "=2.0.21"`;
  - `blake3 = "=1.8.7"`;
  - tests: `wiremock = "=0.6.5"` and `insta = "=1.49.0"`;
  - `indexmap` comes in through serde_json.

  I hand-roll the backoff (about 20 lines) rather than adding `backon`.

## 8. What a decision kind should declare

The pack format will carry these fields. [A, informed by V]

```yaml
id: next_action_concrete
version: 3                      # bump → cache invalidated, eval re-run
question:
  type: boolean                 # boolean | choice | score
  instructions: "..."           # literal: the exact condition (Jev reads literally)
  criteria: {true: "...", false: "..."}   # boundary cases from labeller rationales
state:
  query: plan_item_with_next_action       # named query in the query API; one item per request
  max_tokens: 2000
  fields: [next_action, component_names]  # only what the question needs
group: plan_item                # kinds with the same slice share one request
precheck: rules/next_action.rs  # deterministic rules run first; Jev only if undecided
policy:                         # per outcome, because the stakes differ
  auto:   {p_at_least: 0.92, or_p_at_most: 0.08}   # only if eval.auto_enabled
  card:   {else: true, preselect: jev}             # one-tap, Jev's pick pre-selected
  escalate_to_claude: {p_between: [0.35, 0.65]}    # or: confidence_below for choice/score
  auto_choices: [done, unplanned]                   # choice kinds: which outcomes may auto-act
  on_unavailable: card
eval:
  cases: evals/next_action_concrete.jsonl   # independent labels plus rationales, held out
  min_cases: 60
  measured_on: typesafe-ai/jev-1.14.0
  precision_at_auto: 0.97        # filled in by the eval run, never by hand
  coverage_at_auto: 0.71
  auto_enabled: true             # true only if precision ≥ 0.95 on ≥ 60 cases
budget: {max_calls_per_rescan: 20}
record: true                     # every decision logged with inputs, answer, model, cost and override
```

## 9. Implications for S7

1. **Run the evals on `typesafe-ai/jev-1.14.0` through `/v1/evaluate`**, the same pin and codec as production. Run every case twice, and both runs must meet the bar. Drift of ±0.05 can move a borderline case across a threshold. [V]
2. **Precision is measured over the auto-acted subset.** Report coverage as well. With 60 held-out cases per kind, observed precision ≥0.95 allows at most 3 wrong auto-actions among the acted cases. Even 60/60 correct has a 95% Wilson lower bound of only about 0.94. Treat 60 as the floor, and require at least 30 cases above the threshold before enabling auto-action. [A, a statistical note]
3. **Which kinds can reach auto-action.** The evidence comes from the dev cases only.

   Likely candidates:
   - next-action concreteness;
   - commit addresses review thread;
   - plan vagueness (score), where easy and ambiguous cases were all handled well;
   - file → component, but only once a deterministic path rule decides first and Jev handles only the leftovers.

   Unlikely, in the near term:
   - mismatch classification, because of confident literal-reading disagreements, and because `done` and `regression` are high-stakes. Use cards with Jev's pick pre-selected.
   - dependency inference, because ambiguous evidence gave both middle values and a confident 0.19 miss. Use cards.
4. **Set the card band wide** (about 0.15–0.85 for booleans, and confidence below about 0.9 for choices), so ambiguous items fall into it. Fit the band on the independent labels, not on the 0.3/0.7 default in the docs.
5. **Rescan under $0.001 and at most one card.**
   - One request per changed slice, with all kinds for that slice as questions in it.
   - A cache keyed by the request hash.
   - A `BudgetExceeded` guard at $0.001.
   - Never more than one slice per request.
   - At about $0.00002 per request, the budget allows about 50 uncached requests. The fixture rescan should need fewer than 10. [A]
   - "At most one card" is a policy matter: batch cards, and surface the highest-value one.
6. **CI path.** Every Jev call is replayed from recorded fixtures keyed by the request hash. Live evals run behind `PLANROOM_LIVE_JEV=1`. Expected live spend for a full eval pass is 6 kinds × 60 cases × 2 runs × about $0.00002 ≈ **$0.015**, well inside the $3 build budget. [A from V]

## 10. Risks and open questions

1. **Literal reading against the independent labeller.** Confident disagreements on ambiguous cases (0.96–1.00) cannot be thresholded away. Iterating on instructions and criteria uses the labelled set, so keep a separate tuning split from the held-out split, or S7's "held out from threshold tuning" is violated.
2. **The gateway does not report the model version.** It echoes the requested ID. If TypeSafe silently re-deployed under the same versioned ID, nothing would show it. Watch the eval drift. The direct API reportedly returns the versioned ID [D], but this was not verified, because there is no key.
3. **Docs and live behaviour diverge.** Direct missing-key is 403, not 401. Gateway score needs ≥2 levels while the OpenAPI says ≥1. Gateway score answers carry confidence although the Vercel doc example omits it. Error envelopes differ on the TypeSafe path. The parsers must be tolerant, and contract tests should come from live fixtures where possible.
4. Rate limits, 429s and 529s were not observed. Retry handling is built from the docs only.
5. Latency was measured through the agent proxy from `iad1`, with a fresh connection each time. Real user latency is unknown.
