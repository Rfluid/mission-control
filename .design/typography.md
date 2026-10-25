# Typography

mission-control serves its fonts **from the binary**: Inter for UI and JetBrains Mono for ids, times and numbers, as woff2 under `/assets/fonts/`, `font-display: swap`. Pages still work offline and from loopback; when the fonts can't load, the system stacks below take over with no layout change worth noticing. Both fonts are OFL; their licenses ship next to them.

The files are the variable-weight latin and latin-ext subsets (`web/fonts/`, from Fontsource), ~185 KB in all. The static export (`mission-control export`) doesn't inline them and uses the system stacks.

## Stacks

| Token (tokens.json `font`) | Stack | Use |
|---|---|---|
| `ui` | `Inter, system-ui, -apple-system, "Segoe UI", sans-serif` | All UI |
| `mono` | `"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace` | Ids and keys, times, counts and money, event kinds, config paths, keyboard hints |

## Scale (v2)

| Name | Size / line | Weight | Use |
|---|---|---|---|
| body | 14 / 1.45 | 400 | Page default, need context, doc prose |
| title | 15 | 600 | Mission title in the top bar |
| question | 17, -.01em | 600 | Need question (`h3`) |
| drawer | 20, -.01em | 600 | Record title in the drawer |
| tab | 13.5 | 400; 600 when active | Page tabs |
| label | 11, caps, .07em | 600 | Section titles (`h2`), table headers, KPI and strip labels, phase names |
| kpi | 24 mono, -.03em | 600 | KPI values; the denominator at 14 / 400 `--ink3` |
| strip | 15 mono | 600 | Status strip values |
| table | 13.5 | 400 | Table cells, graph node titles (13) |
| small | 12.5 | 400 | Muted columns, notes, node sub-lines (11.5) |
| pill | 11 | 600 | State pills, lowercase, underscores shown as spaces |
| kbd | 11 mono | 400 | Shortcut hints, boxed in `--line2` |

## Rules

- **Two weights:** 400 and 600. 500 only for "-control" in the header wordmark.
- **Numbers are mono** (tabular by nature): KPIs, counts, money, times, durations. Refreshes never jitter.
- **States** are shown lowercase with spaces (`in_progress` → "in progress").
- **Ids and keys** are mono; titles and names never are.
- **Section titles are labels,** not big headings: the content is the hierarchy, the label just names it.
