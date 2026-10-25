// @ts-check
// Board widgets: `kanban` (cards by state) and `checklist` (gates with requirements). Everything comes from
// records and section props; no domain names. (`flow` is an alias of `plan`, in widgets-plan.js.)
(function () {
  "use strict";
  const W = window.MissionWidget, h = W.h, esc = h.esc;
  /** Lane order when `lanes` isn't given: by tone, so work flows left to right. */
  const TONE_ORDER = ["todo", "warn", "prog", "rev", "block", "done"];

  /** @param {Ctx} ctx @param {string | null} state */
  const tone = (ctx, state) => (ctx.config.states || {})[state || ""] || "todo";
  /** @param {Rec} r @param {Section} s */
  const title = (r, s) => String(h.field(r, s.card_title || "title") ?? h.field(r, "name") ?? r.key);

  /** One card: title, key, chosen fields, row buttons.
   *  @param {Ctx} ctx @param {Section} s @param {Rec} r */
  function card(ctx, s, r) {
    /** @type {string[]} */
    const fields = s.fields || [];
    const meta = fields.map(f => h.field(r, f)).filter(v => v != null && v !== "").map(v => esc(typeof v === "object" ? JSON.stringify(v) : v));
    const acts = s.row_actions && s.row_actions.length ? `<div class="acts">${ctx.buttons(s.row_actions, r)}</div>` : "";
    const t = tone(ctx, r.state);
    return `<div class="kcard"><div class="small muted"><span class="t-${esc(t)}">${window.MissionGlyph.glyph(t)}</span>
      <span class="mono">${esc(r.key)}</span>${r.state ? ` · ${esc(h.word(r.state))}` : ""}</div>
      <div>${h.recordLink(r, esc(title(r, s)))}</div>${meta.length ? `<div class="small muted">${meta.join(" · ")}</div>` : ""}${acts}</div>`;
  }

  /** @type {WidgetRender} */
  function kanban(s, ctx) {
    const recs = [...(ctx.records[s.collection || ""] || [])].sort((a, b) => h.compare(a.key, b.key));
    if (!recs.length) return `<div class="card empty">No ${esc(s.collection || "cards")} yet.</div>`;
    /** @type {string[]} */
    const lanes = s.lanes || [...new Set(recs.map(r => r.state || ""))]
      .sort((a, b) => TONE_ORDER.indexOf(tone(ctx, a)) - TONE_ORDER.indexOf(tone(ctx, b)) || h.compare(a, b));
    const cols = lanes.map(l => {
      const cards = recs.filter(r => (r.state || "") === l);
      return `<div class="lane"><div class="lane-h">${h.pill(ctx, l || null)} <span class="small muted">${cards.length}</span></div>
        ${cards.map(r => card(ctx, s, r)).join("") || `<div class="small muted">—</div>`}</div>`;
    });
    return `<div class="board">${cols.join("")}</div>`;
  }

  /** @param {Rec} r @param {Section} s */
  const note = (r, s) => h.field(r, s.note || "note") ?? (s.note ? undefined : h.field(r, "answer"));

  /** One requirement: ✓ met, ✕ when the record it points at (`ref`) is blocked, else ○; with a word for
   *  screen readers. @param {Ctx} ctx @param {{text: string, done: boolean, ref: string | null}} q */
  function req(ctx, q) {
    const rec = q.ref ? h.find(ctx, q.ref) : null, blocked = !q.done && !!rec && tone(ctx, rec.state) === "block";
    const [mark, cls, word] = q.done ? ["✓", "met", "met"] : blocked ? ["✕", "blk", "blocked"] : ["○", "", "open"];
    const label = rec ? h.recordLink(rec, esc(q.text)) : esc(q.text);
    return `<li class="${cls}"><span class="mk" aria-hidden="true">${mark}</span><span>${label}${blocked ? ' <span class="small t-block">blocked</span>' : `<span class="sr"> (${word})</span>`}</span></li>`;
  }

  /** Gates (.design/components.md, "Checklist"): the next gate first-class with a --warn border, later ones
   *  after, passed ones folded under a line.
   *  @type {WidgetRender} */
  function checklist(s, ctx) {
    const recs = [...(ctx.records[s.collection || ""] || [])].sort((a, b) => h.compare(a.key, b.key));
    if (!recs.length) return `<div class="card empty">No ${esc(s.collection || "gates")} yet.</div>`;
    const passed = recs.filter(r => tone(ctx, r.state) === "done"), open = recs.filter(r => tone(ctx, r.state) !== "done");
    const card = (/** @type {Rec} */ r, /** @type {boolean} */ next) => {
      const items = window.MissionPlan.reqs(r, s.requirements), met = items.filter(i => i.done).length;
      const acts = s.row_actions && s.row_actions.length ? `<div class="acts">${ctx.buttons(s.row_actions, r)}</div>` : "";
      return `<div class="card pad gate${next ? " next" : ""}" data-record="${esc(r.collection)}/${esc(r.key)}"><div class="gate-h"><b>${esc(title(r, s))}</b>${h.pill(ctx, r.state)}</div>
        <div class="mono gmeta">${esc(r.key)}${items.length ? ` · ${met} of ${items.length} met` : ""}${next ? " · next" : ""}</div>
        ${items.length ? `<ul class="reqs">${items.map(q => req(ctx, q)).join("")}</ul>` : ""}
        ${note(r, s) ? `<div class="small md">${h.md(note(r, s))}</div>` : ""}${acts}</div>`;
    };
    const fold = passed.length ? `<details class="card pad gate passed"><summary><b>${passed.length} passed</b> <span class="mono small muted">${passed.map(r => esc(r.key)).join(", ")}</span></summary>
      ${passed.map(r => card(r, false)).join("")}</details>` : "";
    return `<div class="grid gates">${open.map((r, i) => card(r, i === 0)).join("")}${fold}</div>`;
  }

  W.define("kanban", kanban);
  W.define("checklist", checklist);
})();
