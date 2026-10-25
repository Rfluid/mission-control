# Layout

## Pages

A mission has **pages**, shown as tabs. Overview is always first and always exists; config adds the rest (`[[pages]]`, merged by `id`; each section names its page with `page = "<id>"`, default `overview`). The config decides which pages exist and their order; the page never invents one. URL: `/m/<id>` for Overview, `/m/<id>/<page>` for the rest, `?record=<collection>/<key>` for an open drawer, so every view is a link.

Built-in templates ship these pages (each only when it has sections):

| Page | Holds |
|---|---|
| **Overview** | Needs you, phase track, in flight, numbers, burndown, recent activity |
| **Plan** | `plan` graph (or board), gate checklists |
| **Architecture** | `graph`, `doc`, decisions |
| Domain pages | PRs, tests, agents, risks, spend… one topic each |
| **Activity** | The full feed, paged |

## Frame

- Content max width 1360 px (`size.content-w`), 24 px gutters (16 at ≤ 860 px).
- **Top bar → status strip → tabs** (components.md). Only the tabs row is sticky.
- **Issue banner** (config problems) sits above the first section of the page, never inside one.
- Sections follow the resolved order for that page, 28 px apart.

## Overview: two columns

A wrapping row: **main** (`flex: 999 1 640px`) and **rail** (`flex: 1 1 340px`); the rail drops below the main column when they don't fit.

| Main, in order | Rail, in order |
|---|---|
| Needs you | Numbers (2 × 2 KPI tiles) |
| The plan (phase track) | Burndown (or the template's one chart) |
| In flight (non-done, non-todo records) | Activity (latest 7) |

Sections choose their column with `column = "main" | "rail"` (default main). Principle 1 holds: Needs you is first in main, and the strip above both columns carries the count.

## Plan page

The plan graph takes the full width, so four phases (1120 px) fit without scrolling from about 1200 px up; the gate checklists follow under it in a grid: the next gate first with a `--warn` border, later gates after, passed gates folded. The graph keeps its node size and scrolls inside its card (a thin `--line2` bar, no buttons) only when it doesn't fit. (Changed while building: a gates rail beside the graph forced a horizontal scroll at 1440 px.)

## Drawer

Overlays the right 480 px of the window over a scrim; the page behind doesn't reflow. ≤ 860 px: full screen with a back button in place of ✕.

## Missions home

Same frame and top bar (no strip, no tabs). Needs-you card, then the mission card grid (`repeat(auto-fill, minmax(min(100%, 380px), 1fr))`, 16 px gap), then the finished table.

## Inside widgets

| Widget | Layout |
|---|---|
| KPIs | Rail: 2-column grid. Main: auto-fit, tiles ≥ 150 px |
| Table | Full width card; scrolls horizontally inside the card, never the page |
| Needs | One card per open need, stacked; answered ones folded under |
| Feed | Card; rail: latest 7; Activity page: scrolls and pages |
| Plan / graph | Fixed geometry (296 px column pitch, 80 px row pitch), horizontal scroll in its card |

## Phone (≤ 860 px)

- Top bar wraps: title row, then buttons. The full strip becomes a 2-column grid of cells with Health spanning both and Needs you right after it.
- Tabs scroll horizontally on one line.
- Rail sections drop under the main column.
- Feed rows stack (time and kind on one line, text under).
- Tables and graphs scroll inside their card.
- Touch targets grow to 44 px (accessibility.md).

## Spacing and shape

4 px base grid (`space.1` = 4 … `space.6` = 28). Radii: 4 (kbd), 6 (buttons), 8 (graph nodes, wells, strip cells), 10 (pills, cards).
