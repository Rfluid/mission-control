// @ts-check
// `graph` (.design/components.md, "System graph"): a component diagram the agent fills. Nodes are records
// grouped into columns (`nodes = {collection, group, title, task, note}`), edges come from another collection
// (`edges = {collection, from, to, label}`). A node can show a task's live state (`task = "U1"`); an edge into or
// out of a blocked node is drawn blocked. Same geometry, routing and viewport as `plan`.
(function () {
  "use strict";
  const W = window.MissionWidget, G = window.MissionGlyph, h = W.h, esc = h.esc;
  const L = /** @type {any} */ (globalThis).MissionPlanLayout;

  /** @type {WidgetRender} */
  function graph(s, ctx) {
    const N = s.nodes || {}, E = s.edges || {};
    const recs = [...(ctx.records[N.collection || ""] || [])].sort((a, b) => h.compare(a.key, b.key));
    if (!recs.length) return `<div class="card empty">No components yet.</div>`;
    const gf = N.group || "group", groupOf = (/** @type {Rec} */ r) => String(h.field(r, gf) ?? "");
    /** @type {string[]} */
    const groups = Array.isArray(s.groups) ? s.groups.map(String) : [...new Set(recs.map(groupOf))];
    const col = (/** @type {Rec} */ r) => groups.indexOf(groupOf(r));
    const by = new Map(recs.map(r => [r.key, r]));
    const edges = (ctx.records[E.collection || ""] || []).map(e => ({ rec: e, from: String(h.field(e, E.from || "from") ?? ""), to: String(h.field(e, E.to || "to") ?? ""), label: String(h.field(e, E.label || "label") ?? "") }))
      .filter(e => by.has(e.from) && by.has(e.to) && e.from !== e.to);
    // Rows: each node at the barycenter of the nodes linking into it from earlier columns.
    const input = recs.map(r => ({ key: r.key, phase: groupOf(r), deps: edges.filter(e => e.to === r.key && col(/** @type {Rec} */ (by.get(e.from))) < col(r)).map(e => e.from), blocked: false, done: false }));
    const g = L.layout(groups, input, { edges: false });
    const pos = new Map(g.nodes.map((/** @type {any} */ p) => [p.key, p]));
    const task = (/** @type {Rec} */ r) => { const t = h.field(r, N.task || "task"); return t ? h.find(ctx, t) : null; };
    const blocked = (/** @type {Rec} */ r) => { const t = task(r); return !!t && h.tone(ctx, t.state) === "block"; };
    const H = g.height, cx = (/** @type {number} */ i) => L.PAD_X + i * L.COL;
    const bands = g.cols.map((/** @type {string} */ name, /** @type {number} */ i) => `<div class="gband" style="left:${cx(i) - 12}px;width:${L.NODE_W + 24}px;height:${H - 16}px"></div><span class="lbl glabel" style="left:${cx(i)}px">${esc(name || "Other")}</span>`).join("");
    const paths = [], labels = [];
    for (const e of edges) {
      const a = pos.get(e.from), b = pos.get(e.to), bad = blocked(/** @type {Rec} */ (by.get(e.from))) || blocked(/** @type {Rec} */ (by.get(e.to)));
      const r = a.col <= b.col ? L.route(a, b, g.taken, e.label) : L.route(b, a, g.taken, e.label);
      paths.push(`<path class="${bad ? "e-block" : "e-met"}" d="${r.d}" ${a.col > b.col ? `marker-start="url(#gh-r${bad ? "b" : "m"}-${esc(s.id)})"` : `marker-end="url(#gh-${bad ? "b" : "m"}-${esc(s.id)})"`}/>`);
      if (r.label) labels.push(`<span class="plabel glab${bad ? " t-block" : ""}" style="left:${r.label.x - 76}px;top:${r.label.y - 8}px">${esc(e.label)}</span>`);
    }
    const nodes = recs.map(r => {
      const p = pos.get(r.key), t = task(r), title = h.title(r) || r.key;
      const line = t ? h.pill(ctx, t.state).replace(/(<\/svg>)/, `$1${esc(t.key)} `) : `<small>${esc(h.field(r, N.note || "note") ?? "")}</small>`;
      const target = t || r, ref = `${target.collection}/${target.key}`, q = new URLSearchParams(location.search);
      q.set("record", ref);
      const links = edges.filter(e => e.from === r.key).map(e => `${e.label || "to"} ${h.title(/** @type {Rec} */ (by.get(e.to)))}`);
      const name = `${title}, ${groupOf(r) || "Other"}${t ? `, ${t.key} ${h.word(t.state)}` : ""}${links.length ? `, ${links.join(", ")}` : ""}`;
      return `<a class="pnode gnode${blocked(r) ? " blk" : ""}" href="?${esc(q.toString())}" data-record="${esc(ref)}" style="left:${p.x}px;top:${p.y}px" aria-label="${esc(name)}"><span class="pt"><b>${esc(title)}</b>${line}</span></a>`;
    }).join("");
    const defs = `<marker id="gh-m-${esc(s.id)}" orient="auto" markerWidth="8" markerHeight="8" refX="6" refY="4" markerUnits="userSpaceOnUse"><path d="M0 .5L7 4L0 7.5Z" class="ah-m"/></marker>
      <marker id="gh-b-${esc(s.id)}" orient="auto" markerWidth="8" markerHeight="8" refX="6" refY="4" markerUnits="userSpaceOnUse"><path d="M0 .5L7 4L0 7.5Z" class="ah-b"/></marker>
      <marker id="gh-rm-${esc(s.id)}" orient="auto-start-reverse" markerWidth="8" markerHeight="8" refX="6" refY="4" markerUnits="userSpaceOnUse"><path d="M0 .5L7 4L0 7.5Z" class="ah-m"/></marker>
      <marker id="gh-rb-${esc(s.id)}" orient="auto-start-reverse" markerWidth="8" markerHeight="8" refX="6" refY="4" markerUnits="userSpaceOnUse"><path d="M0 .5L7 4L0 7.5Z" class="ah-b"/></marker>`;
    const V = window.MissionViewport;
    return `<div class="card pgraph sysgraph"><div class="pscroll" data-viewport="${esc(s.id)}" tabindex="0" aria-label="${esc(s.title || s.id)}: drag to pan, + and − zoom, 0 fits, f fullscreen">
      <div class="pcanvas vcanvas" style="width:${g.width}px;height:${H - 16}px;${V.style(s.id)}">${bands}
      <svg class="pedges" width="${g.width}" height="${H - 16}" aria-hidden="true"><defs>${defs}</defs>${paths.join("")}</svg>${labels.join("")}${nodes}</div></div>
      <div class="plegend vbar">${V.controls(s.id)}</div></div>`;
  }

  W.define("graph", graph, { count: (s, ctx) => { const n = (ctx.records[(s.nodes || {}).collection || ""] || []).length; return n ? `${n} components` : ""; } });
})();
