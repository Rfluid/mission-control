// @ts-check
// Pages (`[[pages]]`, .design/layout.md): which page the URL names, which sections it holds, and the tabs.
// Overview is always first; a page shows as a tab only when a section names it; a section naming an unknown
// page shows on Overview (the daemon warns about it). Switching pages is client-side (history API).
(function () {
  "use strict";
  const OVERVIEW = "overview";
  const base = `/m/${encodeURIComponent(window.MISSION.id)}`;

  /** Page ids in config order, Overview first.
   *  @param {Config} config @returns {string[]} */
  function known(config) {
    const out = [OVERVIEW];
    for (const p of config.pages || []) if (p && typeof p.id === "string" && !out.includes(p.id)) out.push(p.id);
    return out;
  }

  /** @param {Section} s @param {string[]} ids */
  const pageOf = (s, ids) => (typeof s.page === "string" && ids.includes(s.page) ? s.page : OVERVIEW);

  /** The sections on `page`, in config order.
   *  @param {Config} config @param {string} page @returns {Section[]} */
  function sectionsOn(config, page) {
    const ids = known(config);
    return (config.sections || []).filter((/** @type {Section} */ s) => pageOf(s, ids) === page);
  }

  /** The tabs: Overview, then every page that has a section.
   *  @param {Config} config @returns {{id: string, title: string, count?: CountSpec}[]} */
  function list(config) {
    const ids = known(config), used = new Set((config.sections || []).map((/** @type {Section} */ s) => pageOf(s, ids)));
    /** @type {Record<string, Json>} */
    const defs = Object.fromEntries((config.pages || []).filter((/** @type {Json} */ p) => p && p.id).map((/** @type {Json} */ p) => [p.id, p]));
    return ids.filter(id => id === OVERVIEW || used.has(id))
      .map(id => ({ id, title: String((defs[id] || {}).title || (id === OVERVIEW ? "Overview" : id)), count: (defs[id] || {}).count }));
  }

  /** The page the URL names (`/m/<id>/<page>`); Overview for `/m/<id>`, a snapshot, or a page this config
   *  doesn't show.
   *  @param {Config} config */
  function current(config) {
    const rest = location.pathname.startsWith(`${base}/`) ? decodeURIComponent(location.pathname.slice(base.length + 1)) : "";
    return list(config).some(p => p.id === rest) ? rest : OVERVIEW;
  }

  /** URL of a page, keeping the query (`?draft=`), dropping an open record.
   *  @param {string} page */
  function href(page) {
    const q = new URLSearchParams(location.search);
    q.delete("record");
    const s = q.toString();
    return (page === OVERVIEW ? base : `${base}/${encodeURIComponent(page)}`) + (s ? `?${s}` : "");
  }

  /** Tab links. @param {Ctx} ctx @param {string} page */
  function tabs(ctx, page) {
    const h = ctx.h, esc = h.esc;
    return list(ctx.config).map(p => {
      const n = p.count && typeof p.count === "object" ? h.count(ctx, p.count) : null;
      const cur = p.id === page ? ' aria-current="page"' : "";
      return `<a class="tab" href="${esc(href(p.id))}" data-page="${esc(p.id)}"${cur}>${esc(p.title)}${n != null ? ` <span class="cnt">${esc(n)}</span>` : ""}</a>`;
    }).join("");
  }

  window.MissionPages = { OVERVIEW, known, list, sectionsOn, current, href, tabs };
})();
