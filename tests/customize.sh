#!/usr/bin/env bash
# Customization pass: everything CONFIG.md promises, layer by layer, checked in the resolved config and in
# `config explain`. Temp state and config dirs, random port. Run: `just e2e` (after e2e.sh) or directly.
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd)"
M="$repo/bin/mission-control"
tmp="$(mktemp -d)"
export MISSION_CONTROL_HOME="$tmp/state" MISSION_CONTROL_CONFIG_DIR="$tmp/user" MISSION_CONTROL_PORT="$((20000 + RANDOM % 20000))"
unset MISSION_CONTROL_ID MISSION_CONTROL_SESSION
trap 'kill "$(jq -r .pid "$MISSION_CONTROL_HOME/daemon.json" 2>/dev/null)" 2>/dev/null || true; rm -rf "$tmp"' EXIT
ok() { echo "ok - $*"; }
fail() { echo "FAIL - $*" >&2; exit 1; }
proj="$tmp/proj"
mkdir -p "$MISSION_CONTROL_CONFIG_DIR/templates" "$proj/.mission-control/templates"
ids() { "$M" view -c none | jq -r '[.config.sections[].id] | join(",")'; }
prov() { "$M" config explain "$1" | awk -v k="$1" -F '  ←  ' '$1 == k { print $2; exit }'; }
get() { "$M" config get "$1"; }

# Template layer: a user template on top of a built-in one (extends), and a project file patching it by name.
cat >"$MISSION_CONTROL_CONFIG_DIR/templates/standup.toml" <<'EOF'
name = "standup"
description = "Yesterday, today, blockers"
extends = "minimal"

[[sections]]
id = "blockers"
title = "Blockers"
widget = "table"
collection = "blockers"
after = "needs"

[[sections]]
id = "status"
remove = true
EOF
cat >"$proj/.mission-control/templates/standup.toml" <<'EOF'
[[sections]]
id = "today"
title = "Today"
widget = "table"
collection = "today"
before = "blockers"
EOF
export MISSION_CONTROL_ID="$("$M" new "Custom" -t standup --project "$proj" | jq -r .mission.id)"
[[ "$(ids)" == "needs,today,blockers,overview,recent,activity" ]] || fail "template chain: $(ids)"
[[ "$(prov 'sections[today].title')" == "project template ("* ]] || fail "project template provenance"
[[ "$(prov 'sections[blockers].title')" == "user template ("* ]] || fail "user template provenance"
[[ "$(prov 'sections[overview].widget')" == "builtin templates/minimal.toml" ]] || fail "builtin template provenance"
ok "template layer: extends a built-in; a same-named project file patches it; add, remove, move by id"

# User layer: patch an item from the template, move another, remove one, unset a builtin key.
"$M" config patch --layer user '{"sections":[{"id":"blockers","title":"Stuck"},{"id":"activity","after":"overview"},{"id":"today","remove":true}],"brief":{"max_chars":"!unset"}}' >/dev/null
[[ "$(ids)" == "needs,blockers,overview,activity,recent" ]] || fail "user layer: $(ids)"
[[ "$(get 'sections[blockers].title')" == '"Stuck"' && "$(prov 'sections[blockers].title')" == "user ("* ]] || fail "user patch"
[[ "$(get brief | jq 'has("max_chars")')" == false ]] || fail "!unset"
ok "user layer: patch, move (after), remove by id; !unset removes a builtin key"

# Project layer: bring a removed section back, move it, patch a column list by id.
"$M" config patch --layer project '{"sections":[{"id":"today","title":"Today","widget":"table","collection":"today","before":"needs","columns":[{"id":"key"},{"id":"owner"}]}]}' >/dev/null
"$M" config patch --layer project '{"sections":[{"id":"today","columns":[{"id":"owner","label":"Who"},{"id":"key","remove":true}]}]}' >/dev/null
[[ "$(ids)" == "today,needs,blockers,overview,activity,recent" ]] || fail "project layer: $(ids)"
[[ "$(get 'sections[today].columns' | jq -c .)" == '[{"id":"owner","label":"Who"}]' ]] || fail "columns by id: $(get 'sections[today].columns')"
grep -q 'label = "Who"' "$proj/.mission-control/config.toml" || fail "project file"
ok "project layer: re-add a removed section, move it (before), patch and remove columns by id"

# Mission layer: patch, remove, move; undo; history.
"$M" config patch '{"sections":[{"id":"needs","before":"overview"},{"id":"blockers","remove":true},{"id":"activity","title":"Log"}]}' --note "mine" >/dev/null
[[ "$(ids)" == "today,needs,overview,activity,recent" && "$(prov 'sections[activity].title')" == "mission v"* ]] || fail "mission: $(ids)"
"$M" config undo >/dev/null
[[ "$(ids)" == "today,needs,blockers,overview,activity,recent" ]] || fail "mission undo: $(ids)"
ok "mission layer: patch, remove, move; undo restores"

# replace = true: a user template that drops the built-in one instead of patching it.
cat >"$MISSION_CONTROL_CONFIG_DIR/templates/research.toml" <<'EOF'
replace = true
[[sections]]
id = "only"
title = "Only this"
widget = "markdown"
text = "Replaced"
EOF
r="$("$M" new "Replaced" -t research --project "$proj" | jq -r .mission.id)"
[[ "$("$M" view -m "$r" -c none | jq -r '[.config.sections[].id] | index("questions")')" == null ]] || fail "replace kept the built-in sections"
[[ "$("$M" view -m "$r" -c none | jq -r '[.config.sections[].id] | index("only")')" != null ]] || fail "replace"
ok "replace = true drops the lower template files"

# Agent side: dry run, widget_define, template_save and a new mission from it.
url="$("$M" config patch '{"theme":{"accent":"#2f6fdb"}}' --dry-run | jq -r .preview_url)"
[[ "$url" == *"?draft=drf_"* && "$( (get theme 2>/dev/null || echo "{}") | jq -r '.accent // "none"')" == none ]] || fail "dry run"
"$M" config widget open-blockers --base table --props '{"collection":"blockers","sort":"-updated"}' >/dev/null
"$M" config patch '{"sections":[{"id":"ob","title":"Open blockers","widget":"open-blockers"}]}' >/dev/null
"$M" config template-save standup-plus --layer project >/dev/null
n="$("$M" new "From saved" -t standup-plus --project "$proj" | jq -r .mission.id)"
[[ "$("$M" view -m "$n" -c none | jq -r '.config.sections[] | select(.id=="ob") | .widget')" == open-blockers ]] || fail "saved template"
ok "dry run; widget_define used by a section; template_save; new mission from it"

# Refusals: the mission layer can't run commands, load scripts or grant permissions; html widgets are fine.
for p in '{"actions":{"x":{"kind":"run","command":["id"]}}}' '{"widgets":{"w":{"script":"w.js"}}}' \
  '{"permissions":{"run_actions":true}}' '{"sections":[{"id":"x","widget":"table","columns":[{"id":"c","as":"glow"}]}]}'; do
  if "$M" config patch "$p" >/dev/null 2>&1; then fail "accepted: $p"; fi
done
"$M" config widget note-html --html '<b>hi</b>' >/dev/null || fail "html widget refused in the mission layer"
ok "mission layer refuses run, script, permissions and bad types; accepts an html widget"

echo "customization pass: all passed"
