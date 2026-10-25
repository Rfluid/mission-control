# Data visualization

Charts follow the dataviz method (form first, color by job, validate, thin marks, hover, accessibility). This file is the design-system parameters it needs.

## Categorical series: `--s1` … `--s8`

Fixed order, never cycled; a series keeps its slot (series are ordered by name). Past eight, the rest fold into "other".

| Slot | Hue | Light | Dark |
|---|---|---|---|
| s1 | blue | `#2a78d6` | `#3987e5` |
| s2 | orange | `#eb6834` | `#d95926` |
| s3 | aqua | `#1baf7a` | `#199e70` |
| s4 | yellow | `#eda100` | `#c98500` |
| s5 | magenta | `#e87ba4` | `#d55181` |
| s6 | green | `#008300` | `#008300` |
| s7 | violet | `#6250d6` | `#9085e9` |
| s8 | red | `#e34948` | `#e66767` |

Validated on our panels (`validate_palette.js`, light on `#ffffff`, dark on `#161618`, adjacent pairs): all checks pass; worst adjacent CVD ΔE 9.1 light / 8.4 dark, normal-vision ΔE ≥ 19.3. Light s3, s4, s5 are under 3:1 on the panel, so the relief rule applies: **every chart shows direct value labels and a table view** (`details` under the plot).

## Rules the widgets keep

- One y axis per chart. Two measures of different scale: two sections.
- Status tones (`--done`, `--block`, …) are for state, never for series. The one chart-like use of tones is the **phase mix bar** (components.md): it counts records by state, so its segments are states, not series.
- Text in ink tokens, never in a series color.
- Status (pills, lane borders) always comes with its word, never color alone.
