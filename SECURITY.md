# Security policy

## Supported versions

Only the latest release gets fixes. `mission-control update` moves you to it; `mission-control --version` shows what you run.

## Reporting a vulnerability

Report privately through GitHub: **[Security → Report a vulnerability](https://github.com/Rfluid/mission-control/security/advisories/new)**. Don't open a public issue.

Include the version, what an attacker can do, and steps to reproduce. You'll get an answer within a week. Once a fix ships, the advisory is published with credit to you, unless you'd rather stay anonymous.

## What counts

mission's security model is in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). In short: the daemon binds `127.0.0.1` only; an admin token speaks as the agent, and each mission token speaks as the user on that mission only. Config from the mission layer (written by the agent and the page) can't define shell actions or script widgets. Examples of what we want to hear about:

- Reaching the daemon from another machine, or from a web page under another host name (DNS rebinding: the daemon answers only `127.0.0.1` and `localhost`), or writing to it from a web page without the mission's token.
- A mission token reading or writing another mission, or acting as the agent.
- Script injection through record values, events, link templates or config rendered on the page.
- A `run` action or a script widget getting in through the mission layer, or a `run` action executing without `permissions.run_actions = true` and confirmation.
- A `[theme]` or other config value making the page load something from outside the machine.
- `mission-control install` writing outside the agent config it targets, or clobbering user files.

Answers and button clicks reaching the agent is expected: they are untrusted input to the agent by design, and `SKILL.md` tells agents to treat them that way. Report it if mission-control makes that worse (for example, by presenting a page click as an agent instruction).
