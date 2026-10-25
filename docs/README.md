# mission-control developer docs

| Doc | What's in it |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Goal and pillars, core concepts, components, data model, config resolution, security |
| [CONFIG.md](CONFIG.md) | Config layers, merge rules, every key, built-in widgets, actions, validation |
| [PROTOCOL.md](PROTOCOL.md) | HTTP API, ops, MCP tools, CLI |
| [CODE_STYLE.md](CODE_STYLE.md) | Code patterns, module layout, config/error/async/persistence rules, tooling and lints |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Repo layout, build, `just` recipes, tests, releases, database migrations |
| [DECISIONS.md](DECISIONS.md) | Design and dependency decisions, with the alternatives considered |
| [ROADMAP.md](ROADMAP.md) | Implementation roadmap: phases, tasks, acceptance criteria, open questions, risks |

The user-facing README is [`../README.md`](../README.md). Agent-facing files:
- [`../SKILL.md`](../SKILL.md): how agents run and customize a mission.
- [`../INSTALL.md`](../INSTALL.md): the install steps an agent runs for the user.

The design tokens are in [`../.design/`](../.design/). Sample config layers are in [`../examples/`](../examples/README.md).

Coding agents working on this repo should start at [`../.agent/README.md`](../.agent/README.md).
