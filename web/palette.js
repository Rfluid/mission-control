// @ts-check
// Command palette (.design/components.md, "Command palette"): ⌘K / Ctrl+K (or `/`) opens one search box over
// pages, sections, records, needs and buttons; arrows move, Enter acts, Esc closes and returns focus.
// Everything it lists comes from the resolved config and the records the page already has.
(function () {
  "use strict";
  const W = window.MissionWidget, G = window.MissionGlyph, P = window.MissionPages, h = W.h, esc = h.esc;
  const MAX_PER_GROUP = 8;
  /** @type {() => Ctx | null} */
  let getCtx = () => null;
  /** @type {{group: string, label: string, hint: string, glyph: string, act: () => void}[]} */
  let items = [];
  let active = 0;
  /** @type {HTMLElement | null} */
  let back = null;

  const dlg = () => /** @type {HTMLDialogElement} */ (document.getElementById("palette"));

  /** Goes to a page (in place) and runs `then` once it's drawn. @param {string} page @param {() => void} [then] */
  function go(page, then) {
    document.dispatchEvent(new CustomEvent("mission:go", { detail: page }));
    if (then) requestAnimationFrame(() => requestAnimationFrame(then));
  }

  /** Everything the palette can do, from the config and records. @param {Ctx} ctx */
  function catalog(ctx) {
    /** @type {typeof items} */
    const out = [];
    const keys = window.MissionKeys ? window.MissionKeys.letters(ctx.config) : new Map();
    for (const p of P.list(ctx.config)) {
      out.push({ group: "Pages", label: p.title, hint: keys.has(p.id) ? `g ${keys.get(p.id)}` : "", glyph: "", act: () => go(p.id) });
    }
    const pages = new Map(P.list(ctx.config).map(p => [p.id, p.title]));
    for (const p of P.list(ctx.config)) {
      for (const s of P.sectionsOn(ctx.config, p.id)) {
        out.push({ group: "Sections", label: s.title || s.id, hint: String(pages.get(p.id) || ""), glyph: "",
          act: () => go(p.id, () => { const el = document.getElementById(s.id); if (el) { el.scrollIntoView({ block: "start" }); el.querySelector("h2")?.setAttribute("tabindex", "-1"); /** @type {HTMLElement | null} */ (el.querySelector("h2"))?.focus(); } }) });
      }
    }
    const needs = ctx.records.needs || [];
    for (const n of needs.filter(x => x.state !== "answered")) {
      out.push({ group: "Needs", label: String((n.body || {}).question || n.key), hint: n.key, glyph: G.glyph("warn"),
        act: () => go(P.OVERVIEW, () => { const c = /** @type {HTMLElement | null} */ (document.querySelector(`[data-need-card="${CSS.escape(n.key)}"]`)); c?.scrollIntoView({ block: "center" }); c?.focus(); }) });
    }
    for (const [coll, recs] of Object.entries(ctx.records)) {
      if (coll === "needs" || coll.startsWith("_")) continue;
      for (const r of recs) {
        const title = h.title(r);
        out.push({ group: "Records", label: `${r.key}${title ? ` ${title}` : ""}`, hint: `${coll}${r.state ? ` · ${h.word(r.state)}` : ""}`, glyph: r.state ? h.glyph(ctx, r.state) : "",
          act: () => window.MissionDrawer.show(`${coll}/${r.key}`) });
      }
    }
    const acts = ctx.config.actions || {};
    const ids = new Set([...((ctx.config.toolbar || {}).actions || []), ...(ctx.config.sections || []).flatMap((/** @type {Section} */ s) => s.actions || [])]);
    for (const id of ids) {
      const a = acts[id];
      if (!a) continue;
      out.push({ group: "Actions", label: String(a.label || id), hint: String(a.kind || ""), glyph: "", act: () => {
        const owner = (ctx.config.sections || []).find((/** @type {Section} */ s) => (s.actions || []).includes(id));
        const click = () => /** @type {HTMLElement | null} */ (document.querySelector(`button[data-action="${CSS.escape(id)}"]:not([data-row])`))?.click();
        const onToolbar = ((ctx.config.toolbar || {}).actions || []).includes(id);
        if (!onToolbar && owner) {
          const page = P.list(ctx.config).find(p => P.sectionsOn(ctx.config, p.id).some(s => s.id === owner.id));
          if (page) return go(page.id, click);
        }
        click();
      } });
    }
    return out;
  }

  /** Items whose label or hint holds every word of the query, best (prefix) matches first. @param {string} q */
  function filter(q) {
    const ctx = getCtx();
    if (!ctx) return [];
    const words = q.toLowerCase().split(/\s+/).filter(Boolean), all = catalog(ctx);
    const hit = all.filter(i => words.every(w => `${i.label} ${i.hint}`.toLowerCase().includes(w)));
    const score = (/** @type {typeof items[0]} */ i) => (words.length && i.label.toLowerCase().startsWith(words[0]) ? 0 : 1);
    /** @type {Map<string, typeof items>} */
    const groups = new Map();
    for (const i of hit.sort((a, b) => score(a) - score(b))) {
      const g = groups.get(i.group) || [];
      if (g.length < MAX_PER_GROUP) g.push(i);
      groups.set(i.group, g);
    }
    return ["Pages", "Needs", "Records", "Sections", "Actions"].flatMap(g => groups.get(g) || []);
  }

  function draw() {
    const list = /** @type {HTMLElement} */ (dlg().querySelector(".pal-list"));
    let last = "";
    list.innerHTML = items.length ? items.map((i, n) => {
      const head = i.group !== last ? `<li class="pal-g" role="presentation">${esc(i.group)}</li>` : "";
      last = i.group;
      return `${head}<li role="option" id="pal-${n}" class="pal-i" aria-selected="${n === active}" data-n="${n}">${i.glyph ? `<span class="pal-glyph">${i.glyph}</span>` : ""}<span class="pal-l">${esc(i.label)}</span><span class="pal-h">${esc(i.hint)}</span></li>`;
    }).join("") : `<li class="pal-empty">Nothing matches.</li>`;
    const input = /** @type {HTMLInputElement} */ (dlg().querySelector("input"));
    input.setAttribute("aria-activedescendant", items.length ? `pal-${active}` : "");
    document.getElementById(`pal-${active}`)?.scrollIntoView({ block: "nearest" });
  }

  function open() {
    if (dlg().open) return;
    back = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const input = /** @type {HTMLInputElement} */ (dlg().querySelector("input"));
    input.value = "";
    items = filter("");
    active = 0;
    draw();
    dlg().showModal();
    input.focus();
  }

  /** @param {boolean} [restore] */
  function close(restore = true) {
    if (dlg().open) dlg().close();
    if (restore && back && document.contains(back)) back.focus();
  }

  function run() {
    const i = items[active];
    if (!i) return;
    close(false);
    i.act();
  }

  document.addEventListener("input", e => {
    if (!(e.target instanceof HTMLInputElement) || !e.target.closest("#palette")) return;
    items = filter(e.target.value);
    active = 0;
    draw();
  });
  document.addEventListener("keydown", e => {
    if (!dlg().open || !(e.target instanceof Element) || !e.target.closest("#palette")) return;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      active = items.length ? (active + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length : 0;
      draw();
    } else if (e.key === "Enter") { e.preventDefault(); run(); }
    else if (e.key === "Escape") { e.preventDefault(); close(); }
  });
  document.addEventListener("click", e => {
    const t = e.target instanceof Element ? e.target : null;
    if (!t) return;
    if (t.closest("[data-palette]")) return void open();
    const li = /** @type {HTMLElement | null} */ (t.closest("#palette .pal-i"));
    if (li) { active = Number(li.dataset.n); run(); }
    else if (t.id === "palette") close();
  });

  window.MissionPalette = { attach(/** @type {() => Ctx | null} */ g) { getCtx = g; }, open, close, go };
})();
