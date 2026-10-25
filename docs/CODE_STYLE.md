# Code style and patterns

These conventions apply to all code in this repo. They are redline's, adapted; when this file is silent, do what redline does. Tooling enforces what it can (`just lint`). The rest is review (`.agent/workflows/self-review.md`).

## Tooling

| Tool | Config | Enforced by |
|---|---|---|
| rustfmt | `rustfmt.toml`: width 120, `use_small_heuristics = "Max"` | `cargo fmt --check` |
| clippy | `[lints]` in `Cargo.toml` (pedantic + hygiene), `clippy.toml` | `cargo clippy --all-targets -- -D warnings` |
| rustc | `unsafe_code = "forbid"`, `unused_must_use = "deny"` | build |
| JS | no build step; `// @ts-check` + JSDoc, types in `web/types.d.ts` | `node --check`, `tsc --noEmit` (`just types`) |
| editors | `.editorconfig` | |
| toolchain | `rust-toolchain.toml` (pinned version, with rustfmt + clippy) | rustup |

Every pedantic allow in `Cargo.toml` is deliberate:
- `module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc`: noise for an application crate.
- `needless_pass_by_value`: axum extractors are taken by value.

Tests may `unwrap`/`expect` (`clippy.toml`). Nothing else may.

## Crate layout

- **Library + thin binary.** `src/main.rs` only calls `mission_control::cli::run()`. Everything else lives in the library, so it's testable and reusable.
- **One concept per module.** When a file passes ~250 lines, or holds two concepts, turn it into a folder module (`foo/mod.rs` + parts). Current shape:

```
config/   mod.rs (Stack, layers, templates, resolve) · merge.rs (merge + provenance) · validate.rs · path.rs (key paths) · builtin.rs (defaults/) · tests.rs
store/    mod.rs (Store API, ops) · rows.rs (Diesel rows) · schema.rs · tests.rs
daemon/   mod.rs (App, router, serve, orphan check) · handlers.rs · config.rs · resolve.rs · cache.rs · templates.rs
          page.rs (page events) · run.rs · live.rs (SSE, wait) · tests/
cli/      mod.rs (clap types) · commands.rs (dispatch)
mcp/      mod.rs (server, session binding) · tools.rs (one method per tool)
install/  mod.rs (install, uninstall, doctor, dry-run plan) · agents.rs · link.rs · mcp_config.rs (+ toml_file, tests)
api.rs    blocking client · lifecycle.rs (start / replace / stop the daemon) · layerfile.rs (TOML file edits, history)
```

- **Long tests go in a sibling `tests.rs`** (`#[cfg(test)] mod tests;`). Short ones stay inline.
- **Visibility is as narrow as works:**
  - `pub(super)` for DB rows
  - `pub(crate)` for helpers shared across modules
  - `pub` only for the library API.

## Layers and boundaries

```
cli ──► api (blocking HTTP) ──► daemon (axum) ──► store (Diesel)
mcp ──► api                      │  └──────────► config (pure: files → Resolved)
web/*.js ──────── HTTP ──────────┘
```

- **The daemon owns the database.** Nothing else opens it. Agents and the CLI go through HTTP (`api.rs`), so there's one writer. The one exception is `config resolve` / `config templates`, which read config files locally and need no daemon.
- **`config` is pure.** It reads files and returns values; it never touches the store or the network. The daemon combines it with the stored mission patches.
- **Read config from the resolved value,** never from one layer. The only per-layer code is validation of what a layer may hold.
- **Wire types** (`model.rs`) are separate from **DB rows** (`store/rows.rs`). Convert with `From`/`TryFrom`. Never serialize a row directly.
- **Every MCP tool has a CLI twin,** and both go through `Api`. No logic in `cli/commands.rs` or `mcp/tools.rs` beyond argument mapping.

## Config

- **Config files are TOML; everything inside is `serde_json::Value`** once parsed (`config::parse`). Records are JSON.
- **New keys** follow `.agent/workflows/add-config-key.md`: a default in `defaults/`, validation, a provenance test, docs in `CONFIG.md`.
- **Lists users will patch item by item are arrays of tables with `id`** (sections, columns). Maps keyed by name (actions, widgets, links, states) are tables.
- **Provenance labels** say the layer and the file or version: `user (~/.config/mission-control/config.toml)`, `builtin templates/minimal.toml`, `mission v3`.
- **Mission config is stored as patches,** one per version, applied in order. Never write a merged document back.
- **Editing user files** uses `toml_edit`, so comments and order survive; back up once (`*.mission-control.bak`), write only when something changed.

## Errors

- **Application code:** `anyhow::Result`, with `.context("what we were doing")` at I/O boundaries (`reading <path>`, `parsing <what>`). Use `bail!` for invalid input.
- **HTTP boundary:** handlers return `Result<_, (StatusCode, String)>`: 400 for bad input or invalid config (with the issues as JSON), 401 without the admin token, 404 for an unknown mission, 500 otherwise.
- **MCP boundary:** a tool's `Err` becomes an `isError` result the agent can read. Write messages for the agent: say what to do next ("unknown template `x` (have: …)").
- **No `unwrap`/`expect` outside tests.** For "impossible" cases, return an error with context, or `expect` with the invariant that makes it impossible (`"just made an object"`). Lock poisoning is recovered (`App::store()`), not unwrapped.

## Async and blocking

- **The daemon is async** (tokio/axum). The store is sync (Diesel) behind `std::sync::Mutex`. Take the guard in a statement or a small block, and **never hold it across `.await`**.
- **The agent side is blocking** (`ureq`). Inside async code (MCP), wrap it in `tokio::task::spawn_blocking`.
- **Long waits are long-polls with a deadline,** not sleeps.

## Persistence

- **A batch of ops is one transaction.** Either every op applies or none does.
- **Record bodies change only by RFC 7386 merge patch** (`merge_patch`): objects merge, `null` deletes.
- **Schema changes only via new migrations** (`.agent/workflows/add-migration.md`). Keep `seq` and `version` columns `BigInt`.
- **Tests use `Store::open(Path::new(":memory:"))`,** so migrations run there too.

## Naming and values

- **Ids are prefixed by type:** `msn_` (`ids::new_id`). Tokens are 32 hex chars.
- **Timestamps** are `i64` Unix milliseconds (`ids::now_ms`).
- **No magic numbers.** Use named constants next to their use, with a comment when the value matters (`MAX_EXTENDS`, `VIEW_EVENTS`, `POLL_FALLBACK_S`).
- **Config names** are `snake_case` keys and `kebab-case` ids (`multi-agent-build`, `rerun-ci`).

## Logging

- **`tracing` macros in the daemon,** with fields, not formatted strings.
- **Never log a token.** Log ids and paths.
- **The agent side doesn't log;** it returns errors with context.

## Comments and docs

- **Every file starts with a `//!` header** (or a `//` header in JS, a `#` comment in TOML defaults): what it's for and how it fits. One to three lines.
- **`///` on public items** says behaviour and edge cases, not types.
- **Comment the *why*:** a non-obvious ordering, a lint allow, an "unverified" assumption. Don't narrate code.
- **User-facing text** (`README.md`) has no internals. **Agent-facing text** (`SKILL.md`, `INSTALL.md`, tool descriptions) is imperative and short.

## Browser code (`web/`)

- **No dependencies, no build step:** one IIFE per file, `// @ts-check` then `"use strict"`, served with `include_str!` under `/assets/`.
- **Files:** `app.js` (state, live updates, render), `actions.js` (clicks, page events), `markdown.js`, `glyphs.js` (state glyphs), `widgets.js` (registry, helpers, core widgets), `widgets-chart.js`, `widgets-board.js`, `widgets-embed.js`. Styles: `style.css` (tokens, fonts, base, shared components), `shell.css` (top bar, strip, tabs, page frame), `widgets.css`.
- **States render as glyph + word** (`h.pill`, `h.glyph`): never a colored dot or border alone.
- **Types are JSDoc.** Shared shapes (`View`, `Rec`, `Ctx`, `Section`, `WidgetRender`) live in `web/types.d.ts`, which mirrors `src/model.rs` and never ships. `strict` is on; cast with `/** @type {T} */ (x)` only where the DOM can't know (an element id we control).
- **Widgets render from `ctx` only** (`config`, `records`, `events`, helpers `h`). No network calls in a widget.
- **Escape every string** that goes into `innerHTML` (`h.esc`). Links go through `h.safeUrl` (`http(s)`/`mailto`) and `h.fill` (record values URL-encoded).
- **Colors come from tokens** (`var(--…)`) and `[states]`; theme overrides are set with `style.setProperty`.
- **A broken widget never breaks the page:** `app.js` renders each section in a `try` and shows an error card instead.
- **Storage:** `localStorage` access, if any, is wrapped in try/catch.

## Tests

| Kind | Where | For |
|---|---|---|
| Unit | in the module (`mod tests` or `tests.rs`) | merge rules, provenance, template resolution, validation, store ops |
| HTTP | `src/daemon/tests.rs` (`tower::ServiceExt::oneshot` on `router`, in-memory store) | routes, auth, config patch refusal |
| End-to-end | `tests/e2e.sh` | what crosses processes: daemon auto-start, CLI, MCP stdio |
| Browser | `tests/browser/` (headless Chrome over CDP, `just browser`) | rendering, escaping, themes, phone width |
| Manual browser | `.agent/workflows/manual-browser-test.md` | looks, anything the automated test can't judge |

Unit tests use `tempfile::tempdir()` for files and never touch `$HOME`. E2E uses temp `MISSION_CONTROL_HOME` / `MISSION_CONTROL_CONFIG_DIR` and a random port.

## Commits

Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `chore:`). Subject ≤ 72 chars. The body says *why*. Commit only when the user asks.
