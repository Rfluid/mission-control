# Color

Tokens are in [tokens.css](tokens.css) and [tokens.json](tokens.json); `web/style.css` starts with a verbatim copy. Use the token names in code, never hex.

## Neutrals

| Token | Light | Dark | Role |
|---|---|---|---|
| `--bg` | `#f7f7f5` | `#0e0e0f` | Page background |
| `--panel` | `#ffffff` | `#161618` | Cards, tables, buttons |
| `--ink` | `#141414` | `#ededea` | Main text |
| `--ink2` | `#4a4a4a` | `#b4b4ae` | Secondary text, nav |
| `--ink3` | `#6f6f6a` | `#8c8c86` | Labels, timestamps, muted columns, empty states |
| `--line` | `#e3e3df` | `#2a2a2d` | Borders, row dividers |
| `--line2` | `#d0d0ca` | `#3a3a3e` | Button borders, graph node borders, empty segments in phase bars |
| `--sunk` | `#f2f2ee` | `#1c1c1f` | Wells inside a panel: recommendation box, current phase column, record JSON, graph group bands |
| `--focus` | `#2459b8` | `#6a9cf5` | Focus ring |

The neutrals are slightly warm (paper, not screen grey) and step back so the state tones carry the page.

## State tones

Each tone has a foreground (`--x`) and a tint (`--x-bg`). Pills use both; dots and left borders use the foreground on `--panel`.

| Tone | Light | Dark | Means | Default states (`[states]`) |
|---|---|---|---|---|
| `todo` | `#5f5f5a` | `#9a9a94` | Not started, finished, closed | todo, finished, closed |
| `prog` | `#2459b8` | `#6a9cf5` | Moving | in_progress, running |
| `rev` | `#7440b5` | `#b48af0` | Waiting on review | in_review |
| `block` | `#ad3523` | `#f07a64` | Stuck or failed | blocked, failed, fail |
| `done` | `#1a7341` | `#4cc27e` | Finished well | merged, done, passed, pass, answered |
| `warn` | `#8a5a0c` | `#e0a84a` | Needs attention | open, waiting, pending |

A state with no `[states]` entry renders as `todo`. Config maps any state name to a tone, so a domain never needs a new color.

### Glyphs

Each tone has one glyph, drawn as a 12 px SVG in `currentColor` (`web/glyphs.js`). The glyph goes before the word in pills, on graph nodes, in the drawer history and in legends.

| Tone | Glyph | Drawing |
|---|---|---|
| `todo` | ○ | ring |
| `prog` | ▶ | ring with a play triangle |
| `rev` | ◐ | ring, right half filled |
| `block` | ✕ | filled disc, knocked-out cross |
| `done` | ✓ | filled disc, knocked-out check |
| `warn` | ! | filled disc, knocked-out bang |

Filled glyphs (block, done, warn) are the "settled or loud" states; rings are "open". Knock-outs use `--panel`.

### Where tones may appear

- Pills, glyphs, health, the status strip's *Needs you* cell (`--warn-bg`), need card border (`--warn`), blocked node border (`--block`, 1.5 px), blocked-upstream edges (`--block`, dashed), phase mix bars, gate diamonds.
- Never on buttons, tabs, links, KPI values (except a blocked count, which is a state count) or chart series.

## Ink accents

- **Current phase / active tab / recommended option** are marked in ink: a 3 px `--ink` bar on top of the phase column, a 2 px `--ink` underline on the active tab, an `--ink` border on the recommended need option. Emphasis without a state color.
- **Segmented controls** (Graph / Table / Board) fill the active segment with `--ink` and its text with `--panel`.

## Choosing the theme

1. Default: light, or dark when the OS prefers dark (`prefers-color-scheme`).
2. The **Theme** button sets `data-theme="light" | "dark"` on `<html>`, which wins over the OS.
3. Config `[theme]` keys override tokens per mission at runtime (`accent`, `bg`, …). They apply to both themes; `[theme.dark]` keys override them again in dark mode. Values are set with `style.setProperty`, never as CSS text.

## Contrast (checked)

`python3 .design/check.py` checks every pair the page uses: text ≥ 4.5:1, glyphs (dots, meaningful borders, focus ring) ≥ 3:1. That includes every tone on its own tint (pills), ink on every tint (the recommendation box), and every tone on `--panel`. The light tones are darker than the reference page's on purpose: the originals failed 4.5:1 as pill text (3.75–4.30).
