// @ts-check
// Core widgets: `kpis`, `health`, `table`, `feed`, `markdown`. Everything comes from records and section props.
(function () {
  "use strict";
  const W = window.MissionWidget, h = W.h, esc = h.esc;
  const DEFAULT_FEED = 50;

  /** @param {Ctx} ctx @param {{id: string, link?: string, as?: string, or?: string}} col @param {Rec} rec */
  function cell(ctx, col, rec) {
    let v = h.field(rec, col.id);
    if ((v == null || v === "") && col.or) v = h.field(rec, col.or);
    if (col.link) {
      const tpl = (ctx.config.links || {})[col.link];
      const url = tpl && h.safeUrl(h.fill(ctx, tpl, v, rec));
      if (url && v != null && v !== "") return `<a href="${esc(url)}" target="_blank" rel="noopener">${esc(v)}</a>`;
    }
    switch (col.as) {
      case "pill": return h.pill(ctx, v);
      case "code": return v == null || v === "" || (Array.isArray(v) && !v.length) ? "—" : `<span class="mono">${esc(Array.isArray(v) ? v.join(", ") : v)}</span>`;
      case "muted": return `<span class="small muted">${esc(v ?? "")}</span>`;
      case "time": return `<span class="small">${esc(h.time(v))}</span>`;
      case "age": return `<span class="mono small muted">${esc(h.ago(v))}</span>`;
      case "markdown": return `<div class="md">${h.md(v ?? "")}</div>`;
      case "link": { const u = h.safeUrl(v); return u ? `<a href="${esc(u)}" target="_blank" rel="noopener">${esc(v)}</a>` : esc(v ?? "—"); }
      default: return v == null || v === "" ? "—" : esc(Array.isArray(v) ? v.join(", ") : typeof v === "object" ? JSON.stringify(v) : v);
    }
  }

  /** A KPI tile: label, value (with " / total" and a bar when it has one), optional muted note.
   *  @param {Ctx} ctx @param {Json} it */
  function tile(ctx, it) {
    const m = h.metric(ctx, it);
    const of = m.of != null ? `<span class="of"> / ${esc(h.fmt(m.of))}</span>` : "";
    const bar = m.of ? `<span class="kbar"><i style="width:${Math.min(100, Math.round((Number(m.n) || 0) / m.of * 100))}%"></i></span>` : "";
    const note = it.note == null ? "" : typeof it.note === "string" ? it.note : h.metric(ctx, it.note).text;
    const v = typeof m.n === "number" ? h.fmt(m.n, it.format) : m.n;
    return `<div class="card pad kpi"><div class="l">${esc(it.label)}</div><div class="v">${esc(v ?? "—")}${of}</div>${bar}${
      note ? `<div class="note">${esc(note)}</div>` : ""}</div>`;
  }

  /** A feed row's kind color: needs warn, blocks block (a `state` event into a blocked state), else muted.
   *  @param {Ctx} ctx @param {Ev} e */
  function kindTone(ctx, e) {
    if (e.kind === "need") return "warn";
    if (e.kind === "block") return "block";
    const to = e.data && typeof e.data === "object" ? e.data.to : null;
    const t = e.kind === "state" && to ? h.tone(ctx, to) : "";
    return t === "block" || t === "warn" ? t : "";
  }

  /** HH:MM today, else a short date. @param {number} ms */
  function when(ms) {
    const d = new Date(ms), now = new Date();
    return d.toDateString() === now.toDateString()
      ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })
      : d.toLocaleDateString([], { month: "short", day: "numeric" });
  }

  W.define("kpis", (s, ctx) => `<div class="grid kpis">${(s.items || []).map((/** @type {Json} */ it) => tile(ctx, it)).join("")}</div>`);

  W.define("health", (s, ctx) => {
    const r = s.record && (ctx.records[s.record.collection] || []).find(x => x.key === s.record.key);
    const state = r ? r.state : (ctx.config.vars || {}).health;
    const reason = r ? h.field(r, "reason") : (ctx.config.vars || {}).health_reason;
    if (!state) return `<div class="card empty">No health reported yet.</div>`;
    return `<div class="card pad health">${h.pill(ctx, state)}${reason ? `<span class="why">${esc(reason)}</span>` : ""}</div>`;
  });

  W.define("table", (s, ctx) => {
    /** @type {[string, Json][]} */
    const filter = Object.entries(s.filter || {});
    const rows = (ctx.records[s.collection] || []).filter(r =>
      filter.every(([f, want]) => [want].flat().some(w => String(h.field(r, f)) === String(w))));
    // Default: by key, so rows keep their place as they update. `tone` puts the loudest states first.
    const sort = /** @type {string} */ (s.sort || "key"), desc = sort.startsWith("-"), f = sort.replace(/^-/, "");
    rows.sort((a, b) => (f === "tone" ? h.toneRank(ctx, a.state) - h.toneRank(ctx, b.state) || h.compare(a.key, b.key)
      : h.compare(h.field(a, f), h.field(b, f))) * (desc ? -1 : 1));
    /** @type {{id: string, label?: string, link?: string, as?: string, or?: string}[]} */
    const cols = s.columns || [{ id: "key" }, { id: "state", as: "pill" }];
    const head = cols.map(c => c.label || c.id.replace(/_/g, " "));
    const acts = s.row_actions && s.row_actions.length;
    if (acts) head.push("");
    const body = rows.map(r => `<tr data-collection="${esc(r.collection)}" data-key="${esc(r.key)}" tabindex="0">${cols.map(c => `<td>${cell(ctx, c, r)}</td>`).join("")}${
      acts ? `<td class="acts">${ctx.buttons(s.row_actions, r)}</td>` : ""}</tr>`);
    return `<div class="card tw">${h.table(head, body, s.empty || `No ${s.collection} yet.`)}</div>`;
  });

  W.define("feed", (s, ctx) => {
    /** @type {string[] | null} */
    const kinds = s.kinds ? [s.kinds].flat() : null;
    const all = ctx.events.filter(e => !kinds || kinds.includes(e.kind));
    const evs = all.slice(-((s.limit || DEFAULT_FEED) + (s.go ? 0 : ctx.more))).reverse();
    if (!evs.length) return `<div class="card empty">No activity yet.</div>`;
    // More to show: loaded but cut by the limit, or still on the daemon. A feed that links elsewhere (`go`,
    // the Overview rail) sends the reader there instead of paging.
    const older = !s.go && (all.length > evs.length || ctx.eventCount > ctx.events.length);
    return `<div class="card feed">${evs.map(e => {
      const t = kindTone(ctx, e);
      return `<div class="row"><span class="mono muted">${esc(when(e.at))}</span><span class="mono k${t ? ` t-${t}` : ""}">${esc(e.kind)}</span><span class="tx">${esc(e.text)}</span></div>`;
    }).join("")}${older ? `<div class="row more"><button class="btn" type="button" data-older>Older</button></div>` : ""}</div>`;
  });

  // `doc`: long-form markdown from a record (`record = {collection, key, field}`, default field `body`) or
  // `text`: real headings, 68ch measure, ```mermaid blocks drawn as diagrams (mermaid.js).
  W.define("doc", (s, ctx) => {
    let text = s.text;
    if (s.record) {
      const r = (ctx.records[s.record.collection] || []).find(x => x.key === s.record.key);
      text = r ? h.field(r, s.record.field || "body") : null;
    }
    if (!text) return `<div class="card empty">Nothing written yet.</div>`;
    return `<div class="card doc md">${h.md(text, { headings: true })}</div>`;
  });

  W.define("markdown", (s, ctx) => {
    let text = s.text;
    if (s.record) {
      const r = (ctx.records[s.record.collection] || []).find(x => x.key === s.record.key);
      text = r ? h.field(r, s.record.field || "text") : null;
    }
    if (!text) return `<div class="card empty">Nothing here yet.</div>`;
    return `<div class="card pad md">${h.md(text)}</div>`;
  });
})();
