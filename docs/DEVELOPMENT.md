# Development

## Layout

```
src/main.rs                 thin binary: mission_control::cli::run()
src/lib.rs                  module tree
src/cli/                    clap: mod.rs (commands) · commands.rs (dispatch)
src/config/                 mod.rs (Stack: layers, templates, resolve) · merge.rs (merge by id, provenance) · validate.rs · builtin.rs
src/daemon/                 axum: mod.rs (App, router, serve) · handlers.rs
src/store/                  Diesel: mod.rs (Store API, ops) · rows.rs · schema.rs
src/mcp/                    rmcp server on stdio: mod.rs · tools.rs
src/api.rs                  blocking agent-side client; starts the daemon on demand
src/model.rs                wire types
src/paths.rs, src/ids.rs    state dir, user config dir, project root, tokens; ids, timestamps
defaults/                   built-in config layer + templates (compiled in)
migrations/                 Diesel SQL migrations (embedded)
web/                        mission.html, index.html, app.js (render + refresh), widgets*.js (registry + built-ins), glyphs.js,
                            style.css (tokens, base) · shell.css · widgets.css, fonts/ (Inter + JetBrains Mono woff2, OFL), vendor/ (Mermaid, gzipped; README says how to update)
examples/                   sample user and project config layers
tests/e2e.sh                end-to-end test
tests/browser/              headless-Chrome page tests (no npm deps); demo.mjs seeds the demo missions
docs/screenshots/           README screenshots and shoot.mjs, which regenerates them
bin/mission-control                 wrapper: rebuilds the release binary when sources are newer, or downloads a release binary without cargo
bin/mission-control.ps1, .cmd       the Windows twin, and its cmd launcher
build.rs                    stamps the git sha into --version
scripts/prepare-release.sh  bumps the version + CHANGELOG for the next release (just prepare-release)
scripts/checkout-release.sh moves a clone to the latest release tag (install and update); .ps1 is the Windows twin
.github/workflows/          ci.yml, release.yml, audit.yml (cargo-deny), pr.yml (Conventional Commits, actionlint)
.github/                    dependabot.yml, issue forms, PULL_REQUEST_TEMPLATE.md, CODEOWNERS, rulesets/main.json
.design/                    design tokens (tokens.css, tokens.json), check.py, logo
CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, LICENSE   community files
deny.toml                   cargo-deny policy: advisories, allowed licenses, sources
CHANGELOG.md                release notes (Keep a Changelog)
.agent/                     workspace for coding agents developing mission-control
```

`defaults/` and `web/*` are compiled into the binary with `include_str!`. Rebuild after editing them; `bin/mission-control` does that automatically.

## Tooling

```sh
just              # list recipes
just fmt          # cargo fmt
just lint         # fmt --check, clippy -D warnings (pedantic), node --check, tsc (types), .design/check.py
just types        # tsc --noEmit on web/*.js (installs the pinned typescript into node_modules/ on first run)
just test         # cargo test (unit)
just e2e          # release build + tests/e2e.sh
just browser      # release build + headless-Chrome page test
just screenshots  # release build + regenerate the README screenshots in docs/screenshots/
just check        # lint + test + e2e + browser: what CI runs
just deny         # cargo-deny (deny.toml) (cargo install cargo-deny)
just dev          # foreground daemon in temp MISSION_CONTROL_HOME / MISSION_CONTROL_CONFIG_DIR on :7499
```

Conventions and patterns: [CODE_STYLE.md](CODE_STYLE.md). The Windows notes in redline's DEVELOPMENT.md (Git Bash for `just`, `cargo xwin` checks from Linux) apply unchanged.

## Build and run

```sh
cargo build --release
export MISSION_CONTROL_HOME=$(mktemp -d) MISSION_CONTROL_CONFIG_DIR=$(mktemp -d) MISSION_CONTROL_PORT=7499   # never touches your real state or config
bin/mission-control serve                                   # foreground daemon, or let any command auto-start it
out="$(bin/mission-control new "Demo" -t multi-agent-build --project "$MISSION_CONTROL_HOME")"
export MISSION_CONTROL_ID="$(printf '%s' "$out" | jq -r .mission.id)"; printf '%s' "$out" | jq -r .url
bin/mission-control upsert prs 1 -s open title="feat: x"
bin/mission-control config explain 'sections[prs]'
```

Try the sample layers: `MISSION_CONTROL_CONFIG_DIR=examples/user-config bin/mission-control config resolve --project examples/project`.

Use `printf '%s'`, not `echo`, when piping JSON in zsh. Quote config paths with brackets (`'sections[prs]'`).

## Tests

- **Unit** (`just test`): merge rules and provenance (`config/merge.rs`), template resolution, `extends` cycles, user/project/mission layering, every built-in template resolving cleanly (`config/mod.rs`), validation (`config/validate.rs`), store ops, merge patches and config versions (`store/mod.rs`), CLI field parsing.
- **End-to-end** (`just e2e`, `tests/e2e.sh`): temp `MISSION_CONTROL_HOME` / `MISSION_CONTROL_CONFIG_DIR` and a random port; daemon auto-start, `new`, ops, config patch and refusal, `explain`.
- **Browser** (`just browser`, `tests/browser/page.test.mjs`): the page rendered from config in headless Chrome over CDP (no npm deps). Needs Chrome on PATH or `CHROME=/path/to/chrome`.

## Releases

Same process as redline (see its DEVELOPMENT.md, "Releases"): `just prepare-release`, `just check`, commit `chore(release): vX.Y.Z`, merge through a PR, then push the tag `vX.Y.Z`; `release.yml` builds and publishes the binaries. Users run releases, not main: `INSTALL.md` clones, then `scripts/checkout-release.sh` checks out the highest `vX.Y.Z` tag. `mission-control --version` prints `X.Y.Z (<git sha>)` (`build.rs`).

**CI** (`.github/workflows/ci.yml`): `lint` and `test` (Linux, macOS, Windows) with the pinned toolchain. **Audit** (`audit.yml`): `cargo deny check` on dependency changes and daily. **PR checks** (`pr.yml`): Conventional Commits, no merge or `fixup!` commits, `actionlint`. Repository settings and the ruleset follow redline's (rebase merge only).

## Database (Diesel)

- **Migrations** are embedded (`embed_migrations!("migrations")`) and run on every daemon start.
- **Driver:** `libsqlite3-sys` with `bundled`. No system SQLite is needed.

Adding a migration: see [`.agent/workflows/add-migration.md`](../.agent/workflows/add-migration.md). **`print-schema` gotcha:** `events.seq` and `mission_config.version` print as `Integer`; we use `BigInt`. Put it back after regenerating.

Store conventions:
- A batch of ops runs in one `conn.transaction`.
- Mission config versions are stored as the patch sent, never merged.
- The daemon holds the `Store` in a `std::sync::Mutex`. Never hold the guard across an `.await`.

## Adding things

| To add | Follow |
|---|---|
| A config key | [`.agent/workflows/add-config-key.md`](../.agent/workflows/add-config-key.md) |
| A built-in widget | [`.agent/workflows/add-widget.md`](../.agent/workflows/add-widget.md) |
| A built-in template | a file in `defaults/templates/`, an entry in `config/builtin.rs::TEMPLATES`; `builtin_templates_all_resolve_cleanly` must pass |
| An MCP tool | [`.agent/workflows/add-mcp-tool.md`](../.agent/workflows/add-mcp-tool.md) |
| An agent | [`.agent/workflows/add-agent-support.md`](../.agent/workflows/add-agent-support.md) |

## Conventions

See [CODE_STYLE.md](CODE_STYLE.md). Step-by-step procedures for agents are in [`.agent/workflows/`](../.agent/README.md).
