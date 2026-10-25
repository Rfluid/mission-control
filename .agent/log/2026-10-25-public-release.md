# 2026-10-25 · public-release

**Agent:** claude  **Roadmap:** none (release prep)

## Done
- Squashed the repo into one first commit for the public release: version 0.1.0, one `CHANGELOG.md` entry, ROADMAP reduced to open work (renumbered P1.1–P2.3).
- Dropped earlier session logs and the one-shot `full-build` and `design-v2-build` prompts; their lasting gotchas are in `workflows/implement-task.md`.
- Removed build-phase markers (M0–M8) from docs and code comments, the private-repo notes in `INSTALL.md` and `README.md`, and the link to the private design canvas in `.design/README.md`.
- Replaced the internal example run with a neutral one (`Billing export build`, `acme/shop`, template `billing-build`) in `examples/`, `docs/CONFIG.md`, `docs/PROTOCOL.md`, tests and the MCP tool description.
- Repo made public; tag `v0.1.0` (the old private `v0.1.0` tag was deleted first); `release.yml` builds and publishes.

## Verified
- `just check` green on the release commit.

## Open / next
- ROADMAP P1.1 (clean-install test), P1.2 (browser CI on macOS/Windows), P1.3 (Windows on a real machine).
- Dependabot `ureq` 3 (PR #1) needs code changes in `src/api.rs` and `src/lifecycle.rs`.
- redline's `INSTALL.md` and `bin/redline` still say its repo is private.
