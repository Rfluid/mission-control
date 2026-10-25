// @ts-check
// The record drawer (.design/components.md, "Drawer"): one record in depth, from any node, row, card or
// `?record=<collection>/<key>` link. Why it's blocked, its fields, what it depends on and holds up, its
// history (state changes, answers and buttons, from the daemon's events), the raw record and its row buttons.
// Focus is trapped while open; Esc closes and returns focus; j/k (↓/↑ buttons) walk the widget's records.
(function () {
  "use strict";
  const W = window.MissionWidget, G = window.MissionGlyph, h = W.h, esc = h.esc;
  const HISTORY = 50; // events fetched for a record
  /** Fields shown elsewhere in the drawer, so the field list leaves them out. */
  const SHOWN = ["title", "name", "reason", "note", "question", "context", "options", "depends_on", "requirements", "phase"];
  /** @type {{ref: string, section: string, back: HTMLElement | null, list: string[]} | null} */
  let open = null;
  /** @type {() => Ctx | null} */
  let getCtx = () => null;
  /** @type {Map<string, {stamp: string, events: Ev[] | null}>} */
  const past = new Map();

  const $ = (/** @type {string} */ id) => /** @type {HTMLElement} */ (document.getElementById(id));

  /** The record a `collection/key` ref names. @param {Ctx} ctx @param {string} ref */
  function recOf(ctx, ref) {
    const i = ref.indexOf("/");
    return i > 0 ? (ctx.records[ref.slice(0, i)] || []).find(r => r.key === ref.slice(i + 1)) || null : null;
  }

  /** The section a record came from: the one it was opened in, else one on this page that reads its
   *  collection, else any. @param {Ctx} ctx @param {Rec} r @param {string} hint */
  function sectionOf(ctx, r, hint) {
    const all = /** @type {Section[]} */ (ctx.config.sections || []);
    const reads = (/** @type {Section} */ s) => s.collection === r.collection || (s.nodes || {}).collection === r.collection;
    const page = window.MissionPages.sectionsOn(ctx.config, ctx.page || "overview");
    return all.find(s => s.id === hint && reads(s)) || page.find(reads) || all.find(reads) || null;
  }

  /** A linked line for another record: glyph, key, title, state word. @param {Ctx} ctx @param {Rec} r @param {string} [extra] */
  function line(ctx, r, extra) {
    const t = h.tone(ctx, r.state);
    return h.recordLink(r, `<span class="t-${t}">${G.glyph(t)}</span><span class="mono muted">${esc(r.key)}</span><span class="dt">${esc(h.title(r) || (r.body || {}).question || "")}</span><span class="dw">${esc(extra ?? h.word(r.state || ""))}</span>`);
  }

  /** Depends on / Holding up: deps from the section's `nodes.deps` (default `depends_on`), records whose deps
   *  name this one, gates whose requirements point at it, open needs that block it. @param {Ctx} ctx @param {Rec} r @param {Section | null} s */
  function relations(ctx, r, s) {
    const depField = (s && s.nodes && s.nodes.deps) || "depends_on";
    const same = ctx.records[r.collection] || [];
    const deps = [h.field(r, depField) ?? []].flat().map(String).map(k => same.find(x => x.key === k)).filter(Boolean);
    const asks = (ctx.records.needs || []).filter(n => n.state !== "answered" && [n.body && n.body.blocks].flat().some(b => b && [r.key, `${r.collection}/${r.key}`].includes(String(b))));
    const holds = same.filter(x => [h.field(x, depField) ?? []].flat().map(String).includes(r.key));
    const gcoll = s && s.gates && s.gates.collection;
    const gates = gcoll ? (ctx.records[gcoll] || []).filter(g => window.MissionPlan.reqs(g).some(q => q.ref === r.key)) : [];
    const on = [...deps.map(d => line(ctx, /** @type {Rec} */ (d))), ...asks.map(n => line(ctx, n, "needs you"))];
    const up = [...holds.map(x => line(ctx, x)), ...gates.map(g => {
      const q = window.MissionPlan.reqs(g), met = q.filter(x => x.done).length;
      return h.recordLink(g, `${G.diamond(h.tone(ctx, g.state) === "done" ? "done" : "warn", 12)}<span class="mono muted">${esc(g.key)}</span><span class="dt">${esc(h.title(g))}</span><span class="dw">${met}/${q.length} met</span>`);
    })];
    const list = (/** @type {string} */ label, /** @type {string[]} */ items) => items.length ? `<div class="dsec"><span class="lbl">${label}</span><div class="deps">${items.join("")}</div></div>` : "";
    return list("Depends on", on) + list("Holding up", up);
  }

  /** @param {Ctx} ctx @param {Ev} e */
  function event(ctx, e) {
    const d = e.data && typeof e.data === "object" ? e.data : {};
    const at = new Date(e.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
    if ((e.kind === "state" || e.kind === "need") && d.to) {
      const t = h.tone(ctx, d.to);
      return `<li><span class="mono muted">${esc(at)}</span><span class="t-${t}">${G.glyph(t)}</span><span>${d.from ? `${esc(h.word(d.from))} → ` : ""}<b>${esc(h.word(d.to))}</b></span></li>`;
    }
    return `<li><span class="mono muted">${esc(at)}</span><span class="mono muted dk">${esc(e.kind)}</span><span>${esc(e.text)}</span></li>`;
  }

  /** Fetches the record's history when something new happened to it. @param {Ctx} ctx @param {Rec} r */
  function events(ctx, r) {
    const ref = `${r.collection}/${r.key}`;
    const stamp = `${r.updated}:${ctx.events.filter(e => e.data && (e.data.key === r.key || (e.data.row || {}).key === r.key)).length}`;
    const got = past.get(ref);
    if (window.MISSION.snapshot) return ctx.events.filter(e => e.data && e.data.collection === r.collection && e.data.key === r.key);
    if (!got || got.stamp !== stamp) {
      past.set(ref, { stamp, events: got ? got.events : null });
      const q = new URLSearchParams({ collection: r.collection, key: r.key, limit: String(HISTORY) });
      fetch(`/api/missions/${encodeURIComponent(window.MISSION.id)}/events?${q}`, { cache: "no-store" })
        .then(x => (x.ok ? x.json() : []))
        .then(list => { past.set(ref, { stamp, events: list }); if (open && open.ref === ref) render(); })
        .catch(() => {});
    }
    return got ? got.events : null;
  }

  function render() {
    const ctx = getCtx(), el = $("drawer");
    if (!open || !ctx) return;
    const r = recOf(ctx, open.ref);
    if (!r) { put(el, `<div class="dhead"><span class="mono muted">${esc(open.ref.replace("/", " / "))}</span><span class="grow"></span>${closeBtn()}</div><div class="dbody"><div class="empty">This record doesn't exist (any more).</div></div>`); return; }
    const s = sectionOf(ctx, r, open.section), b = r.body || {}, t = h.tone(ctx, r.state);
    /** @type {string[]} */
    const names = s && s.drawer && Array.isArray(s.drawer.fields) ? s.drawer.fields : Object.keys(b).filter(k => !SHOWN.includes(k));
    /** @type {[string, Json][]} */
    const fields = names.map(k => /** @type {[string, Json]} */ ([k, b[k]])).filter(([, v]) => v != null && v !== "" && (typeof v !== "object" || Array.isArray(v)));
    const dl = fields.length ? `<dl class="kv">${fields.map(([k, v]) => `<dt>${esc(String(k).replace(/_/g, " "))}</dt><dd${typeof v === "number" ? ' class="mono"' : ""}>${esc(Array.isArray(v) ? v.join(", ") : v)}</dd>`).join("")}<dt>updated</dt><dd class="mono">${esc(h.time(r.updated))}</dd></dl>` : "";
    const why = t === "block" && (b.reason || b.note) ? `<div class="why"><b>Why blocked:</b> ${esc(b.reason || b.note)}</div>` : "";
    const hist = events(ctx, r);
    const histHtml = `<div class="dsec"><span class="lbl">History</span>${hist == null ? `<div class="small muted">Loading…</div>` : hist.length
      ? `<ul class="hist">${[...hist].reverse().map(e => event(ctx, e)).join("")}</ul>` : `<div class="small muted">No changes recorded yet.</div>`}</div>`;
    const note = b.note && !(why && !b.reason) ? `<div class="dsec"><span class="lbl">Note</span><div class="md">${h.md(b.note)}</div></div>` : "";
    const acts = s && s.row_actions && s.row_actions.length ? ctx.buttons(s.row_actions, r) : "";
    const i = open.list.indexOf(open.ref);
    const age = h.ago(r.updated), when = age === "now" ? "just now" : `for ${age}`;
    put(el, `<div class="dhead"><div class="drow"><span class="mono muted">${esc(r.collection)} / ${esc(r.key)}</span><span class="grow"></span><span class="kbd">esc</span>${closeBtn()}</div>
      <h2 id="drawer-title">${esc(h.title(r) || b.question || r.key)}</h2>
      <div class="drow">${h.pill(ctx, r.state)}<span class="small muted">${esc([phaseName(ctx, r, s), when].filter(Boolean).join(" · "))}</span></div></div>
      <div class="dbody">${why}${dl}${relations(ctx, r, s)}${histHtml}${note}
        <details class="raw"><summary>Record</summary><pre>${esc(JSON.stringify({ key: r.key, state: r.state, ...b }, null, 2))}</pre></details></div>
      <div class="dfoot">${acts}<span class="grow"></span>
        <button class="btn icon" type="button" data-dstep="-1" aria-label="Previous record" ${i <= 0 ? "disabled" : ""}>↑</button>
        <button class="btn icon" type="button" data-dstep="1" aria-label="Next record" ${i < 0 || i >= open.list.length - 1 ? "disabled" : ""}>↓</button></div>`);
  }

  /** Sets the drawer's markup if it changed, keeping focus on the same control.
   *  @param {HTMLElement} el @param {string} html */
  function put(el, html) {
    if (el.dataset.html === html) return;
    const a = document.activeElement instanceof HTMLElement && el.contains(document.activeElement) ? document.activeElement : null;
    const same = a && ["data-dclose", "data-dstep", "data-record", "data-action"].map(k => [k, a.getAttribute(k)]).find(([, v]) => v != null);
    el.innerHTML = html;
    el.dataset.html = html;
    const back = same ? el.querySelector(`[${same[0]}${same[1] ? `="${CSS.escape(String(same[1]))}"` : ""}]`) : null;
    if (back instanceof HTMLElement) back.focus();
    else if (a) /** @type {HTMLElement | null} */ (el.querySelector("[data-dclose]"))?.focus();
  }

  /** "Phase 2 · Build" when the record sits in a plan, else its `phase` field. @param {Ctx} ctx @param {Rec} r @param {Section | null} s */
  function phaseName(ctx, r, s) {
    if (s && (s.widget === "plan" || s.widget === "flow") && (s.nodes || {}).collection === r.collection) {
      const m = window.MissionPlan.model(s, ctx), i = m.phases.findIndex((/** @type {any} */ p) => p.key && p.key === m.phaseOf(r));
      if (i >= 0) return `Phase ${i + 1} · ${m.phases[i].name}`;
    }
    const p = h.field(r, "phase");
    return p == null ? "" : String(p);
  }

  const closeBtn = () => `<button class="btn icon dclose" type="button" data-dclose aria-label="Close"><span class="x" aria-hidden="true">✕</span><span class="back">← Back</span></button>`;

  /** Records of the same widget, in page order, for j/k. @param {string} sectionId */
  function siblings(sectionId) {
    const sec = sectionId ? document.getElementById(sectionId) : null;
    if (!sec) return [];
    return [...new Set([...sec.querySelectorAll("[data-record], tr[data-key]")].map(e => {
      const x = /** @type {HTMLElement} */ (e);
      return x.dataset.record || `${x.dataset.collection}/${x.dataset.key}`;
    }))];
  }

  /** @param {string} ref @param {string} [section] @param {HTMLElement | null} [from] */
  function show(ref, section = "", from = null) {
    const wasOpen = !!open, ctx = getCtx(), r = ctx && recOf(ctx, ref);
    if (!section && ctx && r) section = (sectionOf(ctx, r, "") || { id: "" }).id;
    open = { ref, section, back: wasOpen && open ? open.back : from || /** @type {HTMLElement | null} */ (document.activeElement), list: siblings(section) };
    const q = new URLSearchParams(location.search);
    q.set("record", ref);
    window.history.replaceState(null, "", `${location.pathname}?${q}`);
    $("drawer").hidden = false;
    $("scrim").hidden = false;
    document.body.classList.add("drawer-open");
    render();
    if (!wasOpen) /** @type {HTMLElement | null} */ ($("drawer").querySelector("[data-dclose]"))?.focus();
  }

  function close() {
    if (!open) return;
    const back = open.back;
    open = null;
    $("drawer").hidden = true;
    $("scrim").hidden = true;
    document.body.classList.remove("drawer-open");
    const q = new URLSearchParams(location.search);
    q.delete("record");
    const s = q.toString();
    window.history.replaceState(null, "", location.pathname + (s ? `?${s}` : ""));
    if (back && document.contains(back)) back.focus();
  }

  /** @param {number} d */
  function step(d) {
    if (!open) return;
    const i = open.list.indexOf(open.ref), next = open.list[i + d];
    if (i >= 0 && next) { open.ref = next; show(next, open.section); }
  }

  document.addEventListener("click", e => {
    const t = e.target instanceof Element ? e.target : null;
    if (!t || e.metaKey || e.ctrlKey || e.shiftKey) return;
    if (t.closest("[data-dclose]") || t.id === "scrim") return void close();
    const st = /** @type {HTMLElement | null} */ (t.closest("[data-dstep]"));
    if (st) return void step(Number(st.dataset.dstep));
    // Controls inside a clickable card (its buttons, a summary) keep their own clicks.
    if (t.closest("button, input, select, textarea, summary") && !t.closest("a[data-record]")) return;
    const a = /** @type {HTMLElement | null} */ (t.closest("[data-record]"));
    const row = a ? null : /** @type {HTMLElement | null} */ (t.closest("tr[data-key]"));
    if (row && t.closest("a, button, input, summary")) return;
    if (!a && !row) return;
    e.preventDefault();
    const ref = a ? a.dataset.record || "" : `${row && row.dataset.collection}/${row && row.dataset.key}`;
    show(ref, (t.closest("section[id]") || {}).id || (open ? open.section : ""), /** @type {HTMLElement} */ (a || row));
  });

  document.addEventListener("keydown", e => {
    const t = e.target instanceof HTMLElement ? e.target : null;
    if (t && t.matches("tr[data-key]") && e.key === "Enter") { e.preventDefault(); t.click(); return; }
    if (!open) return;
    if (e.key === "Escape") { e.preventDefault(); return void close(); }
    const typing = t && t.closest("input, textarea, select");
    if (!typing && !e.metaKey && !e.ctrlKey && (e.key === "j" || e.key === "k") && $("drawer").contains(t)) {
      e.preventDefault();
      return void step(e.key === "j" ? 1 : -1);
    }
    if (e.key === "Tab") {
      // Keep focus inside the drawer while it's open.
      const items = /** @type {HTMLElement[]} */ ([...$("drawer").querySelectorAll("a[href], button:not([disabled]), summary, [tabindex]")]);
      if (!items.length) return;
      const first = items[0], last = items[items.length - 1];
      if (e.shiftKey && (document.activeElement === first || !$("drawer").contains(document.activeElement))) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && (document.activeElement === last || !$("drawer").contains(document.activeElement))) { e.preventDefault(); first.focus(); }
    }
  });

  window.MissionDrawer = {
    /** @param {() => Ctx | null} get */
    attach(get) { getCtx = get; },
    /** Live updates: redraw the open record. Opens the URL's `?record=` the first time. */
    refresh() {
      if (open) return render();
      const ref = new URLSearchParams(location.search).get("record");
      if (ref && !$("drawer").dataset.tried) { $("drawer").dataset.tried = "1"; show(ref); }
    },
    show, close,
  };
})();
