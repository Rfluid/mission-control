// @ts-check
// The plan model shared by the `plan` graph, the Overview's phase track (`track`) and the strip's phase cell:
// phases (columns), their records, gates between them and the current phase. All names come from props:
// `phases = {collection, label}`, `nodes = {collection, phase, deps, title}`, `gates = {collection, after}`.
(function () {
  "use strict";
  const W = window.MissionWidget, G = window.MissionGlyph, h = W.h, esc = h.esc;

  /** First present value among one field or a list of them. @param {Rec} r @param {Json} f */
  const pick = (r, f) => {
    for (const k of [f].flat()) { const v = h.field(r, k); if (v != null && v !== "") return v; }
    return undefined;
  };

  /** A gate's requirements: strings or `{text, done, ref}` in `requirements` (or `items`, `requires`).
   *  @param {Rec} r @param {string} [field] */
  function reqs(r, field) {
    const raw = (field ? h.field(r, field) : h.field(r, "requirements") ?? h.field(r, "items") ?? h.field(r, "requires")) ?? [];
    return (Array.isArray(raw) ? raw : []).map(x => (typeof x === "object" && x
      ? { text: String(x.text ?? x.title ?? ""), done: !!x.done, ref: x.ref == null ? null : String(x.ref) }
      : { text: String(x), done: false, ref: null }));
  }

  /** @param {Section} s @param {Ctx} ctx */
  function model(s, ctx) {
    const P = s.phases || {}, N = s.nodes || {}, Gt = s.gates || {};
    const phaseField = N.phase || ["phase", "wave"], depField = N.deps || "depends_on";
    // `phases.collection` may list several names: the first one with records wins.
    const pcoll = [P.collection || []].flat().find(c => (ctx.records[c] || []).length);
    const precs = [...(ctx.records[pcoll || ""] || [])].sort((a, b) => h.compare(a.key, b.key));
    const nodes = [...(ctx.records[N.collection || ""] || [])].sort((a, b) => h.compare(a.key, b.key));
    const phaseOf = (/** @type {Rec} */ r) => String(pick(r, phaseField) ?? "");
    const keys = precs.length ? precs.map(p => p.key) : [...new Set(nodes.map(phaseOf).filter(Boolean))].sort(h.compare);
    const name = (/** @type {string} */ k) => {
      const r = precs.find(p => p.key === k);
      return r ? String(h.field(r, P.label || "title") ?? h.field(r, "name") ?? k) : k;
    };
    const tone = (/** @type {Rec} */ r) => h.tone(ctx, r.state);
    const done = (/** @type {Rec} */ r) => tone(r) === "done";
    const phases = keys.map(k => ({ key: k, name: name(k), nodes: nodes.filter(n => phaseOf(n) === k) }));
    const loose = nodes.filter(n => !keys.includes(phaseOf(n)));
    if (loose.length) phases.push({ key: "", name: "Unphased", nodes: loose });
    const current = phases.findIndex(p => p.nodes.some(n => !done(n)));
    const gates = [...(ctx.records[Gt.collection || ""] || [])].sort((a, b) => h.compare(a.key, b.key))
      .map(g => ({ rec: g, after: String(h.field(g, Gt.after || "after") ?? ""), reqs: reqs(g, Gt.requirements) }));
    // The next gate is the first one not passed; it's the one marked warn.
    const next = gates.find(g => tone(g.rec) !== "done");
    const gateTone = (/** @type {{rec: Rec}} */ g) => (tone(g.rec) === "done" ? "done" : g === next ? "warn" : "");
    /** @param {Rec} r @returns {string[]} */
    const deps = r => [h.field(r, depField) ?? []].flat().map(String);
    return { phases, nodes, gates, current, tone, done, deps, gateTone, phaseOf, title: (/** @type {Rec} */ r) => String(pick(r, N.title || ["title", "name"]) ?? "") };
  }

  /** Segments of a phase mix bar: one per record, settled states first (done → rev → prog → block → todo).
   *  @param {ReturnType<typeof model>} m @param {Rec[]} recs */
  function mix(m, recs) {
    const order = ["done", "rev", "prog", "warn", "block", "todo"];
    return recs.map(r => m.tone(r)).sort((a, b) => order.indexOf(a) - order.indexOf(b))
      .map(t => `<i class="m-${t}"></i>`).join("");
  }

  /** A tone in a phase's counts. */
  const TONE_WORD = { done: "done", rev: "review", prog: "running", warn: "waiting", block: "blocked", todo: "todo" };
  /** "1 done · 1 review · 1 blocked" (blocked in its tone), or "2/2 done" when the phase is finished.
   *  @param {Ctx} ctx @param {ReturnType<typeof model>} m @param {Rec[]} recs */
  function counts(ctx, m, recs) {
    if (recs.length && recs.every(r => m.done(r))) return `${recs.length}/${recs.length} done`;
    /** @type {Map<string, number>} */
    const by = new Map();
    for (const r of recs) by.set(m.tone(r), (by.get(m.tone(r)) || 0) + 1);
    return Object.entries(TONE_WORD).filter(([t]) => by.has(t))
      .map(([t, w]) => `<span${t === "block" ? ' class="t-block"' : ""}>${by.get(t)} ${w}</span>`).join(" · ");
  }

  /** The current phase for the strip's phase cell. @param {Section} s @param {Ctx} ctx */
  function phase(s, ctx) {
    const m = model(s, ctx);
    if (!m.phases.length) return null;
    const i = m.current < 0 ? m.phases.length - 1 : m.current;
    return { index: i, name: m.phases[i].name, phases: m.phases.map((_, j) => (j < i || m.current < 0 ? "done" : j === i ? "now" : "")) };
  }

  /** Overview's small plan: phase blocks with gates between them. `plan` names the plan section it reads.
   *  @type {WidgetRender} */
  function track(s, ctx) {
    const src = (ctx.config.sections || []).find((/** @type {Section} */ x) => x.id === s.plan) || s;
    const m = model(src, ctx);
    if (!m.phases.length) return `<div class="card empty">No phases yet.</div>`;
    const blocks = m.phases.map((p, i) => {
      const gate = m.gates.find(g => g.after === p.key);
      const met = gate ? gate.reqs.filter(r => r.done).length : 0;
      const gt = gate ? m.gateTone(gate) : "";
      const g = gate && i < m.phases.length - 1 ? `<div class="tgate" title="${esc(m.title(gate.rec) || gate.rec.key)}">${G.diamond(gt)}<span class="mono${gt ? ` t-${gt}` : ""}">${met}/${gate.reqs.length}</span></div>` : "";
      const now = i === m.current;
      return `<div class="tphase${now ? " now" : ""}"><span class="lbl">${p.key ? `Phase ${i + 1}` : "Unphased"}${now ? " · now" : ""}</span><b>${esc(p.name)}</b>
        <span class="mixbar" aria-hidden="true">${mix(m, p.nodes)}</span><span class="mono small muted">${counts(ctx, m, p.nodes) || "empty"}</span></div>${g}`;
    }).join("");
    return `<div class="card track">${blocks}</div>`;
  }

  /** "4 phases · 8 tasks · 3 gates". @type {WidgetCount} */
  function trackCount(s, ctx) {
    const src = (ctx.config.sections || []).find((/** @type {Section} */ x) => x.id === s.plan) || s;
    const m = model(src, ctx), n = (/** @type {number} */ k, /** @type {string} */ w) => `${k} ${w}${k === 1 ? "" : "s"}`;
    return m.phases.length ? [n(m.phases.length, "phase"), n(m.nodes.length, "task"), m.gates.length ? n(m.gates.length, "gate") : ""].filter(Boolean).join(" · ") : "";
  }

  W.define("track", track, { count: trackCount });
  window.MissionPlan = { phase, model, mix, counts, reqs };
})();
