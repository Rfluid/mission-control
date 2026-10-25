// @ts-check
// Charts: `chart` (line, bar, sparkline) and `spend`. Inline SVG from records; series colors are the
// categorical tokens --s1..--s8 in fixed order (by series name, so a series keeps its color). Every chart has
// direct labels and a table view (three light-mode slots are under 3:1 on the panel; see .design/dataviz.md).
(function () {
  "use strict";
  const W = window.MissionWidget, h = W.h, esc = h.esc;
  const CARD_PAD = 34; // .card.pad horizontal padding + border
  const MIN_W = 240;
  const PAD = { l: 44, r: 56, t: 12, b: 26 }; // room for y labels, the direct label and x labels
  const BARE_PAD = { l: 4, r: 8, t: 6, b: 6 }; // sparklines: just the line and its end dot
  const SERIES_MAX = 8; // categorical slots; more series fold into "other"

  /** @param {number} v */
  const fmt = v => (Math.abs(v) >= 1000 ? v.toLocaleString(undefined, { maximumFractionDigits: 0 })
    : v.toLocaleString(undefined, { maximumFractionDigits: 2 }));

  /** Rows of `{x, y, s}` from a collection: `y` may name several fields (one series each), or `series` a
   *  field that splits rows into series.
   *  @param {Section} s @param {Ctx} ctx */
  function points(s, ctx) {
    const recs = ctx.records[s.collection || ""] || [];
    const xf = s.x || "key";
    /** @type {string[]} */
    const ys = [s.y || "value"].flat();
    /** @type {{x: any, y: number, s: string, rec: Rec}[]} */
    const out = [];
    for (const r of recs) {
      for (const yf of ys) {
        const y = Number(h.field(r, yf));
        if (!Number.isFinite(y)) continue;
        const name = s.series ? String(h.field(r, s.series) ?? "—") : ys.length > 1 ? yf : (s.label || yf);
        out.push({ x: h.field(r, xf), y, s: name, rec: r });
      }
    }
    out.sort((a, b) => h.compare(a.x, b.x));
    return out;
  }

  /** Series names in a fixed order, at most SERIES_MAX (the rest become "other").
   *  @param {{s: string}[]} pts */
  function seriesOf(pts) {
    const names = [...new Set(pts.map(p => p.s))].sort((a, b) => h.compare(a, b));
    if (names.length <= SERIES_MAX) return names;
    const keep = names.slice(0, SERIES_MAX - 1);
    for (const p of pts) if (!keep.includes(p.s)) p.s = "other";
    return [...keep, "other"];
  }

  /** @param {any} x */
  const xLabel = x => (typeof x === "number" && x > 1e11 ? h.time(x) : String(x ?? ""));

  /** @param {Section} s @param {{x: any, y: number, s: string}[]} pts @param {string[]} names */
  function table(s, pts, names) {
    const xs = [...new Set(pts.map(p => p.x))];
    const head = [s.x_label || s.x || "key", ...names];
    const rows = xs.map(x => `<tr><td>${esc(xLabel(x))}</td>${names.map(n => {
      const p = pts.find(q => q.x === x && q.s === n);
      return `<td class="num">${p ? esc(fmt(p.y)) : "—"}</td>`;
    }).join("")}</tr>`);
    return `<details class="viz-table"><summary class="small muted">Table</summary>${h.table(head, rows, "")}</details>`;
  }

  /** @param {string[]} names */
  const legend = names => (names.length < 2 ? "" : `<div class="legend small">${names.map((n, i) =>
    `<span><i style="background:var(--s${i + 1})"></i>${esc(n)}</span>`).join("")}</div>`);

  /** @param {Section} s @param {{x: any, y: number, s: string}[]} pts @param {string[]} names @param {number} height @param {number} vw */
  function line(s, pts, names, height, vw, bare = false) {
    const P = bare ? BARE_PAD : PAD;
    const xsAll = [...new Set(pts.map(p => p.x))];
    const numeric = xsAll.every(x => typeof x === "number");
    const xmin = numeric ? Math.min(...xsAll) : 0, xmax = numeric ? Math.max(...xsAll) : Math.max(xsAll.length - 1, 1);
    const ys = pts.map(p => p.y), ymin = Math.min(0, ...ys), ymax = Math.max(...ys, ymin + 1);
    const X = (/** @type {any} */ x) => P.l + ((numeric ? x : xsAll.indexOf(x)) - xmin) / ((xmax - xmin) || 1) * (vw - P.l - P.r);
    const Y = (/** @type {number} */ y) => height - P.b - (y - ymin) / (ymax - ymin) * (height - P.t - P.b);
    const ticks = [ymin, (ymin + ymax) / 2, ymax];
    const grid = bare ? "" : ticks.map(t => `<line class="grid" x1="${P.l}" x2="${vw - P.r}" y1="${Y(t)}" y2="${Y(t)}"/>` +
      `<text class="axis" x="${P.l - 6}" y="${Y(t) + 4}" text-anchor="end">${esc(fmt(t))}</text>`).join("");
    const xl = bare ? "" : [xsAll[0], xsAll[xsAll.length - 1]].filter((x, i, a) => i === 0 || x !== a[0]).map((x, i) =>
      `<text class="axis" x="${X(x)}" y="${height - 6}" text-anchor="${i ? "end" : "start"}">${esc(xLabel(x))}</text>`).join("");
    const lines = names.map((n, i) => {
      const ps = pts.filter(p => p.s === n);
      if (!ps.length) return "";
      const d = ps.map((p, j) => `${j ? "L" : "M"}${X(p.x).toFixed(1)},${Y(p.y).toFixed(1)}`).join("");
      const last = ps[ps.length - 1];
      const dots = ps.map(p => `<circle class="hit" cx="${X(p.x)}" cy="${Y(p.y)}" r="9"><title>${esc(`${n} · ${xLabel(p.x)}: ${fmt(p.y)}`)}</title></circle>`).join("");
      return `<path d="${d}" fill="none" stroke="var(--s${i + 1})" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>` +
        `<circle cx="${X(last.x)}" cy="${Y(last.y)}" r="4" fill="var(--s${i + 1})" stroke="var(--panel)" stroke-width="2"/>` +
        (bare ? `<title>${esc(`${n}: ${fmt(last.y)}`)}</title>` : `<text class="direct" x="${X(last.x) + 8}" y="${Y(last.y) + 4}">${esc(fmt(last.y))}</text>${dots}`);
    }).join("");
    return `<svg class="viz" width="${vw}" height="${height}" viewBox="0 0 ${vw} ${height}" role="img" aria-label="${esc(s.title || s.id)}">${grid}${xl}${lines}</svg>`;
  }

  /** @param {Section} s @param {{x: any, y: number, s: string}[]} pts @param {string[]} names @param {number} height @param {number} vw */
  function bars(s, pts, names, height, vw) {
    const xs = [...new Set(pts.map(p => p.x))];
    const ymax = Math.max(...pts.map(p => p.y), 1);
    const band = (vw - PAD.l - PAD.r) / Math.max(xs.length, 1), GAP = 2;
    const bw = Math.max(Math.min((band - 8) / names.length, 48), 2);
    const Y = (/** @type {number} */ y) => height - PAD.b - y / ymax * (height - PAD.t - PAD.b - 14);
    const base = height - PAD.b;
    const marks = xs.map((x, xi) => names.map((n, si) => {
      const p = pts.find(q => q.x === x && q.s === n);
      if (!p) return "";
      const bx = PAD.l + xi * band + (band - bw * names.length) / 2 + si * bw + GAP / 2, top = Math.min(Y(p.y), base - 1);
      const r = Math.min(4, (bw - GAP) / 2, base - top);
      const path = `M${bx},${base}V${top + r}Q${bx},${top} ${bx + r},${top}H${bx + bw - GAP - r}Q${bx + bw - GAP},${top} ${bx + bw - GAP},${top + r}V${base}Z`;
      return `<path d="${path}" fill="var(--s${si + 1})"><title>${esc(`${n} · ${xLabel(x)}: ${fmt(p.y)}`)}</title></path>` +
        (names.length === 1 || xs.length <= 6 ? `<text class="direct" x="${bx + (bw - GAP) / 2}" y="${top - 4}" text-anchor="middle">${esc(fmt(p.y))}</text>` : "");
    }).join("") + `<text class="axis" x="${PAD.l + xi * band + band / 2}" y="${height - 6}" text-anchor="middle">${esc(xLabel(x).slice(0, 14))}</text>`).join("");
    return `<svg class="viz" width="${vw}" height="${height}" viewBox="0 0 ${vw} ${height}" role="img" aria-label="${esc(s.title || s.id)}"><line class="grid" x1="${PAD.l}" x2="${vw - PAD.r}" y1="${base}" y2="${base}"/>${marks}</svg>`;
  }

  /** @type {WidgetRender} */
  function chart(s, ctx) {
    const pts = points(s, ctx);
    if (!pts.length) return `<div class="card empty">No data in <code>${esc(s.collection || "?")}</code> yet.</div>`;
    const names = seriesOf(pts);
    const kind = s.kind || "line";
    if (kind === "sparkline") {
      const last = pts[pts.length - 1];
      const vw = Math.max(Math.min(ctx.width - CARD_PAD - 80, 420), MIN_W);
      return `<div class="card pad spark">${line({ ...s, title: s.title || s.id }, pts.filter(p => p.s === names[0]), [names[0]], 48, vw, true)}<span class="v">${esc(fmt(last.y))}</span></div>`;
    }
    const height = Number(s.height) || 200;
    const vw = Math.max(ctx.width - CARD_PAD, MIN_W);
    const svg = kind === "bar" ? bars(s, pts, names, height, vw) : line(s, pts, names, height, vw);
    return `<div class="card pad">${legend(names)}${svg}${table(s, pts, names)}</div>`;
  }

  /** Cost totals from the `spend` collection: `usd` (or `field`) per record, largest first.
   *  @type {WidgetRender} */
  function spend(s, ctx) {
    const f = s.field || "usd";
    const recs = (ctx.records[s.collection || "spend"] || [])
      .map(r => ({ r, v: Number(h.field(r, f)) || 0 })).sort((a, b) => b.v - a.v);
    if (!recs.length) return `<div class="card empty">No spend recorded yet.</div>`;
    const total = recs.reduce((t, x) => t + x.v, 0);
    const tokens = recs.reduce((t, x) => t + (Number(h.field(x.r, "input_tokens")) || 0) + (Number(h.field(x.r, "output_tokens")) || 0), 0);
    const max = Math.max(recs[0].v, 1e-9);
    const money = (/** @type {number} */ v) => (f === "usd" ? h.fmt(v, "usd") : fmt(v));
    const rows = recs.map(({ r, v }) => `<div class="bar-row"><span class="small">${esc(h.field(r, s.label || "key"))}</span>
      <span class="bar"><i style="width:${(v / max * 100).toFixed(1)}%"></i></span><span class="small num">${esc(money(v))}</span></div>`).join("");
    return `<div class="grid kpis"><div class="card pad kpi"><div class="l">Total${f === "usd" ? " (USD)" : ""}</div><div class="v">${esc(money(total))}</div></div>
      ${tokens ? `<div class="card pad kpi"><div class="l">Tokens</div><div class="v">${esc(h.fmt(tokens, "tokens"))}</div></div>` : ""}</div>
      <div class="card pad bars">${rows}</div>`;
  }

  W.define("chart", chart);
  W.define("spend", spend);
})();
