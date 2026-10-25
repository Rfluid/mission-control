# Protocol

Everything here is served by `mission-control serve` at `http://127.0.0.1:7466`. The port can be overridden with `MISSION_CONTROL_PORT` or `--port`.

## Auth

| Token | Where it comes from | Sent as | Scope |
|---|---|---|---|
| admin | `$MISSION_CONTROL_HOME/admin.token` (mode 0600) | `Authorization: Bearer <token>` | every write |
| mission | created with the mission, embedded in its page | `X-Mission-Token: <token>` | page events for that mission, nothing else |

Reads need no token. The daemon sends no CORS headers, so other origins can neither read responses nor send the custom header. It answers only requests whose `Host` is `127.0.0.1:<port>` or `localhost:<port>` (else 421), which stops DNS rebinding.

## HTTP API

| Method | Path | Token | Body / query | Returns |
|---|---|---|---|---|
| GET | `/health` | none | | `{name: "mission", version, pid, started}` (`started`: Unix ms; clients restart an older or pre-rebuild daemon) |
| GET | `/` | none | | Missions home |
| GET | `/m/{id}`, `/m/{id}/{page}` | none | | Mission page (carries the mission token), open on Overview or on `page` (an unknown page shows Overview) |
| GET | `/assets/{file}` | none | | Page assets: `app.js`, `actions.js`, `markdown.js`, `glyphs.js`, `widgets*.js`, `style.css`, `shell.css`, `widgets.css`, `favicon.svg` (`Cache-Control: no-cache`) |
| GET | `/assets/vendor/{file}` | none | | `mermaid-11.12.3.js` (sent gzipped: `Content-Encoding: gzip`; cached for a year) and `LICENSE-mermaid.txt`. The page loads the renderer only when markdown holds a ` ```mermaid ` block |
| GET | `/assets/fonts/{file}` | none | | Bundled fonts (`inter-v20-latin.woff2`, `-latin-ext`, `jetbrains-mono-v24-…`) and their licenses (`OFL-inter.txt`, `OFL-jetbrains-mono.txt`); cached for a year |
| GET | `/m/{id}/widgets/{name}.js` | none | | A script widget's file, from the user or project `widgets/` dir that set its `script` key; 403 otherwise |
| GET | `/api/missions` | none | `?archived=true` (include archived), `?summary=true` | `Mission[]`, newest first; with `summary`: `{open_needs, records, last_activity, health, health_tone, reason, template, needs: [{key, question, blocks, since}], progress: {kind: "phases" \| "share", segments: [tone \| "done" \| "now" \| ""], caption} \| null, stats: [{label, value, tone?}]}` on each, computed from the mission's resolved config (health from `[strip] health`, progress from its first `plan` section or else its first non-needs `[brief]` collection, stats from its strip cells): everything the missions home draws |
| POST | `/api/missions` | admin | `{title, session?, project_dir?, template?, force?}` | `{mission: Mission, url}`; 400 for an unknown template; 409 if `session` already has a live mission with this `title` (unless `force`) |
| GET | `/api/missions/{id}` | none | `?draft=` (a dry run's draft, kept an hour), `events=N` (default 100, ≤ 500), `config=false` (leave the resolved config out; `config` is `null`), `page=P` (only that page's sections in `config.sections`, and only records of the collections they read, plus `needs`; 404 for an unknown page) | `View` |
| GET | `/api/missions/{id}/brief` | none | `?session=S&max_chars=N` | `{text, chars, truncated}`: open needs, `S`'s unread page events (peeked, not consumed), `[brief].collections` by state, newest first |
| GET | `/api/missions/{id}/events` | none | `?before=SEQ&limit=N` (≤ 500); or `?collection=C&key=K&limit=N` | `Event[]` older than `SEQ`, oldest first (feed paging); with `collection` + `key`: the events about that record (its `state` changes, answers to it, buttons on its row), oldest first (the drawer's history) |
| GET | `/api/update` | none | | `UpdateInfo`: `{current, latest, available, notify, check, skipped, checked, command, notes}` (the daemon checks the release tags at most daily) |
| POST | `/api/missions/{id}/update/dismiss` | mission | `{version}` or `{all: true}` | `UpdateInfo`: skip that release, or turn checks and notices off |
| GET | `/api/doctor` | none | | Per live mission: `issues`, `big_collections`, `empty_sections`, `unused_actions`, `idle`, `records` |
| POST | `/api/missions/{id}/archive` | admin | `{archived?}` (default `true`; `false` restores) | `Mission` |
| GET | `/api/missions/{id}/export` | none | | The static snapshot (`text/html`) |
| POST | `/api/missions/{id}/fork` | admin | `{title, records?}` (default `true`) | `{mission, url}`: same project and config versions, records copied, fresh history |
| POST | `/api/missions/{id}/attach` | admin | `{session?}` | `{mission: Mission, url}`: binds the mission to `session`; records and history stay |
| POST | `/api/missions/{id}/ops` | admin | `Op[]` | `{applied}`; one transaction, all or nothing |
| GET | `/api/missions/{id}/config` | none | | `{config, provenance, template, issues}` |
| POST | `/api/missions/{id}/config` | admin | `{patch, note?, layer?: "mission"\|"user"\|"project", dry_run?}` | mission: `{layer, version, issues}`; files: `{layer, file, changed, issues}`; `dry_run`: `{dry_run, draft, preview_url, config, issues, file_text}`. 400 with the error issues if the result is invalid |
| POST | `/api/missions/{id}/config/undo` | admin | `{layer?}` | Drops the newest mission version, or restores the file's previous content; 400 when there's nothing to undo |
| GET | `/api/missions/{id}/config/history` | none | `?layer=` | Newest first: mission `{versions: [{version, note, at, patch}]}`, files `{file, versions: [{at, note}]}` |
| POST | `/api/missions/{id}/templates` | admin | `{name, layer?: "user"\|"project", description?, replace?}` | `{template, file, extends}`: the mission's patches composed into `templates/<name>.toml`; 409 if it exists |
| POST | `/api/missions/{id}/events` | mission | `PageEvent` (≤ 16 KiB; text ≤ 8 KiB; 60 per minute per mission; answers only to `needs` or a collection a `needs` section shows) | `{ok, seq, exit}` (`exit`: a `run` action's exit code); 401 wrong token, 403 a `run` action the mission layer touched, 404 unknown need/row, 400 unknown or page-side action or `run` off, 413, 429 |
| GET | `/api/missions/{id}/stream` | none | | Server-sent events: `change` with `Change`; `reset` when the page fell behind (re-read the view) |
| GET | `/api/missions/{id}/wait` | admin | `?session=S&since=N&timeout=T` (s, ≤ 540) | `{events, next, timed_out}`: page events (`answer`, `action`, `customize`) after the cursor; advances `S`'s cursor |

- **Errors:** a plain-text message with status 400 (bad input, invalid config), 401 (no admin token), 404 (unknown mission), 409 (duplicate title in a session), 422 (a JSON body of the wrong shape) or 500.
- **Config patches** follow the merge rules in [CONFIG.md](CONFIG.md). Each accepted patch becomes the next mission-control config version and logs a `config` event.

## Daemon lifecycle

- The first agent command starts the daemon (`mission-control serve` in its own process group, stderr to `$MISSION_CONTROL_HOME/daemon.log`) and finds it again through `$MISSION_CONTROL_HOME/daemon.json` (`{port, pid, version, started}`).
- A client replaces a daemon that is an older version, or the same version started before the client's binary was last rebuilt. A newer daemon is left alone.
- The daemon stops on SIGTERM / Ctrl-C, and by itself within ~10 s once `daemon.json` no longer names it (state dir deleted, or another daemon took over). It removes `daemon.json` only if it still names it.

## Page events and `wait`

- The page posts `PageEvent`s. An action is named by id; the daemon looks it up in the resolved config and does what the config says, so a page can't invent behaviour. `open` and `copy` never reach the daemon (400 if posted).
- Every page event is logged as an event of kind `answer`, `action` or `customize`, with the details in `data`. `wait` returns only those kinds.
- **Cursor:** each `(mission, session)` has one. `wait` without `since` continues it and moves it past what it returns. A session new to a mission starts where the furthest other session stopped (a restarted agent picks up what's new, not old history). An explicit `since` replays.
- `mcp --channel` runs the same wait for the bound mission and pushes each event as a Claude Code channel message, sharing the session's cursor (each event is delivered once).

## Types

```jsonc
// Mission
{ "id": "msn_…", "title": "Billing export build", "session": null, "project_dir": "/repo",
  "created": 1761236000000, "archived": false }
// Record
{ "collection": "prs", "key": "17777", "state": "open",
  "body": { "title": "feat: contracts", "checks": "green" }, "updated": 1761236000000 }
// Event
{ "seq": 42, "mission": "msn_…", "kind": "log", "text": "Run started", "data": null, "at": 1761236000000 }
// View
{ "mission": Mission, "template": "multi-agent-build",
  "config": { …resolved… }, "issues": [Issue], "records": [Record],
  "events": [Event], "event_count": 120 }   // newest 100 events, oldest first; event_count: all of them
// PageEvent (POST /events)
{ "kind": "answer", "need": "N-1", "collection": "needs", "answer": "later" }            // collection optional
{ "kind": "action", "action": "rerun-ci", "collection": "prs", "key": "17", "text": "…" } // row and text optional
// Change (SSE `change` data)
{ "mission": "msn_…", "records": [Record], "removed": [{"collection": "prs", "key": "9"}], "events": [Event],
  "config": false }   // config: true → re-read the view
// Issue
{ "severity": "error" | "warning", "path": "sections[prs].widget", "message": "unknown widget `x`" }
```

Provenance is a map from resolved key path (`mission.template`, `sections[prs].columns[state].as`) to the label of the layer that set it (`builtin`, `builtin templates/minimal.toml`, `user (<path>)`, `project (<path>)`, `mission v3`).

## Ops

```jsonc
{ "op": "upsert", "collection": "prs", "key": "17777", "state": "open", "patch": { "title": "…" } }
{ "op": "remove", "collection": "prs", "key": "17777" }
{ "op": "log", "kind": "log", "text": "Run started", "data": null, "at": 1761236000000 }   // at: optional, default now
```

- `upsert` creates the record if needed and applies `patch` as an RFC 7386 merge patch to its body (`null` deletes a field). `state` is kept when omitted.
- `log` appends an event; `kind` defaults to `log`.
- The daemon logs two kinds itself, in the same transaction: `state` when an upsert changes an existing record's state (text `"S1 Storage adapter → blocked"`, data `{collection, key, from, to}`), and `need` when a `needs` record opens (text `"N-3 opened: <question>"`). New records and answered needs don't log.
- A batch applies in order, in one transaction: if any op fails (400), none applied.
- `collection` and `key` are 1 to 200 bytes. A body that isn't an array of known ops is 422.

## MCP tools

`mission-control mcp` serves these on stdio, one server per agent session. The session id is `$MISSION_CONTROL_SESSION` (else a new `ses_…`); the session binds one mission (`mission_new`, `mission_attach`, or `$MISSION_CONTROL_ID` at start) and tools default to it. A failed call is an `isError` result whose text says what to do next.

| Tool | Args | CLI twin |
|---|---|---|
| `mission_new` | `title, template?, project_dir?, force?` | `mission-control new` |
| `mission_attach` | `id` | `mission-control attach` |
| `mission_list` | | `mission-control list` |
| `batch` | `ops: Op[], mission?` | `mission-control batch` (and `upsert`, `rm`, `log`, `need`) |
| `view` | `collections?, events? (default 20), config? (default false), page?, mission?` | `mission-control view [-c …] [--events N] [--no-config] [--page P]` |
| `config_get` | `path?, mission?` | `mission-control config get` |
| `config_explain` | `prefix?, mission?` | `mission-control config explain` |
| `config_patch` | `patch (object or TOML text), note?, layer?, dry_run?, mission?` | `mission-control config patch [--layer] [--dry-run]` |
| `config_undo` | `layer?, mission?` | `mission-control config undo [--layer]` |
| `config_history` | `layer?, mission?` | `mission-control config history [--layer]` |
| `template_save` | `name, layer?, description?, replace?, mission?` | `mission-control config template-save` |
| `widget_define` | `name, base? + props? \| html?, description?, layer?, mission?` | `mission-control config widget` |
| `template_list` | `project_dir?` | `mission-control config templates` |
| `config_schema` | | `mission-control schema` |
| `wait` | `timeout_s?, since?, mission?` | `mission-control wait` |
| `brief` | `max_chars?, mission?` | `mission-control brief` |

Key paths (`config_get`, `config explain`) use the provenance syntax: `mission.template`, `sections[prs].columns[state].as`.

## CLI

Output is JSON on stdout (except `config explain`, which prints `path ← layer` lines and issues). `-m/--mission` defaults to `$MISSION_CONTROL_ID`.

| Command | Does |
|---|---|
| `mission-control serve [--port P]` | Run the daemon in the foreground |
| `mission-control status` | `{running, port, pid, version, started}`; never starts a daemon |
| `mission-control stop` | `{stopped: pid \| null}`; SIGTERM, then SIGKILL (Windows: `taskkill /F`). The next command starts a fresh one |
| `mission-control mcp [--channel]` | MCP server on stdio; `--channel` also pushes page events to Claude Code |
| `mission-control wait -m ID [--session S] [--since N] [--timeout T]` | Block until page events; `--session` defaults to `$MISSION_CONTROL_SESSION`, else `default`. Rides through a daemon restart |
| `mission-control new <title> [-t template] [--session S] [--project DIR] [--force]` | Create a mission; `--project` defaults to the nearest dir with `.mission-control/` (else the git root); `--session` defaults to `$MISSION_CONTROL_SESSION` |
| `mission-control attach <id> [--session S]` | Bind a mission to this session |
| `mission-control review -m ID [--off] [--redline CMD]` (and `mission-control new --review`) | Register the page with redline (`$MISSION_CONTROL_REDLINE`, default `redline`), store its artifact in `_review/redline`, set `[mission] review = true`: the page loads redline's overlay (only from a `http://127.0.0.1:<port>` base) |
| `mission-control list [--all]` | Missions (`--all`: archived too, with summaries) |
| `mission-control archive -m ID [--restore]` | Archive (or restore) a mission; nothing is deleted |
| `mission-control fork -m ID <title> [--no-records]` | A new mission from this one |
| `mission-control view -m ID [-c collection …] [--page P]` | The `View`, optionally only some collections' records, or one page's worth |
| `mission-control upsert -m ID <collection> <key> [-s state] [field=value…]` | One upsert; a value that parses as JSON is stored as JSON |
| `mission-control rm -m ID <collection> <key>` | One remove |
| `mission-control log -m ID <text> [-k kind]` | One log line |
| `mission-control need -m ID <key> <question> [-o option…] [--context] [--recommendation] [--why] [--blocks KEY…]` | Upsert into `needs` with state `open` |
| `mission-control batch -m ID <file \| ->` | Apply a JSON array of ops |
| `mission-control config resolve [-t template] [--project DIR]` | Resolve locally without a mission (no daemon) |
| `mission-control install [--agents a,b] [--dry-run] [--channel] [--no-mcp] [--force] [--bin-dir D] [--skill-dir D] [--claude-config-dir D]` | Link the skill and CLI, register the MCP server per agent (configs backed up once to `<file>.mission-control.bak`) |
| `mission-control uninstall` / `mission-control doctor` (same flags) | Undo `install` / check skill, CLI and MCP per detected agent, then every live mission's health |
| `mission-control config get -m ID [path]` | The resolved config, or the part at a key path |
| `mission-control config explain -m ID [prefix]` | Provenance and issues |
| `mission-control config patch -m ID <toml or json> [--note N] [--layer mission\|user\|project] [--dry-run]` | Patch a layer (files keep their comments; backed up once to `<file>.mission-control.bak`) |
| `mission-control config undo -m ID [--layer L]` / `mission-control config history -m ID [--layer L]` | Undo the last change / list changes |
| `mission-control config template-save -m ID <name> [--layer user\|project] [--description D] [--replace]` | Save the mission's customizations as a template |
| `mission-control config widget -m ID <name> (--base B [--props JSON] \| --html H) [--description D] [--layer L]` | Define a named widget |
| `mission-control config templates [--project DIR]` | Templates and the files that define each |
| `mission-control schema` | The config's JSON Schema |
| `mission-control hook` / `mission-control hook --print-config` | For Claude Code hooks: reads the hook JSON on stdin and records subagents (`agents`), `gh pr create/merge` (`prs`) and transcript token usage (`spend`) per `[ingest]`; the mission is `$MISSION_CONTROL_ID`, else the one whose `session` is the hook's `session_id`, else the newest in the hook's project. Always exits 0. `--print-config` prints the `hooks` block for `settings.json` |
| `mission-control update` / `mission-control update check\|skip [V]\|off\|on` | Move this clone to the latest release tag and rebuild (refuses over local changes) / check now, skip a release, turn notices off or on (`[update]` in the user's `config.toml`). `new` and `mission_new` carry `update` + `update_hint` while a notice is due |
| `mission-control import <state.json> [--title T] [-t template] [--project DIR] [--write-config]` | A new mission from an old mission-control `state.json`: arrays → collections, `activity` → events (original times), `run` → `run/run`; prints `suggested_config` (`[states]` for unknown states; `--write-config` writes it to the project layer) |
| `mission-control brief -m ID [--session S] [--max-chars N]` | The `brief` text |
| `mission-control export -m ID [-o file.html]` | A static snapshot: one HTML file, assets inlined, the view embedded, no token, buttons inert |
