# 2026-10-25 · public-release

**Agent:** claude  **Roadmap:** none (release prep)

## Done
- Squashed the repo into one first commit for the public release: version 0.1.0, one `CHANGELOG.md` entry, ROADMAP reduced to open work (renumbered P1.1–P2.4).
- Dropped earlier session logs and the one-shot `full-build` and `design-v2-build` prompts; their lasting gotchas are in `workflows/implement-task.md`.
- Removed build-phase markers (M0–M8) from docs and code comments, the private-repo notes in `INSTALL.md` and `README.md`, and the link to the private design canvas in `.design/README.md`.
- Replaced the internal example run with a neutral one (`Billing export build`, `acme/shop`, template `billing-build`) in `examples/`, `docs/CONFIG.md`, `docs/PROTOCOL.md`, tests and the MCP tool description.
- The old private repo still held the pre-squash history in PR refs, so it was renamed to `mission-control-private-history` and archived (private); a fresh public `Rfluid/mission-control` got the one commit, the repo settings (rebase merge only, delete branch on merge, no wiki), the `main` ruleset from `.github/rulesets/main.json`, private vulnerability reporting and Dependabot alerts.
- First real CI run on Windows (the private repo's runs never started) found, fixed in the same commit:
  - `batch -` breaks under `powershell -File` (a bare `-` is a parameter with no name): `batch` with no file now reads stdin; docs and e2e use that.
  - `bin/mission-control.ps1` split arguments holding both whitespace and quotes on Windows PowerShell 5.1 (it counts `\"` as quotes); `Protect-Arg` quotes those itself and writes inner quotes as `""`.
  - `mission-control mcp` lingered after its client left: Tokio waited for the channel push loop's blocking long-poll, whose reconnect restarted a stopped daemon. `mcp::serve` now uses `shutdown_background`.
  - e2e: native paths in hook payloads (`cygpath -m`), `.exe` in the hook command, a `.cmd` stand-in for redline, a portable millisecond clock (macOS `date` has no `%N`), a looser wake-up limit on Windows; the `/` palette test starts from a settled page.
- Tag `v0.1.0` on the squashed commit; `release.yml` builds and publishes.

## Verified
- `just check` green on the release commit; CI green on Linux, macOS and Windows.

## Open / next
- ROADMAP P1.1 (clean-install test), P1.2 (Windows on a real machine); P2.4 (`review` can't find redline's `.cmd` shim on Windows).
- Dependabot's `ureq` 3 bump needs code changes in `src/api.rs` and `src/lifecycle.rs`.
- redline's `INSTALL.md` and `bin/redline` still say its repo is private.
