# .design: mission-control's design system

The source of truth for how mission-control looks and reads, on every surface it draws: the mission pages and the missions home (`web/`), the logo (README, favicon, release pages), and any widget, built-in or user-written.

> The page is a control room, not a report. It answers three questions in order: *is the run healthy*, *what needs me*, *where are we*. Everything else is one click away.

## The idea in one line

**A calm ops console: ink on paper, color only for state.** Chrome is neutral. Six state tones (todo, prog, rev, block, done, warn) carry all meaning, always as a glyph and a word with the tone, so a glance down any column reads the run. A status strip holds the answer to the three questions on every page; tabs and a record drawer hold the detail.

## Files

| File | What | Read when |
|---|---|---|
| [principles.md](principles.md) | The rules every design decision is checked against | Always, first |
| [brand.md](brand.md) | Name, the beacon logo, favicon, voice and copy | Using the logo or writing UI text |
| [color.md](color.md) | Palette, state tones and glyphs, ink accents, theme, contrast | Picking any color |
| [typography.md](typography.md) | Inter + JetBrains Mono from the binary, scale | Setting any text |
| [components.md](components.md) | Shell (top bar, strip, tabs), cards, pills, needs, plan and system graphs, drawer, palette, attention signals, home, every widget | Building or changing a widget |
| [layout.md](layout.md) | Pages, Overview's two columns, Plan, drawer, home, phone | Positioning anything |
| [dataviz.md](dataviz.md) | Chart series palette and rules | Drawing a chart |
| [accessibility.md](accessibility.md) | Contrast, focus, keyboard map, screen readers, notifications, motion | Before calling UI done |
| [tokens.css](tokens.css) | Every color token as CSS custom properties, light and dark | Implementing |
| [tokens.json](tokens.json) | The same tokens plus fonts, type, space, radius and sizes, for tools | Implementing outside CSS |
| [check.py](check.py) | Contrast of every token pair the page uses; tokens.css ≡ web/style.css ≡ tokens.json | After changing a token |
| [logo/](logo/) | Outlined SVGs and `build.py`, their generator | Using or changing the logo |

## Reference mockups

The v2 mockups (logo concepts, states and legend, Overview, Plan, Architecture, drawer, missions home; light and dark) were drawn on a design canvas that isn't public. This folder is the spec, and wins where the two disagree; `docs/screenshots/` shows the result.

## Rules for changing the design

- **Tokens first.** Change a value in `tokens.json` *and* `tokens.css` *and* the copy at the top of `web/style.css` in the same change, then run `python3 .design/check.py`. Widget code uses tokens (`var(--prog)`), never raw hex.
- **Config can override tokens** per mission (`[theme]` in config, see `docs/CONFIG.md`). Overrides are the user's call; `check.py` covers only the defaults.
- **A new component or state** gets a section in `components.md` before code.
- **A change to a principle** needs a `docs/DECISIONS.md` entry.

## Status

- **v2** is specified here (2026-10-24) and implemented (2026-10-24): fonts from the binary, state glyphs, pages and tabs, the status strip, need cards v2, the plan and system graphs with their viewport, docs with Mermaid, the record drawer, the command palette and shortcuts, attention signals and the missions home. Screenshots: `docs/screenshots/` (`just screenshots`).
- Where building it changed the spec, the files say so ("changed while building"): gates under the plan graph, graph nodes 184 px wide, no notification sound; plus the diagram viewport, added on request.
- **v1** (one long page) is gone; `flow` sections still draw, as a plan without edges.
