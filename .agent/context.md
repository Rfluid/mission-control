# Context

**mission-control** gives a coding-agent session a live **mission control** page: what the run is doing, what's waiting on the user, and buttons to act on it. One local daemon serves every mission from every session. Everything the page shows is config, in layers (built-in → template → user → project → mission), so users and agents can customize defaults, layouts, widgets and buttons without code. It is modelled on the sibling `redline` skill, which is the scaffolding reference for structure, tooling and conventions.

## One-screen architecture

```
browser /m/<id> + web/app.js, widgets.js ──HTTP──► mission-control serve (axum, 127.0.0.1:7466, SQLite/Diesel)
                                                      ▲ HTTP                        ▲ HTTP
                                         mission-control mcp (rmcp, stdio)     mission-control new|upsert|log|need|config (CLI)
                                                      ▲ MCP                         ▲
                                                 agent session              agent shell · Claude Code hooks

config: defaults/ (compiled in) + ~/.config/mission-control/ + <repo>/.mission-control/ + mission patches (DB) ──► Resolved {config, provenance, issues}
```

There is one Rust binary, built as a library (`src/lib.rs`) with a thin `main.rs`. Modules:

| Module | Role |
|---|---|
| `cli/` | clap commands and dispatch |
| `config/` | layers, template resolution, merge with provenance, validation, built-in defaults |
| `daemon/` | axum: routes, auth, pages and assets |
| `store/` | Diesel: rows, schema, missions, records, events, config versions |
| `mcp/` | rmcp server and tools |
| `lifecycle` | find, start, replace and stop the daemon |
| `api` | blocking HTTP client; starts the daemon on demand |
| `model` | wire types (`Mission`, `Record`, `Event`, `Op`, `View`) |
| `paths` | state dir, user config dir, project root, tokens |
| `ids` | ids, tokens, time |

## Invariants (break one only with a DECISIONS.md entry)

1. **Loopback only.** The daemon binds `127.0.0.1`.
2. **Two token kinds.** The admin token (`$MISSION_CONTROL_HOME/admin.token`) writes anything. A mission token (in its page) may only post page events for that mission, in `X-Mission-Token`; the daemon sends no CORS headers, so other origins can't write.
3. **Everything on the page is config.** No widget, column, link, color or button is hard-coded for one domain. Domain shapes (waves, gates, PRs) live in templates.
4. **Layers resolve with provenance.** Every resolved leaf records the layer (and file or version) that set it last; `config explain` must always be able to answer "where did this come from".
5. **Arrays of tables merge by `id`.** A higher layer patches, removes (`remove = true`) or moves (`before`/`after`) one item without restating the list.
6. **The mission layer is untrusted.** It is written by agents and the page, so it may never define `run` actions or `script` widgets. Those live only in user/project files.
7. **Mission config versions are patches, applied in order.** Never store a pre-merged document: a patch may target items defined in lower layers. Undo drops the last version.
8. **Typed ops, never `exec`.** Writes are `upsert` / `remove` / `log` (RFC 7386 merge patches on record bodies), and a batch is one transaction.
9. **Records are generic collections.** No per-domain tables. `needs` is the only collection with a dedicated widget.
10. **Every MCP tool has a CLI twin.** Agents without MCP (and hooks) must be able to do everything.
11. **Any agent:** nothing agent-specific in the core. Agent differences live in `install/` and optional Claude Code integrations (channels, hooks).

## Status

See `docs/ROADMAP.md` (phase checklist) and the newest entry in `.agent/log/`.
