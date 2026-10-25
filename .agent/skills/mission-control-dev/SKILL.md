---
name: mission-control-dev
description: Develop the mission codebase (Rust daemon + layered config + CLI + MCP server + browser page). Use when working inside the mission-control repo on features, fixes, tests, templates or docs. Sets up a sandboxed dev loop and points to the repo's workflows and rules.
---

# mission-control-dev

You are working **on** mission, not using it. The source of truth is `.agent/` at the repo root.

1. Read `.agent/context.md` and `.agent/rules.md`.
2. **Sandbox:** every manual run uses `MISSION_CONTROL_HOME="$(mktemp -d)"`, `MISSION_CONTROL_CONFIG_DIR="$(mktemp -d)"` and `MISSION_CONTROL_PORT=7499`. Never touch `~/.local/state/mission-control` or `~/.config/mission-control`. Never run `install`/`uninstall` without asking.
3. **Inner loop:**
   - `cargo check`
   - `cargo test <module>` for a focused run (`cargo test config` for layering)
   - `just dev` (foreground daemon in temp dirs)
   - `just check` before you finish
4. **Pick the workflow** in `.agent/workflows/` that matches the task (implement-task, add-config-key, add-widget, add-mcp-tool, add-agent-support, add-migration, manual-browser-test, self-review).
5. **Conventions** are in `docs/CODE_STYLE.md`; contracts are in `docs/PROTOCOL.md` and `docs/CONFIG.md`. Update the docs in the same change (sync table in `rules.md`).
6. **End with** a `.agent/log/` entry.

To make this skill discoverable by an agent that loads project skills, symlink this folder into that agent's project-skills dir (e.g. `.claude/skills/mission-control-dev`). Ask the user before adding agent-specific config.
