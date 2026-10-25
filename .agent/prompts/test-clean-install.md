# Prompt: test a clean install of mission-control

Give this to an agent **after** the user has installed mission-control with the README sentence (ROADMAP P1.1):

> Install mission-control from <repo URL> for all my coding agents. Follow its `INSTALL.md`.

````
Test the mission-control install on this machine. The user just installed it with its INSTALL.md (`mission-control install` for every detected agent). Verify it really works end to end, for every agent, and report. This is the user's real install: don't run `mission-control install` / `uninstall`, don't edit agent configs, don't edit ~/.config/mission-control, and don't edit the mission-control clone. If something is broken, diagnose it and propose the fix; don't apply it. Use temp dirs (`mktemp -d`) for anything you create, and a temp MISSION_CONTROL_CONFIG_DIR for any mission you create yourself.

1. Static checks
   - `which mission`, `mission-control --version` (prints `X.Y.Z (<git sha>)`), `mission-control doctor`. Every detected agent must show its skill and MCP entry as ok. Note anything missing.
   - How was it built? With cargo, `bin/mission-control` builds `target/release/mission-control`; without it, it downloads a release binary into `<clone>/target/prebuilt/`. Say which path was used.
   - `mission-control status`: `{running: false}` is fine before first use. Run `mission-control list`, then `mission-control status` again: it shows port, pid and version.
   - Each agent config that `doctor` lists must hold exactly one `mission-control` entry pointing at `<clone>/bin/mission-control mcp`, with the other servers intact. Diff each against its `<file>.mission-control.bak` and report what changed: only the mission entry should differ. Don't print Claude's backups; they hold secrets.

2. Agent loop per agent
   - Start each installed agent headless with a prompt to create a mission from the `minimal` template (MCP `mission_new`), upsert a record, log a line and add a need. Check with `curl -s 127.0.0.1:7466/api/missions/<id>` that all three arrived, and that the page at the returned URL loads.
   - Several agents in parallel must share one daemon.

3. Browser checks (needs the chrome-devtools MCP or headless Chrome)
   - Open the mission page. Check: sections follow the template; KPI counts match the records; needs show question, options and recommendation; record text with `<img src=x onerror=alert(1)>` renders as text; a `javascript:` link value is not a link; dark mode (emulate `prefers-color-scheme: dark`) and 390 px width work; the console has no errors.
   - Patch the mission layer (`mission-control config patch -m <id> '[[sections]]\nid = "activity"\nremove = true'`): after the next refresh the section is gone, and `mission-control config explain` attributes the change to the mission version.
   - A mission-layer patch adding a `run` action must be refused.

4. Skills
   - In a fresh Claude session the `mission-control` skill must be listed. For the others, ask each one headless "Which skills do you have? Answer with names only." and check that mission-control is listed (once).

5. Clean up what you created
   - Stop the processes your tests started, delete your temp dirs. Leave `~/.local/state/mission-control`, `~/.config/mission-control`, the clone and the user's configs alone.

Report a table: agent × (skill, MCP config, MCP listing, agent loop, notes). Then the browser checks (pass/fail), and the problems, each with its evidence and a proposed fix (in mission or in the user's setup).
````
