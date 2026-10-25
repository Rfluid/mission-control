// @ts-check
// `needs` (.design/components.md, "Need card"): questions waiting on the user. Each open need is a card with
// a --warn border: what it blocks, the question, context, the recommendation and why, one button per option
// (number keys 1–9 pick one while the card has focus), and a free answer. Answered needs fold under a count.
// Answers go through actions.js.
(function () {
  "use strict";
  const W = window.MissionWidget, h = W.h, esc = h.esc;
  const MAX_KEYED = 9; // options 1–9 get a number key

  /** @param {Rec} n */
  const isOpen = n => n.state !== "answered";

  /** When the need was asked: `since` (ISO or Unix ms) when the asker recorded one, else its last change.
   *  @param {Rec} n */
  function since(n) {
    const b = n.body || {};
    return typeof b.since === "string" ? Date.parse(b.since) || n.updated : typeof b.since === "number" ? b.since : n.updated;
  }

  /** Links to what a need blocks (`blocks`: a key, `collection/key`, or a list).
   *  @param {Ctx} ctx @param {Rec} n */
  function blocks(ctx, n) {
    const refs = [n.body && n.body.blocks].flat().filter(x => x != null && x !== "");
    return refs.map(ref => {
      const r = h.find(ctx, ref);
      return r ? h.recordLink(r) : `<span>${esc(ref)}</span>`;
    }).join(", ");
  }

  /** @param {Ctx} ctx @param {Rec} n @param {string} coll */
  function card(ctx, n, coll) {
    const b = n.body || {}, on = `data-need="${esc(n.key)}" data-collection="${esc(coll)}"`;
    /** @type {string[]} */
    const opts = Array.isArray(b.options) ? b.options.map(String) : [];
    const rec = b.recommendation == null ? "" : String(b.recommendation);
    const age = h.ago(since(n)), held = blocks(ctx, n);
    const meta = [esc(n.key), age === "now" ? "just now" : `${esc(age)} ago`, held ? `blocks ${held}` : ""].filter(Boolean).join(" · ");
    const buttons = opts.map((o, i) => `<button class="btn${o === rec ? " rec" : ""}" type="button" ${on} data-answer="${esc(o)}">${
      i < MAX_KEYED ? `<span class="kbd" aria-hidden="true">${i + 1}</span>` : ""}${esc(o)}${o === rec ? `<span class="note">recommended</span>` : ""}</button>`).join("");
    const id = `ans-${esc(coll)}-${esc(n.key)}`;
    return `<div class="card need" data-need-card="${esc(n.key)}" tabindex="-1" aria-label="Question ${esc(n.key)}">
      <div class="meta">${meta}</div>
      <h3>${esc(b.question || b.title || n.key)}</h3>${b.context ? `<div class="ctx md">${h.md(b.context)}</div>` : ""}
      ${rec ? `<div class="rec"><b>Recommended:</b> ${esc(rec)}${b.why ? ` <span class="muted">${esc(b.why)}</span>` : ""}</div>` : ""}
      ${opts.length ? `<div class="opts">${buttons}</div>` : ""}
      <div class="answer"><label class="sr" for="${id}">Your answer to ${esc(n.key)}</label><input type="text" id="${id}" data-draft="${esc(coll)}/${esc(n.key)}"
        placeholder="${opts.length ? "Or type an answer" : "Type an answer"}" maxlength="8000">
        <button class="btn" type="button" ${on} data-send>Send</button></div></div>`;
  }

  /** @param {Ctx} ctx @param {Rec} n */
  const answered = (ctx, n) => `<li><span class="mono muted">${esc(n.key)}</span> ${esc((n.body || {}).question || n.key)}
    <span class="muted">→</span> ${esc((n.body || {}).answer ?? "")}</li>`;

  /** @type {WidgetRender} */
  function needs(s, ctx) {
    const coll = s.collection || "needs";
    const all = [...(ctx.records[coll] || [])].sort((a, b) => since(a) - since(b));
    const open = all.filter(isOpen), done = all.filter(n => !isOpen(n)).reverse();
    const fold = done.length ? `<details class="answered"><summary>${done.length} answered</summary><ul>${done.map(n => answered(ctx, n)).join("")}</ul></details>` : "";
    if (!open.length) return `<div class="card empty">Nothing waiting on you right now.</div>${fold}`;
    return `<div class="grid needs">${open.map(n => card(ctx, n, coll)).join("")}</div>${fold}`;
  }

  /** "1 open · 2 answered". @type {WidgetCount} */
  function count(s, ctx) {
    const all = ctx.records[s.collection || "needs"] || [], open = all.filter(isOpen).length;
    return all.length ? [open ? `${open} open` : "", all.length - open ? `${all.length - open} answered` : ""].filter(Boolean).join(" · ") : "";
  }

  // 1–9 pick that option of the need card holding focus (never while typing in its text box).
  document.addEventListener("keydown", e => {
    if (e.metaKey || e.ctrlKey || e.altKey || !/^[1-9]$/.test(e.key)) return;
    const t = e.target instanceof Element ? e.target : null;
    if (!t || t.closest("input, textarea, select, [contenteditable]")) return;
    const cardEl = t.closest(".need");
    const btn = cardEl && cardEl.querySelectorAll(".opts button[data-answer]")[Number(e.key) - 1];
    if (btn instanceof HTMLElement) { e.preventDefault(); btn.click(); }
  });

  W.define("needs", needs, { count });
})();
