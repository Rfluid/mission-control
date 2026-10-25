# Architecture

## Goal and non-goals

- **Goal:** give a coding-agent session (Claude Code first) a live **mission control**: one page that shows what the run is doing, what's waiting on the user, and lets the user act on it. `mission-control` is the layer between agents and their mission controls, and owns six things:

  | Pillar | What it means |
  |---|---|
  | **Centralization** | One daemon (`127.0.0.1:7466`), one SQLite store, every mission from every session. `/` lists them; `/m/<id>` shows one. Survives session restarts; a new session re-attaches by id. |
  | **Configuration** | Everything the page shows is config: sections, widgets, columns, links, states and colors, buttons, theme. Layers: built-in → template → user → project → mission, merged with per-key provenance. |
  | **Defaults** | Built-in templates (`minimal`, `multi-agent-build`, `pr-babysit`, `research`) plus the user's and the project's own defaults and templates, applied to every new mission. |
  | **AI customization** | The agent edits any layer through typed tools, validated against a schema, versioned and undoable. The user asks in chat *or* from a **Customize** button on the page. |
  | **Optimization** | The agent spends few tokens: hooks fill agents/PRs/spend automatically, `batch` writes, `brief` gives a budgeted digest instead of re-reading state, retention keeps the store small, the page gets diffs instead of re-polling everything. |
  | **Operation** | Lifecycle (`start`/`stop`/`status`), missions (`new`/`list`/`attach`/`archive`/`fork`), `doctor`, `export --html`, `import` of the old `state.json`, self-update like redline. |

- **Origin:** a hand-built mission control for one multi-agent run (static `index.html`, a 200 KB `state.json` re-fetched every 10 s, `mc.py` running the agent's Python through `exec`, `python -m http.server`, redline for answers). `mission-control` generalizes it: nothing domain-specific in code, typed writes, live and light.
- **Non-goal: remote access.** Everything binds `127.0.0.1`. No multi-user auth.
- **Non-goal: inline comments.** That's redline's job. A mission page can carry the redline embed.
- **Any agent.** MCP is the common interface, and a CLI covers the rest (and hooks).

## Core concepts

- **Mission:** id (`msn_…`), title, session, project dir, a per-mission token. The page at `/m/<id>`.
- **Record:** a row in a named collection: `(mission, collection, key) → {state, body JSON, updated}`. Packages, PRs, gates, agents, risks and tests are all just collections; the template decides what each means. `needs` is a reserved collection with a dedicated widget.
- **Event:** append-only log (`log`, `config`, `answer`, `action`, `customize`, …). Feeds the activity widget and the agent's `wait` cursor.
- **Config layer:** a TOML document. Layers merge into the **resolved config** that drives the page. See [CONFIG.md](CONFIG.md).
- **Widget:** how a section renders. Built-in types, named widgets (a built-in with preset props), and script widgets (trusted, from user/project `widgets/*.js`) and `html` widgets (sandboxed iframe, any layer).
- **Action:** a button: `ask_agent`, `customize`, `answer`, `set`, `open`, `copy`, `run`. Clicks become events the agent receives; `run` (shell) exists only in user/project files and only with `permissions.run_actions = true`.

## Components

```
                browser                                         this device
 ┌───────────────────────────────────┐      ┌──────────────────────────────────────┐
 │ /m/<id>: mission.html             │      │  mission-control serve  (daemon)     │
 │  ├─ app.js (fetch, render, live) ─┼HTTP──►   axum · 127.0.0.1:7466              │
 │  └─ widgets*.js (registry)        │      │   SQLite (Diesel) state dir          │
 └───────────────────────────────────┘      │   config: Stack::load + patches      │
                                            └────▲─────────────────────▲───────────┘
                                                 │ HTTP                │ HTTP
                                  ┌──────────────┴────────┐  ┌─────────┴──────────────────────────┐
                                  │ mission-control mcp   │  │ mission-control new|upsert|log|…   │
                                  │ rmcp, stdio           │  │ CLI: any agent, hooks              │
                                  └──────▲────────────────┘  └────────────────────────────────────┘
                                         │ MCP
                                   agent session

 config sources: defaults/ (compiled in) · ~/.config/mission-control/ · <repo>/.mission-control/ · mission patches (DB)
```

**One binary** (`mission-control`), with these subcommands:

| Subcommand | Role | Module |
|---|---|---|
| `serve` | The daemon. One per device, shared by all sessions | `src/daemon/` |
| `new`, `attach`, `list`, `view`, `upsert`, `rm`, `log`, `need`, `batch`, `status`, `stop` | Agent operations from a shell | `src/cli/` → `src/api.rs` |
| `config resolve\|explain\|patch\|templates` | Inspect and change config | `src/cli/` → `src/config/`, `src/api.rs` |
| `mcp` | MCP server on stdio, one per agent session | `src/mcp/` |
| `wait`, `brief` | Page events and a digest for the agent | `src/cli/` → `src/daemon/` |
| `review`, `archive`, `fork`, `hook`, `schema`, `doctor`, `import`, `export` | redline overlay, mission lifecycle, Claude Code hooks, config schema, health, import and snapshots | `src/cli/` |
| `install`, `uninstall`, `update` | Agent setup and self-update | `src/install/`, `src/update/` |

| Module | Job |
|---|---|
| `config` | Loads layers (builtin compiled in from `defaults/`, user dir, project `.mission-control/`), resolves templates (`extends`, same-name patching, `replace`), merges with provenance, validates. Pure; no I/O beyond reading files. |
| `store` | Missions, records (all collections), events, mission-control config versions. Batches are one transaction. |
| `daemon` | axum. Pages and assets (`web/`, `include_str!`'d), agent API, config explain/patch, page events (`page.rs`), live delivery (`live.rs`: one broadcast channel → SSE per page, long-poll `wait` per agent). Config writes on every layer (`config.rs`), template save (`templates.rs`), resolution with overlays and drafts (`resolve.rs`), cached until a config file's size/mtime or the mission layer changes (`cache.rs`). |
| `layerfile` | Patches user/project TOML files in place with `toml_edit`; backup once; history in the state dir for undo. |
| `api` | Blocking client used by the CLI and MCP. Shares local helpers (project root, local config stack). |
| `lifecycle` | Finds, starts, replaces (older version or pre-rebuild) and stops the daemon. Ported from redline. |
| `mcp` | rmcp server on stdio, one per agent session; binds a mission; every tool wraps `Api` and has a CLI twin. |
| `hooks` | Claude Code hook payloads → ops (agents, PRs, spend from the transcript); pure, applied by `mission-control hook`. |
| `import` | Converts an old `state.json` into ops (pure); the CLI applies them through the daemon. |
| `install` | Skill and CLI links, MCP registration per agent (Claude, Codex, Cursor, Antigravity, opencode, Gemini CLI), `uninstall`, `doctor`. Ported from redline. |
| `cli` | `clap`. Every agent verb; JSON out. |
| `web/` | `widgets.js` (registry + built-ins, `MissionWidget.define`), `app.js` (fetch, render sections from config, refresh), `style.css` (tokens). |

## Data model

| Table | Columns | Notes |
|---|---|---|
| `missions` | `id`, `title`, `session?`, `project_dir?`, `token`, `created`, `archived` | `project_dir` picks the project layer |
| `records` | `mission-control`, `collection`, `key`, `state?`, `body` (JSON), `updated` | PK `(mission, collection, key)`. `upsert` applies an RFC 7386 merge patch to `body`; `state` is kept unless given |
| `events` | `seq`, `mission-control`, `kind`, `text`, `data?`, `at` | Append-only, global `seq` |
| `mission_config` | `mission-control`, `version`, `doc` (JSON patch), `note?`, `at` | One row per version, each the *patch* sent; applied in order at resolve time. Undo = drop the last row |
| `cursors` | `mission-control`, `session`, `seq`, `updated` | PK `(mission, session)`: where that agent session's `wait` stopped |

All timestamps are Unix ms. Deleting a mission cascades.

## Config resolution

1. Merge builtin, user, project and the mission patches, only to read `mission.template`.
2. Resolve the template chain (`extends`, parent first); each name merges its files builtin → user → project, starting from the last `replace = true`. Template metadata (`name`, `description`, `extends`, `replace`) is stripped.
3. Merge: builtin → template chain → user → project → mission patches. Record the label of the last writer of every leaf path (`sections[prs].columns[state].as`).
4. Validate: errors (unknown action kind, `run`/`script` in the mission layer) block a patch; warnings (unknown widget, action or link) show on the page and in `explain`.

The template sits below the user layer on purpose: a user's "always" preference (a section, a button) applies to every template; a preference for one template goes in a same-named template file.

## Security

- Loopback bind only, and only for the host names `127.0.0.1` and `localhost` (a request whose `Host` names anything else gets 421), so a DNS-rebinding page can't become same-origin with the daemon. Agent writes need the admin token (`$MISSION_CONTROL_HOME/admin.token`, mode 0600).
- Browser writes need the mission token in `X-Mission-Token`; page events are capped at 16 KiB and 60 a minute per mission, and name actions by id (the daemon reads what they do from the config). A custom header forces a CORS preflight, and the daemon sends no CORS headers, so other origins can't write. Reads are unauthenticated but, without CORS, other origins can't read the responses.
- Page rendering escapes all record and config strings; links are `http(s)`/`mailto` only; record values in link templates are URL-encoded (config `vars` go in as written).
- The mission layer, which the agent and the page write, cannot hold `run` actions, the keys that say what a `run` action executes, `permissions`, or script widgets. `run` buttons additionally need `permissions.run_actions = true`, and the daemon checks provenance before executing (`daemon/run.rs`).
- Theme values are set as CSS custom properties (`style.setProperty`), never injected as CSS text, and only when they look like a color (no `url()`), so config can't make the page fetch from elsewhere.
- Tokens are compared in constant time.
