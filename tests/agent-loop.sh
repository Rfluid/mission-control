#!/usr/bin/env bash
# Cross-agent loop test (ported from redline's): a headless agent session creates a mission over its mission
# MCP server, asks a question in `needs`, waits; the harness answers it the way the page does (mission token);
# the agent records the answer. Passes once the answer is back in the mission.
#
#   tests/agent-loop.sh claude                 # against the INSTALLED mission (agent's own MCP config)
#   tests/agent-loop.sh --sandbox claude       # temp state and config dirs; the MCP server passed by flag
#   tests/agent-loop.sh claude codex gemini    # several at once, sharing one daemon
#
# Agents: claude, codex, opencode, agy, cursor-agent, gemini (--sandbox: claude only).
# Env: MISSION (CLI, default `mission-control` on PATH), LOOP_TIMEOUT (s, default 300), CODEX_MODEL, KEEP_LOGS=1,
#      LOOP_KIND=customize (the user asks for a page change instead of answering; the agent must config_patch).
# Needs: jq, curl. Read-only for agent configs: it only starts agent sessions and talks to the daemon.
set -uo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"

sandbox=""
if [[ "${1:-}" == --sandbox ]]; then sandbox=1; shift; fi
if (($# == 0)); then
  sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'
  exit 2
fi

if [[ -n $sandbox && -z "${MISSION_CONTROL_LOOP_SANDBOXED:-}" ]]; then
  sb="$(mktemp -d)"
  export MISSION_CONTROL_HOME="$sb/home" MISSION_CONTROL_CONFIG_DIR="$sb/config" MISSION_CONTROL_PORT="$((20000 + RANDOM % 20000))"
  export MISSION="${MISSION:-$repo/bin/mission-control}" MISSION_CONTROL_LOOP_SANDBOXED=1
  mkdir -p "$MISSION_CONTROL_CONFIG_DIR"
  trap 'kill "$(jq -r .pid "$MISSION_CONTROL_HOME/daemon.json" 2>/dev/null)" 2>/dev/null; rm -rf "$sb"' EXIT
fi

if (($# > 1)); then
  pids=() fails=0
  for a in "$@"; do "$0" ${sandbox:+--sandbox} "$a" & pids+=($!); sleep 1; done
  for p in "${pids[@]}"; do wait "$p" || fails=$((fails + 1)); done
  echo "== $(($# - fails))/$# passed"
  exit $((fails > 0))
fi

agent=$1
M=${MISSION:-mission}
limit=${LOOP_TIMEOUT:-300}
tag="loop-$agent-$(date +%s)-$$"
work="$(mktemp -d)"
out="$work/$agent.log"
command -v "$M" >/dev/null || { echo "FAIL $agent: mission-control CLI not found (set MISSION=<clone>/bin/mission-control)"; exit 1; }
unset MISSION_CONTROL_ID MISSION_CONTROL_SESSION

prompt="This is an automated test of the mission-control MCP server. Use the mission-control MCP tools directly (not the shell, not the mission-control CLI).
1. Call mission_new with title \"$tag\" and template \"minimal\".
2. Call batch with one op: upsert into collection \"needs\", key \"Q1\", state \"open\", patch {\"question\": \"Pick a color\", \"options\": [\"red\", \"blue\"]}.
3. Call wait with timeout_s 240. If it returns no events, call it once more.
4. When you get the answer event, call batch with: a log op with text \"got <answer>\", and an upsert into collection \"notes\", key \"result\", patch {\"text\": \"ack-$agent: <answer>\"}.
5. Then stop and print the mission id. Do not create or edit any files."

kind=${LOOP_KIND:-answer}
if [[ $kind == customize ]]; then
  prompt="This is an automated test of the mission-control MCP server. Use the mission-control MCP tools directly (not the shell, not the mission-control CLI).
1. Call mission_new with title \"$tag\" and template \"minimal\".
2. Call batch with one op: upsert into collection \"needs\", key \"Q1\", state \"open\", patch {\"question\": \"Anything to change on the page?\"}.
3. Call wait with timeout_s 240. If it returns no events, call it once more.
4. You will get a customize event: the user asks to change the page. Make that change with config_patch on the mission layer (sections merge by id).
5. Then stop and print the mission id. Do not create or edit any files."
fi

before_serve="$(pgrep -f 'opencode serve' 2>/dev/null | sort)"
cd "$work" || exit 1
if [[ -n $sandbox ]]; then
  [[ $agent == claude ]] || { echo "FAIL $agent: --sandbox supports claude only"; exit 2; }
  jq -n --arg cmd "$M" --arg home "$MISSION_CONTROL_HOME" --arg cfg "$MISSION_CONTROL_CONFIG_DIR" --arg port "$MISSION_CONTROL_PORT" \
    '{mcpServers: {"mission-control": {command: $cmd, args: ["mcp"], env: {MISSION_CONTROL_HOME: $home, MISSION_CONTROL_CONFIG_DIR: $cfg, MISSION_CONTROL_PORT: $port}}}}' \
    >"$work/mcp.json"
  claude -p "$prompt" --mcp-config "$work/mcp.json" --strict-mcp-config --allowedTools "mcp__mission-control" </dev/null >"$out" 2>&1 &
else
  case $agent in
    claude) claude -p "$prompt" --allowedTools "mcp__mission-control" </dev/null >"$out" 2>&1 & ;;
    codex) codex exec --skip-git-repo-check ${CODEX_MODEL:+-m "$CODEX_MODEL"} \
      -c 'mcp_servers.mission-control.default_tools_approval_mode="approve"' "$prompt" >"$out" 2>&1 & ;;
    opencode) opencode run --auto "$prompt" >"$out" 2>&1 & ;;
    agy) agy --dangerously-skip-permissions -p "$prompt" >"$out" 2>&1 & ;; # flags before -p
    cursor-agent) cursor-agent -p --force "$prompt" >"$out" 2>&1 & ;;
    gemini) gemini -y -p "$prompt" >"$out" 2>&1 & ;;
    *) echo "FAIL $agent: unknown agent (claude, codex, opencode, agy, cursor-agent, gemini)"; exit 2 ;;
  esac
fi
pid=$!

finish() { # code message
  kill "$pid" 2>/dev/null
  if [[ $agent == opencode ]]; then # only this test may: in a parallel run the others' serve is opencode's
    for p in $(comm -13 <(echo "$before_serve") <(pgrep -f 'opencode serve' 2>/dev/null | sort)); do kill "$p" 2>/dev/null; done
  fi
  echo "$2"
  if (($1 != 0)); then echo "--- last lines of $out"; tail -25 "$out"; fi
  [[ "${KEEP_LOGS:-}" == 1 || $1 != 0 ]] || rm -rf "$work"
  exit "$1"
}
alive() { kill -0 "$pid" 2>/dev/null; }
deadline=$((SECONDS + limit))

id=""
while ((SECONDS < deadline)) && [[ -z $id ]]; do
  id="$("$M" list 2>/dev/null | jq -r --arg t "$tag" 'first(.[] | select(.title == $t) | .id) // empty' 2>/dev/null)"
  [[ -n $id ]] || { alive || finish 1 "FAIL $agent: exited before mission_new"; sleep 2; }
done
[[ -n $id ]] || finish 1 "FAIL $agent: never called mission_new"

asked=""
while ((SECONDS < deadline)) && [[ -z $asked ]]; do
  asked="$("$M" view -m "$id" -c needs 2>/dev/null | jq -r 'first(.records[] | select(.key == "Q1") | .key) // empty')"
  [[ -n $asked ]] || { alive || finish 1 "FAIL $agent: exited before asking"; sleep 2; }
done
[[ -n $asked ]] || finish 1 "FAIL $agent: never asked Q1"

# Answer the way the page does: the mission token from the page, in X-Mission-Token.
port="$("$M" status | jq -r .port)"
tok="$(curl -fsS "http://127.0.0.1:$port/m/$id" | grep -o 'token: "[^"]*"' | cut -d'"' -f2)"
if [[ $kind == customize ]]; then
  ev='{"kind":"action","action":"customize","text":"Rename the Status section to Notes"}'
  want="Notes"
  result() { "$M" view -m "$id" -c none 2>/dev/null | jq -r 'first(.config.sections[] | select(.id == "status") | .title) // empty'; }
else
  ev='{"kind":"answer","need":"Q1","answer":"blue"}'
  want="ack-$agent: blue"
  result() { "$M" view -m "$id" -c notes 2>/dev/null | jq -r 'first(.records[] | select(.key == "result") | .body.text) // empty'; }
fi
curl -fsS -X POST "http://127.0.0.1:$port/api/missions/$id/events" -H "X-Mission-Token: $tok" \
  -H 'Content-Type: application/json' -d "$ev" >/dev/null || finish 1 "FAIL $agent: page event rejected"

while ((SECONDS < deadline)); do
  got="$(result)"
  [[ $got == "$want" ]] && finish 0 "PASS $agent ($kind): $got"
  if ! alive; then # the write can land just as the agent exits: look once more
    sleep 3
    got="$(result)"
    [[ $got == "$want" ]] && finish 0 "PASS $agent ($kind): $got"
    finish 1 "FAIL $agent: exited without the result (got: ${got:-nothing})"
  fi
  sleep 2
done
finish 1 "FAIL $agent: no answer recorded within ${limit}s"
