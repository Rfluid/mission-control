# Rules

## Safety

- **Never touch the user's real mission-control state or config.** Use `MISSION_CONTROL_HOME="$(mktemp -d)"`, `MISSION_CONTROL_CONFIG_DIR="$(mktemp -d)"` and a non-default `MISSION_CONTROL_PORT` (7499) for anything you run by hand (`just dev` does this). Never read or write `~/.local/state/mission-control` or `~/.config/mission-control`, and never point `--project` at a real repo's `.mission-control/` unless the task is about that repo.
- **Stop sandbox daemons by pid** from `$MISSION_CONTROL_HOME/daemon.json`. Never `pkill -f "mission-control serve"`: it also kills the user's real daemon (and can kill your own shell).
- **Never run `mission-control install` / `uninstall` without the user's explicit OK**. They edit the user's agent configs (`~/.claude*`, `~/.codex`, `~/.cursor`, `~/.gemini`, opencode).
- **Never commit or push unless the user asks.** Conventional Commits when you do. PRs merge with rebase only, so every commit lands on main: each one self-contained and passing `just check`, no merge or `fixup!` commits (`pr.yml` checks).
- **Never weaken auth, the loopback bind or the mission-layer restrictions** to make something easier to test.

## Quality gates (all must pass before you say "done")

```sh
just check     # cargo fmt --check · clippy -D warnings (pedantic) · node --check · cargo test · e2e · browser
```

- **Behaviour change:** add or adjust a unit test (in the module), `tests/e2e.sh` (cross-process) or a browser test (page/widgets).
- **Config change:** every built-in template must still resolve with no errors (`builtin_templates_all_resolve_cleanly`).
- **Lints:** no new `#[allow(...)]` without a comment saying why. Prefer fixing the lint.
- **File size:** split any source file that grows past ~250 lines (see `docs/CODE_STYLE.md`).

## Docs stay in sync (same change, not later)

| You changed | Update |
|---|---|
| An HTTP route, op, MCP tool or CLI flag | `docs/PROTOCOL.md` |
| A config key, merge rule, widget or action kind | `docs/CONFIG.md` |
| A built-in default or template | `defaults/` (and `docs/CONFIG.md` if a key is new) |
| Component boundaries, the data model, config resolution or security | `docs/ARCHITECTURE.md` |
| A dependency or a design trade-off | `docs/DECISIONS.md` |
| A pattern or convention | `docs/CODE_STYLE.md` |
| How the page looks (widgets, tokens, layout, copy) | `.design/` |
| Something user-visible | `README.md` (user language, no internals) |
| How agents use mission-control | `SKILL.md` |
| How agents install mission-control | `INSTALL.md` |
| Finished a roadmap task | tick it in `docs/ROADMAP.md` |

## Scope

- **Do the task asked.** Note adjacent problems in your log entry or as a new ROADMAP item instead of fixing them silently.
- **Ask the user** when a choice changes the user-facing behaviour, the protocol, the config format or security, and the docs don't settle it.
