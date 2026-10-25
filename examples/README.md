# Config examples

How the layers stack (see `docs/CONFIG.md`): builtin → template → user → project → mission.

| Folder | Layer | Copy to |
|---|---|---|
| `user-config/` | Your defaults for every mission | `~/.config/mission-control/` (or `$MISSION_CONTROL_CONFIG_DIR`) |
| `project/.mission-control/` | A repo's defaults, shared with the team | `<repo>/.mission-control/` |

Try them without touching your real config:

```sh
MISSION_CONTROL_CONFIG_DIR=examples/user-config bin/mission-control config resolve --project examples/project -t billing-build
```

`project/.mission-control/config.toml` shows one button of each kind (`ask_agent`, `set`, `open`, `copy`) on PR rows and a named widget; `user-config/config.toml` a `run` button (off until you set `permissions.run_actions`). Every template resolves cleanly with both (a unit test checks).

Per-mission changes don't live in files: the agent sends them with `mission-control config patch` (validated, versioned).
