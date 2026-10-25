# Installing mission-control (instructions for an agent)

The user said something like "install mission-control from <repo> for all my agents", or "…for claude and codex". Follow these steps in order. Run the commands yourself; don't just print them.

**Assume nothing about this device.** Detect what you can and ask about the rest. Needs `git`. With a Rust toolchain (`cargo`), the first run builds from source. Without one, `bin/mission-control` downloads the release binary for Linux (x86_64/aarch64), macOS (arm64/x86_64) or Windows (x86_64/arm64) from the GitHub release of this version. (`MISSION_CONTROL_RELEASE_BASE` points the download at a mirror.)

If neither works (no `cargo` and no release binary for this platform), offer to install Rust from https://rustup.rs before going on.

**Windows:** everything below works the same, with the Windows twins of the scripts (Windows PowerShell runs them; it comes with Windows): `scripts\checkout-release.ps1` instead of `scripts/checkout-release.sh`, and `bin\mission-control.cmd` instead of `bin/mission-control`. From Git Bash, `bin/mission-control` works too (it hands over to `bin\mission-control.ps1`).

## 1. Get the repo

If you're not already inside a clone, ask the user where to keep it. It has to be a permanent location, because agents link to it and run its binary; never use a temp dir. Clone it there, then switch it to the latest release (main is where development happens; releases are what users run):

```sh
git clone https://github.com/Rfluid/mission-control <clone>
<clone>/scripts/checkout-release.sh    # checks out the highest vX.Y.Z tag; prereleases are skipped
```

On Windows: `powershell -NoProfile -ExecutionPolicy Bypass -File <clone>\scripts\checkout-release.ps1`.

Only if the user asks for a prerelease (e.g. `v0.3.0-beta`): `<clone>/scripts/checkout-release.sh v0.3.0-beta` (Windows: add `v0.3.0-beta` after the `.ps1`).

Below, `M=<clone>/bin/mission-control` (Windows: `M=<clone>\bin\mission-control.cmd`). The first run builds the release binary (about a minute) or downloads it.

## 2. Install for the agents

Run `$M install --dry-run` first and show the user the plan. Then `$M install` (every agent detected on this device) or `$M install --agents claude,codex` (just those). Show the user the output: which agents were found, where the skill was linked, and where the MCP server was registered.

- Known agents: `claude`, `codex`, `opencode`, `cursor`, `agy`, `gemini`.
- For each agent, install does two things:
  - It symlinks this repo as the `mission-control` skill into that agent's skills dir. On Windows the link is a directory junction, which needs no admin rights.
  - It registers the MCP server `mission-control` → `<clone>/bin/mission-control mcp` (Windows: `<clone>\target\bin\mission-control.exe mcp`, the binary `bin\mission-control.ps1` keeps up to date).
    - Claude: `claude mcp add --scope user`.
    - Codex: `[mcp_servers.mission-control]` in `~/.codex/config.toml` (`$CODEX_HOME/config.toml` if set).
    - Cursor, Gemini, Antigravity: `mcpServers.mission-control` in their JSON config (Cursor: `~/.cursor/mcp.json`, with `"type": "stdio"`; Gemini CLI: `~/.gemini/settings.json`; Antigravity: `~/.gemini/config/mcp_config.json`).
    - opencode: `mcp.mission-control` in `opencode.json`.
  - Skills: Codex, opencode, Cursor and Gemini CLI share `~/.agents/skills`; Antigravity reads `~/.gemini/config/skills` and `~/.gemini/antigravity-cli/skills`. Cursor also reads `~/.claude/skills`, so with Claude Code installed it lists mission-control twice; that's harmless.
  - Configs are backed up once to `<file>.mission-control.bak` before the first edit (for Claude, `~/.claude.json` or `<config dir>/.claude.json`). Key order and comments are kept.
- **The user runs Claude with another config dir** (a second profile, e.g. `CLAUDE_CONFIG_DIR=~/.claude-work`): ask about this, then add `--claude-config-dir <dir>` (repeatable). `$CLAUDE_CONFIG_DIR` is picked up automatically if set.
- **Another skills dir:** add `--skill-dir <dir>`.
- **A target already exists and isn't mission-control:** it is skipped. Tell the user. Pass `--force` (backs it up, then replaces it) only if they agree.
- **A config isn't plain JSON** (comments, as in JSONC): that agent is reported as FAILED along with the exact entry to add. Add it by hand only if the user agrees.
- **CLI link:** `mission-control`, and its short alias `mctl`, are linked into `~/.local/bin`. If that dir isn't on PATH, ask where to put it (`--bin-dir`). On Windows that's `%USERPROFILE%\.local\bin`, which usually isn't on PATH: offer to add it, or pick another dir. It gets small shims there: `mission-control.cmd` and `mctl.cmd` (cmd, PowerShell), and `mission-control` and `mctl` (Git Bash).
- **Claude Code, push instead of polling:** `--channel` registers `mission-control mcp --channel`, so what the user does on the page arrives in the session by itself. (Already registered without it: `claude mcp remove mission-control`, then install again with `--channel`.)
- **The user wants the skill but no MCP:** `--no-mcp`. Agents then use the CLI, which does the same thing.

## 2b. Claude Code hooks (optional, ask first)

Hooks fill the page's agents, pull requests and spend by themselves, so the agent spends no tokens on them. They go in the user's Claude Code settings, so **ask before adding them**:

```sh
$M hook --print-config     # the "hooks" block, with this clone's path
```

Merge that block into `~/.claude/settings.json` (keep any hooks already there; back the file up first). To try it without touching settings: `claude --settings <file with that block>`.

## 3. Explain (short)

> mission-control gives your agent session a live page: what the run is doing, what's waiting on you, and buttons to act on it. You shape it with config: your own defaults in `~/.config/mission-control/`, a repo's in `.mission-control/`, or just ask the agent to change "this page" or "every mission". Everything stays on this machine (127.0.0.1).

## 4. Check

Run `$M doctor` and report it: every installed agent should show its skill `ok` and its `mcp` entry `ok`. Then `$M config templates` lists the layouts available.

Then tell the user:
- Restart open agent sessions so they load the skill.
- Try it: ask the agent to "start a mission control for this run".
- Their defaults go in `~/.config/mission-control/config.toml` and `~/.config/mission-control/templates/` (examples: `<clone>/examples/`).

## Update

`$M update` moves the clone to the latest release (`scripts/checkout-release.sh`), then rebuilds or downloads the binary; the next command replaces the running daemon by itself. It refuses when the clone has local changes. A daily check shows a notice on the page and to the agent; `$M update skip` (this release) or `$M update off` (no notices) quiets it.

## Uninstall

`$M uninstall` removes the skill links, the CLI link and the MCP entries (restoring an untouched config byte for byte). Mission data stays in `~/.local/state/mission-control/` (Windows: `%LOCALAPPDATA%\mission-control\`) and config in `~/.config/mission-control/` until the user deletes them.
