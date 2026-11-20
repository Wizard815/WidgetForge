# agent-bench: choosing the Forge Assistant model

This benchmark picks the local model for Forge Assistant (DESIGN.md §11). It runs each
candidate model on the same widget requests, with the output constrained to the
manifest schema. It then scores whether each manifest is valid and whether the widget
does what was asked.

| File | What it is |
|---|---|
| `widget.schema.json` | JSON Schema (draft 2020-12) for the declarative manifest, in the JSON form of `widget.yaml`. It is sent as `response_format: {type: "json_schema", json_schema: {name, strict: false, schema}}`. Nesting is bounded with no recursion: `block` (top level) → `innerBlock` → `leafBlock`, so containers nest at most 2 levels. |
| `system_prompt.md` | The system prompt the assistant ships with: fields, block reference, binding paths and permissions, and one example. About 1.1k tokens. |
| `bench.py` | The runner. Python 3 standard library only (`urllib`), no pip installs. |
| `results.json` | Every call: the raw output, mode, provider, tokens, cost, latency, errors and check results. |
| `RESULTS.md` | The generated tables, followed by hand-written notes and the recommendation. Text below the `<!-- notes -->` marker is kept when the report is regenerated. |

## Running it

```sh
cd tools/agent-bench
export OPENROUTER_API_KEY=sk-or-...   # optional when a proxy injects auth; defaults to a dummy token
python3 bench.py --fresh                                  # every model, every task, 1 run
python3 bench.py --models qwen/qwen3.5-9b --runs 1        # add another run (appends to results.json)
python3 bench.py --repair --runs 2 --models ibm-granite/granite-4.2-8b   # self-repair experiment
python3 bench.py --retry-failed                           # re-run calls that hit 429/5xx/timeouts
python3 bench.py --report-only                            # rebuild RESULTS.md from results.json
```

Other options: `--tasks a,b`, `--workers N` (parallel calls for each model),
`--floor X` (budget guard), `--tag`. `BENCH_API` points the runner at another
OpenAI-compatible endpoint, such as `http://localhost:8080/v1` for `llama-server`.
Model ids then have to match what that server expects.

**Budget guard:** before each model batch, the runner reads `GET /api/v1/key`. If
`limit_remaining` is below `--floor` (default `41.10`, or the `BENCH_BUDGET_FLOOR`
environment variable), it stops. Each call uses `max_tokens` 1500. A full run of all 8
models costs about $0.03. Most of that is the cloud reference model.

## What it measures

For each model and task:

1. **Mode.** The runner tries `json_schema` first, with `provider.require_parameters`.
   If the provider rejects it, the runner falls back to `json_object` and puts the schema
   in the prompt, then to a plain prompt. Each record stores the mode that was used.
   Reasoning is switched off where the API allows it, since the shipped assistant runs
   without thinking.
2. **Parse.** Strict `json.loads`. Fenced or embedded JSON is recovered and flagged as
   such.
3. **Schema.** A small built-in validator (`MiniValidator`) covers the subset of the
   schema that is used. It resolves `anyOf` by the block `type`, so the error names the
   real problem. These messages are also what the repair loop sends back.
4. **Rules (lint).** These are WidgetForge rules that the schema cannot express:
   - Every binding parses and uses known filters.
   - Each binding root is a declared data name, `settings.<defined key>`, or `item.`
     inside `list.item`.
   - System and clock fields exist.
   - `permissions.system` covers every field group in use.
   - `notes` and `checklist` blocks require storage.
   - `list.items` binds an array.
   - A drawer has an `edge`.
   - No duplicate permissions or storage keys.

   **Valid** means the manifest passes parse, schema and rules.
5. **Semantic checks.** Each task has 4 to 7 checks of the form "is this the widget
   that was asked for". For example: a gauge whose value binds `cpu.usage`; a drawer
   with `edge: right`; date text below the clock; the three exact URLs; in the edit
   task, the id is kept, the accent is red, and a RAM bar was added. Most tasks also
   check **least privilege**: the system permissions equal the groups actually used,
   network is `[]`, and storage is requested only for notes or checklists.
   **Semantic** is the share of these checks that pass. **Full pass** means valid and
   every check passes.
6. **Latency, tokens, cost.** Cost comes from the response's `usage.cost` when
   present. Otherwise it is computed from `/models` pricing. Latency is from
   OpenRouter, so it reflects network and provider speed, not a local GPU.
7. **Repair** (with `--repair`). If the first manifest is invalid, its errors (up to
   12) go back once as a follow-up turn, and the new answer is scored again.

### Tasks

`cpu_ram_gauges`, `clock_date`, `sticky_note`, `todo_checklist`, `right_drawer`,
`uptime_hostname`, `network_graph`, `disk_list`, `quick_launch`, and `edit_red_ram`.
The last one gives the model an existing blue CPU-gauge manifest and asks it to "make
it red and add a RAM bar". The prompts and checks are in `TASKS` in `bench.py`.

### Caveats

- OpenRouter providers differ in whether they actually enforce `json_schema`. Mistral
  endpoints sometimes produced keys or nesting that the schema forbids, and Together
  (Qwen) once produced a runaway string. Locally, llama.cpp turns the schema into a
  grammar, so schema errors cannot occur there. Only rule and semantic errors remain.
- Grammar engines (llama.cpp, XGrammar on DeepInfra and Parasail) reject some
  keywords. `uniqueItems` caused a hard 400, and `propertyNames` was dropped for the
  same reason. Both checks moved into the lint step. Patterns are matched against the
  whole string, so a URL pattern has to be `^https?://[^ ]+$`, not `^https?://`.
- Gemini's constrained decoding rejects this schema ("too many states"), so the
  reference model ran in `json_object` mode.
- Runs are small (1–2 per task), so read the scores as rough rankings, not exact rates.
