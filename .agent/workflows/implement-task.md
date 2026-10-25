# Workflow: implement a roadmap task

1. **Orient.**
   - Read `.agent/context.md`, `.agent/rules.md` and the newest `.agent/log/` entry.
   - Run `just check`, so you know the baseline is green. If it isn't, fix that first or report it.
2. **Pick.** Take the first unchecked task in the lowest open phase of `docs/ROADMAP.md`, unless the user named one. Read its *Done when* criteria.
3. **Read the code it touches,** and the matching section of `docs/ARCHITECTURE.md` / `docs/PROTOCOL.md` / `docs/CONFIG.md`. If redline already solves the same problem (lifecycle, install, update, MCP, browser tests), read its version first and port it rather than reinvent it.
4. **Plan briefly.** List the files you'll change and the tests you'll add. If the plan breaks an invariant in `context.md`, stop and ask.
5. **Implement.**
   - Follow `docs/CODE_STYLE.md`.
   - Small steps. Run `cargo check` / `cargo test <module>` as you go.
   - Use a matching workflow if there is one (`add-config-key`, `add-widget`, `add-mcp-tool`, `add-migration`, …).
6. **Test.**
   - Unit tests in the module.
   - `tests/e2e.sh` for anything crossing processes.
   - `workflows/manual-browser-test.md` for UI.
7. **Docs.** Use the sync table in `rules.md`.
8. **Gate.** `just check` must be green, then run `workflows/self-review.md`.
9. **Close.**
   - Tick the task in `docs/ROADMAP.md`. Add follow-ups as new unchecked items.
   - Write a `.agent/log/` entry.
   - Report to the user: what changed, how it was verified, anything left open.

## Gotchas

- Output-filtering shell proxies (such as `rtk`) can hide clippy errors ("No issues found") and test counts. Run `just check` unfiltered when lint results matter.
- The toolchain is pinned in `rust-toolchain.toml`. Bump it only on purpose.
- `defaults/` and `web/` are compiled in with `include_str!`. Rebuild after editing them; `bin/mission-control` does that automatically.
- zsh: quote config paths with brackets (`'sections[prs]'`), or the shell globs them. zsh `echo` mangles `\n` inside JSON, so use `printf '%s'`; pass multi-line TOML with `$'…\n…'`.
- `set_var` is unsafe in edition 2024 and `unsafe_code` is forbidden, so env-dependent logic takes an injected path (like `Stack::load(user_dir, project)`) for tests.
- `pkill -f <pattern>` run from an agent's shell tool can match that shell's own command line and kill it. Kill sandbox daemons by the pid in `$MISSION_CONTROL_HOME/daemon.json`.
- A mission-control config version is a patch. Don't "simplify" storage to a merged document: a `remove` aimed at a template's section would be lost (regression test `mission_patches_apply_in_order_over_the_template`).
- rmcp runs tool calls concurrently. Piping several `tools/call` lines into `mission-control mcp` at once races (an `attach` and the `view` after it); send one call and wait for its response (`call` in `tests/e2e.sh`).
- The e2e exports `MISSION_CONTROL_ID` early, and an MCP server started later binds it. Use `env -u MISSION_CONTROL_ID` when a test needs an unbound session.
- Gate commits on the check's own exit (`just check > log && git commit …`). `just check > log; echo $? && git commit` commits a failing tree.
