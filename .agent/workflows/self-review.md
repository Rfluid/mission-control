# Workflow: self-review before "done"

Go through the diff (`git diff`) with these questions:

- [ ] **Correctness.**
  - Error paths return useful messages (`.context(...)`).
  - Nothing panics on user input or on a malformed config file.
  - No `unwrap` outside tests.
- [ ] **Invariants** (`.agent/context.md`).
  - Loopback only.
  - Token scopes intact.
  - Nothing domain-specific hard-coded in the page or the daemon: it belongs in a template.
  - Provenance still recorded for every key the change can set.
  - Mission layer can't gain `run` actions or script widgets.
  - Mission config stored as ordered patches.
  - Batches are one transaction.
  - MCP tool ↔ CLI twin.
- [ ] **Async hygiene.**
  - No `std::sync::Mutex` guard held across `.await`.
  - Blocking I/O runs via `spawn_blocking`.
- [ ] **Security.**
  - New routes check auth.
  - Mission tokens can't reach other missions.
  - Page rendering escapes every record and config string; links are `http(s)`/`mailto` only; record values in link templates are URL-encoded.
  - Nothing logs tokens.
- [ ] **Tests.** New behaviour is covered: unit, e2e, or a manual browser check noted in the log. Every built-in template still resolves cleanly.
- [ ] **Style** (`docs/CODE_STYLE.md`).
  - Module header `//!` present.
  - Files under ~250 lines.
  - Constants for magic numbers.
  - Names say what, not how.
- [ ] **Docs** synced (table in `.agent/rules.md`).
  - README still user-facing.
  - PROTOCOL and CONFIG match the code.
- [ ] **`just check` green.**
