# Workflow: test the page by hand

Use this for changes to `web/` or to anything the page renders (widgets, defaults, templates). First extend and run the automated test (`just browser`); do the manual pass for what it can't judge (layout, looks, density).

```sh
export MISSION_CONTROL_HOME="$(mktemp -d)" MISSION_CONTROL_CONFIG_DIR="$(mktemp -d)" MISSION_CONTROL_PORT=7499
cargo build --release
M=target/release/mission-control
out="$($M new Demo -t multi-agent-build --project "$MISSION_CONTROL_HOME")"
export MISSION_CONTROL_ID="$(printf '%s' "$out" | jq -r .mission.id)"; printf '%s' "$out" | jq -r .url
$M upsert packages R0 -s merged name="Contracts" wave=W0 pr=1
$M upsert prs 1 -s open title="feat: contracts" checks=green
$M need N-1 "Ship now or after review?" -o now -o later --recommendation later --why "review is close"
$M log "Run started"
```

1. **Open** the printed URL. If you have a browser tool (chrome-devtools MCP, headless Chrome), drive it; otherwise ask the user to.
2. **Check:**
   - sections and nav follow the template; KPI counts match the records
   - pills use the state colors from `[states]`; links follow `[links]` and open in a new tab
   - the need card shows question, options, recommendation and why
   - a config issue (e.g. `mission-control config patch '[[sections]]\nid = "x"\nwidget = "nope"'`) shows in the banner
   - hostile record text (`title='<img src=x onerror=alert(1)>'`) renders as text
   - compare with `.design/`: light, dark (theme button, and `prefers-color-scheme: dark`), and 390 px wide
3. **Act as a user customizing it:** put a section patch in `$MISSION_CONTROL_CONFIG_DIR/config.toml`, reload, and check `mission-control config explain` names that file.
4. **Clean up:** kill the daemon (pid in `$MISSION_CONTROL_HOME/daemon.json`).
