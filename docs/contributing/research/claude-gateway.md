# Research: Claude through the Vercel AI Gateway (for the Rust engine)

Phase 1 research, topic `claude-gateway`. Spike date 2026-10-25. All live calls went from this build container to `https://ai-gateway.vercel.sh`. The container's proxy injects credentials for `*.vercel.sh`, so no request carried a key. **Total live spend: about $0.0219** (the sum of the gateway-reported `cost` over every call; budget was $0.15). Of that, $0.0135 was the Sonnet 5.5 cache check. Everything else ran on `anthropic/claude-haiku-5.5`.

## Evidence key

- **[V]**: I ran it here. The request and the sanitized response are in `claude-fixtures/<name>.json` (named in brackets). Each fixture holds `request` (method, url, non-auth headers, body), `response` (status, headers with auth and cookies removed, body) and `measured` (`ttfb_s`, `total_s`, UTC time). The harness was a 60-line Python wrapper around `curl -D … -w "%{http_code} %{time_starttransfer} %{time_total}"`. It is throwaway and lives in the scratchpad, not the repo.
- **[D]**: read in Vercel docs (URL given), not checked here.
- **[S]**: read in the Claude API skill bundled with Claude Code 2.1.310 (`claude-api/shared/prompt-caching.md`), not checked against the gateway.
- **[A]**: assumption or inference.

Secret scan [V]: I grepped all 46 fixtures for `authorization`, `x-api-key`, `bearer `, `vck_` and `sk-` and found no hits. The fixtures still contain gateway generation ids, provider request ids and a `clientSessionId` fingerprint. These are opaque and not credentials.

---

## 1. Endpoints

| Endpoint | Claude works? | Evidence |
|---|---|---|
| `POST /v1/chat/completions` (OpenAI-compatible) | **Yes** | [V] `chat-completions-basic`: 200, `"content":"pong"`, `finish_reason:"stop"` |
| `POST /v1/messages` (Anthropic-compatible) | **Yes** | [V] `messages-basic`: 200, Anthropic `message` shape, `stop_reason:"end_turn"` |
| `POST /v1/messages/count_tokens` | **Yes** | [V] `messages-count-tokens`: 200 `{"input_tokens":10264}`, no cost reported |
| `GET /v1/models` | **Yes** | [V] `models-list-anthropic-subset`: 480 models in total, 32 `anthropic/*` |
| `POST /v1/messages` with `stream:true` | **Yes**, Anthropic SSE events | [V] `stream-messages-haiku` |
| `/v1/messages/batches` | not tried (docs say early access, team must be enabled) | [D] |

Facts about the Messages endpoint:

- The base URL is `https://ai-gateway.vercel.sh`. The documented path is `/v1/messages`. Auth is `x-api-key: <key>` or `Authorization: Bearer <key>`, and Vercel OIDC tokens go as Bearer. [D] https://vercel.com/docs/ai-gateway/sdks-and-apis/anthropic-messages-api
- The `anthropic-version` header is **not required**: a call without it returned 200 [V] `messages-no-version-header`. Send `2023-06-01` anyway for SDK parity.
- Model ids are `anthropic/<name>` with dotted versions (`anthropic/claude-haiku-5.5`). The bare Anthropic id `claude-haiku-5-5` was also accepted. It was resolved to `canonicalSlug: anthropic/claude-haiku-5.5`, and the response echoed `model: "claude-haiku-5-5"` [V] `lenient-messages-bare-model-id`. Pin the gateway form.
- The response carries an extra top-level `provider_metadata` object, which the Anthropic API does not return. `provider_metadata.anthropic.usage` is the raw upstream usage. `provider_metadata.gateway` holds `routing` (provider attempts, fallbacks, affinity) and the **cost** fields [V].
- By default the gateway may route one Claude model to `anthropic`, `bedrock`, `vertexAnthropic` or `claudeaws`, in that order, with fallthrough on failure. Every routing block in the fixtures showed this list [V]. Setting `providerOptions.gateway.only: ["anthropic"]` in the JSON body restricted routing to Anthropic: `"planningReasoning":"Provider set restricted to: anthropic…"`, and `fallbacksAvailable: []` [V] `messages-only-anthropic`.

**The gateway is not a byte-for-byte proxy [V].** It validates and re-encodes requests through its own model layer. Error types like `AI_APICallError` point to the AI SDK [A]. Requests the Anthropic API would reject, according to the skill [S], went through with 200 here, and no warning appeared in `provider_metadata.gateway`:

| Request | Anthropic API (per [S]) | Gateway [V] |
|---|---|---|
| `max_tokens: 200000` on Haiku 5.5 (cap 128000) | 400 | 200 `lenient-messages-max-tokens-over-cap` (also on chat-completions) |
| `temperature: 0.3` on Haiku 5.5 | 400 (non-default sampling) | 200 `lenient-messages-temperature` |
| `thinking: {type:"enabled", budget_tokens:1024}` on Haiku 5.5 | 400 | 200 `lenient-messages-budget-tokens` |
| 5 `cache_control` breakpoints (max 4) | 400 | 200 `lenient-messages-five-breakpoints` |
| forced `tool_choice: {type:"tool"}` on Sonnet 5.5 | 400 | 200 with `tool_use` `tool-forced-messages-sonnet55` |

On the forced-tool case: Sonnet 5.5 reported `input_tokens: 450`. Haiku 5.5 with forced choice reported 576, and Haiku 5.5 with `auto` plus "Use the tool" in the prompt reported 460. That pattern suggests the gateway silently downgraded the choice to `auto` [A]. **Consequence for Planroom:** contract tests against the gateway cannot prove a request is valid on the direct Anthropic API, and the reverse holds too. The adapter should build only shapes valid on both APIs (no sampling params, no `budget_tokens`, ≤ 4 breakpoints, `tool_choice: auto`).

## 2. Claude models on the gateway

These are `GET /v1/models` results, filtered to `anthropic/`, non-batch [V] (`models-list-anthropic-subset`). Prices are USD per 1M tokens, converted from the per-token strings the endpoint returns.

| id | ctx | max out | in | out | cache read | cache write (5m) |
|---|---|---|---|---|---|---|
| `anthropic/claude-haiku-5.5` | 1M | 128k | 0.10 (0.50 above 100k) | 0.50 (2.50 above 100k) | 0.01 | 0.125 |
| `anthropic/claude-haiku-4.5` | 200k | 64k | 1.00 | 5.00 | 0.10 | 1.25 |
| `anthropic/claude-sonnet-5.5` | 1M | 128k | 2.00 | 10.00 | 0.10 | 2.50 |
| `anthropic/claude-sonnet-5` | 1M | 128k | 2.00 | 10.00 | 0.20 | 2.50 |
| `anthropic/claude-sonnet-4.6`, `-4.5` | 1M | 128k / 64k | 3.00 | 15.00 | 0.30 | 3.75 |
| `anthropic/claude-opus-5.5` | 1M | 128k | 4.00 | 20.00 | 0.20 | 5.00 |
| `anthropic/claude-opus-5`, `-4.8`, `-4.7`, `-4.6`, `-4.5` | 1M (4.5: 200k) | 128k (4.5: 64k) | 5.00 | 25.00 | 0.50 | 6.25 |
| `anthropic/claude-fable-5.1` / `-fable-5` | 1M | 128k | 10.00 | 50.00 | 0.25 / 1.00 | 12.50 |

The listing also has `-batch` variants at half price, `-fast` variants (`opus-4.8-fast`, `opus-5-fast`, `opus-5.5-fast`), and `regional` price overrides (us/eu/au/jp, roughly +10%). Every Claude model is tagged `explicit-caching`, `tool-use` and `structured-output` [V].

I checked these models live: `claude-haiku-5.5`, `claude-sonnet-5.5` and `claude-opus-5.5` all returned 200 [V] (`messages-basic`, `messages-basic-claude-sonnet-5.5`, `messages-basic-claude-opus-5.5`).

**Recommended pins (for config):**

- **Cheap tier: `anthropic/claude-haiku-5.5`.** It is the cheapest Claude listed by 10×, has a 1M context, and handled structured output, tools and caching correctly in every test. Watch the price step above 100k input tokens (5×).
- **Capable tier: `anthropic/claude-sonnet-5.5`.** It costs half of Opus 5.5, and the gateway lists its cache reads at $0.10, half of Opus 5.5's $0.20. Make `anthropic/claude-opus-5.5` a config-only upgrade for plan-level (S3) synthesis if evals show a need. These are dated releases, not "latest" aliases. The gateway ids carry no date suffix, so pinning means pinning this exact string and recording the `pricing` block alongside it [A].

## 3. Prompt caching

### Formats that work [V]

All six formats below wrote the cache on call 1 and read it on call 2. The model was Haiku 5.5, with a stable system prefix of 5,145 to 5,167 tokens:

| Endpoint | Format | Fixture |
|---|---|---|
| messages | `system: [{type:"text", text, cache_control:{type:"ephemeral"}}]` (explicit block marker) | `cache-messages-haiku-{1,2,3}` |
| messages | top-level `cache_control: {type:"ephemeral"}` (Anthropic automatic caching) | `cache-messages-toplevel-haiku-{1,2}` |
| messages | body `providerOptions: {gateway: {caching: "auto"}}` (gateway inserts markers) | `cache-messages-auto-haiku-{1,2}` |
| messages | `cache_control: {type:"ephemeral", ttl:"1h"}` → counted in `cache_creation.ephemeral_1h_input_tokens` | `cache-messages-1h-haiku-1` |
| chat/completions | `cache_control` as a sibling key on the message object (`{role:"system", content:"…", cache_control:{…}}`) | `cache-chatcompletions-haiku-{1,2,3}` |
| chat/completions | `cache_control` on a content part (`content:[{type:"text", text, cache_control}]`) | `cache-chatcompletions-parts-haiku-{1,2}` |
| chat/completions | `providerOptions.gateway.caching:"auto"` | `cache-chatcompletions-auto-haiku-{1,2}` |

Sonnet 5.5 also cached [V] (`cache-messages-sonnet55-{1,2}`): call 1 wrote 5,145 tokens, call 2 read 5,145.

### The 3× cached-prefix experiment [V]

Request: `anthropic/claude-haiku-5.5`, `max_tokens: 60`, a system block of 110 synthetic "Rule NNN" lines tagged `[A]` with `cache_control: ephemeral`, then one short user question. The header `x-session-affinity: planroom-spike-a` was set. The same body was sent three times within about 5 s.

| Call | input_tokens | cache_creation_input_tokens | cache_read_input_tokens | output (thinking) | gateway cost $ | total_s | affinity |
|---|---|---|---|---|---|---|---|
| 1 `messages` | 26 | 5145 (`ephemeral_5m`) | 0 | 59 (54) | 0.000675 | 1.46 | `no_pin` |
| 2 `messages` | 26 | 0 | 5145 | 5 (0) | 0.0000566 | 0.81 | `confirmed`, pinned `anthropic` |
| 3 `messages` | 26 | 0 | 5145 | 5 (0) | 0.0000566 | 0.72 | confirmed |
| 1 `chat/completions` | prompt 5171 | 5145 | cached 0 | 5 | 0.000648 | 2.10 | — |
| 2 `chat/completions` | prompt 5171 | 0 | cached 5145 | 5 | 0.0000566 | 1.78 | — |
| 3 `chat/completions` | prompt 5171 | 0 | cached 5145 | 48 (43) | 0.0000781 | 2.34 | — |

The costs reconcile with list prices. Call 1 is 5145 × $0.125/M + 26 × $0.10/M + 59 × $0.50/M ≈ $0.000675. Call 2 is 5145 × $0.01/M + 26 × $0.10/M + 5 × $0.50/M ≈ $0.0000566. A read call costs **12× less** than a write call.

### Usage fields and cache-hit rate

**Messages endpoint [V]:** `usage.input_tokens` counts **only uncached** input. The other fields are `usage.cache_creation_input_tokens`, `usage.cache_read_input_tokens`, and `usage.cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}`. `usage.output_tokens_details.thinking_tokens` is a subset of `output_tokens`.

```
prompt_total   = input_tokens + cache_creation_input_tokens + cache_read_input_tokens
cache_hit_rate = cache_read_input_tokens / prompt_total
```

**Chat-completions endpoint [V]:** `usage.prompt_tokens` **includes** cached tokens (5171 = 26 + 5145). `usage.prompt_tokens_details.cached_tokens` counts reads. `usage.cache_creation_input_tokens` sits at the top level of `usage` and is a gateway extension that counts writes. `completion_tokens_details.reasoning_tokens` counts thinking.

```
cache_hit_rate = prompt_tokens_details.cached_tokens / prompt_tokens
```

For token budgets (KICKOFF S7), aggregate over a run as `Σ cache_read / Σ prompt_total`, not as a mean of per-call rates. Also store `cache_write_tokens` per call: a repeated write on a prefix that should be stable is the regression signal. The budget test should fail on it.

### Caching gotchas

- **Minimum prefix.** The skill [S] gives 512 tokens for Haiku, Sonnet and Opus 5.5, and 4096 for Haiku 4.5. The Vercel page says "currently 1024 tokens for most Claude models" [D]. I did not test the threshold. Keep Planroom's stable prefix ≥ 1024 tokens, or measure. A prefix below the minimum fails **silently** (`cache_creation_input_tokens: 0`).
- **Cache lives per provider.** Anthropic, Bedrock and Vertex each keep their own cache [A, standard behavior]. The gateway's default four-provider fallthrough means a write on `anthropic` followed by a read routed to `bedrock` is a miss. Pin `only: ["anthropic"]`, or at least send `x-session-affinity`. The affinity block showed `skipped_below_min_prefix` for small prompts and `confirmed`/`pinnedProvider` for cached ones [V].
- **Adaptive thinking adds noise.** Haiku 5.5 has adaptive thinking on by default (effort `medium` per [S]). The same cached request used 0 or 54 thinking tokens on different calls [V], which makes output-token budgets noisy. Set `output_config.effort: "low"` for routine calls (accepted [V] `structured-messages-haiku`).
- **`caching: "auto"` behaves differently.** It also marks the last message (writes covered 5167 vs 5145 tokens), so each request pays a small write premium on its unique tail [V/D]. For Planroom's "stable prefix + varying question" shape, an explicit marker at the end of the stable prefix is cheaper and deterministic.

## 4. Structured output and tool use [V]

All four mechanisms returned schema-valid JSON on Haiku 5.5. The schema was an object with `group`, `members[]` and `confidence`, with `additionalProperties:false` and everything required.

| Mechanism | Request | Result | Fixture |
|---|---|---|---|
| messages structured output | `output_config: {effort:"low", format:{type:"json_schema", schema}}` | `stop_reason:end_turn`, one `text` block containing the JSON | `structured-messages-haiku` |
| chat/completions structured output | `response_format: {type:"json_schema", json_schema:{name, strict:true, schema}}` | `message.content` is the JSON string | `structured-chatcompletions-haiku` |
| messages tool use | `tools:[{name, description, strict:true, input_schema}]`, `tool_choice:{type:"auto"}` | `stop_reason:tool_use`, `tool_use.input` is an object | `tool-messages-haiku` |
| chat/completions tool use | `tools:[{type:"function", function:{name, strict:true, parameters}}]`, `tool_choice:"auto"` | `finish_reason:tool_calls`, `function.arguments` is a JSON **string** | `tool-chatcompletions-haiku` |

Forced `tool_choice:{type:"tool"}` worked on Haiku 5.5 [V] `tool-forced-messages-haiku`. See §1 for the Sonnet 5.5 case. **Recommendation:** use `output_config.format` for single-decision calls such as grouping or labelling. It is GA, needs no beta header [D], and gives one `text` block to parse. Use `strict` tools only when Claude must choose between actions. In both cases, check `stop_reason` before parsing (`max_tokens` or `refusal` mean the JSON may be incomplete or missing).

## 5. Latency and reported cost

**Latency [V]** is single-sample wall time from this container (iad1 edge per `X-Vercel-Id`), non-streaming, so read it as order-of-magnitude only:

- Haiku 5.5, short prompt: 0.65 to 1.3 s. 5k-token cache write: 0.9 to 2.1 s. Cache read: 0.65 to 2.3 s. Chat-completions ran slower than messages in this sample (1.8 to 2.3 s vs 0.7 to 0.8 s for the same reads), but n=3.
- Sonnet 5.5: short prompt 0.88 s, 5k cache write **9.7 s**, cache read 1.17 s. Opus 5.5 short: 1.31 s.
- Streaming Haiku: TTFB 0.74 s, total 0.77 s.
- Gateway validation errors (400 before routing) take about 0.1 s. Upstream errors with the default four-provider fallthrough take about 1.1 s. With `only:["anthropic"]` they take 0.48 s.

**Cost [V]:** no response **header** carries cost. CORS-exposed headers are `retry-after`, `x-ai-gateway-token-count` and `x-should-retry`, and the last two were absent from these responses. Cost is in the **body**:

- messages: `provider_metadata.gateway.{cost, marketCost, gatewayCost, inferenceCost, inputInferenceCost, outputInferenceCost, surchargeCost}`, as decimal **strings** (e.g. `"0.0000038"`).
- chat/completions: `usage.{cost, market_cost, gateway_cost}` as **numbers**, plus `is_byok`. The same `provider_metadata` is nested under `choices[0].message.provider_metadata`.
- streaming messages: cost and the full usage arrive only on the `message_delta` event, inside `provider_metadata`. `message_start.usage` is all zeros [V]. This differs from Anthropic, which reports input tokens in `message_start` [A].
- Every response carries a `generationId` (`gen_…`, also used as the message `id`). Record it so a run's spend can be reconciled later [D: generation lookup API not checked].

## 6. Error shapes [V]

There are **three distinct error envelopes**. The adapter must parse all of them.

1. **Gateway request validation**, HTTP 400, before routing, about 0.1 s. Same as Anthropic's envelope:
   `{"type":"error","error":{"type":"invalid_request_error","message":"max_tokens: Invalid input: expected number, received undefined"}}` (`error-messages-missing-max-tokens`). Also seen: empty `messages` (`"messages: Too small: expected array to have >=1 items"`) and malformed JSON (`"Invalid JSON in request body"`).
2. **Unknown model**, HTTP **404**. No top-level `type`, and `param` is an object, the same on both endpoints:
   `{"error":{"message":"Model 'anthropic/claude-nonexistent-9' not found","type":"model_not_found","param":{"modelId":"anthropic/claude-nonexistent-9"}}}` (`error-messages-unknown-model`, `error-chatcompletions-unknown-model`).
3. **Upstream provider rejection**, HTTP 400, after fallthrough across all providers. There is no top-level `type`. `error.type` is `"AI_APICallError"` and `error.param` has `statusCode`/`isRetryable`. The envelope also carries a top-level **`providerMetadata`** (camelCase, unlike the success `provider_metadata`) with per-attempt errors:
   `{"error":{"message":"tools.0.custom.name: String should match pattern '^[a-zA-Z0-9_-]{1,128}$'","type":"AI_APICallError","param":{"statusCode":400,"isRetryable":false,…}},"providerMetadata":{"gateway":{"routing":{…"providerAttemptCount":4…}}}}` (`error-messages-bad-tool-name`, `error-messages-bad-image`).
   A deterministic 400 is retried on bedrock, vertexAnthropic and claudeaws: 4 attempts, about 1.1 s. With `only:["anthropic"]` it is 1 attempt, 0.48 s (`error-messages-bad-tool-name-only-anthropic`).

Not tested: 401/403 (the proxy injects credentials), 429, 5xx, context overflow, `refusal`. I skipped context overflow on purpose: a 200k+ token prompt that was accepted by mistake would have cost $0.20 or more. The docs list 400/401/403/404/429/500 [D] (anthropic-messages-api page). Treat `retry-after`/`x-should-retry` as the retry hints when present [D, from the CORS expose list].

---

## 7. Recommended Rust adapter design

**Endpoint: `POST {base}/v1/messages`** (Anthropic-compatible), not chat-completions. Reasons:

- Messages exposes all four cache counters natively (`input`, `creation`, `read`, and `cache_creation.{5m,1h}`). On chat-completions the write count is a gateway extension field.
- Block-level `cache_control` and `output_config.format` are first-class there.
- Thinking tokens are reported separately.
- The same body shape works on the direct Anthropic API, so a future `--direct` mode or BYOK can share types.

Use chat-completions only if a non-Claude model has to share the client.

Toolchain [V]: a minimal client built and ran against the gateway through this container's proxy. Cargo 1.97.0, edition 2024, `reqwest =0.13.5` (`default-features=false, features=["json","rustls"]`), `serde =1.0.229`, `serde_json =1.0.151`, `tokio =1.53.2`. Result: `200 OK 1.24s "pong"`. Also suggested, not compiled here: `thiserror =2.0.21`, `sha2 =0.11.0` (versions from the crates.io API, ≥ 2 weeks old).

```toml
# planroom.toml (engine config). The key never lives here, only the env var name.
[claude]
base_url      = "https://ai-gateway.vercel.sh"
api_key_env   = "AI_GATEWAY_API_KEY"        # also accept VERCEL_OIDC_TOKEN; none => proxy/OIDC mode
model_capable = "anthropic/claude-sonnet-5.5"
model_cheap   = "anthropic/claude-haiku-5.5"
providers     = ["anthropic"]               # -> providerOptions.gateway.only
effort        = "low"                       # default output_config.effort; per-call override
cache_ttl     = "5m"
max_spend_usd = 3.00                        # hard stop, summed from gateway cost
```

Shape:

```rust
pub trait Transport: Send + Sync {            // seam for live vs replay
    async fn post_json(&self, path: &str, body: &serde_json::Value) -> Result<RawResponse, TransportError>;
}
pub struct ReqwestTransport { client: reqwest::Client, base: Url, auth: Option<SecretString> }
pub struct ReplayTransport  { dir: PathBuf, mode: Replay /* Strict | RecordMissing */ }

pub struct ClaudeRequest {
    pub tier: Tier,                       // Capable | Cheap -> model id from config
    pub stable_prefix: Vec<SystemBlock>,  // byte-stable; last block gets cache_control
    pub question: String,                 // volatile tail, after the breakpoint
    pub output: Output,                   // Text | JsonSchema(Value) | Tools(Vec<Tool>)
    pub max_tokens: u32,
}
pub struct Usage {                        // normalised from either endpoint
    pub uncached_input: u64, pub cache_write: u64, pub cache_read: u64,
    pub cache_write_1h: u64, pub output: u64, pub thinking: u64,
    pub cost_usd: Option<Decimal>,        // parse the gateway's string; keep exact
    pub generation_id: Option<String>, pub provider: Option<String>,
}
impl Usage {
    pub fn prompt_total(&self) -> u64 { self.uncached_input + self.cache_write + self.cache_read }
    pub fn cache_hit_rate(&self) -> f64 { self.cache_read as f64 / self.prompt_total().max(1) as f64 }
}
```

Rules the adapter enforces:

1. **Pin everything in the body.** Send the model from config, `providerOptions.gateway.only = providers`, `output_config.effort`, and `x-session-affinity: <workspace-id>:<prefix-hash>`. Send no `temperature`/`top_p`/`top_k`, no `budget_tokens`, no forced `tool_choice`, and at most 4 breakpoints. The gateway would accept these (§1), but the direct API would not.
2. **One explicit breakpoint** at the end of `stable_prefix`, with `ttl` from config. Do not use `caching:"auto"`. Hash the serialized prefix bytes (sha2) and log the hash with every call. When the hash is unchanged and `cache_write > 0` on a call after the first within the TTL, emit a `cache_regression` event.
3. **Auth.** If `api_key_env` resolves, send `Authorization: Bearer`. Otherwise send none (OIDC or this container's proxy). Never log headers.
4. **Errors.** Use one enum: `Validation{message}` (envelope 1), `ModelNotFound{model}` (404, envelope 2), `Upstream{status, retryable, attempts}` (envelope 3, with `isRetryable` from `error.param`), `RateLimited{retry_after}`, `Server`, and `Transport`. Retry only on 429, 5xx, connection errors, or `isRetryable:true`, with jittered backoff and a cap of 2. Before parsing output, check `stop_reason` for `max_tokens` and `refusal`.
5. **Spend guard.** Add `cost_usd` to a per-run and per-workspace ledger in SQLite. Refuse calls once `max_spend_usd` is reached. Live tests are gated behind `PLANROOM_LIVE_GATEWAY=1`.
6. **Accounting for budgets (S7).** Persist `Usage` per call with `(question_id, prefix_hash, model, generation_id)`. The budget test sums `cache_read / prompt_total` per question and fails on a growth of more than 20% in prompt or output tokens, or on a drop in hit rate.

**Replay fixtures for deterministic CI:**

- The fixture format is the one used in `claude-fixtures/`: `{name, request:{method,url,headers,body}, response:{status,headers,body|sse}, measured}`. In CI, key each fixture by `sha256(canonical_json(request.body))` plus the path. Use canonical JSON (sorted keys) for the key only. Send the original bytes so the prefix stays byte-stable.
- `ReplayTransport::Strict` fails a test on any request with no fixture, which also proves "0 Claude calls on the routine path" (S7). `RecordMissing` is allowed only when `PLANROOM_LIVE_GATEWAY=1` and writes new fixtures with auth headers stripped. CI runs a secret-scan step over the fixture dir (the same grep as above).
- Contract tests parse **every** fixture here through the adapter's response and error decoders. That covers the success shapes, the SSE stream, all three error envelopes and the lenient cases. They also assert `Usage` and `cache_hit_rate` for the 3× experiment: 0.0, then 5145/5171 ≈ 0.995, twice.

## Top risks

1. **The gateway rewrites requests silently (§1).** Invalid parameters are accepted or dropped with no warning, so gateway-only tests can hide bugs that the direct API or a future gateway release would reject. Mitigation: build only shapes valid on both APIs, and run a small contract test against the docs-derived Anthropic shapes.
2. **Cache misses from routing and thresholds.** The default four-provider fallthrough and session-affinity behavior can move a request off the provider that holds its cache. The minimum cacheable prefix is undocumented and conflicting (512 per [S] vs 1024 per [D]). Mitigation: `only:["anthropic"]`, an affinity header, a stable prefix ≥ 1024 tokens, and the prefix-hash regression alarm.
3. **Gateway-specific response and error shapes.** Cost appears as strings on messages and as numbers on chat-completions. There are three error envelopes, with `providerMetadata` and `provider_metadata` spelled differently, and streaming usage arrives only on `message_delta`. These are undocumented or partly documented and may change. Mitigation: tolerant decoders, with every fixture in this directory as a regression test.
