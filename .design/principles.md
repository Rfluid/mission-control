# Principles

Check every design decision against these. When two conflict, the lower number wins.

1. **What needs the user comes first.** Health and *Needs you* sit at the top of every built-in template's Overview page, and the status strip carries the open-needs count on every page. A question waiting on the user is never below the fold on a fresh load, never on a tab the user isn't on without a visible count, and never hidden by a collapsed section.
2. **Color means state, nothing else.** The six tones (todo, prog, rev, block, done, warn) are reserved for record states, health and needs. Buttons, nav, tabs and chrome are ink. Red never means "primary action".
3. **Shape, word, then tone.** A state always shows as its glyph (✓ ▶ ◐ ✕ ○ !) and its word next to its color, so it reads in grayscale, to color-blind users and to screen readers. Color alone is never the signal; the glyph alone is allowed only where the word is one hover or one legend away (plan graph nodes, which always ship with a legend).
4. **Config decides, the page obeys.** The page draws what the resolved config says, in its order: pages, then sections. No widget invents sections, reorders them or hides one the config shows. When config is wrong, say so in the issue banner instead of guessing.
5. **One glance, then drill.** The Overview answers *is it healthy, what needs me, where are we* in one screen. Detail lives one step away: a tab for a topic, a drawer for one record. Never put detail on the Overview that has its own page.
6. **Draw relations, don't imply them.** When records depend on each other, the page draws the edge (plan graph, system graph, drawer "Depends on / Holding up"). A reader should never have to infer order from ids or lane position.
7. **Dense but calm.** Tables over cards for lists. One line per row where possible; long text goes muted and small. Numbers carry their denominator ("3 / 8", not "3").
8. **Quiet until needed.** Empty sections say so in one muted line. No spinners for live updates. Attention signals (title count, favicon, notification) fire on new needs and new blocks only, never on progress.
9. **Keyboard is first class.** Everything a mouse can do, the keyboard can: ⌘K jumps anywhere, numbers pick a need's option, `j`/`k` walk records, Esc closes. Shortcuts are shown where they apply.

## What this rules out

- A red or blue "primary" button. Use the neutral `.btn`; the recommended need option gets an ink border, not a color.
- A state shown only as a colored dot or colored border, without its glyph and word.
- A dependency shown only by lane position or by an id in a note.
- A widget that hard-codes a domain (waves, gates, PR numbers). Put the shape in a template.
- Emoji as icons or status. Glyphs are drawn SVG in `currentColor`.
- `alert()` for errors. Use the config-issue banner or an inline empty state.
- Notifications or sounds for anything but a new need or a new block.
