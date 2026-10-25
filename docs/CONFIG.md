# Configuration

Everything a mission page shows and every button on it comes from config. Config is TOML, in layers.

## Layers

Lowest to highest. A higher layer patches the result of the lower ones.

| # | Layer | Where | Who writes it |
|---|---|---|---|
| 1 | `builtin` | `defaults/` in this repo, compiled into the binary | releases |
| 2 | `template` | the mission's template, resolved by name across builtin/user/project (see below) | user, agent |
| 3 | `user` | `$MISSION_CONTROL_CONFIG_DIR` or `~/.config/mission-control/config.toml` | user, agent on request |
| 4 | `project` | `<repo>/.mission-control/config.toml` (committed; shared by the team) | user, agent on request |
| 5 | `mission-control` | stored in the daemon per mission, versioned | agent, page **Customize** |

Two passes: layers 1, 3, 4, 5 are merged first to read `mission.template`; then the template is resolved and the
full stack 1–5 is merged.

**Templates** live in `templates/<name>.toml` in each of builtin, user and project. Files with the same name merge
(builtin < user < project), so a user can patch a built-in template without copying it; `replace = true` in a file
drops the lower ones. A template may `extends = "<other>"`.

`mission-control config explain [--mission ID] [path]` prints each resolved key with the layer (and file) that set it.

## Changing config

Every layer above the template can be changed by command (`mission-control config patch --layer …`, MCP `config_patch`), and every change is validated first, undoable and listed in `config history`:

| Layer | Stored | Undo |
|---|---|---|
| `mission-control` | a new version in the daemon | drops the newest version |
| `user`, `project` | the file, edited in place: comments, order and formatting kept; backed up once to `<file>.mission-control.bak` | restores the previous content (kept in the state dir, not in the repo) |

- **Patch format:** the merge rules below. In a file, `null` deletes a key; `"!unset"` is written as is.
- **Dry run** (`--dry-run`): stores nothing; returns the resolved result, its issues and a `preview_url` that shows the page with the change (for an hour).
- **Templates from a mission:** `config template-save <name>` composes the mission's patches into `templates/<name>.toml` (user or project), with `extends = "<its template>"`. Start a mission from it with `-t <name>`.
- **Named widgets:** `config widget <name> --base <widget> --props '{…}'` (or `--html`) writes `[widgets.<name>]` on any layer.
- A file edited by hand is picked up on the next read (resolved configs are cached until a file or the mission layer changes).

## Merge rules

- Tables merge key by key.
- **Arrays of tables whose items have `id` merge by id**: a patch item with an existing id merges into it; a new id
  is appended, or placed with `before = "<id>"` / `after = "<id>"`; `remove = true` deletes it.
- Any other array is replaced as a whole, unless the patch is `{ "+" = [...], "-" = [...] }`: that adds and removes
  items instead (`toolbar = { actions = { "+" = ["rerun"] } }` adds one button without restating the list).
- A key set to the string `"!unset"` is removed.
- A section that still has no `widget` after the merge was a patch for a section this page doesn't have (say, a
  user default for the `prs` table, under a template with no PR table). It is dropped.

## Keys

```toml
schema = 1

[mission]
title = "Billing export build"
template = "multi-agent-build"
refresh = "live"            # live (server-sent events, diffs) | poll:<secs>
review = false              # load redline's overlay (comment anywhere); `mission-control review` sets it up

[vars]                      # free values for templates: {vars.repo}
repo = "acme/shop"
linear = "acme"

[links]                     # named link templates: {value}, {row.<field>}, {vars.<name>}
pr = "https://github.com/{vars.repo}/pull/{value}"
issue = "https://linear.app/{vars.linear}/issue/{value}"

[states]                    # state → color token: todo | prog | rev | block | done | warn
in_review = "rev"

[theme]                     # token overrides, e.g. accent = "#2f6fdb"; dark.* for dark mode

[collections.prs]           # optional schema: key field, field types, default sort
key = "number"
fields = { number = "int", title = "text", state = "state", checks = "text" }

[[pages]]                   # tabs after Overview (which always exists and comes first); merged by id
id = "prs"
title = "Pull requests"
count = { collection = "prs", state = "open" }   # optional number on the tab

[[sections]]                # top to bottom; each shows on its page
id = "prs"
title = "Pull requests"
page = "prs"                # a [[pages]] id; default "overview"
column = "main"             # main | rail: the two columns of a page (Overview: rail holds numbers, a chart, recent activity)
widget = "table"
collection = "prs"
columns = [
  { id = "number", label = "PR", link = "pr" },
  { id = "title" },
  { id = "state", as = "pill" },
]
actions = ["ask"]           # section buttons
row_actions = ["rerun-ci"]  # per-row buttons; the row is passed as payload

[widgets.burndown]          # a named widget: a built-in with preset props …
base = "chart"
props = { kind = "line", x = "at", y = "open" }

[widgets.risk-heat]         # … or a script widget (user/project layers only; mission layer → sandboxed iframe)
script = "widgets/risk-heat.js"

[strip]                     # the status strip under the top bar (full on Overview, health + needs pills elsewhere)
health = { collection = "run", key = "run" }   # its state and `reason` are the run's health (default; else vars.health)
needs = "needs"             # the collection of questions for the user
cells = [                   # between health and needs; merged by id
  { id = "tasks", label = "Tasks", count = { collection = "packages", state = ["merged", "done"] }, of = { collection = "packages" }, suffix = "done" },
  { id = "blocked", label = "Blocked", count = { collection = "packages", state = "blocked" }, tone = "block", hide_zero = true },
  { id = "spend", label = "Spend", sum = { collection = "spend", field = "usd" }, format = "usd" },
]

[ui]
shortcuts = true            # single-key shortcuts (/, ?, g then a page letter, n, j/k); ⌘K / Ctrl+K always opens the palette

[actions.rerun-ci]          # buttons
label = "Re-run CI"
kind = "ask_agent"          # ask_agent | customize | answer | set | open | copy | run
message = "Re-run CI on PR {row.number}"
confirm = true

[actions.mark-passed]
label = "Mark passed"
kind = "set"
state = "passed"

[actions.open-logs]
label = "Logs"
kind = "run"                # shell; user/project layers only, needs permissions.run_actions = true
command = ["gh", "run", "view", "--log-failed"]

[permissions]
run_actions = false

[retention]
events_keep = 2000          # newest events kept per mission; older ones are deleted
page_size = 50              # events per "Older" click in the feed
config_keep = 50            # mission-control config versions kept; older ones fold into one (undo stops there)

[brief]                     # what `brief` returns to the agent, within a character budget
max_chars = 4000
collections = ["needs", "prs"]   # templates set their key collections

[ingest]                    # Claude Code hooks (`mission-control hook`) that fill the mission without agent calls
subagents = true            # → collection "agents" (Task/Agent tool: running, then finished with its report)
prs = true                  # → collection "prs" (`gh pr create` → open, `gh pr merge N` → merged)
spend = true                # → collection "spend" (per session and model: tokens and an estimated `usd`)

[update]                    # release checks (read from the user's config.toml only)
check = true                # daily check and notices; `mission-control update off|on`
skip = []                   # releases not to notify about; `mission-control update skip`

[ingest.prices]             # USD per million tokens by model-id prefix, longest match wins (estimates)
claude-opus-5-5 = { input = 4.0, output = 20.0, cache_read = 0.2 }   # cache_read default 0.1×, cache_write 1.25× input
```

## Pages

A mission page is a set of **pages**, shown as tabs, each with its own URL (`/m/<id>/<page>`). Overview always exists and comes first; `[[pages]]` adds the rest in order (merged by `id`, so a layer can add, retitle, move or remove one). Sections stay one flat list and each names its page with `page` (default `overview`), so moving a section to another page is one key from any layer. A page shows as a tab only when a section is on it. A section naming a page that doesn't exist is a warning and shows on Overview. `config explain pages` names the layer of each page.

Every section may also take `drawer = { fields = ["agent", "branch", …] }` (the fields the record drawer lists for its records; default every plain field not shown elsewhere), `column = "main" | "rail"` (a page with rail sections lays out in two columns; the rail drops below on narrow screens) and `go = { page, label? }` (a link in its header to another page, like "All" on a short feed).

## Status strip

`[strip]` fills the strip under the top bar: **Health** first (the `health` record's state and `reason`), then `cells` in order, then **Needs you** (open records in `needs`, linking to the Overview's needs section; "Nothing waits on you" at 0). A cell is a label over a number: `count` or `sum` (or `value`), optionally `of` (shown as "3/8"), `suffix`, `format`, `tone` (colors the number: only for a count of that state) and `hide_zero`. `plan = "<section id>"` makes a phase cell from a `plan` section. On pages other than Overview, the strip shrinks to health and needs pills in the top bar.

## Built-in widgets

Every widget's props are typed: `mission-control schema` lists them under `x-widget-props`, and a section with a wrong prop type is a config error naming the section.

| Widget | Shows | Main props |
|---|---|---|
| `kpis` | Number tiles | `items = [{label, count = {collection, state?, field?, value?} \| {events = true}, sum = {collection, field (one or a list), state?}, value, of = {count spec} (shows "3 / 8" and a bar), format = "number" \| "usd" \| "tokens", note = "text" or a metric {count \| sum, format, prefix, suffix}}]` |
| `health` | Run health line | `record = {collection, key}` (its state and `reason`), else `vars.health`, `vars.health_reason` |
| `needs` | Questions waiting on the user: what each blocks, the recommendation (marked in ink), numbered options (keys 1–9), a text box; answered ones fold under a count | `collection` (default `needs`). Record fields: `question`, `context` (markdown), `options`, `recommendation`, `why`, `blocks` (a key, `collection/key`, or a list: linked to that record), `since` |
| `table` | Rows of a collection | `columns` (by id; `as` = `pill` \| `code` \| `muted` \| `time` \| `age` \| `link` \| `markdown`; `link` = a `[links]` name; `or` = a field shown when this one is empty), `sort` (`-field` descending, `tone` for loudest state first; default `key`), `filter = {field = value or [values]}`, `empty`, `row_actions` |
| `feed` | Event log, newest first | `kinds`, `limit` (default 50) |
| `markdown` | Text, rendered as safe markdown | `text`, or `record = {collection, key, field?}` |
| `kanban` | Cards in lanes by state | `collection`, `lanes` (states, in order; default by tone), `fields`, `card_title`, `row_actions` |
| `checklist` | Gates: state, "n of m met", requirements (✓ met, ✕ when the record a requirement points at is blocked, ○ open), note; the next gate marked, passed ones folded | `collection`, `requirements` (field: strings or `{text, done, ref}`; `ref` = a record key, linked to its drawer), `note` (field, markdown), `card_title`, `row_actions` |
| `plan` | The dependency graph: phases as columns, records as nodes, `depends_on` edges styled met / waiting / blocked upstream, gates between phases, the current phase, a legend; Graph / Table / Board views, Trace blocked path, zoom, fit, fullscreen, pan | `phases = {collection (a name or a list: the first with records), label?}` (else the distinct values of the nodes' phase field), `nodes = {collection, phase? (field or list, default ["phase", "wave"]), deps? (default "depends_on"), title?}`, `gates = {collection, after? (default "after"), requirements?}`, `columns` (Table view, as in `table`), `edges` (default true). A dependency cycle is reported in the issue banner and its edges aren't drawn |
| `graph` | A component diagram: groups as columns, components as boxes (a linked task shows its live state; a blocked one draws its edges blocked), labelled edges; zoom, fit, fullscreen, pan | `nodes = {collection, group? (default "group"), title?, task? (default "task": a record key whose state the box shows), note?}`, `edges = {collection, from?, to?, label?}`, `groups` (column order) |
| `doc` | Long-form markdown: headings, 68ch prose, ` ```mermaid ` blocks drawn as diagrams in the page's tokens (a block that doesn't parse shows its source and the error) | `record = {collection, key, field? (default "body")}`, or `text` |
| `track` | The Overview's small plan: phase blocks with a mix bar and counts, gates between | `plan` = the id of the `plan` section it reads |
| `flow` | v1 alias of `plan` that draws no edges | `lanes = {collection, label?}`, `nodes = {collection, lane}`, `gates = {collection, after?}` |
| `chart` | Line, bar or sparkline from records | `collection`, `kind` (`line` \| `bar` \| `sparkline`), `x` (default `key`; numbers and Unix-ms times plot to scale), `y` (a field, or several: one series each), `series` (a field that splits rows), `height` |
| `spend` | Total cost and tokens, cost per row | `collection` (default `spend`: `usd`, `input_tokens`, `output_tokens`), `field`, `label` |
| `html` | HTML in a sandboxed iframe (its scripts can't reach the page, its token or cookies) | `html`, or `record = {collection, key, field?}`; `height` |

**Named widgets** (`[widgets.<name>]`): `base` + `props` (a built-in with presets), `html` (sandboxed, any layer), or `script = "widgets/<name>.js"` (user/project config only). A script widget is a file in the user's or the project's `widgets/` folder that calls `MissionWidget.define("<name>", (section, ctx) => html)`; it runs in the page like a built-in, so it must escape what it shows (`ctx.h.esc`, `ctx.h.md`). The daemon serves it only for a `script` key that came from that file layer.

## Actions

| Kind | Keys | On click | Delivered to agent |
|---|---|---|---|
| `ask_agent` | `message` (template), `prompt` (bool), `placeholder` | Prompt box when `prompt = true` or there's no `message`, then send | `action` event: text = filled `message` (+ typed text); data `{action, label, row, message, text}` |
| `customize` | | Prompt box: "what should change?" | `customize` event with the request; the agent answers with `config_patch` |
| `set` | `state`, or `field` + `value` | Sets the row's state or one body field | `action` event (informational) |
| `open` | `url` (link template) | Opens the link in a new tab (`http(s)`/`mailto` only) | — |
| `copy` | `text` (template) | Copies the filled text to the clipboard | — |
| `run` | `command` (argv), `cwd`, `env`, `timeout_s` (default 60, max 600) | Always asks first, then runs the argv (no shell) on this machine, in the mission's project dir unless `cwd` | `action` event: `data.exit` and the last 4000 bytes of output |

Every kind takes `label` and `confirm = true` (ask before acting). Templates fill `{row.<field>}`, `{row.key}`, `{row.state}` and `{vars.<name>}` (in `open` URLs, record values are URL-encoded). Section `actions` render in the section header, `row_actions` on each row, `[toolbar] actions` in the page header.

Needs answer themselves: each open need shows its `options` as buttons (the `recommendation` is marked) and a text box. Answering sets the need `answered` with `answer` in its body and sends the agent an `answer` event.

## Validation

Every write is checked against the schema (`mission-control schema` prints it; `src/config/schema.rs` is its only definition). A wrong type is an error naming the key by id (`sections[prs].columns[state].as`); an unknown top-level key is a warning. Layer-specific rules:

- `run` actions and `script` widgets are refused in the `mission-control` layer: it is written by the agent and the page.
  So are the keys that decide what a `run` action executes (`command`, `cwd`, `env`, `timeout_s`) on any action, and
  `permissions`. Templates are files in builtin/user/project, so they may define all of these.
- A `run` button runs only when `permissions.run_actions = true` and `config explain` shows its command and the
  permission coming from a file (user, project or builtin), never a mission version.
- Unknown widget names, actions that no section references, and `link` names that don't exist are warnings.
