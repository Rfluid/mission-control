# Accessibility and motion

## Contrast

- Text ≥ 4.5:1 against its own background; glyphs, meaningful borders, graph edges and the focus ring ≥ 3:1. Every pair the page uses is in `PAIRS` in `check.py`; add one when a widget starts using a new combination, then run `python3 .design/check.py`.
- Never tell states apart by hue alone: every state has its glyph and its word (principle 3). Edge styles differ by dash and width, not only color.
- `forced-colors: active`: pills, graph nodes, the strip's needs cell and the banner keep a 1 px `CanvasText` border so they stay visible when fills drop; glyphs use `CanvasText`.

## Focus

- Every control is a real `<button>` or `<a>`; graph nodes are `<a>`. Icon-only buttons get `aria-label` (theme, close, previous/next, bell).
- `:focus-visible` = `outline: 2px solid var(--focus); outline-offset: 2px` (set globally in `web/style.css`). Never remove an outline without replacing it.
- The drawer, palette and prompt dialog trap focus while open and return it to what opened them.
- Disabled buttons stay focusable through their `title` only as long as they are disabled for a stated reason.

## Keyboard

| Key | Does |
|---|---|
| ⌘K / Ctrl+K, `/` | Command palette |
| `?` | Shortcut sheet |
| `g` then `o` / `p` / `a` | Go to Overview / Plan / Activity; other pages take the first free letter of their title (the `?` sheet lists them) |
| `1`–`9` | Pick that option of the focused need |
| `n` | Focus the first open need |
| `j` / `k` | Next / previous record in the focused widget or the drawer |
| Enter | Open the focused record's drawer; send an answer |
| `+` / `−` / `0` / `f` | Zoom in, zoom out, fit, fullscreen, in a focused diagram (arrow keys scroll it) |
| Esc | Close drawer, palette or dialog; leave fullscreen |

- Shortcuts never fire while typing in a text box, and single-letter keys can be turned off (`[ui] shortcuts = false`; WCAG 2.1.4). ⌘K / Ctrl+K always works.
- Tab order: top bar, strip, tabs, then each section of the page in config order.

## Screen readers

- One `h1` per page (the mission title, visually the top bar title). Section titles are `h2`; need questions are `h3`; the drawer title is the dialog's `h2`.
- Tabs are a `nav` of links with `aria-current="page"`, not an ARIA tablist: each page is its own URL.
- Graphs: the SVG edge layer is `aria-hidden`; each node's accessible name carries its state, deps and dependents. A **Table** view of the same records is one click away.
- Tables have header cells (`th`).
- Live region (polite): "New question: …" for new needs and "S1 is blocked: …" for new blocks. Live updates never announce otherwise.

## Notifications

- Browser notifications are opt-in through the bell button; never ask on page load.
- No sound.

## Touch

- Targets ≥ 44 × 44 under `(pointer: coarse)` or ≤ 860 px, even if the visual is smaller (graph nodes already are).

## Motion

- No animation on live updates: content swaps in place, and unchanged views don't re-render.
- The drawer slides in 160 ms, the palette fades 120 ms; both are instant under `prefers-reduced-motion: reduce`, as is smooth scrolling.
- The live dot doesn't pulse.
