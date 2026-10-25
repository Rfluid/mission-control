// @ts-check
// The shell around the sections (.design/components.md, "Shell"): the status strip (full on Overview, compact
// pills in the top bar elsewhere), the live indicator, the template chip and the theme button.
(function () {
  "use strict";
  const W = window.MissionWidget, G = window.MissionGlyph, P = window.MissionPages, h = W.h, esc = h.esc;
  const TICK_MS = 5000; // how often "live · 4s ago" refreshes

  /** The run's health: `[strip] health` (a record's state and `reason`), else `vars.health`.
   *  @param {Ctx} ctx @returns {{state: string | null, reason: string}} */
  function health(ctx) {
    const ref = (ctx.config.strip || {}).health;
    const r = ref && (ctx.records[ref.collection] || []).find(x => x.key === ref.key);
    if (r && r.state) return { state: r.state, reason: String(h.field(r, "reason") ?? "") };
    const v = ctx.config.vars || {};
    return { state: v.health || null, reason: String(v.health_reason ?? "") };
  }

  /** Open needs and the link to them (the Overview's needs section).
   *  @param {Ctx} ctx */
  function needs(ctx) {
    const coll = (ctx.config.strip || {}).needs || "needs";
    const open = (ctx.records[coll] || []).filter(n => n.state !== "answered");
    const sec = P.sectionsOn(ctx.config, P.OVERVIEW).find(s => s.widget === "needs" && (s.collection || "needs") === coll);
    return { open, href: `${P.href(P.OVERVIEW)}#${encodeURIComponent(sec ? sec.id : "needs")}` };
  }

  /** One cell's value markup, or null to hide it.
   *  @param {Ctx} ctx @param {Json} c */
  function value(ctx, c) {
    if (c.plan) {
      const sec = (ctx.config.sections || []).find((/** @type {Section} */ s) => s.id === c.plan);
      const ph = sec && window.MissionPlan ? window.MissionPlan.phase(sec, ctx) : null;
      if (!ph) return null;
      const bar = ph.phases.map((/** @type {string} */ t) => `<i class="${t}"></i>`).join("");
      return `<span class="v"><b>${ph.index + 1}</b><span class="of"> of ${ph.phases.length}</span> · ${esc(ph.name)}</span><span class="pbar" aria-hidden="true">${bar}</span>`;
    }
    const m = h.metric(ctx, c);
    if (c.hide_zero && !m.n) return null;
    const n = typeof m.n === "number" ? h.fmt(m.n, c.format) : m.n;
    const rest = m.of != null ? `/${h.fmt(m.of)}${c.suffix ? ` ${c.suffix}` : ""}` : c.suffix ? ` ${c.suffix}` : "";
    return `<span class="v${c.tone ? ` t-${esc(c.tone)}` : ""}"><b>${esc(n ?? "—")}</b>${rest ? `<span class="of">${esc(rest)}</span>` : ""}</span>`;
  }

  /** The full strip (Overview): health, the configured cells, then Needs you.
   *  @param {Ctx} ctx */
  function strip(ctx) {
    const hl = health(ctx), nd = needs(ctx);
    const pill = hl.state ? h.pill(ctx, hl.state) : `<span class="muted">No health reported yet</span>`;
    const cells = ((ctx.config.strip || {}).cells || []).map((/** @type {Json} */ c) => {
      const v = c && c.label ? value(ctx, c) : null;
      return v == null ? "" : `<div class="cell" data-cell="${esc(c.id)}"><span class="l">${esc(c.label)}</span>${v}</div>`;
    }).join("");
    const n = nd.open.length;
    const ask = n
      ? `<a class="cell needs on" href="${esc(nd.href)}"><span class="l">Needs you</span><span class="v">${G.glyph("warn")}<b>${n}</b> ${n === 1 ? "question" : "questions"}</span></a>`
      : `<div class="cell needs"><span class="l">Needs you</span><span class="v quiet">Nothing waits on you</span></div>`;
    return `<div class="stripin"><div class="cell hl"><span class="l">Health</span><span class="hv">${pill}${
      hl.reason ? `<span class="why">${esc(hl.reason)}</span>` : ""}</span></div>${cells}${ask}</div>`;
  }

  /** Health and needs as pills in the top bar (every page but Overview).
   *  @param {Ctx} ctx */
  function compact(ctx) {
    const hl = health(ctx), nd = needs(ctx), n = nd.open.length;
    return `${hl.state ? h.pill(ctx, hl.state) : ""}${n ? `<a class="pill c-warn" href="${esc(nd.href)}">${G.glyph("warn")}${n} needs you</a>` : ""}`;
  }

  /** @param {HTMLElement} el @param {string} html */
  function put(el, html) {
    if (el.dataset.html !== html) { el.innerHTML = html; el.dataset.html = html; }
  }

  /** @param {Ctx} ctx @param {string} template */
  function render(ctx, template) {
    const over = (ctx.page || P.OVERVIEW) === P.OVERVIEW;
    const s = /** @type {HTMLElement} */ (document.getElementById("strip"));
    put(s, over ? strip(ctx) : "");
    s.hidden = !over;
    put(/** @type {HTMLElement} */ (document.getElementById("compact")), over ? "" : compact(ctx));
    put(/** @type {HTMLElement} */ (document.getElementById("tpl")), esc(template));
  }

  /** "live · 4s ago", "reconnecting", or the poll time. @param {{state: string, last: number}} st */
  function fresh(st) {
    const el = document.getElementById("fresh");
    if (!el) return;
    const secs = Math.max(0, Math.round((Date.now() - (st.last || Date.now())) / 1000));
    const ago = secs < 60 ? `${secs}s ago` : `${Math.floor(secs / 60)}m ago`;
    const text = st.state === "reconnecting" ? "reconnecting" : st.state === "connecting" ? "connecting" : `${st.state === "live" ? "live" : "updated"} · ${ago}`;
    el.dataset.state = st.state;
    put(/** @type {HTMLElement} */ (el.querySelector(".lt")), esc(text));
  }

  setInterval(() => { if (!window.MISSION.snapshot) fresh(window.MissionLive.status()); }, TICK_MS);

  window.MissionShell = { render, fresh, health, needs };
})();
