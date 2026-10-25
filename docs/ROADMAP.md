# Implementation roadmap

This is the plan agents work from. **Take the first unchecked task in the lowest open phase,** unless the user names one. Follow [`.agent/workflows/implement-task.md`](../.agent/workflows/implement-task.md). When a task is done, tick it (`[x]`) and add a log entry. Add newly discovered work as unchecked tasks in the right phase.

Each task has an **ID** (for prompts and logs) and **Done when** criteria. A task isn't done until its criteria hold and `just check` is green.

Legend: 🔒 needs the user's explicit OK (it touches their machine, accounts or configs, or the public surface).

---
## Phase 1: Verification on real setups

- [ ] **P1.1 🔒 Clean-install test.** After the user installs mission-control with the README sentence, an agent runs [`.agent/prompts/test-clean-install.md`](../.agent/prompts/test-clean-install.md): static checks, `tests/agent-loop.sh` for every agent (including `LOOP_KIND=customize`), the browser checklist, cleanup.
  *Done when:* its report shows every installed agent passing (or failing only for a documented setup issue), and any problems it found are fixed or added here.
- [ ] **P1.2 Browser tests on macOS and Windows.** `ci.yml` runs `just browser` on all three platforms; only Linux has been seen green so far.
  *Done when:* CI is green on macOS and Windows, or each failure is fixed or recorded here.
- [ ] **P1.3 🔒 Windows on a real machine.** CI covers Windows (unit, e2e through `bin/mission-control.ps1`, browser) and releases build the x86_64 and arm64 zips; a real install hasn't been checked yet.
  *Done when:* an install on a real Windows machine, with and without cargo, passes `mission-control doctor` and a mission loop in one agent.

## Phase 2: Later

- [ ] **P2.1** Shared `skills-core` crate with redline for lifecycle, install and update (ported by copy today; see `DECISIONS.md`, "Open questions settled").
- [ ] **P2.2** Per-template `[states]` overrides, so the `research` template's `open` questions stop showing in the warn tone on home progress bars.
- [ ] **P2.3** redline has the same DNS-rebinding exposure mission-control fixed with a `Host` check (`DECISIONS.md`); port the fix there.

## Risks

| Risk | Mitigation |
|---|---|
| Config power makes pages break silently | Validation on every write; `doctor`; versioned undo; the page shows a config-issue banner with the offending key. |
| Script widgets as an XSS vector | Mission-layer HTML only in a sandboxed iframe with no token; script widgets only from files the user owns. |
| Localhost CSRF from other pages | Writes need a token in a custom header (forces a CORS preflight, which the daemon refuses). |
| Agent token cost of keeping the page current | Hooks, `batch`, `brief`, and SKILL.md guidance: update on state changes, not narration. |
| Copies of redline code drift | Port deliberately and note the source in the log; extracting a shared crate is P2.1. |
