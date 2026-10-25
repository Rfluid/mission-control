// @ts-check
// `plan` (.design/components.md, "Plan graph"): phases as columns, records as nodes, dependencies as edges
// styled by their source (met, waiting, blocked upstream), gates between phases, the current phase banded,
// an always-on legend. Graph / Table / Board views and "Trace blocked path" in the section header; j/k walk
// the nodes. `flow` is an alias that draws no edges. Layout: plan-layout.js; model: widgets-track.js.
(function () {
  "use strict";
  const W = window.MissionWidget, G = window.MissionGlyph, h = W.h, esc = h.esc;
  const L = /** @type {any} */ (globalThis).MissionPlanLayout;
  /** @type {Map<string, {view: string, trace: boolean}>} per section, kept per browser */
  const state = new Map();
  const store = (/** @type {string} */ id) => `mission-control.plan.${window.MISSION.id}.${id}`;

  /** @param {string} id */
  function ui(id) {
    if (!state.has(id)) {
      let saved = null;
      try { saved = JSON.parse(localStorage.getItem(store(id)) || "null"); } catch { /* private window */ }
      state.set(id, { view: "graph", trace: false, ...(saved && typeof saved === "object" ? saved : {}) });
    }
    return /** @type {{view: string, trace: boolean}} */ (state.get(id));
  }

  /** A node's sub-line: state · agent/PR, or · age when blocked, or · waits on X; plus "needs you".
   *  @param {Ctx} ctx @param {any} m @param {Rec} r @param {Map<string, Rec>} by @param {Set<string>} asked */
  function sub(ctx, m, r, by, asked) {
    const waiting = m.deps(r).filter((/** @type {string} */ d) => by.has(d) && !m.done(by.get(d)));
    const parts = [esc(h.word(r.state || "todo"))];
    if (m.tone(r) === "block") parts.push(esc(h.ago(r.updated)));
    else if (waiting.length && !m.done(r)) parts.push(`waits on ${esc(waiting.length === 1 ? waiting[0] : waiting.length)}`);
    else {
      const agent = h.field(r, "agent"), pr = h.field(r, "pr");
      if (agent) parts.push(esc(agent));
      if (pr) parts.push(`#${esc(pr)}`);
    }
    if (asked.has(r.key)) parts.push('<span class="t-warn">needs you</span>');
    return parts.join(" · ");
  }

  /** Keys of records an open need points at (`blocks`). @param {Ctx} ctx */
  function askedOf(ctx) {
    const out = new Set();
    for (const n of ctx.records.needs || []) {
      if (n.state === "answered") continue;
      for (const ref of [n.body && n.body.blocks].flat()) if (ref) out.add(String(ref).split("/").pop());
    }
    return out;
  }

  /** @param {Section} s @param {Ctx} ctx @param {any} m @param {{trace: boolean}} st */
  function graph(s, ctx, m, st) {
    const recs = /** @type {Rec[]} */ (m.nodes), by = new Map(recs.map(r => [r.key, r]));
    const input = recs.map(r => ({ key: r.key, phase: m.phaseOf(r), deps: m.deps(r), blocked: m.tone(r) === "block", done: m.done(r) }));
    const g = L.layout(m.phases.filter((/** @type {any} */ p) => p.key).map((/** @type {any} */ p) => p.key), input, { edges: s.edges !== false });
    if (g.cycles.length) ctx.issue && ctx.issue(`sections[${s.id}]`, `dependency cycle ${g.cycles.map((/** @type {string[]} */ c) => c.join(" → ")).join("; ")}: those edges aren't drawn`);
    const hot = new Set([...recs.filter(r => m.tone(r) === "block").map(r => r.key), ...g.downstream]);
    const dim = (/** @type {boolean} */ on) => (st.trace && !on ? " dim" : "");
    const asked = askedOf(ctx), holds = (/** @type {string} */ k) => recs.filter(r => m.deps(r).includes(k)).map(r => r.key);
    const H = g.height, col = (/** @type {number} */ i) => L.PAD_X + i * L.COL;
    const bands = m.current >= 0 ? `<div class="pband" style="left:${col(m.current) - 12}px;width:${L.NODE_W + 24}px;height:${H - 8}px"></div>` : "";
    const heads = m.phases.map((/** @type {any} */ p, /** @type {number} */ i) => {
      const doneN = p.nodes.filter((/** @type {Rec} */ r) => m.done(r)).length, now = i === m.current;
      return `<div class="phead${now ? " now" : ""}" style="left:${col(i)}px"><span class="lbl">${p.key ? `Phase ${i + 1} · ` : ""}${esc(p.name)}${now ? " · now" : ""}</span>
        <span class="phbar"><span class="mixbar" aria-hidden="true">${window.MissionPlan.mix(m, p.nodes)}</span><span class="mono">${doneN}/${p.nodes.length}</span></span></div>`;
    }).join("");
    let rules = "", gates = "";
    m.phases.forEach((/** @type {any} */ p, /** @type {number} */ i) => {
      if (i >= m.phases.length - 1) return;
      const x = col(i) + L.NODE_W + L.GAP / 2;
      rules += `<path d="M${x} 52V${H - 8}"/>`;
      const mine = m.gates.filter((/** @type {any} */ gt) => gt.after === p.key);
      gates += mine.map((/** @type {any} */ gt, /** @type {number} */ j) => {
        const t = m.gateTone(gt), met = gt.reqs.filter((/** @type {any} */ q) => q.done).length;
        return `<div class="pgate" style="left:${x - 32}px;top:${10 + j * 34}px" title="${esc(m.title(gt.rec) || gt.rec.key)}">${G.diamond(t)}<span class="mono${t ? ` t-${t}` : ""}">${esc(gt.rec.key)} ${met}/${gt.reqs.length}</span></div>`;
      }).join("");
    });
    const edges = g.edges.map((/** @type {any} */ e) => `<path class="e-${e.style}${dim(e.style === "block")}" d="${e.d}" marker-end="url(#ah-${e.style === "block" ? "b" : "m"}-${esc(s.id)})"/>`).join("");
    const labels = g.edges.filter((/** @type {any} */ e) => e.label).map((/** @type {any} */ e) =>
      `<span class="plabel" style="left:${e.label.x - 76}px;top:${e.label.y - 8}px">${esc(e.label.text)}</span>`).join("");
    const nodes = g.nodes.map((/** @type {any} */ p, /** @type {number} */ i) => {
      const r = /** @type {Rec} */ (by.get(p.key)), t = m.tone(r), title = m.title(r);
      const deps = m.deps(r), held = holds(r.key);
      const name = `${r.key} ${title}, ${h.word(r.state || "todo")}${deps.length ? `, depends on ${deps.join(", ")}` : ""}${held.length ? `, holds ${held.join(", ")}` : ""}`;
      const ref = `${r.collection}/${r.key}`, q = new URLSearchParams(location.search);
      q.set("record", ref);
      return `<a class="pnode${t === "block" ? " blk" : ""}${dim(hot.has(r.key))}" href="?${esc(q.toString())}" data-record="${esc(ref)}" data-walk="${i}"
        style="left:${p.x}px;top:${p.y}px" aria-label="${esc(name)}"><span class="t-${t} pg">${G.glyph(t)}</span><span class="pt"><span class="mono muted">${esc(r.key)}</span> ${esc(title)}
        <small class="${t === "block" ? "t-block" : ""}">${sub(ctx, m, r, by, asked)}</small></span></a>`;
    }).join("");
    const defs = ["m", "b"].map(k => `<marker id="ah-${k}-${esc(s.id)}" orient="auto" markerWidth="8" markerHeight="8" refX="6" refY="4" markerUnits="userSpaceOnUse"><path d="M0 .5L7 4L0 7.5Z" class="ah-${k}"/></marker>`).join("");
    const V = window.MissionViewport, vid = s.id;
    return `<div class="card pgraph"><div class="pscroll" data-viewport="${esc(vid)}" tabindex="0" aria-label="${esc(s.title || s.id)}: drag to pan, + and − zoom, 0 fits, f fullscreen">
      <div class="pcanvas vcanvas" style="width:${g.width}px;height:${H}px;${V.style(vid)}">${bands}
      <svg class="pedges" width="${g.width}" height="${H}" aria-hidden="true"><defs>${defs}</defs><g class="prules">${rules}</g>${edges}</svg>
      ${heads}${gates}${labels}${nodes}</div></div>${legend(V.controls(vid))}</div>
      <p class="phint small muted">Click a task for its drawer. <span class="kbd">j</span> <span class="kbd">k</span> move between tasks, <span class="kbd">⏎</span> opens.</p>`;
  }

  /** Always shown, never collapsible: node glyphs, edge styles, the gate and the current-phase bar. */
  /** @param {string} tools viewport controls, at the legend's right end */
  function legend(tools) {
    const glyph = (/** @type {string} */ t, /** @type {string} */ w) => `<span><span class="t-${t}">${G.glyph(t)}</span>${w}</span>`;
    const line = (/** @type {string} */ c, /** @type {string} */ w) => `<span><svg width="28" height="8" aria-hidden="true"><path d="M1 4h26" class="${c}"/></svg>${w}</span>`;
    return `<div class="plegend"><span class="lbl">Legend</span>${glyph("done", "done")}${glyph("prog", "in progress")}${glyph("rev", "in review")}${glyph("block", "blocked")}${glyph("todo", "todo")}
      ${line("e-met", "met")}${line("e-wait", "waiting")}${line("e-block", "blocked upstream")}<span>${G.diamond("warn", 12)}gate</span><span><i class="nowbar"></i>current phase</span>${tools}</div>`;
  }

  /** Table view columns when the section names none. */
  const COLUMNS = [{ id: "key", label: "Task", as: "code" }, { id: "title", label: "Name", or: "name" }, { id: "phase", as: "code" },
    { id: "state", as: "pill" }, { id: "depends_on", label: "Depends on", as: "code" }, { id: "agent", as: "code" }];

  /** The same records as a table (the `table` widget, with the section's `columns`). @param {Section} s @param {Ctx} ctx */
  function table(s, ctx) {
    const tw = W.get("table"), coll = (s.nodes || {}).collection;
    return tw ? tw({ ...s, collection: coll, columns: s.columns || COLUMNS, sort: s.sort || "key", empty: "No tasks yet." }, ctx) : "";
  }

  /** @type {WidgetRender} */
  function plan(s, ctx) {
    const m = window.MissionPlan.model(s, ctx);
    if (!m.nodes.length && !m.phases.length) return `<div class="card empty">Nothing in the plan yet.</div>`;
    const st = ui(s.id);
    if (st.view === "table") return table(s, ctx);
    if (st.view === "board") {
      const kb = W.get("kanban");
      return kb ? kb({ ...s, collection: (s.nodes || {}).collection, fields: ["phase"] }, ctx) : table(s, ctx);
    }
    return graph(s, ctx, m, st);
  }

  /** "3/8 done · 1 blocked". @type {WidgetCount} */
  function count(s, ctx) {
    const m = window.MissionPlan.model(s, ctx), blocked = m.nodes.filter((/** @type {Rec} */ r) => m.tone(r) === "block").length;
    return m.nodes.length ? `${m.nodes.filter((/** @type {Rec} */ r) => m.done(r)).length}/${m.nodes.length} done${blocked ? ` · ${blocked} blocked` : ""}` : "";
  }

  /** Graph / Table / Board and Trace blocked path, in the section header. @param {Section} s */
  function tools(s) {
    const st = ui(s.id), on = (/** @type {boolean} */ b) => `aria-pressed="${b}"`;
    const seg = ["graph", "table", "board"].map(v => `<button type="button" data-plan-view="${v}" data-plan="${esc(s.id)}" ${on(st.view === v)}>${v[0].toUpperCase()}${v.slice(1)}</button>`).join("");
    return `<span class="seg" role="group" aria-label="View">${seg}</span>${st.view === "graph" && s.edges !== false
      ? `<button class="btn sm" type="button" data-plan-trace="${esc(s.id)}" ${on(st.trace)}>Trace blocked path</button>` : ""}`;
  }

  document.addEventListener("click", e => {
    const b = e.target instanceof Element ? /** @type {HTMLElement | null} */ (e.target.closest("[data-plan-view], [data-plan-trace]")) : null;
    if (!b) return;
    const id = b.dataset.plan || b.dataset.planTrace || "", st = ui(id);
    if (b.dataset.planView) st.view = b.dataset.planView;
    else st.trace = !st.trace;
    try { localStorage.setItem(store(id), JSON.stringify(st)); } catch { /* not kept */ }
    document.dispatchEvent(new CustomEvent("mission:render"));
  });

  // j / k (and the arrow keys) walk the nodes in phase, then row order; Enter opens one (it's a link).
  document.addEventListener("keydown", e => {
    const t = e.target instanceof HTMLElement ? e.target : null;
    if (!t || !t.matches(".pnode") || e.metaKey || e.ctrlKey || e.altKey) return;
    const step = e.key === "j" || e.key === "ArrowDown" ? 1 : e.key === "k" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    const all = [...(t.closest(".pcanvas") || document).querySelectorAll(".pnode")];
    const next = /** @type {HTMLElement | undefined} */ (all[all.indexOf(t) + step]);
    if (next) { e.preventDefault(); next.focus(); }
  });

  W.define("plan", plan, { count, tools });
  // `flow` (v1): lanes of nodes with gates; now a plan with no edges.
  W.define("flow", (s, ctx) => plan({ ...s, phases: s.lanes, nodes: { ...(s.nodes || {}), phase: (s.nodes || {}).lane || "lane" }, edges: false }, ctx),
    { count: (s, ctx) => count({ ...s, phases: s.lanes, nodes: { ...(s.nodes || {}), phase: (s.nodes || {}).lane || "lane" } }, ctx) });
})();
