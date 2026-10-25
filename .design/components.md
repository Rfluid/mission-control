# Components

Every value here is a token (tokens.css) or a scale step (typography.md). Classes live in `web/style.css` (tokens, base, shared components), `shell.css`, `widgets.css`, `graph.css`, `drawer.css` and `home.css`; widgets build their markup from these and nothing else, so script widgets look native by reusing the same classes. Everything here is implemented (design v2). The reference mockups are the design canvas linked from `README.md` in this folder.

## Shell

### Top bar

`header.top` › `.topin`, one wrapping row on `--panel` with a 1 px `--line` bottom border: mark (24 px, links to the missions home) · `/` in `--line2` · mission title (title scale) · template chip (mono 11.5, `--ink3`, 1 px `--line` box, radius 4) · spacer · live indicator (7 px dot in `--done` + "live · 4s ago", mono 12 `--ink3`; the dot turns `--todo` and the text says "reconnecting" when the stream drops) · **Jump to ⌘K** · toolbar buttons (config `toolbar.actions`) · bell (notifications, icon, `aria-pressed`) · theme button (icon, `aria-label`). On phones the template chip hides and **Jump to** shrinks to its icon. Not sticky; the tabs row is.

### Status strip

Under the top bar, on every page. Two forms:

- **Full (Overview only):** a wrapping row of cells. *Health* first and widest: label, then the health pill (12 px, 22 px line) and the reason in `--ink2`. Then cells split by 1 px `--line` rules, each a label over a strip-scale value: *Phase* ("2 of 4 · Build" plus a 4-segment bar: `--done` passed, `--ink` current, `--line2` ahead), *Tasks* ("3/8 done"), *Blocked* (value in `--block`, hidden at 0), *Agents*, *Spend*. Last, the **Needs you** cell: `--warn-bg` fill, radius 8, label and value in `--warn` with the warn glyph, linking to the needs section. At 0 needs it reads "Nothing waits on you" in `--ink3` with no fill.
- **Compact (every other page):** health pill and needs pill sit in the top bar row, before the live indicator. Same links.

Which cells show is config (`[strip]`): each cell is a count, a sum or a phase, like KPI items (`docs/CONFIG.md`, "Status strip").

### Tabs

`nav.tabs` (`aria-label="Pages"`): one link per `[[pages]]` entry in config order, Overview first. Tab scale, `--ink2`; active: `--ink`, 600, 2 px `--ink` underline, `aria-current="page"`. A tab may show a count (mono 11 `--ink3`) from config (`count = {collection, state}`). Sticky at the top, `--panel` at 92% with an 8 px backdrop blur. Scrolls horizontally on phones, never wraps. Page URL: `/m/<id>/<page>`; Overview is `/m/<id>`.

## Section

`<section id="<id>">` › header row: `h2` in label scale, optional count (mono 11 `--ink3`, e.g. "1 open · 2 answered"), spacer, optional link ("Open plan") and `.acts` (section buttons) › widget body. The id is the config section id, so links and tests can target it. Sections are 28 px apart; header to body 10 px.

## Card

`.card`: `--panel`, 1 px `--line`, radius 10. `.pad` adds 14 / 16 padding. Every widget body is one or more cards. A card that waits on the user (need) takes a `--warn` border; nothing else colors a card border.

## State glyph and pill

- **Glyph:** 12 px SVG per tone (color.md, "Glyphs"), `aria-hidden`, always followed by the word somewhere visible or in the legend.
- **Pill** `.pill.c-<tone>`: glyph + state word, tone foreground on tone tint, 11 / 600, 18 px line, radius 10, padding 0 8 0 6, gap 5, no wrap. Lowercase, underscores as spaces. Unknown states get `c-todo`. An empty state renders "—", not an empty pill.

## KPI tile

`.card.pad.kpi` › `.l` (label) › `.v` (kpi scale, mono). **v2:** a value with a total shows its denominator (`3` then ` / 8` at 14 / 400 `--ink3`) and may show a 4 px bar (`--line` track, `--ink2` fill); a value without one may show one muted sub-line ("3 merged", "of 5 spawned"). Config: `count = {...}` or `sum = {...}`, `of = {...}`, `format`, `note`. Units go in the label or the sub-line.

## Health

Card with the health pill (glyph + state) and the reason. From a record (`record = {collection, key}`) or `vars.health`. **v2:** on the Overview, health lives in the status strip; the `health` section stays for templates that place it elsewhere.

## Need card

`.card.need` with a `--warn` border (v2 drops the 3 px left bar). Top to bottom, gap 12:

1. Meta line, mono 12 `--ink3`: key · age · **blocks** `<record link>` (from the need's `blocks` field).
2. Question, `h3`, question scale.
3. Context, 13.5 `--ink2`, markdown.
4. Recommendation well: `--sunk`, radius 8, **Recommended:** answer, then the why in `--ink3`.
5. Options: one `.btn` per option, each with a `.kbd` number (1–9). The recommended option gets `.btn.rec` (1 px `--ink` border plus a 0.5 px inset ink ring) and "recommended" in 11.5 `--ink3`. No color.
6. Free answer: labelled text box + **Send**; Enter sends.

Number keys pick an option while focus is inside a need card (on it or on one of its buttons; never in its text box). Answered needs fold into one muted line ("2 answered") that expands to a list. On the Overview, open needs come first in the main column.

## Phase track

The Overview's small plan: one card holding phase blocks in a wrapping row, with a gate between each pair. Phase block: label ("Phase 2 · now" for the current one, in `--ink`), name (600), a 6 px **phase mix bar** (one segment per record, colored by tone, in tone order done → rev → prog → block → todo, 2 px gaps; `--line2` for todo), and counts by state in mono 12 ("1 done · 1 review · 1 blocked", blocked in `--block`). Current phase: `--sunk` fill and a 3 px `--ink` bar on top. Gate: 16 px diamond (fill `<tone>-bg`, stroke `<tone>`; passed = done, next = warn, later = `--line2` on `--panel`) over "n/m" in mono 10.5. Links to the Plan page.

## Plan graph: widget `plan`

The dependency diagram. Replaces `flow` (`flow` stays as an alias that draws no edges).

- **Columns** are phases (`phases = {collection}` or distinct values of the node's `phase` field), left to right in key order. Column header: label "Phase n · Name", a 5 px mix bar (80 px) and "done/total" mono 11. The current phase (first with any non-done node) gets a `--sunk` band with a 3 px `--ink` top bar and "· now" in its label.
- **Nodes** are records (`nodes = {collection, phase = "phase", deps = "depends_on"}`): 184 × 56, radius 8, `--panel`, 1 px `--line2`. Inside: the glyph, then key (mono `--ink3`) and title (13), then a sub-line (11.5 `--ink3`): state word · agent or PR · age, or "waits on U1" when its deps aren't done. A blocked node gets a 1.5 px `--block` border and its sub-line in `--block`. A node with an open need pointing at it says "needs you" in `--warn`. Nodes are links: click opens the drawer.
- **Rows:** within a column, nodes are ordered to keep edges short (barycenter of their deps' rows), 80 px pitch. Ties by key.
- **Edges** go from the right edge of a dependency to the left edge of the dependent, as horizontal lines or cubic curves that bend in the gap between columns. Style by the source's state: **met** (source done) solid 1.5 px `--ink3`; **waiting** dashed 5/4 `--ink3`; **blocked upstream** (source blocked, or any blocked ancestor) dashed 2 px `--block`, with a short `--block` label ("S1 holds R1") placed in an empty row. Arrowheads 7 px, filled with the edge's color. Same-column deps draw a short vertical edge.
- **Gates** sit in the gap between two phases: a dotted 1 px `--line2` rule the full height, and at the top the gate diamond with "G1 1/3" in the gate's tone. Gates come from `gates = {collection, after = "after"}` as today.
- **Legend** under the graph, always shown, never collapsible: the five node glyphs with words, the three edge styles, the gate diamond and the current-phase bar.
- **Controls** in the section header: a segmented **Graph / Table / Board** (the same records as a table or a kanban) and a **Trace blocked path** toggle that dims everything not on a blocked node's downstream path.
- **Size:** columns 296 px apart; the graph takes the page's full width and scrolls inside its card (thin `--line2` bar) only when it doesn't fit, never squeezing nodes. Gate checklists sit under it (layout.md).
- **Viewport** (`web/viewport.js`, shared with `graph`): at the legend's right end, **−**, the zoom level, **+**, **Fit** and **Fullscreen** (icon buttons, `aria-label`s). Zoom 40–200 %, Ctrl/⌘ + wheel zooms at the pointer, dragging the background pans, and with focus in the diagram `+` / `−` / `0` (fit) / `f` (fullscreen) work. Fullscreen holds the whole section (header, graph, legend) on `--bg` and survives live updates; Esc leaves it. Zoom and scroll position survive re-renders.
- **Accessibility:** the SVG edge layer is `aria-hidden`; each node link carries "S1 Storage adapter, blocked, depends on C0, holds R1". `j`/`k` move between nodes in phase then row order; Enter opens the drawer.

## System graph: widget `graph`

A component diagram the agent fills: `nodes = {collection, group = "group"}`, `edges = {collection, from = "from", to = "to", label = "label"}`. Groups are columns with a `--sunk` band and a label at the top; nodes are 184 × 56 boxes (the plan's geometry and routing, so one layout serves both; changed from 176 while building) (title 600, then either a muted line or a pill for the linked task: `task = "U1"` pulls that record's state). Edges as in the plan graph; an edge into or out of a blocked node is drawn blocked. Edge labels 11.5 `--ink3`, placed in the gap. Same scrolling, viewport and accessibility rules as `plan`; a component box opens its task's drawer (else its own).

## Doc: widget `doc`

Long-form markdown from a record (`record = {collection, key, field = "body"}`) or from section text: prose at body scale, max 68ch, `h3` 15 / 600, inline code on `--sunk`. Fenced ` ```mermaid ` blocks render as diagrams with the page's tokens (Mermaid 11, compiled in and loaded only when a page has a block; strict mode, sanitized SVG: see DECISIONS); a block that fails to parse shows its source and the error in `--ink3`.

## Drawer

Opens from any node, table row, kanban card or `?record=<collection>/<key>` in the URL. 480 px from the right, full height, `--panel`, 1 px `--line` left border, `--ink` at 20% scrim on the page (phones: full screen). Focus moves in and is trapped; Esc closes and returns focus.

- **Head:** path (mono 12 `--ink3`: "packages / S1") · Esc hint · close button; title (drawer scale); the pill plus phase and age.
- **Why blocked** well (`--block-bg`, only when blocked): the record's `reason` or `note`.
- **Fields:** a `dl` of the section's configured fields (`drawer.fields`, default: every scalar field not shown elsewhere).
- **Depends on / Holding up:** links to records and gates, each with its glyph and state word; click moves the drawer to that record.
- **History:** the record's events, newest first: state changes (time, glyph, "from → **to**", logged by the daemon), answers and row buttons (time, kind, text).
- **Note** (markdown) and **Record** (`details`, the raw JSON on `--sunk`).
- **Foot:** the row actions for this record, then ↑ / ↓ for previous / next record in the same widget (`k` / `j`).

## Command palette

⌘K / Ctrl+K, or `/`. A 560 px dialog at 20% from the top, `--panel`, radius 10, one search box, results grouped: **Pages**, **Sections**, **Records** (key, title, glyph), **Needs**, **Actions** (toolbar and section buttons), **Missions** (on the home page); at most 8 per group, prefix matches first. Arrow keys move, Enter acts, Esc closes and returns focus. The top bar's **Jump to** button shows ⌘K (Ctrl K off macOS). `?` opens the shortcut sheet, which lists every key on one card.

## Attention signals

- **Title:** "(n) Mission title · mission-control" while n needs are open.
- **Favicon:** the beacon redrawn on a canvas in the logo's colors (ink tile, paper brackets, the bright dark-theme tones as on the static icon): signal in the mission's health tone, plus a `--warn` ring (and a smaller signal) when needs are open.
- **Notification:** opt-in per browser (a bell button asks for permission once; the choice is stored per browser). Fires only for a new need or a record newly blocked, with the question or the reason as body; clicking focuses the tab and opens the need or drawer. Only while the page isn't in front of the user. No sound (changed while building: nothing asked for a sound setting yet).
- Live region: "New question: …" (polite) for new needs, as today.

## Missions home

The index page. Top bar: wordmark, daemon address and version (mono 12 `--ink3`), **Find a mission ⌘K**, theme. Then:

1. **Needs you** across missions: one card with a `--warn` border, one row per open need: mission, question (600), what it blocks, age. Hidden when none.
2. **Missions:** segmented filter (Active · Needs you · Finished, with counts; Finished = archived; the choice is kept per browser), then a grid of mission cards (min 380 px): title, needs pill when any, repo path and template (mono 12 `--ink3`), health pill and reason, a progress bar (phase segments when the mission has a `plan`, else one segment per record of its main collection, in tone order) with a one-line caption, then mono stats (tasks, agents, spend) and last activity. The whole card is a link.
3. **Recently finished:** a table (mission, template, result pill, ended).

## Table

`.card.tw` › `table`: header row in label scale `--ink3`, cells table scale with 9 × 16 padding and a `--line` divider. Column renderers (`as`): `pill`, `code` (mono), `muted` (small `--ink3`), `time` (short local date), `age` (mono "38m"), `link` (http/https/mailto only). A `link = "<name>"` column fills the named link template. Rows open the drawer on click or Enter (they're focusable). Empty: one `.empty` line ("No prs yet."). Row buttons sit in a trailing unlabeled column.

## Feed row

`.feed .row`: grid `44px 52px 1fr` (v2, was `120px 82px 1fr`): time (mono `--ink3`, HH:MM today, date otherwise), kind (mono; `--warn` for need, `--block` for a block, else `--ink3`), text. Newest first. On the Overview rail the feed shows the latest 7 and links to the Activity page; the Activity page scrolls and pages.

## Config-issue banner

`.card.banner` above all sections of the current page: 3 px left border, `--block` when any issue is an error, `--warn` (`.warn`) for warnings only. **Config:** then each issue as `<code>path</code> message`, separated by " · ". Shown whenever the resolved config has issues; it never blocks rendering.

## Buttons

`.btn`: `--panel` fill, 1 px `--line2`, `--ink` text, radius 6, 13 / 18, padding 6 × 12, gap 6 for an icon or `.kbd`. One style for toolbar, section and row actions; the action's `label` is the text. `[disabled]` at 55% opacity with `cursor: not-allowed`. `.btn.rec` for the recommended need option only. **Segmented** `.seg`: buttons joined in one `--line2` box; the active one is `--ink` on `--panel` text. No colored buttons (principle 2).

## Kbd

`.kbd`: mono 11 `--ink3`, 1 px `--line2`, radius 4, padding 0 4. Shows a shortcut next to the control it triggers.

## Prompt dialog and toast

`dialog#prompt`: `--panel`, 1 px `--line2`, radius 10, at most 520 px wide; title (`h3`, 16), a textarea, then **Cancel** / **Send** right-aligned. Backdrop is `--ink` at 30%. `.toast`: bottom center, `--ink` on `--bg` (`.bad`: `--block`), 13.5 px, fades in 150 ms (none with reduced motion), gone after 2.6 s; `role=status`.

## Empty and unavailable

`.empty` (14 px, `--ink3`, 16 px padding) inside a card. Unknown or not-yet-built widget: "Widget `x` is not available yet." A widget that throws: "Widget `x` failed: <message>". Neither stops the other sections.

## Chart

`.card.pad` › optional `.legend` (≥ 2 series: swatch + name, `--ink2`) › `svg.viz` drawn at the card's pixel width (text stays 11–11.5 px at every size) › `details.viz-table` ("Table", the same numbers). Lines 2 px, round joins; the last point gets a 4 px dot ringed in `--panel` and a direct value label (`--ink2`). Bars: 4 px rounded tops anchored to the baseline, 2 px gaps, value labels above. Grid: 3 recessive `--line` rules, axis labels `--ink3`. Hover: a 9 px transparent hit target per point with a native tooltip. Series colors are `--s1…--s8` in order of series name, so a series keeps its color. Sparkline: a bare line plus the latest value at 22 / 600. On the Overview rail a chart is 84 px tall with a mono caption row (start · now).

## Spend

Two KPI tiles (total, tokens), then 12 px (the grid gap), then `.bars`: one row per record, largest first: label, an 8 px `--line` track with an `--s1` fill, the amount (mono).

## Kanban, flow, checklist

Lanes are `--panel` cards (radius 10) holding cards on `--bg` with the state glyph and word (v2; was a 3 px left border in the tone, `.t-<tone>`). Lane head: state pill (kanban) or title + pill + done/total (flow). `flow` is an alias of `plan` that draws no edges. Checklist: one card per gate: title and pill, "n of m met" (mono), the requirements with ✓ (`--done`) / ○ (`--ink3`) / ✕ (`--block`, when the requirement's record is blocked) and a screen-reader word, then the note as markdown. The next gate gets a `--warn` card border; passed gates fold into a `details`. At ≤ 860 px lanes stack.

## Html

`.card.embed` › an iframe (`sandbox="allow-scripts"`, no referrer), full width, the configured height, white behind it in both themes (agent HTML assumes a white page).
