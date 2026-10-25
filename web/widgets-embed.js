// @ts-check
// `html`: HTML from config or a record, shown in a sandboxed iframe (scripts run, but in an opaque origin:
// no access to the page, its token, cookies or the daemon's API with credentials; no top navigation, no popups).
(function () {
  "use strict";
  const W = window.MissionWidget, h = W.h, esc = h.esc;
  const DEFAULT_HEIGHT = 240;

  /** @type {WidgetRender} */
  function html(s, ctx) {
    let src = s.html;
    if (s.record) {
      const r = (ctx.records[s.record.collection] || []).find(x => x.key === s.record.key);
      src = r ? h.field(r, s.record.field || "html") : null;
    }
    if (!src) return `<div class="card empty">Nothing here yet.</div>`;
    const height = Math.min(Number(s.height) || DEFAULT_HEIGHT, 2000);
    return `<div class="card embed"><iframe sandbox="allow-scripts" referrerpolicy="no-referrer" loading="lazy"
      title="${esc(s.title || s.id)}" style="height:${height}px" srcdoc="${esc(src)}"></iframe></div>`;
  }

  W.define("html", html);
})();
