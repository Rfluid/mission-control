# Workflow: add a built-in widget

A widget is `render(section, ctx) -> html string`, registered in `web/widgets.js`. Section props come from config; data comes from `ctx.records` (by collection), `ctx.events` and `ctx.config`.

1. **Design first.** Read `.design/` (components, color, layout). Decide the props: names that read well in TOML, defaults for everything optional. Reuse existing prop names (`collection`, `columns`, `limit`, `record`, `sort`) where they mean the same thing.
2. **Implement** in the matching file (`web/widgets.js` core, `widgets-chart.js`, `widgets-board.js`, `widgets-embed.js`, or a new `web/widgets-<group>.js`: add it to `ASSETS` in `src/daemon/handlers.rs` and a `<script>` in `web/mission.html`):
   - escape every string with `h.esc`; links through `h.safeUrl` / `h.fill`
   - empty state: an `.empty` card that says what will appear
   - colors only from tokens (`var(--…)`) and `[states]` via `h.pill`
   - no network calls; render from `ctx` only.
   - typed: `/** @type {WidgetRender} */` on the function, prop shapes in JSDoc; new shared shapes go in `web/types.d.ts`. `just types` must pass.
3. **Register the name** in `BUILTIN_WIDGETS` (`src/config/validate.rs`), and type its props in `src/config/widgets.rs` (a struct, an arm in `check`, an entry in `schemas`).
4. **Use it** in a built-in template if it belongs there (`defaults/templates/`).
5. **Test:** steps in `tests/browser/widgets.test.mjs` (renders, empty state, escaping, 390 px); `cargo test config` still passes. Charts: follow `.design/dataviz.md`.
6. **Docs:** the widget table in `docs/CONFIG.md`, `.design/components.md`, and the ROADMAP item.
