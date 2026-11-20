# Forge Assistant benchmark results

Generated 2026-11-20 15:18:44. Endpoint: OpenRouter (provider-hosted, so latency is network + provider, **not** local-GPU latency). temperature=0.2, max_tokens=1500, reasoning disabled where supported.

**Valid** = JSON parses, matches `widget.schema.json`, and passes the WidgetForge rules (permissions match the fields used, bindings point at declared data / real fields / known filters, storage for notes/checklist, drawer has an edge). **Semantic** = share of per-task checks passed (does the widget do what was asked, least privilege). **Full pass** = valid and every semantic check passed.

## Overall (all non-repair runs)

| Model | Params | Runs | Mode | Parse | Schema ok | Valid (schema+rules) | Semantic | Full pass | Avg latency | Avg out tok | Cost | VRAM Q4_K_M (est.) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `google/gemini-3.1-flash-lite` | cloud | 10 | json_object | 100% | 100% | 100% | 100% | 100% | 1.5s | 372 | $0.0147 | cloud |
| `mistralai/ministral-8b-2512` | 8.4B | 20 | json_schema | 100% | 100% | 75% | 96% | 70% | 3.0s | 358 | $0.0032 | ~5.1 + 1 = ~6.1 GB (borderline) |
| `qwen/qwen3.5-9b` | 9.0B | 20 | json_schema | 100% | 90% | 80% | 91% | 70% | 3.5s | 267 | $0.0047 | ~5.6 + 1 = ~6.6 GB (over 6 GB; partial offload) |
| `ibm-granite/granite-4.2-8b` | 8.0B | 20 | json_schema | 100% | 100% | 75% | 86% | 45% | 2.9s | 295 | $0.0014 | ~4.9 + 1 = ~5.9 GB (borderline) |
| `mistralai/ministral-3b-2512` | 3.4B | 10 | json_schema | 100% | 50% | 30% | 74% | 30% | 1.8s | 330 | $0.0012 | ~2.1 + 1 = ~3.1 GB |
| `meta-llama/llama-3.2-3b-instruct` | 3.2B | 20 | json_schema | 100% | 100% | 45% | 58% | 15% | 5.4s | 256 | $0.0029 | ~2.0 + 1 = ~3.0 GB |
| `google/gemma-3-4b-it` | 3.9B | 10 | json_schema | 100% | 100% | 40% | 57% | 10% | 9.2s | 260 | $0.0007 | ~2.5 + 1 = ~3.5 GB |
| `liquid/lfm-3-1.7b` | - | 10 | json_schema | all calls failed | | | | | | | | |

## Per task

Each cell is one run: `V`/`x` = valid / invalid, then the semantic score (0-100).

| Task | `gemma-3-4b-it` | `ministral-3b-2512` | `llama-3.2-3b-instruct` | `lfm-3-1.7b` | `qwen3.5-9b` | `ministral-8b-2512` | `granite-4.2-8b` | `gemini-3.1-flash-lite` |
|---|---|---|---|---|---|---|---|---|
| cpu_ram_gauges | V80 | x80 | x40 x60 | x0 | V100 V100 | V100 V100 | V100 V100 | V100 |
| clock_date | x60 | x60 | V80 V80 | x0 | x80 V100 | x100 x100 | V80 V80 | V100 |
| sticky_note | V80 | x80 | V80 V80 | x0 | V100 V100 | V100 V100 | V80 V60 | V100 |
| todo_checklist | x75 | x100 | V100 V100 | x0 | V100 x75 | V100 V100 | x100 x100 | V100 |
| right_drawer | V100 | V100 | V100 x100 | x0 | V57 x71 | x71 x71 | V100 V100 | V100 |
| uptime_hostname | x0 | V100 | x0 x0 | x0 | V100 V100 | V80 V100 | x80 x80 | V100 |
| network_graph | x25 | x25 | x25 x25 | x0 | V100 V100 | V100 V100 | V100 x25 | V100 |
| disk_list | x0 | x0 | x0 x0 | x0 | x50 V100 | V100 V100 | V100 V100 | V100 |
| quick_launch | x100 | x100 | V80 V80 | x0 | V100 V100 | V100 V100 | V100 V100 | V100 |
| edit_red_ram | V50 | V100 | x67 x67 | x0 | V100 V83 | x100 V100 | V67 V67 | V100 |

## Self-repair loop

On an invalid manifest the validation errors (max 12) are sent back once as a follow-up user turn; the corrected manifest is validated again.

| Model | Task runs | Needed repair | Valid before | Valid after | Full pass before | Full pass after | Extra cost | Avg extra latency |
|---|---|---|---|---|---|---|---|---|
| `ibm-granite/granite-4.2-8b` | 20 | 5 | 15/20 | 20/20 | 9/20 | 13/20 | $0.00037 | 3.1s |
| `meta-llama/llama-3.2-3b-instruct` | 20 | 14 | 6/20 | 10/20 | 2/20 | 3/20 | $0.00285 | 6.6s |
| `mistralai/ministral-8b-2512` | 20 | 8 | 12/20 | 16/20 | 11/20 | 15/20 | $0.00129 | 3.3s |

- `granite-4.2-8b` `todo_checklist` run 1: 1 error(s) fed back -> valid (semantic 100%). First error: checklist block.visible: 'true' in {{ true }} is not a declared data name (declared: none)
- `granite-4.2-8b` `uptime_hostname` run 1: 1 error(s) fed back -> valid (semantic 80%). First error: text block.text: unknown filter 'seconds' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, uppe
- `granite-4.2-8b` `network_graph` run 1: 4 error(s) fed back -> valid (semantic 100%). First error: graph block.value: system field 'rx' does not exist
- `granite-4.2-8b` `todo_checklist` run 2: 1 error(s) fed back -> valid (semantic 100%). First error: checklist block.visible: 'true' in {{ true }} is not a declared data name (declared: none)
- `granite-4.2-8b` `uptime_hostname` run 2: 1 error(s) fed back -> valid (semantic 100%). First error: text block.text: unknown filter 'seconds' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, uppe
- `llama-3.2-3b-instruct` `cpu_ram_gauges` run 1: 2 error(s) fed back -> still invalid (semantic 40%). First error: gauge block.value: system field 'usage' does not exist
- `llama-3.2-3b-instruct` `uptime_hostname` run 1: 3 error(s) fed back -> still invalid (semantic 0%). First error: text block.text: unknown filter 'bold' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, upper)
- `llama-3.2-3b-instruct` `network_graph` run 1: 9 error(s) fed back -> still invalid (semantic 0%). First error: graph block.value: system field 'download' does not exist
- `llama-3.2-3b-instruct` `disk_list` run 1: 5 error(s) fed back -> still invalid (semantic 0%). First error: list block.item: 'disk' in {{ disk.name }} is not a declared data name (declared: sys)
- `llama-3.2-3b-instruct` `edit_red_ram` run 1: 4 error(s) fed back -> valid (semantic 83%). First error: bar block.value: 'memory.used' needs "memory" in permissions.system
- `llama-3.2-3b-instruct` `cpu_ram_gauges` run 2: 2 error(s) fed back -> still invalid (semantic 40%). First error: gauge block.value: system field 'usage' does not exist
- `llama-3.2-3b-instruct` `clock_date` run 2: 1 error(s) fed back -> valid (semantic 40%). First error: text block.text: system field 'os.date' does not exist
- `llama-3.2-3b-instruct` `todo_checklist` run 2: 3 error(s) fed back -> still invalid (semantic 100%). First error: checklist block.visible: settings.todo.enabled is not defined in settings
- `llama-3.2-3b-instruct` `right_drawer` run 2: 1 error(s) fed back -> valid (semantic 100%). First error: bar block.max: system field 'cpu.max' does not exist
- `llama-3.2-3b-instruct` `uptime_hostname` run 2: 3 error(s) fed back -> still invalid (semantic 0%). First error: text block.text: unknown filter 'bold' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, upper)
- `llama-3.2-3b-instruct` `network_graph` run 2: 7 error(s) fed back -> still invalid (semantic 0%). First error: graph block.value: system field 'rx' does not exist
- `llama-3.2-3b-instruct` `disk_list` run 2: 5 error(s) fed back -> still invalid (semantic 0%). First error: list block.items: 'disk' in {{ disk.name }} is not a declared data name (declared: sys)
- `llama-3.2-3b-instruct` `quick_launch` run 2: 4 error(s) fed back -> valid (semantic 80%). First error: button block.visible: settings.github is not defined in settings
- `llama-3.2-3b-instruct` `edit_red_ram` run 2: 4 error(s) fed back -> still invalid (semantic 67%). First error: bar block.value: system field 'ram.used' does not exist
- `ministral-8b-2512` `clock_date` run 1: 2 error(s) fed back -> valid (semantic 100%). First error: clock block.visible: 'true' in {{ true }} is not a declared data name (declared: clock)
- `ministral-8b-2512` `right_drawer` run 1: 3 error(s) fed back -> still invalid (semantic 71%). First error: bar block.value: bad binding {{ sys.disks[0].used }}
- `ministral-8b-2512` `edit_red_ram` run 1: 2 error(s) fed back -> valid (semantic 100%). First error: text block.visible: settings.show_ram is not defined in settings
- `ministral-8b-2512` `clock_date` run 2: 2 error(s) fed back -> valid (semantic 100%). First error: clock block.visible: 'true' in {{ true }} is not a declared data name (declared: clock)
- `ministral-8b-2512` `right_drawer` run 2: 3 error(s) fed back -> still invalid (semantic 71%). First error: bar block.value: bad binding {{ sys.disks[0].used }}
- `ministral-8b-2512` `uptime_hostname` run 2: 1 error(s) fed back -> still invalid (semantic 60%). First error: $.ui.blocks[0].children[1]: unknown key 'label'
- `ministral-8b-2512` `network_graph` run 2: 4 error(s) fed back -> still invalid (semantic 100%). First error: $.ui.blocks[0].children[1].children[0].type: unknown or not-allowed block type 'column' here (allowed: bar, button, chec
- `ministral-8b-2512` `edit_red_ram` run 2: 1 error(s) fed back -> valid (semantic 100%). First error: text block.visible: 'true' in {{ true }} is not a declared data name (declared: sys)

## Common validation errors

- `google/gemma-3-4b-it`: a system data source is declared but permissions.system is empty (x2); text block.text: 'today' in {{ today }} is not a declared data name (declared: clock) (x1); list block.item: unknown item field 'item.' (x1); list.items must bind an array field (disks or cpu.cores), got 'item.text' (x1)
- `mistralai/ministral-3b-2512`: listBlock block.item: unknown filter 'map' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, upper) (x3); listBlock block.item: item.* is only valid inside list.item (x3); button block.visible: 'false' in {{ false }} is not a declared data name (declared: none) (x3); gauge block.format: 'value' in {{ value }} is not a declared data name (declared: sys) (x2)
- `meta-llama/llama-3.2-3b-instruct`: gauge block.value: system field 'usage' does not exist (x2); gauge block.format: system field 'usage' does not exist (x2); text block.text: unknown filter 'bold' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, upper) (x2); text block.text: system field 'hostname' does not exist (x2)
- `liquid/lfm-3-1.7b`: HTTP 200: {"message": "empty content (finish=length)"} (x10)
- `qwen/qwen3.5-9b`: $.ui.blocks[]: does not match any allowed form ($.ui.blocks[]: does not match any allowed form ($.ui.blocks[]: expected object, got str)) (x2); $.permissions.system[]: "clock" not one of ['cpu', 'memory', 'disk', 'network', 'os'] (x1); list block.item: unknown item field 'item.' (x1); list block.visible: bad binding {{ sys.disks | length > 0 }} (x1)
- `mistralai/ministral-8b-2512`: clock block.visible: 'true' in {{ true }} is not a declared data name (declared: clock) (x2); text block.visible: 'true' in {{ true }} is not a declared data name (declared: clock) (x2); bar block.value: bad binding {{ sys.disks[].used }} (x2); bar block.max: bad binding {{ sys.disks[].total }} (x2)
- `ibm-granite/granite-4.2-8b`: text block.text: unknown filter 'seconds' (allowed: bytes, date, default, fixed, lower, percent, rate, round, time, upper) (x2); checklist block.visible: unbalanced braces in "]]}}]}}Request: A to-do list titled 'Today' where I can tick items off.{" (x1); notes/checklist keys must be unique (x1); checklist block.visible: unbalanced braces in "]]}}]}}  # Note: The visible condition for checklist would normally reference a setting, but since no t (x1)

## Spend

OpenRouter key `limit_remaining`: start 42.938309394 -> end 42.877640019 (sum of per-call cost in results: $0.0428).

## Models: VRAM and licence

| Model | Params | VRAM at Q4_K_M (weights + ~1 GB ctx) | Licence |
|---|---|---|---|
| `google/gemma-3-4b-it` | 3.9B | ~2.5 + 1 = ~3.5 GB | Gemma Terms of Use (custom, redistribution allowed with use policy) - verify |
| `mistralai/ministral-3b-2512` | 3.4B | ~2.1 + 1 = ~3.1 GB | Apache 2.0 - verify |
| `meta-llama/llama-3.2-3b-instruct` | 3.2B | ~2.0 + 1 = ~3.0 GB | Llama 3.2 Community Licence (attribution, AUP, 700M MAU) - verify |
| `liquid/lfm-3-1.7b` | 1.7B | ~1.1 + 1 = ~2.1 GB | LFM Open Licence (revenue cap for commercial use) - verify |
| `qwen/qwen3.5-9b` | 9.0B | ~5.6 + 1 = ~6.6 GB (over 6 GB; partial offload) | Apache 2.0 - verify |
| `mistralai/ministral-8b-2512` | 8.4B | ~5.1 + 1 = ~6.1 GB (borderline) | Apache 2.0 - verify |
| `ibm-granite/granite-4.2-8b` | 8.0B | ~4.9 + 1 = ~5.9 GB (borderline) | Apache 2.0 - verify |
| `google/gemini-3.1-flash-lite` | cloud | cloud | proprietary API |

<!-- notes: hand-written below this line, kept when the report is regenerated -->
## Notes

- **Total spend for this whole exercise:** /key `limit_remaining` went from 42.9749 to 42.8728, about **$0.10**. That includes smoke tests, the prompt-v1 run, debugging calls and retries; per-call costs in `results.json` add up to about $0.05. The Spend section above covers only the final `results.json`.
- **Prompt and schema revisions.** The first full run used prompt v1, which listed fields
  under group headings ("os: uptime, ..."). Four models then wrote `sys.os.uptime`, and
  several added `"visible": "{{ true }}"` to blocks. v1 also had a URL pattern
  `^https?://`, which grammar engines match against the whole string, so Qwen produced
  `"open": "https://"` and nothing else. Prompt v2 (the shipped `system_prompt.md`)
  spells out the full paths and says to leave `visible` out. With v2 and the fixed
  schema, valid rates rose: ministral-8b 0% -> 75% (v1 echoed the schema's `$id`, which
  is now stripped from the request), granite 70% -> 75%, gemini 90% -> 100%.
  The tables above are v2 only.
- **Errors that are left are mostly binding errors, not structure errors.** Every
  local model that the provider actually constrained got 100% on the schema. Invalid
  manifests come from bindings: invented fields (`sys.rx`, `sys.usage`, `ram.used`,
  `sys.disks[0].used`), invented filters (`bold`, `seconds`, `map`), and literal
  `{{ true }}` in `visible`. Grammar mode cannot catch these, so they need the lint
  step plus the repair loop.
- **The 2–4B models are not good enough as the default.** gemma-3-4b, llama-3.2-3b and
  ministral-3b reached 30–45% valid and 10–30% full pass. They fail mainly on `list`
  bindings, network field names and permissions. Repair barely helps at this size
  (llama-3.2-3b went from 6/20 to 10/20 valid and from 2/20 to 3/20 full pass). One
  exception: ministral-3b's endpoint did not enforce the schema (5 of its 7 failures
  were schema errors such as `columnOuter` or `listBlock` types), so under a local
  llama.cpp grammar it could do noticeably better. It is the one small model worth
  re-testing locally.
- **liquid/lfm-3-1.7b could not be scored.** Its OpenRouter endpoint says reasoning is
  mandatory, and it spent all 1500 tokens thinking before emitting any JSON, so 0/10.
  It needs a local test with thinking disabled or a grammar forced from the first
  token.
- **qwen3.5-9b** had the most valid manifests among local models (80%) and does well
  on edits. It twice ran away to 1500 tokens (degenerate output), so cap `max_tokens`
  at about 1000. Thinking must be off (`enable_thinking=false` in the chat template).
- **granite-4.2-8b** responds best to repair: 5 of 5 invalid manifests fixed, 75% ->
  100% valid. It is also the fastest and cheapest. Its weak point is semantics: it
  ignored "make it red" in all 4 edit runs, kept the blue accent, and picked odd
  categories (clock as `info`, sticky note as `fun`). One sticky note came out red.
- **ministral-8b-2512** has the best semantics of the local models (96%, full pass 70%,
  semantically correct in all 4 edit runs). Its rule errors were `{{ true }}` in `visible` and
  `sys.disks[0].used`. In the repair runs it failed only on errors that a local
  grammar would make impossible (an unknown `label` key, and 3-level nesting) and on
  the `disks[0]` index.
- **Latency** in these tables is from cloud providers. For a rough local estimate:
  an 8B model at Q4_K_M on a 6 GB laptop GPU generates about 35–50 tok/s, so a manifest
  of about 300 tokens takes about 6–10 s, plus about 1 s of prompt processing and 2–5 s
  to load the model from disk.

## Recommendation

| | Model | Why | VRAM at Q4_K_M (weights + ~1 GB ctx) | Licence |
|---|---|---|---|---|
| **Default (6 GB GPU)** | `mistralai/ministral-8b-2512` | Best match with the request among local models (96% semantic, 70% full pass, correct edits). 75% valid in the main runs; in the repair runs, 60% -> 80% valid with one repair. Most of what was left would be removed by grammar decoding. | ~5.1 GB + ~1 GB ≈ 6.1 GB. Use 4k ctx and a q8_0 KV cache; on a 6 GB card that also drives the desktop, keep 1–2 layers on the CPU. | Apache 2.0 (verify) |
| **Runner-up** | `ibm-granite/granite-4.2-8b` | Smallest of the 8Bs, fastest, 100% valid after one repair. Weaker on following colour, edit and category instructions. | ~4.9 GB + ~1 GB ≈ 5.9 GB | Apache 2.0 (verify) |
| Quality option (≥ 8 GB GPU) | `qwen/qwen3.5-9b` | Most valid manifests among local models (80%), but too large for 6 GB at Q4_K_M. Needs IQ4_XS (~5 GB) or partial offload. | ~5.6 GB + ~1 GB ≈ 6.6 GB | Apache 2.0 (verify) |
| Cloud reference | `google/gemini-3.1-flash-lite` | 10/10 valid and full pass in `json_object` mode, ~$0.0015 per widget. A good "Cloud" provider default. | – | proprietary |

**Self-repair: yes, keep it on.** It costs one extra call, only when validation fails
(5 of 20 runs for granite, 8 of 20 for ministral-8b), adding about 3 s on the cloud
(about 5–10 s locally). It lifted valid output from 75% to 100% for granite and from
60% to 80% for ministral-8b. It fixes binding and rule errors reliably. It rarely
fixes semantic mistakes and does little for 3B models. Cheap product fixes would
remove the most common failure without spending a model call:

- Treat a literal `{{ true }}` or `{{ false }}` in `visible` as a constant, or drop
  `visible` from the grammar when a manifest has no bool settings.
- Accept `sys.os.uptime` as an alias of `sys.uptime`.

**Before shipping:**

1. Re-run the top three and ministral-3b locally through `llama-server` with grammar
   decoding (`BENCH_API=http://localhost:8080/v1`) to measure real VRAM, tokens/s, and
   validity under an enforced grammar.
2. Verify each licence from the model card. The licences above come from general
   knowledge.

**Model sizes:** Ministral 3 and Gemma 3 4B ship vision encoders, so use text-only
GGUFs, without the mmproj file, to get the VRAM figures above.
