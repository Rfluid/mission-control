# 2026-10-26 · shadow-dom-shortcuts

**Agent:** claude  **Roadmap:** none (bug report)

## Done
- Single-key shortcuts fired while typing in redline's comment box: its overlay lives in a shadow root, so `e.target` at `document` is the shadow host and `typing()` saw no text box. `n` swallowed the letter and focused the first need.
- `web/keys.js`, `web/index.js`, `web/viewport.js`: typing guards read `e.composedPath()[0]` instead of `e.target`.
- `tests/browser/keys.test.mjs`: a shadow-root textarea takes `g/p?n` as text (fails on the old code).
- Released as v0.1.1.

## Verified
- `just check` passes.

## Gotchas
- Any document-level keydown guard must use `composedPath()[0]`; overlays (redline, extensions) mount in shadow roots.
