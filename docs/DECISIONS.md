# Decisions

Each entry: the choice, what else was considered, and why.

## Name: `mission-control` (renamed from `mission`, 2026-10-23)

A *mission control* is the page this builds, and `mission-control` says so. The first name, `mission`, read as grand ("the mission of his life") rather than as a tool. The CLI is `mission-control`, with a short alias `mctl` that `install` links next to it. A tracked run is still "a mission" (`msn_` ids, `/m/<id>`, `[mission]`, the mission layer, `X-Mission-Token`, `mission_new`): that's the domain noun, not the product name. Env vars (`MISSION_CONTROL_*`), the state and config dirs (`mission-control`) and the project folder (`.mission-control/`) were renamed outright, with no fallback to the old names: nothing had shipped. Considered: `mc` (Midnight Commander), `control`, `cockpit`, `runboard`, and `mctl` alone (short, but cryptic as a product name).

## Shape: a separate skill, modelled on redline

- **Separate binary and repo,** not a redline feature: different domain (run state + config vs. comment threads), different data model, different release cadence.
- **Same shape as redline:** one daemon per device (`127.0.0.1:7466`, next to redline's 7465), a thin client per agent session (MCP), a CLI twin for every tool, one Rust binary, the same `bin/` wrapper, CI, `.agent/` and docs layout.
- **redline is the scaffolding reference.** Lifecycle, install, update, browser tests and release scripts are ported from it rather than redesigned. A shared crate (`skills-core`) may come once the copies settle (see ROADMAP, open questions).

## Everything on the page is config, in layers

The hand-built mission control it grew from hard-coded its waves, gates, layers and links in the page. Every new run shape meant a new page.

- **Layers:** builtin (compiled in) → template → user (`~/.config/mission-control/`) → project (`<repo>/.mission-control/`) → mission (stored, versioned). Users set defaults once; repos share team defaults in git; agents customize one mission without touching files.
- **Template below user:** a user's "always" preference applies to every template. A preference for one template is a same-named template file, which patches the built-in one instead of copying it.
- **Provenance per key** (`config explain`), so "why does the page look like this" always has an answer, for users and for agents editing config.

Considered: one config file per mission (no defaults, no sharing); a JS plugin per page (powerful, but not something an agent can safely edit or a user can review); YAML (indentation-sensitive, harder to patch by hand).

## TOML for config, JSON for records

TOML is what users and agents hand-edit; comments survive edits through `toml_edit`. Inside the program everything is `serde_json::Value`, so merging and validation are one code path. Records are machine data, so JSON.

## Arrays of tables merge by `id`

Sections and columns are ordered lists. Replacing a whole list to change one item makes every patch fragile and every provenance entry wrong. Matching by `id`, with `remove`, `before` and `after`, lets a higher layer touch one item. Arrays without ids (KPI items, plain values) still replace as a whole, which is what users expect for short lists.

## Mission config stored as ordered patches

Each version row holds the patch that was sent; resolution applies them in order on top of the other layers. The first scaffold stored one merged document instead: a `remove` aimed at a template's section merged into nothing, left `sections = []`, and that replaced the whole page. Patches also give history and undo (drop the last row) for free. Cost: resolution replays every version; retention compacts old versions (`[retention]`).

## Generic collections, no domain tables

Packages, PRs, gates, agents, tests, risks and decisions are all `records` rows in named collections, with a free JSON body and an optional `state`. Templates give them meaning. Considered: typed tables per concept (the old `state.json` shape) — rejected because every new run shape would need a migration.

## Typed ops, never `exec`

The old `mc.py` ran the agent's Python string through `exec` and rewrote the whole file. Writes are now `upsert` (RFC 7386 merge patch), `remove` and `log`, in batches applied as one transaction. Safe, concurrent, and cheap for the agent.

## The mission layer is untrusted

Agents and the page's **Customize** button write the mission layer. So it can't define `run` (shell) actions or script widgets; those must come from files the user owns. `run` is also off until the user sets `permissions.run_actions = true`.

## Network scope: localhost only

Like redline. Remote viewing (a phone, a teammate) needs a relay, auth and TLS; deferred. A static `export --html` covers sharing a snapshot.

## Page: vanilla JS, no build step, type-checked with JSDoc

Same as redline: tiny, `include_str!`'d into the binary, nothing to install, easy for an agent to read. Widgets register in a small registry (`MissionWidget.define`), so user script widgets use the same API as built-ins.

Every `web/*.js` file carries `// @ts-check` and JSDoc types, and `tsc --noEmit` runs in `just lint` (2026-10-23, user's choice). Considered: plain JS (no type safety as the widget library grows), TypeScript with a build step (an emit step and generated files in the binary, against the no-build rule). `typescript` is the only npm dev dependency; it never ships.

## Dependencies

| Area | Choice | Considered | Why |
|---|---|---|---|
| CLI | `clap` 4 (derive, env) | argh, bpaf | Same as redline; env fallbacks (`MISSION_CONTROL_ID`, `MISSION_CONTROL_PORT`) |
| HTTP server | `axum` 0.8 | actix-web, raw hyper | Same as redline; tokio-native |
| Async runtime | `tokio` | none | Required by axum (and rmcp) |
| Storage | `diesel` 2.3 (sqlite) + `diesel_migrations` + `libsqlite3-sys` (`bundled`) | rusqlite, sqlx, JSON files | Same evaluation as redline (typed schema, embedded migrations, boxed filters); the old JSON file couldn't take concurrent writers |
| Config parsing | `toml` 1.x (into `serde_json::Value`) | `toml_edit` only | Plain parsing for reads; `toml_edit` for comment-preserving writes (agent configs in `install`; user/project layers) |
| Windows links | `junction` (Windows only) | symlinks | Same as redline: junctions need no admin rights or Developer Mode |
| Agent-side HTTP client | `ureq` 2 (blocking) | reqwest | Small, sync, same as redline |
| Logging | `tracing` + `tracing-subscriber` | `log` | Same as redline |
| Misc | `serde`/`serde_json` (`preserve_order`), `anyhow`, `uuid`, `dirs` | | |
| Tests (dev only) | `tempfile` | | Temp config dirs per test |
| MCP | `rmcp` 3 + `schemars` 1 | hand-rolled JSON-RPC | Same as redline; tool schemas come from the arg structs |

## Code quality: pedantic clippy as errors, lib + thin bin, pinned toolchain

Inherited from redline unchanged: `clippy::pedantic` with `-D warnings` in CI, `unsafe_code` forbidden, a library with a thin binary, files split at ~250 lines, one `just check` entry point, and the toolchain pinned in `rust-toolchain.toml` so local and CI clippy agree.

## Agent workspace: `.agent/`

Inherited from redline: one agent-agnostic folder (context, rules, workflows, prompts, skills, session log); `CLAUDE.md`, `AGENTS.md` and `AGENT.md` only point to it. mission-control adds two workflows of its own: `add-config-key` and `add-widget`.

## Auth: admin token + per-mission token

Agent writes carry the admin token (`$MISSION_CONTROL_HOME/admin.token`, mode 0600). The page carries only its mission's token, and only for page events, sent in a custom header so a cross-origin page can't send it without a CORS preflight the daemon refuses. Reads need no token: without CORS headers other origins can't read the responses, and the data is the user's own on their own machine.

## Open questions settled (2026-10-23)

The user settled the ROADMAP's four open questions before the full build:

- **redline embed: opt-in.** `--review` (or `[mission] review = true`) loads redline's overlay when a redline daemon answers. Default pages stay light, and mission-control has no hard dependency on redline. Considered: always on (heavier page, a second daemon on every view), never.
- **Project layer: `.mission-control/` at the repo root.** Agent-agnostic (invariant 11). Considered: `.claude/mission/` (ties the core to one agent), both (two lookup rules to explain).
- **Spend: transcript usage only.** The `Stop` hook reads token usage from the session transcript. No credentials, no outbound network. Considered: Gateway/billing APIs when configured (secrets handling, a network path from the daemon).
- **`skills-core`: later (ROADMAP P2.1).** Lifecycle, install and update are ported from redline by copy, with the source noted in the session log. Extracting a shared crate mid-build would churn two repos while the copies are still moving.

## Live page: SSE diffs; agent `wait`: long-poll with a stored cursor

- **Pages get server-sent events,** not WebSockets: the flow is one-way (daemon → page), SSE reconnects by itself, and writes from the page are plain POSTs with the mission token. Each write publishes one `Change` (records as they are now, removed keys, new events, a config flag) on a broadcast channel; a page that falls behind gets `reset` and re-reads the view. `poll:<s>` stays for users who want it. Considered: WebSockets like redline's overlay (two-way, but nothing needs the page → daemon direction on the socket), polling (the old page's 200 KB every 10 s).
- **The `wait` cursor lives in the daemon** (`cursors` table, per mission and session), so the CLI and the MCP server share it, and a restarted session continues instead of replaying old answers. A session new to a mission starts at the furthest cursor of that mission. Considered: a cursor in the MCP process (redline's; lost on restart, and the CLI has none).
- **The page names actions by id;** the daemon reads what they do from the resolved config. A page (or anything holding its token) can only trigger buttons the config defines.
- `tokio-stream` (for `BroadcastStream`) is the one new dependency.

## Writing config files

- **The daemon writes user and project files,** not the CLI: one writer, the config cache stays right, and every open page whose config changed re-renders (all pages for the user layer, the project's for a project file).
- **Edits go through `toml_edit`,** so comments, order and formatting survive; a value's trailing comment is kept when the value changes.
- **Undo history for files lives in the state dir** (`history/<path>/<ms>.toml`), never next to the file: a project's `.mission-control/` is committed, and its history isn't the team's. The first edit also leaves `<file>.mission-control.bak`, like `install`.
- **Dry runs leave a draft** (in memory, one hour) so the agent can hand the user a preview URL before applying. Considered: a preview flag on the stored version (it would leak into undo and history).
- **`template_save` composes the mission's patches** into one document instead of diffing resolved configs: composition keeps `remove`/`before`/`after` exactly (a removal then a re-add stays two steps), and the template stays a patch on its base (`extends`).

## Hooks

- **One `mission-control hook` command for every event,** reading Claude Code's payload on stdin. It never fails the hook: problems go to stderr and it exits 0, so a down daemon can't block the agent.
- **Which mission:** `$MISSION_CONTROL_ID`, else the mission whose `session` is the hook's `session_id`, else the newest mission of the hook's project (from `cwd`). MCP sessions don't learn Claude Code's session id, so the project fallback is what usually applies; the agent can pin it with `mission-control attach <id> --session <session id>`.
- **Spend is an estimate:** token counts from the transcript (deduplicated by message id) times `[ingest.prices]`, which ship with list prices and can be overridden. No billing API, per the "transcript only" decision.
- **Hooks aren't installed by `install`:** they edit the user's Claude Code settings, so INSTALL.md has the agent ask first and merge the printed block.

## Updates (ported from redline)

- **Same mechanism as redline:** the daemon checks the clone's remote tags at most daily (`git ls-remote`, prereleases ignored), caches the answer in `update.json`, and notices appear on the page and in `new` / `mission_new` for the agent. `mission-control update` runs `scripts/checkout-release.sh` and rebuilds.
- **`[update]` lives in the user's `config.toml`,** the same file as the user layer, so there's one file to know. It is a known schema key, read only from that file (a mission or project patch to it does nothing).
- Dismissing a notice from the page needs a mission token (it only changes notices).

## Host check against DNS rebinding (security pass)

Loopback binding and "no CORS" don't stop DNS rebinding: a page at `http://evil.example:7466` whose name is re-pointed at 127.0.0.1 is same-origin with the daemon, so it could list missions, read a page (and its mission token) and post page events: answers, customize requests that reach the agent, `run` buttons when enabled. Browsers always send the name they used in `Host`, so the daemon answers only `127.0.0.1`/`localhost` on its port (421 otherwise). Requests with no `Host` (non-browser clients) pass. redline has the same exposure; it's noted for that repo.

## Design v2: a calm ops console (2026-10-24)

The v1 page was one long scroll: fourteen nav links in two rows, KPIs without totals, a single-sentence health card, and a `flow` widget that put packages in lanes but drew no dependencies, no legend and no "where are we". Burndown even sat above *Needs you* in the dogfood config. Choices made with the user, from mockups (`.design/README.md` links them):

- **Direction: calm ops console.** Keep "color means state"; add density and hierarchy instead of color: a status strip, mono numbers, labels as section titles. Considered: refining v1 as is (doesn't fix the scroll or the diagram), a dark-first "HUD" (character, but louder and harder to keep accessible).
- **Pages as tabs: Overview + `[[pages]]`.** `[[pages]]` merges by `id` like every other array of tables; a section joins one with `page = "<id>"` (default `overview`). Sections stay one flat list, so a higher layer can move a section to another page with one key. Each page has its own URL. Considered: sections nested inside pages (moving one from a higher layer means restating it), a left sidebar (costs width the graphs need), foldable sections on one page (doesn't scale to architecture docs).
- **Status strip on every page,** full on Overview and compact (health + needs pills) elsewhere, so principle 1 holds on any tab.
- **New `plan` widget; `flow` becomes its alias.** Phases are columns, records are nodes, dependencies are edges styled by the source's state (met, waiting, blocked upstream), gates sit between phases, the current phase is marked, the legend is always on. Considered: upgrading `flow` in place (a breaking visual change hidden behind an old name), a Gantt (needs time estimates agents don't have).
- **Dependencies are a field on the record** (`depends_on = ["C0"]`, field name configurable with `deps`). One upsert per task, no second collection to keep in sync. The `graph` widget (architecture) does use an edges collection, because its edges carry labels. Considered: edges collection for both, both supported for `plan` (more to document, little gain).
- **Diagrams: native `graph` and Mermaid both.** `graph` is for live, stateful diagrams (boxes show their task's state); Mermaid inside `doc` markdown is for quick static diagrams agents already know how to write. The Mermaid renderer is a large script, so it's an asset the page loads only when a doc has a ` ```mermaid ` block; how it ships is below ("Mermaid: compiled in").
- **Fonts served from the binary:** Inter and JetBrains Mono woff2 (OFL, ~250 KB), `font-display: swap`. The page stays offline-capable and looks the same on every OS; the console style leans on a good mono. Considered: system stacks (v1; zero bytes, but uneven across Linux, macOS and Windows).
- **Needs: pinned in the strip, first on Overview,** with number keys for options and the recommendation marked by an ink border, not a color. Considered: a modal that takes over (too loud for attended runs).
- **New surfaces:** a record drawer, a command palette with keyboard shortcuts, attention signals (title count, health-tone favicon, opt-in notifications for new needs and blocks only) and a redesigned missions home with needs across missions.
- **Logo: the beacon** (a signal in brackets), replacing the three lanes, which read as a generic progress icon. Picked from four concepts (lanes refined, scope, plan graph, beacon).
- **Principles** (`.design/principles.md`) gained: shape + word + tone (glyphs per state), one glance then drill, draw relations, keyboard first class; "dense but calm" now asks for denominators.
- **One new token, `--sunk`,** for wells inside panels (recommendation box, current phase, graph group bands). The light value is the lightest shade that keeps `--ink3` text at 4.5:1.

## Design v2 build: fonts, record history (2026-10-24)

- **Fonts: Fontsource variable woff2, latin + latin-ext.** Inter v20 and JetBrains Mono v24 from `@fontsource-variable/*` (OFL), vendored in `web/fonts/` with their license texts and compiled in (~185 KB). Variable files give 400 and 600 (and the header's 500) from one file per subset; `unicode-range` means a page fetches latin-ext only when it shows such a character. Served with a year-long `immutable` cache since the file names carry the font version. Considered: static 400/600 files (more files, same bytes), the full character sets (Cyrillic, Greek, Vietnamese: ~300 KB more for text agents rarely write). The static export leaves them out and uses the system stacks.
- **Record history from daemon-logged `state` events.** An upsert that changes an existing record's state appends a `state` event (`data: {collection, key, from, to}`), and an opening need a `need` event, in the same transaction. The feed shows them (the approved mockup's feed is exactly this), the drawer reads them back with `GET /events?collection=&key=` (`json_extract` on `data`), and agents stop logging what the page can infer. New records don't log, so a 20-task plan isn't 20 feed lines. Considered: a separate history table (feed unchanged, but a migration and a second retention policy), opt-in via config (an agent would rarely turn it on). Cost: more events toward `events_keep`.
- **Mermaid: compiled in, gzipped, loaded on demand.** Measured: Mermaid 11.12.3's `mermaid.min.js` is 2.8 MB, 818 KB gzipped; 12.2.0 is 5.2 MB / 1.57 MB and was 11 days old. 11.12.3 ships in the binary as the gzipped file (`web/vendor/`, MIT) and is sent as stored with `Content-Encoding: gzip`, so the binary grows ~0.8 MB (13.8 → 14.6 MB) and pages stay offline-capable. The page fetches it only when markdown has a ` ```mermaid ` block. It renders in `securityLevel: "strict"` with HTML labels off, themed from the resolved tokens, and the SVG is sanitized again (no scripts, foreign objects, links or event attributes) before it goes in. Considered: fetch once into the state dir (a network dependency and a trust-on-first-use download), a hand-written renderer for flowcharts and sequences (agents write the whole Mermaid language).
- **Diagram viewport** (asked for during the build): CSS `zoom` on the canvas inside its own scroller, so native scrolling, keyboard and focus work at every zoom level; fullscreen on the `<section>` (whose markup re-renders, while the element stays), so live updates don't leave fullscreen. Considered: SVG `viewBox` transforms (nodes are HTML links, not SVG), a pan-zoom library (no dependencies in `web/`).
