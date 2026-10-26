# Changelog

All notable changes to mission-control. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/) (before 1.0, a minor bump may change the protocol or the config format).

## [Unreleased]

## [0.1.1] - 2026-10-26

### Fixed
- **Typing in an overlay no longer triggers shortcuts.** Keys typed into a text box drawn by an overlay on the page (such as a redline comment) were read as shortcuts: `n` jumped to the first question instead of typing, and `/`, `?`, `g` did the same.

## [0.1.0] - 2026-10-25

First public release.

### Added
- **One page per run, one service for all of them.** A local daemon on `127.0.0.1:7466` serves every mission from every session: a missions home at `/` (the questions waiting on you across all missions first, then mission cards with health, progress and last activity) and one page per mission at `/m/<id>`. It starts on demand, keeps state in SQLite, and survives session restarts: a new session re-attaches by id.
- **A calm ops console.** A status strip answers *is it healthy, what needs me, where are we* on every page; the Overview adds what's in flight, the numbers and recent activity; plans, PRs, tests, agents and decisions get their own tabs. Light and dark themes, works on phones, fonts served from the binary (nothing loads from the network).
- **Live and two-way.** Updates arrive as they happen. Questions come with options, a recommendation and why; answer with a click, a number key or a sentence. Row and toolbar buttons ask the agent, set a state, open a link, copy text, or run a command from your own config. **Customize** asks the agent to change the page.
- **Your agent hears you** through `mission-control wait`, or, in Claude Code, as channel messages (`--channel`).
- **The Plan.** Phases as columns, tasks as boxes, every dependency drawn (met, waiting, blocked upstream), gates between phases, Trace blocked path, Graph / Table / Board views, zoom, fit and fullscreen. Architecture pages with live `graph` diagrams and Mermaid inside markdown docs.
- **Record drawer, command palette and shortcuts.** Click any task, row or card for its fields, dependencies and history (every drawer is a link). ⌘K / `/` jumps to pages, records, needs and actions; `?` lists the shortcuts.
- **Attention signals.** The tab title counts open questions, the favicon shows the run's health, and an opt-in browser notification fires for a new question or a newly blocked task, never for progress.
- **Everything on the page is config, in layers:** built-in defaults, the mission's template, your defaults (`~/.config/mission-control/`), the repo's (`.mission-control/`) and per-mission patches. Lists of pages, sections, columns and buttons merge by `id`, so a layer can change, remove or move one item without restating the rest. `mission-control config explain` shows which layer set every key.
- **Change any layer from chat:** "on this page", "for this repo", "always". Your files keep their comments; every change is validated against a schema (`mission-control schema`), can be previewed first, and can be undone (`config undo`, `config history`). Save a page as a template (`config template-save`). Shell (`run`) buttons and script widgets are allowed only in files you own, never in agent-written mission patches.
- **Widgets:** KPI tiles, health, needs, tables (links, state pills, filters, per-row buttons), plan, graph, doc, kanban, gates checklist, charts (line, bar, sparkline), spend, markdown, activity feed, sandboxed HTML, and your own script widgets from `widgets/*.js`. Four built-in templates: `minimal`, `multi-agent-build`, `pr-babysit`, `research`.
- **MCP tools for agents** (`mission-control mcp`), each with a CLI twin, so agents without MCP and hooks can do everything: start or re-attach a mission, write in batches (one transaction), read a short `brief`, wait for the user, and change config on any layer.
- **Fills itself in Claude Code:** hooks record subagents, pull requests and token spend (an estimate from the transcript) with no agent calls (`mission-control hook --print-config`).
- **Install for your agents** (`mission-control install`): skill, CLI (also as `mctl`) and MCP server for Claude Code, Codex, Cursor, Antigravity, opencode and Gemini CLI; `--dry-run` shows the plan; configs are backed up once and keep their key order and comments. `uninstall` and `doctor`.
- `mission-control import` brings over a hand-built mission control's `state.json`; `mission-control export` writes a static, tokenless snapshot to share; `archive` and `fork`; event retention with an **Older** button in the feed.
- `mission-control review` puts redline's overlay on the page, to comment on anything.
- Update notices: a daily release check, shown on the page and to your agent. `mission-control update` installs it; skip a version (`update skip`) or turn notices off (`update off`).
- Linux, macOS and Windows. Release binaries for Linux (x86_64, aarch64), macOS (arm64, x86_64) and Windows (x86_64, arm64); `bin/mission-control` downloads one when `cargo` isn't installed. Install and update use the latest release, not the development branch.

[Unreleased]: https://github.com/Rfluid/mission-control/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/Rfluid/mission-control/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Rfluid/mission-control/releases/tag/v0.1.0
