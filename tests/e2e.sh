#!/usr/bin/env bash
# End-to-end: daemon + CLI (agent side) + curl (page side). Temp state and config dirs, random port.
# Only cross-process behaviour lives here; merge rules, validation and the store are unit-tested in their modules.
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"
M="$repo/bin/mission-control"
E="$M" EXE="" # E: the binary itself, for stdio pipes (the Windows wrapper is PowerShell)
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*)
  win=1 E="$repo/target/release/mission-control.exe" EXE=".exe"
  jq() { command jq -b "$@"; } # native jq writes CRLF otherwise
  ;;
esac
# Daemon pids come from daemon.json: Windows pids on Windows, which bash's kill doesn't know.
kill_pid() { if [[ -n "${win:-}" ]]; then taskkill //F //PID "$1" >/dev/null 2>&1; else kill "$1" 2>/dev/null; fi; }
alive() { if [[ -n "${win:-}" ]]; then tasklist //FI "PID eq $1" 2>/dev/null | grep -q "$1"; else kill -0 "$1" 2>/dev/null; fi; }
tmp="$(mktemp -d)"
export MISSION_CONTROL_HOME="$tmp/state" MISSION_CONTROL_CONFIG_DIR="$tmp/config" MISSION_CONTROL_PORT="$((20000 + RANDOM % 20000))"
export RUST_LOG="mission_control=debug"
unset MISSION_CONTROL_ID
trap 'kill_pid "$(jq -r .pid "$MISSION_CONTROL_HOME/daemon.json" 2>/dev/null)" || true; sleep 0.5; rm -rf "$tmp" 2>/dev/null || true' EXIT
ok() { echo "ok - $*"; }
fail() { echo "FAIL - $*" >&2; exit 1; }
base="http://127.0.0.1:$MISSION_CONTROL_PORT"

# A user layer (a button on the PR rows, everywhere) and a project layer (vars for link templates).
mkdir -p "$MISSION_CONTROL_CONFIG_DIR" "$tmp/proj/.mission-control"
cat > "$MISSION_CONTROL_CONFIG_DIR/config.toml" <<'EOF'
[actions.rerun-ci]
label = "Re-run CI"
kind = "ask_agent"
message = "Re-run CI on {row.key}"

[[sections]]
id = "prs"
row_actions = ["rerun-ci"]
EOF
cat > "$tmp/proj/.mission-control/config.toml" <<'EOF'
[vars]
repo = "acme/widgets"
EOF

created="$("$M" new "E2E build" -t multi-agent-build --project "$tmp/proj")"
id="$(jq -r .mission.id <<<"$created")"
url="$(jq -r .url <<<"$created")"
[[ "$id" == msn_* && "$url" == "$base/m/$id" ]] || fail "new: $created"
[[ "$(curl -fsS "$base/health" | jq -r .name)" == mission-control ]] || fail "health"
ok "new $id from a template (daemon auto-started on $MISSION_CONTROL_PORT)"

if "$M" new "bad" -t no-such-template --project "$tmp/proj" >/dev/null 2>&1; then fail "unknown template accepted"; fi
ok "unknown template refused"

export MISSION_CONTROL_ID="$id"
"$M" upsert packages R0 -s merged name="Contracts" wave=W0 pr=17 >/dev/null
"$M" upsert prs 17 -s open title="feat: contracts" checks=green >/dev/null
"$M" upsert prs 17 checks=null review=APPROVED >/dev/null
"$M" upsert prs 99 -s open title="to remove" >/dev/null
"$M" rm prs 99 >/dev/null
"$M" log "Run started" >/dev/null
"$M" need N-1 "Ship now?" -o yes -o no --recommendation no --why "CI is red" >/dev/null
printf '%s' '[{"op":"upsert","collection":"agents","key":"a1","state":"running","patch":{"pkg":"R0"}},
  {"op":"log","kind":"agent","text":"a1 started"}]' | "$M" batch - >/dev/null

view="$("$M" view)"
pr="$(jq -c '.records[] | select(.collection=="prs") | [.key, .state, .body]' <<<"$view")"
[[ "$pr" == '["17","open",{"title":"feat: contracts","review":"APPROVED"}]' ]] || fail "upsert merge patch / rm: $pr"
[[ "$(jq -r '.records[] | select(.collection=="needs") | "\(.state) \(.body.recommendation)"' <<<"$view")" == "open no" ]] \
  || fail "need"
[[ "$(jq -r '.records[] | select(.collection=="agents") | .state' <<<"$view")" == running ]] || fail "batch upsert"
[[ "$(jq -r '[.events[].text] | join("|")' <<<"$view")" == *"Run started|N-1 opened: Ship now?|a1 started"* ]] || fail "log + batch log (and the need event)"
ok "upsert (merge patch), rm, log, need, batch"

# view --page: one page's sections and only the records they read (plus needs, for the status strip).
pv="$("$M" view --page overview)"
[[ "$(jq -r '[.config.sections[].id] | length > 0' <<<"$pv")" == true ]] || fail "view --page: $pv"
"$M" view --page nope >/dev/null 2>&1 && fail "view --page accepted an unknown page"
ok "view --page"
"$M" need N-9 "Which first?" -o a -o b --blocks R0 >/dev/null
[[ "$("$M" view -c needs | jq -r '.records[] | select(.key=="N-9") | .body.blocks')" == R0 ]] || fail "need --blocks"
"$M" rm needs N-9 >/dev/null
ok "need --blocks"
"$M" upsert prs 17 -s merged >/dev/null
hist="$(curl -fsS "$base/api/missions/$MISSION_CONTROL_ID/events?collection=prs&key=17")"
[[ "$(jq -r '[.[] | select(.kind=="state")][-1] | "\(.data.from)>\(.data.to) \(.text)"' <<<"$hist")" == "open>merged 17 feat: contracts → merged" ]] || fail "state event + record history: $hist"
"$M" upsert prs 17 -s open >/dev/null
ok "state changes log themselves; a record's history reads them back"

# Config: an accepted mission patch, a refused one, and provenance across every layer.
out="$("$M" config patch $'[states]\nat_risk = "warn"\n[[sections]]\nid = "tests"\nremove = true' --note "drop tests")"
[[ "$(jq -r .version <<<"$out")" == 2 ]] || fail "config patch: $out"
if "$M" config patch '{"actions":{"x":{"kind":"run","command":["ls"]}}}' >"$tmp/refused" 2>&1; then
  fail "run action accepted in the mission layer"
fi
grep -q 'only in user/project config' "$tmp/refused" || fail "refusal message: $(cat "$tmp/refused")"
ok "config patch accepted; run action refused in the mission layer"

explain="$("$M" config explain)"
grep -q '^sections\[prs\]\.row_actions  ←  user (' <<<"$explain" || fail "user provenance"
grep -q '^vars\.repo  ←  project (' <<<"$explain" || fail "project provenance"
grep -q '^states\.at_risk  ←  mission v2' <<<"$explain" || fail "mission provenance"
grep -q '^sections\[plan\]\.widget  ←  builtin templates/multi-agent-build.toml' <<<"$explain" || fail "template provenance"
ok "config explain: builtin template, user, project, mission"

sections="$(curl -fsS "$base/api/missions/$id" | jq -r '[.config.sections[].id] | join(",")')"
[[ "$sections" == "needs,track,inflight,overview,burn,recent,activity,plan,gates,system,howitworks,prs,agents,spend,decisions,risks" ]] \
  || fail "view sections: $sections"
[[ "$(curl -fsS "$base/api/missions/$id" | jq -r '.config.sections[] | select(.id=="prs") | .row_actions[0]')" == rerun-ci ]] \
  || fail "user row_actions on the template's section"
ok "view: template sections, minus the removed one, plus the user's button"

code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$base/api/missions/$id/ops" -H 'content-type: application/json' -d '[]')"
[[ "$code" == 401 ]] || fail "unauthenticated ops: $code"
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$base/api/missions/$id/ops" -H 'authorization: Bearer nope' \
  -H 'content-type: application/json' -d '[]')"
[[ "$code" == 401 ]] || fail "wrong-token ops: $code"
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "$base/api/missions/$id/config" -H 'content-type: application/json' \
  -d '{"patch":{}}')"
[[ "$code" == 401 ]] || fail "unauthenticated config patch: $code"
ok "writes without the admin token are 401"

page="$(curl -fsS "$url")"
[[ "$page" == *'window.MISSION'* && "$page" == *"E2E build"* ]] || fail "page"
for a in app.js actions.js widgets.js style.css; do curl -fsS "$base/assets/$a" >/dev/null || fail "asset $a"; done
[[ "$(curl -s -o /dev/null -w '%{http_code}' "$base/m/msn_nope")" == 404 ]] || fail "unknown mission page"
ok "page and assets served; unknown mission is 404"

[[ "$(jq -r .running <<<"$("$M" status)")" == true ]] || fail "status"
[[ "$("$M" --version)" == "mission-control "* ]] || fail "--version"
ok "status, --version"

# Sessions: a duplicate title in one session is refused; attach re-binds after a restart.
s1="$("$M" new "Dup" --session ses_a --project "$tmp/proj")"
dup="$(jq -r .mission.id <<<"$s1")"
if "$M" new "Dup" --session ses_a --project "$tmp/proj" >"$tmp/dup" 2>&1; then fail "duplicate title accepted"; fi
grep -q "409.*$dup" "$tmp/dup" || fail "duplicate refusal: $(cat "$tmp/dup")"
"$M" new "Dup" --session ses_b --project "$tmp/proj" >/dev/null || fail "same title, other session"
"$M" new "Dup" --session ses_a --force --project "$tmp/proj" >/dev/null || fail "--force"
"$M" upsert -m "$dup" notes status text=kept >/dev/null
att="$("$M" attach "$dup" --session ses_restarted)"
[[ "$(jq -r .mission.session <<<"$att")" == ses_restarted && "$(jq -r .url <<<"$att")" == "$base/m/$dup" ]] \
  || fail "attach: $att"
[[ "$("$M" view -m "$dup" -c notes | jq -r '.records[0].body.text')" == kept ]] || fail "records after attach"
[[ "$("$M" view -m "$id" -c prs | jq -r '[.records[].collection] | unique | join(",")')" == prs ]] || fail "view -c"
if "$M" attach msn_nope >/dev/null 2>&1; then fail "attach to an unknown mission"; fi
ok "duplicate title refused (409) unless --force; attach keeps id and records; view -c filters"

[[ "$("$M" config get 'sections[prs].columns[state].as')" == '"pill"' ]] || fail "config get path"
if "$M" config get 'sections[nope]' >/dev/null 2>&1; then fail "config get of a missing key"; fi
"$M" schema | jq -e '.properties.sections and .properties.actions' >/dev/null || fail "schema"
ok "config get by key path; schema"

# Customization on every layer: user and project files (comments kept), dry run, undo, history, templates, widgets.
printf '# my defaults\n[vars]\nowner = "me" # keep this comment\n' >"$MISSION_CONTROL_CONFIG_DIR/config.toml.tmp"
cat "$MISSION_CONTROL_CONFIG_DIR/config.toml" >>"$MISSION_CONTROL_CONFIG_DIR/config.toml.tmp" && mv "$MISSION_CONTROL_CONFIG_DIR/config.toml.tmp" "$MISSION_CONTROL_CONFIG_DIR/config.toml"
out="$("$M" config patch --layer user '{"actions":{"burn":{"label":"Burndown","kind":"ask_agent","message":"Update the burndown"}},"toolbar":{"actions":["ask","customize","burn"]}}' --note "burndown button everywhere")"
[[ "$(jq -r .changed <<<"$out")" == true ]] || fail "user patch: $out"
grep -q 'owner = "me" # keep this comment' "$MISSION_CONTROL_CONFIG_DIR/config.toml" || fail "comment lost: $(cat "$MISSION_CONTROL_CONFIG_DIR/config.toml")"
[[ -f "$MISSION_CONTROL_CONFIG_DIR/config.toml.mission-control.bak" ]] || fail "user config not backed up"
grep -q '^toolbar\.actions  ←  user (' <<<"$("$M" config explain toolbar)" || fail "user provenance for toolbar"
"$M" config patch --layer project '[vars]
team = "core"' >/dev/null
grep -q 'team = "core"' "$tmp/proj/.mission-control/config.toml" || fail "project file"
dry="$("$M" config patch '{"sections":[{"id":"activity","title":"Log"}]}' --dry-run)"
[[ "$(jq -r .dry_run <<<"$dry")" == true && "$(jq -r .preview_url <<<"$dry")" == "$base/m/$id?draft=drf_"* ]] || fail "dry run: $dry"
curl -fsS "$(jq -r .preview_url <<<"$dry" | sed 's#/m/#/api/missions/#')" | jq -e '.config.sections[] | select(.id=="activity") | .title == "Log"' >/dev/null \
  || fail "draft preview"
[[ "$("$M" view -c none | jq -r '.config.sections[] | select(.id=="activity") | .title')" == Activity ]] || fail "dry run applied"
"$M" config undo --layer project >/dev/null
! grep -q 'team' "$tmp/proj/.mission-control/config.toml" 2>/dev/null || fail "project undo"
[[ "$("$M" config history --layer user | jq -r '.versions[0].note')" == "burndown button everywhere" ]] || fail "history"
"$M" config widget burnchart --base table --props '{"collection":"burn"}' --description "Burndown rows" >/dev/null
[[ "$("$M" config get widgets.burnchart.base)" == '"table"' ]] || fail "widget define"
saved="$("$M" config template-save e2e-build --layer project --description "From the e2e")"
[[ "$(jq -r .extends <<<"$saved")" == multi-agent-build ]] || fail "template save: $saved"
from="$("$M" new "From template" -t e2e-build --project "$tmp/proj" | jq -r .mission.id)"
[[ "$("$M" view -m "$from" -c none | jq -r '[.config.sections[].id] | index("tests")')" == null ]] || fail "template keeps the removed section removed"
[[ "$("$M" config get -m "$from" 'widgets.burnchart.base')" == '"table"' ]] || fail "template keeps the widget"
"$M" config undo --layer user >/dev/null
! grep -q burn "$MISSION_CONTROL_CONFIG_DIR/config.toml" || fail "user undo"
ok "config on every layer: user/project files keep comments, dry run + draft preview, undo, history, widget, template save"

# MCP over stdio: every tool, the way an agent calls them (one call at a time, each awaited).
mcp_init='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"e2e","version":"0"}}}'
mcp_ready='{"jsonrpc":"2.0","method":"notifications/initialized"}'
mcp_log="$tmp/mcp.out"
exec 7> >(env -u MISSION_CONTROL_ID MISSION_CONTROL_SESSION=ses_mcp "$E" mcp >"$mcp_log" 2>/dev/null)
printf '%s\n%s\n' "$mcp_init" "$mcp_ready" >&7
# call id tool args-json: sends a request and waits for its response line
call() {
  if [[ "$2" == tools/list ]]; then printf '{"jsonrpc":"2.0","id":%s,"method":"tools/list"}\n' "$1" >&7
  else printf '{"jsonrpc":"2.0","id":%s,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2" "$3" >&7; fi
  for _ in $(seq 100); do jq -e --argjson i "$1" 'select(.id == $i)' "$mcp_log" >/dev/null 2>&1 && return 0; sleep 0.1; done
  fail "mcp call $1 ($2) got no response"
}
call 2 tools/list
call 3 batch '{"ops":[]}'
call 4 mission_new "{\"title\":\"Via MCP\",\"template\":\"pr-babysit\",\"project_dir\":\"$tmp/proj\"}"
call 5 batch '{"ops":[{"op":"upsert","collection":"prs","key":"5","state":"open","patch":{"title":"t","checks":"red"}},{"op":"log","text":"from mcp"}]}'
call 6 view '{"collections":["prs"]}'
call 7 config_get '{"path":"sections[prs].row_actions"}'
call 8 config_patch '{"patch":"[states]\nflaky = \"warn\"","note":"toml patch"}'
call 9 config_explain '{"prefix":"states.flaky"}'
call 10 config_patch '{"patch":{"actions":{"x":{"kind":"run","command":["ls"]}}}}'
call 11 template_list "{\"project_dir\":\"$tmp/proj\"}"
call 12 mission_list '{}'
call 13 mission_attach "{\"id\":\"$id\"}"
call 14 view '{"collections":["agents"]}'
call 15 wait '{"timeout_s":0}'
call 16 widget_define '{"name":"w1","base":"feed","props":{"limit":5}}'
call 17 config_undo '{}'
call 18 config_history '{"layer":"user"}'
call 19 template_save '{"name":"mcp-saved","layer":"project","replace":true}'
call 20 brief '{"max_chars":300}' 
exec 7>&-
mcp_out="$(cat "$mcp_log")"
res() { jq -s -c --argjson i "$1" '.[] | select(.id == $i) | .result' <<<"$mcp_out"; }
txt() { res "$1" | jq -r '.content[0].text'; }
tools="$(res 2 | jq -r '[.tools[].name] | sort | join(",")')"
[[ "$tools" == "batch,brief,config_explain,config_get,config_history,config_patch,config_schema,config_undo,mission_attach,mission_list,mission_new,template_list,template_save,view,wait,widget_define" ]] \
  || fail "tools/list: $tools"
[[ "$(res 3 | jq -r .isError)" == true && "$(txt 3)" == *"mission_new"* ]] || fail "unbound batch: $(res 3)"
mcp_id="$(txt 4 | jq -r .mission_id)"
[[ "$mcp_id" == msn_* && "$(txt 4 | jq -r .url)" == "$base/m/$mcp_id" ]] || fail "mission_new: $(res 4)"
[[ "$(txt 5 | jq -r .applied)" == 2 ]] || fail "batch: $(res 5)"
[[ "$(txt 6 | jq -c '[.records[] | [.key, .state, .body.checks]]')" == '[["5","open","red"]]' ]] || fail "view: $(res 6)"
[[ "$(txt 7 | jq -c .)" == '["rerun-ci"]' ]] || fail "config_get (the user layer's row_actions win): $(res 7)"
[[ "$(txt 8 | jq -r .version)" == 2 ]] || fail "config_patch toml: $(res 8)"
[[ "$(txt 9 | jq -r '.provenance["states.flaky"]')" == "mission v2" ]] || fail "config_explain: $(res 9)"
[[ "$(res 10 | jq -r .isError)" == true && "$(txt 10)" == *"only in user/project"* ]] || fail "run refused: $(res 10)"
[[ "$(txt 11 | jq -r '.["multi-agent-build"].extends')" == minimal ]] || fail "template_list: $(res 11)"
txt 12 | jq -e --arg m "$mcp_id" 'map(.id) | index($m) != null' >/dev/null || fail "mission_list: $(res 12)"
[[ "$(txt 13 | jq -r .mission_id)" == "$id" ]] || fail "mission_attach: $(res 13)"
[[ "$(txt 14 | jq -r '.records[0].key')" == a1 ]] || fail "view after attach uses the attached mission: $(res 14)"
[[ "$(txt 15 | jq -r .timed_out)" == true ]] || fail "mcp wait: $(res 15)"
[[ "$(txt 16 | jq -r .layer)" == mission ]] || fail "widget_define: $(res 16)"
[[ "$(txt 17 | jq -r .layer)" == mission ]] || fail "config_undo: $(res 17)"
txt 18 | jq -e '.versions | type == "array"' >/dev/null || fail "config_history: $(res 18)"
[[ "$(txt 19 | jq -r .template)" == mcp-saved ]] || fail "template_save: $(res 19)"
[[ "$(txt 20)" == "# E2E build"* && "$(txt 20 | wc -c)" -le 301 ]] || fail "brief: $(res 20)"
[[ "$("$M" list | jq -r --arg m "$mcp_id" '.[] | select(.id == $m) | .session')" == ses_mcp ]] || fail "MCP session stamp"
ok "mcp: tools/list and every tool over stdio; unbound and refused calls are tool errors"

# Two-way: a page event (mission token, like the page sends it) wakes a waiting agent within a second.
tok="$(curl -fsS "$url" | grep -o 'token: "[^"]*"' | cut -d'"' -f2)"
page_event() { curl -s -o /dev/null -w '%{http_code}' "$base/api/missions/$id/events" -H 'content-type: application/json' "$@"; }
"$M" need N-9 "Proceed?" -o yes -o no >/dev/null
[[ "$(page_event -H "x-mission-token: nope" -d '{"kind":"answer","need":"N-9","answer":"yes"}')" == 401 ]] || fail "wrong page token"
"$M" wait --session ses_w --timeout 0 >/dev/null # catch the cursor up
"$M" wait --session ses_w --timeout 10 >"$tmp/w.json" & wp=$!
sleep 0.5
t0="$(date +%s%N)"
[[ "$(page_event -H "x-mission-token: $tok" -d '{"kind":"answer","need":"N-9","answer":"yes"}')" == 200 ]] || fail "page answer"
wait "$wp" || fail "wait failed"
ms=$(( ($(date +%s%N) - t0) / 1000000 ))
[[ "$(jq -r '.events[0].kind + " " + .events[0].data.answer' "$tmp/w.json")" == "answer yes" ]] || fail "wait got: $(cat "$tmp/w.json")"
(( ms < 1000 )) || fail "wait woke after ${ms}ms"
[[ "$("$M" view -c needs | jq -r '.records[] | select(.key=="N-9") | .state')" == answered ]] || fail "need not answered"
[[ "$("$M" wait --session ses_w --timeout 0 | jq '.events | length')" == 0 ]] || fail "cursor did not advance"
ok "page answer wakes mission-control wait in ${ms}ms; need answered; cursor advances; wrong token 401"

# MCP wait, and Claude Code channel push (`mcp --channel` pushes page events of the bound mission).
exec 8> >(MISSION_CONTROL_SESSION=ses_chan "$E" mcp --channel >"$tmp/c.out" 2>/dev/null)
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"e2e","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&8
for _ in $(seq 50); do jq -e 'select(.id==1)' "$tmp/c.out" >/dev/null 2>&1 && break; sleep 0.1; done
jq -se '.[0].result.capabilities.experimental["claude/channel"] != null' "$tmp/c.out" >/dev/null || fail "channel capability: $(cat "$tmp/c.out")"
sleep 1 # the push loop is waiting now
[[ "$(page_event -H "x-mission-token: $tok" -d '{"kind":"action","action":"customize","text":"hide spend"}')" == 200 ]] || fail "customize event"
for _ in $(seq 50); do grep -q 'notifications/claude/channel' "$tmp/c.out" && break; sleep 0.1; done
note="$(jq -c 'select(.method=="notifications/claude/channel") | .params' "$tmp/c.out" | head -1)"
[[ "$(jq -r .meta.kind <<<"$note")" == customize && "$(jq -r .content <<<"$note")" == *"hide spend"*"config_patch"* ]] \
  || fail "channel note: $note"
exec 8>&-
ok "mcp --channel advertises claude/channel and pushes page events"

# Import an old mission-control state.json: arrays → collections, activity → events (original times), run → run/run.
cat >"$tmp/state.json" <<'EOF'
{"updated": "2026-10-21T15:00:00Z",
 "run": {"title": "Old run", "health": "at_risk", "health_reason": "drift"},
 "packages": [{"id": "R0", "state": "merged", "wave": "W0"}, {"id": "R1", "state": "fixing", "wave": "W0"}],
 "tests": [{"pkg": "R0", "result": "pass"}, {"pkg": "R0", "result": "pass"}],
 "needs_you": [{"id": "N-1", "state": "open", "question": "Ship?"}],
 "activity": [{"at": "2026-10-21T15:05:00Z", "kind": "start", "text": "Run started"}]}
EOF
imp="$("$M" import "$tmp/state.json" --project "$tmp/proj")"
iid="$(jq -r .mission.id <<<"$imp")"
[[ "$(jq -r .mission.title <<<"$imp")" == "Old run" && "$(jq -c .imported <<<"$imp")" == '{"records":6,"events":1}' ]] || fail "import: $imp"
iv="$("$M" view -m "$iid")"
[[ "$(jq -r '[.records[] | select(.collection=="tests")] | length' <<<"$iv")" == 2 ]] || fail "tests rows kept apart"
[[ "$(jq -r '.records[] | select(.collection=="run") | "\(.state) \(.body.reason)"' <<<"$iv")" == "at_risk drift" ]] || fail "run record"
[[ "$(jq -r '.events[] | select(.kind=="start") | .at' <<<"$iv")" == 1792595100000 ]] || fail "original event time"
[[ "$(jq -r '.suggested_config.states.fixing' <<<"$imp")" == prog ]] || fail "suggested states: $imp"
ok "import: collections, run, needs, activity with original times, suggested states"

# Claude Code hooks: payloads on stdin fill agents, prs and spend of the project's newest mission, with no agent calls.
mkdir -p "$tmp/hookproj/.mission-control"
hm="$(env -u MISSION_CONTROL_ID "$M" new "Hooked" --project "$tmp/hookproj" --force | jq -r .mission.id)"
printf '%s\n' '{"type":"assistant","message":{"id":"m1","model":"claude-opus-5-5","usage":{"input_tokens":1000000,"output_tokens":0}}}' >"$tmp/transcript.jsonl"
hook() { printf '%s' "$1" | env -u MISSION_CONTROL_ID "$M" hook; }
hook "{\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Task\",\"tool_use_id\":\"tu_9\",\"session_id\":\"s1\",\"cwd\":\"$tmp/hookproj\",\"tool_input\":{\"description\":\"lint\"}}"
hook "{\"hook_event_name\":\"PostToolUse\",\"tool_name\":\"Bash\",\"session_id\":\"s1\",\"cwd\":\"$tmp/hookproj\",\"tool_input\":{\"command\":\"gh pr create --title fix\"},\"tool_response\":{\"stdout\":\"https://github.com/a/b/pull/7\"}}"
hook "{\"hook_event_name\":\"Stop\",\"session_id\":\"s1\",\"cwd\":\"$tmp/hookproj\",\"transcript_path\":\"$tmp/transcript.jsonl\"}"
hook 'not json' 2>/dev/null || fail "a bad payload must not fail the hook"
hv="$("$M" view -m "$hm")"
[[ "$(jq -r '[.records[] | "\(.collection)/\(.key)/\(.state)"] | sort | join(" ")' <<<"$hv")" == "agents/tu_9/running prs/7/open spend/s1-claude-opus-5-5/null" ]] \
  || fail "hooks: $(jq -c '[.records[] | [.collection, .key, .state]]' <<<"$hv")"
[[ "$(jq -r '.records[] | select(.collection=="spend") | .body.usd == 4' <<<"$hv")" == true ]] || fail "spend estimate"
"$M" hook --print-config | jq -e '.hooks.Stop[0].hooks[0].command | endswith("mission-control hook")' >/dev/null || fail "hook config"
ok "hooks: subagent, gh pr create, Stop spend → the project's mission; bad payloads never fail"

# Review: `mission-control review` registers the page with redline (a stand-in script here) and the page loads its overlay.
cat >"$tmp/fake-redline" <<'EOF'
#!/usr/bin/env bash
printf '%s' '{"artifact_id":"art_e2e","embed":"<!-- redline -->\n<script src=\"http://127.0.0.1:7465/client.js\" data-redline-artifact=\"art_e2e\" data-redline-token=\"tok123\"></script>"}'
EOF
chmod +x "$tmp/fake-redline"
rv="$(MISSION_CONTROL_REDLINE="$tmp/fake-redline" "$M" new "Reviewed" --review --project "$tmp/proj")"
rid="$(jq -r .mission.id <<<"$rv")"
[[ "$(jq -r .review.artifact_id <<<"$rv")" == art_e2e ]] || fail "new --review: $rv"
curl -fsS "$base/m/$rid" | grep -q 'src="http://127.0.0.1:7465/overlay.js"' || fail "overlay not embedded"
"$M" review -m "$rid" --off >/dev/null
! curl -fsS "$base/m/$rid" | grep -q 'redline' || fail "overlay still there after --off"
ok "review: redline registered, overlay on the page, --off removes it"

# Updates (ported from redline): the release check, notices for people and agents, dismissing them, `mission-control update`.
# A local "remote" stands in for GitHub: releases v0.0.1 and v99.0.0, plus a prerelease that must be ignored.
gitq() { git -c user.name=e2e -c user.email=e2e@example.com -c init.defaultBranch=main "$@" >/dev/null 2>&1; }
up="$tmp/up"
mkdir -p "$up/src/bin" "$up/src/scripts"
gitq init --bare "$up/remote.git"
gitq -C "$up/src" init
stub() { # version: bin/mission-control and its Windows twin print it
  printf '#!/usr/bin/env bash\necho "mission-control %s (stub)"\n' "$1" >"$up/src/bin/mission-control"; chmod +x "$up/src/bin/mission-control"
  printf 'Write-Output "mission-control %s (stub)"\n' "$1" >"$up/src/bin/mission-control.ps1"
}
stub 0.0.1
cp "$repo/scripts/checkout-release.sh" "$repo/scripts/checkout-release.ps1" "$up/src/scripts/"
gitq -C "$up/src" add -A; gitq -C "$up/src" commit -m one; gitq -C "$up/src" tag v0.0.1
stub 99.0.0
gitq -C "$up/src" commit -am two; gitq -C "$up/src" tag v99.0.0
gitq -C "$up/src" commit --allow-empty -m three; gitq -C "$up/src" tag v100.0.0-rc.1
gitq -C "$up/src" push --tags "$up/remote.git" HEAD:main
ucfg="$MISSION_CONTROL_CONFIG_DIR/config.toml"
u="$(MISSION_CONTROL_UPDATE_REMOTE="$up/remote.git" "$M" update check)"
[[ "$(jq -r '"\(.latest) \(.available) \(.notify)"' <<<"$u")" == "99.0.0 true true" ]] || fail "update check: $u"
[[ "$(jq -r .update.latest <<<"$("$M" new "Update notice" --project "$tmp/proj")")" == 99.0.0 ]] || fail "new should carry update"
[[ "$(curl -fsS "$base/api/update" | jq -r .notify)" == true ]] || fail "page update route"
[[ "$(curl -s -o /dev/null -w '%{http_code}' "$base/api/missions/$id/update/dismiss" -H 'content-type: application/json' -d '{"all":true}')" == 401 ]] \
  || fail "dismiss without the mission token"
ok "release check finds v99.0.0 (prerelease ignored); new and the page route report it"

[[ "$("$M" update skip | jq -r '"\(.skipped) \(.notify)"')" == "true false" ]] || fail "update skip"
grep -q 'skip = \["99.0.0"\]' "$ucfg" || fail "skip not in config.toml"
grep -q 'row_actions = \["rerun-ci"\]' "$ucfg" || fail "the user's own config in that file survived"
[[ "$(jq 'has("update")' <<<"$("$M" new "Skipped" --project "$tmp/proj")")" == false ]] || fail "skipped release still reported"
[[ "$("$M" update off | jq -r .check)" == false ]] && grep -q '^check = false' "$ucfg" || fail "update off"
[[ "$("$M" update on | jq -r .check)" == true ]] || fail "update on"
! "$M" update skip 1.2 >/dev/null 2>&1 || fail "update skip accepted a non-release version"
[[ "$("$M" config get 'update.check' 2>/dev/null)" == true ]] || fail "[update] is a known config key"
ok "update skip / off / on write the user's config.toml (keeping the rest); a skipped release is no longer reported"

# `mission-control update` on a clone at v0.0.1: checkout-release.sh moves it to v99.0.0, then bin/mission-control runs.
exe="$repo/target/release/mission-control$EXE"
gitq clone "$up/remote.git" "$up/clone"; gitq -C "$up/clone" checkout --detach v0.0.1
out="$(MISSION_CONTROL_REPO="$up/clone" "$exe" update 2>/dev/null)" || fail "mission-control update"
[[ "$(jq -r '"\(.updated) \(.to) \(.version)"' <<<"$out")" == "true 99.0.0 mission-control 99.0.0 (stub)" ]] || fail "update result: $out"
[[ "$(git -C "$up/clone" describe --tags)" == v99.0.0 ]] || fail "clone not on v99.0.0"
[[ "$(MISSION_CONTROL_REPO="$up/clone" "$exe" update 2>/dev/null | jq -r .updated)" == false ]] || fail "second update should be a no-op"
echo edit >>"$up/clone/bin/mission-control"
gitq -C "$up/clone" checkout --detach v0.0.1 2>/dev/null || true
! MISSION_CONTROL_REPO="$up/clone" "$exe" update >/dev/null 2>&1 || fail "update ran over local edits"
ok "mission-control update moves the clone to the latest release and refuses over local edits"

# Prebuilt binaries: without cargo (MISSION_CONTROL_PREBUILT=1), bin/mission-control downloads this version's release archive once,
# checks its sha256 and runs it. A local server stands in for GitHub Releases (packaged the way release.yml does).
if [[ -z "${win:-}" ]] && command -v python3 >/dev/null; then
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) triple=x86_64-unknown-linux-gnu ;; Linux-aarch64 | Linux-arm64) triple=aarch64-unknown-linux-gnu ;;
    Darwin-arm64) triple=aarch64-apple-darwin ;; Darwin-x86_64) triple=x86_64-apple-darwin ;;
  esac
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -1)"
  rel="$tmp/release" name="mission-control-$version-$triple"
  mkdir -p "$rel/$name" "$tmp/fakeclone/bin"
  cp "$repo/target/release/mission-control" "$rel/$name/"
  tar -C "$rel" -czf "$rel/$name.tar.gz" "$name"
  (cd "$rel" && { sha256sum "$name.tar.gz" 2>/dev/null || shasum -a 256 "$name.tar.gz"; } >"$name.tar.gz.sha256")
  cp "$repo/bin/mission-control" "$tmp/fakeclone/bin/"; cp "$repo/Cargo.toml" "$tmp/fakeclone/"
  rport="$((MISSION_CONTROL_PORT + 2))"
  (cd "$rel" && exec python3 -m http.server "$rport" --bind 127.0.0.1 >/dev/null 2>&1) & srv=$!
  for _ in $(seq 30); do curl -fs "http://127.0.0.1:$rport/" >/dev/null && break; sleep 0.1; done
  got="$(MISSION_CONTROL_PREBUILT=1 MISSION_CONTROL_RELEASE_BASE="http://127.0.0.1:$rport" "$tmp/fakeclone/bin/mission-control" --version 2>/dev/null)"
  [[ "$got" == "mission-control $version "* && -x "$tmp/fakeclone/target/prebuilt/mission-control" ]] || { kill "$srv"; fail "prebuilt download: $got"; }
  echo tampered >>"$rel/$name.tar.gz"; rm "$tmp/fakeclone/target/prebuilt/mission-control"
  if MISSION_CONTROL_PREBUILT=1 MISSION_CONTROL_RELEASE_BASE="http://127.0.0.1:$rport" "$tmp/fakeclone/bin/mission-control" --version >/dev/null 2>&1; then
    kill "$srv"; fail "a tampered archive was accepted"
  fi
  kill "$srv"
  ok "prebuilt: downloads the release archive, checks sha256 (refuses a tampered one), runs it"
fi

# Lifecycle: status → stop → auto-restart.
st="$("$M" status)"
[[ "$(jq -r .running <<<"$st")" == true && "$(jq -r .port <<<"$st")" == "$MISSION_CONTROL_PORT" ]] || fail "status: $st"
pid1="$(jq -r .pid <<<"$st")"
"$M" stop | jq -e ".stopped == $pid1" >/dev/null || fail "stop"
[[ ! -e "$MISSION_CONTROL_HOME/daemon.json" ]] || fail "daemon.json left behind after stop"
"$M" status | jq -e '.running == false' >/dev/null || fail "status after stop"
"$M" stop | jq -e '.stopped == null' >/dev/null || fail "second stop"
"$M" list >/dev/null
pid2="$(jq -r .pid "$MISSION_CONTROL_HOME/daemon.json")"
[[ "$pid2" != "$pid1" ]] || fail "no auto-restart"
[[ -s "$MISSION_CONTROL_HOME/daemon.log" ]] || fail "auto-started daemon logs to daemon.log"
ok "status, stop, auto-restart (logging to daemon.log)"

# Version handshake: a binary rebuilt after the daemon started replaces it on the next call.
sleep 1.1
touch "$repo/target/release/mission-control$EXE"
[[ "$("$M" view -m "$id" -c prs | jq '.records | length')" == 1 ]] || fail "records survive the restart"
pid3="$(jq -r .pid "$MISSION_CONTROL_HOME/daemon.json")"
[[ "$pid3" != "$pid2" ]] || fail "stale daemon not replaced after rebuild"
"$M" list >/dev/null
[[ "$(jq -r .pid "$MISSION_CONTROL_HOME/daemon.json")" == "$pid3" ]] || fail "fresh daemon restarted again"
ok "rebuilt binary replaces the stale daemon"

# A daemon whose state dir is deleted stops by itself (no orphans after cleanup).
orphan_home="$(mktemp -d)"
MISSION_CONTROL_HOME="$orphan_home" MISSION_CONTROL_PORT="$((MISSION_CONTROL_PORT + 1))" "$M" list >/dev/null
opid="$(jq -r .pid "$orphan_home/daemon.json")"
rm -rf "$orphan_home" 2>/dev/null || true # Windows: the open database stays until the daemon exits
for _ in $(seq 30); do alive "$opid" || break; sleep 1; done
! alive "$opid" || { kill_pid "$opid"; fail "daemon outlived its state dir"; }
rm -rf "$orphan_home"
ok "daemon stops when its state dir is deleted"

# Install into a sandbox HOME only (never the user's agent configs). Claude is left out: it needs the real
# `claude` CLI; the agent-loop test (tests/agent-loop.sh) covers it in its own sandbox.
if [[ -z "${win:-}" ]]; then # Windows links are junctions and .cmd shims, unit-tested in src/install/
  ih="$tmp/ihome"
  mkdir -p "$ih/.codex" "$ih/.cursor" "$ih/.config/opencode" "$ih/.gemini/config"
  printf '%s\n' '{"mcpServers":{"other":{"command":"x"}}}' >"$ih/.cursor/mcp.json"
  printf '%s\n' '# mine' '[mcp_servers.other]' 'command = "x"' >"$ih/.codex/config.toml"
  inst() { env -u CLAUDE_CONFIG_DIR -u CODEX_HOME HOME="$ih" USERPROFILE="$ih" "$M" "$@" --agents codex,cursor,opencode,agy,gemini --bin-dir "$ih/bin"; }
  plan="$(inst install --dry-run)"
  grep -q "skill $ih/.agents/skills/mission-control: would link" <<<"$plan" || fail "dry run plan: $plan"
  grep -q "would register" <<<"$plan" || fail "dry run mcp: $plan"
  [[ ! -e "$ih/.agents" && ! -e "$ih/bin" && ! -e "$ih/.cursor/mcp.json.mission-control.bak" ]] || fail "dry run wrote something"
  out="$(inst install)"
  [[ "$(readlink "$ih/.agents/skills/mission-control")" == "$repo" ]] || fail "skill link: $out"
  [[ "$(readlink "$ih/bin/mission-control")" == "$repo/bin/mission-control" ]] || fail "cli link: $out"
  [[ "$(readlink "$ih/bin/mctl")" == "$repo/bin/mission-control" ]] || fail "mctl alias: $out"
  [[ "$(jq -r '.mcpServers | keys | join(",")' "$ih/.cursor/mcp.json")" == "mission-control,other" ]] || fail "cursor mcp: $(cat "$ih/.cursor/mcp.json")"
  [[ "$(jq -r '.mcpServers["mission-control"].args[0]' "$ih/.cursor/mcp.json")" == mcp ]] || fail "cursor args"
  grep -q '^\[mcp_servers.mission-control\]' "$ih/.codex/config.toml" && grep -q '^# mine' "$ih/.codex/config.toml" || fail "codex toml"
  [[ "$(jq -r '.mcp["mission-control"].command[1]' "$ih/.config/opencode/opencode.json")" == mcp ]] || fail "opencode"
  [[ -f "$ih/.cursor/mcp.json.mission-control.bak" ]] || fail "backup"
  # Capture first: `cmd | grep -q` can SIGPIPE cmd, which pipefail turns into a failure.
  again="$(inst install)"; grep -q "already" <<<"$again" || fail "second install isn't idempotent: $again"
  doc="$(inst doctor)"; grep -q "mcp   ~/.cursor/mcp.json: ok" <<<"$doc" || fail "doctor: $doc"
  inst uninstall >/dev/null
  [[ ! -e "$ih/.agents/skills/mission-control" && ! -e "$ih/bin/mission-control" && ! -e "$ih/bin/mctl" ]] || fail "uninstall left links"
  [[ "$(jq -c . "$ih/.cursor/mcp.json")" == '{"mcpServers":{"other":{"command":"x"}}}' ]] || fail "uninstall: $(cat "$ih/.cursor/mcp.json")"
  grep -q 'mcp_servers.mission-control' "$ih/.codex/config.toml" && fail "codex entry left"
  ok "install (dry run, then a sandbox HOME): skill, CLI, MCP for codex/cursor/opencode/agy/gemini; idempotent; doctor; uninstall restores"

fi

echo "all e2e tests passed"
