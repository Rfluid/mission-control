// @ts-check
// Widget registry and the helpers every widget gets as `ctx.h`. A widget is `render(section, ctx) -> html`.
// Built-ins register from widgets-*.js; user/project script widgets call `MissionWidget.define(name, render)`.
(function () {
  "use strict";
  /** @type {Map<string, WidgetRender>} */
  const registry = new Map();
  /** Optional header counts per widget ("1 open · 2 answered"), shown after the section title. */
  /** @type {Map<string, WidgetCount>} */
  const counts = new Map();
  /** Optional header controls per widget (a view toggle), shown on the right of the section header. */
  /** @type {Map<string, WidgetTools>} */
  const tools = new Map();
  /** @param {unknown} s */
  const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[/** @type {"&"|"<"|">"|'"'} */ (c)]));
  /** @param {number | null | undefined} ms */
  const time = ms => (ms ? new Date(ms).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }) : "");
  /** Tones from loudest to calmest: `sort = "tone"` puts blocked first. */
  const TONE_RANK = ["block", "warn", "rev", "prog", "todo", "done"];
  const MIN = 60000, HOUR = 60 * MIN, DAY = 24 * HOUR;

  // Helpers handed to every widget as `ctx.h`.
  /** @type {Helpers} */
  const h = {
    esc,
    time,
    /** How long ago, short: "now", "38m", "5h", "3d". */
    ago(ms) {
      if (!ms) return "";
      const d = Math.max(0, Date.now() - Number(ms));
      return d < MIN ? "now" : d < HOUR ? `${Math.floor(d / MIN)}m` : d < DAY ? `${Math.floor(d / HOUR)}h` : `${Math.floor(d / DAY)}d`;
    },
    /** Field of a record: `key`, `state`, `updated` are record-level, anything else is in the body. */
    field(rec, f) {
      if (f === "key" || f === "state" || f === "updated") return rec[f];
      return rec.body ? rec.body[f] : undefined;
    },
    /** The tone `[states]` gives a state (`todo` when it has none). */
    tone(ctx, state) {
      return (ctx.config.states || {})[state ?? ""] || "todo";
    },
    toneRank(ctx, state) {
      return TONE_RANK.indexOf(h.tone(ctx, state));
    },
    /** A state as words: lowercase with spaces (`in_progress` → "in progress"). */
    word(state) {
      return String(state ?? "").replace(/_/g, " ");
    },
    glyph(ctx, state) {
      return window.MissionGlyph.glyph(h.tone(ctx, state));
    },
    /** Glyph and word on the tone's tint; "—" for no state. */
    pill(ctx, state) {
      if (state == null || state === "") return "—";
      const tone = h.tone(ctx, state);
      return `<span class="pill c-${esc(tone)}">${window.MissionGlyph.glyph(tone)}${esc(h.word(state))}</span>`;
    },
    /** Fills `{value}`, `{vars.x}`, `{row.x}` in a link template. Record data is URL-encoded; vars are config
     *  (e.g. `repo = "org/name"`) and go in as written. */
    fill(ctx, tpl, value, rec) {
      return String(tpl).replace(/\{([a-z_.]+)\}/gi, (_, k) => {
        if (k.startsWith("vars.")) return String((ctx.config.vars || {})[k.slice(5)] ?? "");
        const v = k === "value" ? value : k.startsWith("row.") && rec ? h.field(rec, k.slice(4)) : "";
        return encodeURIComponent(v ?? "");
      });
    },
    md(src, opts) {
      return window.MissionMarkdown.render(src, opts);
    },
    /** Orders values: numbers numerically, strings naturally (`M2` < `M10`), empty last. */
    compare(x, y) {
      if (x == null || x === "") return y == null || y === "" ? 0 : 1;
      if (y == null || y === "") return -1;
      if (typeof x === "number" && typeof y === "number") return x - y;
      return String(x).localeCompare(String(y), undefined, { numeric: true });
    },
    safeUrl(u) {
      return /^(https?:|mailto:)/i.test(String(u)) ? String(u) : null;
    },
    table(cols, rows, empty) {
      if (!rows.length) return `<div class="empty">${esc(empty)}</div>`;
      return `<table><thead><tr>${cols.map(c => `<th>${esc(c)}</th>`).join("")}</tr></thead><tbody>${rows.join("")}</tbody></table>`;
    },
    /** Counts per a `count` spec: `{collection, state?, field?, value?}` or `{events: true}`. */
    count(ctx, spec) {
      if (spec.events) return ctx.eventCount;
      /** @type {(string | null)[] | null} */
      const states = spec.state == null ? null : [spec.state].flat();
      return (ctx.records[spec.collection || ""] || []).filter(r =>
        (!states || states.includes(r.state)) && (spec.field == null || h.field(r, spec.field) === spec.value)).length;
    },
    /** Sums `field` (or several fields) over a collection's records, optionally only some states. */
    sum(ctx, spec) {
      /** @type {(string | null)[] | null} */
      const states = spec.state == null ? null : [spec.state].flat();
      const fields = [spec.field].flat();
      let total = 0;
      for (const r of ctx.records[spec.collection || ""] || []) {
        if (states && !states.includes(r.state)) continue;
        for (const f of fields) { const n = Number(h.field(r, f)); if (Number.isFinite(n)) total += n; }
      }
      return total;
    },
    /** A number for display: `usd` → "$14.20", `tokens` → "3.1M", else grouped digits. */
    fmt(n, format) {
      if (typeof n !== "number" || !Number.isFinite(n)) return String(n ?? "—");
      if (format === "usd") return `$${n.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
      if (format === "tokens") {
        const a = Math.abs(n);
        return a >= 1e9 ? `${+(n / 1e9).toFixed(1)}B` : a >= 1e6 ? `${+(n / 1e6).toFixed(1)}M` : a >= 1e3 ? `${+(n / 1e3).toFixed(1)}K` : String(n);
      }
      return n.toLocaleString("en-US", { maximumFractionDigits: 2 });
    },
    /** The record a reference names: `collection/key`, or a bare key (the first record with it outside
     *  `needs`, which are what point at records). */
    find(ctx, ref) {
      const s = String(ref ?? ""), i = s.indexOf("/");
      if (i > 0) {
        const r = (ctx.records[s.slice(0, i)] || []).find(x => x.key === s.slice(i + 1));
        if (r) return r;
      }
      for (const [c, list] of Object.entries(ctx.records)) {
        if (c === "needs") continue;
        const r = list.find(x => x.key === s);
        if (r) return r;
      }
      return null;
    },
    /** The title a record goes by: `title`, else `name`, else nothing. */
    title(rec) {
      const t = h.field(rec, "title") ?? h.field(rec, "name");
      return t == null ? "" : String(t);
    },
    /** A link that opens a record in the drawer (`?record=<collection>/<key>`, so it's also a URL). */
    recordLink(rec, label) {
      const ref = `${rec.collection}/${rec.key}`;
      const q = new URLSearchParams(location.search);
      q.set("record", ref);
      return `<a href="?${esc(q.toString())}" data-record="${esc(ref)}">${label ?? esc(`${rec.key} ${h.title(rec)}`.trim())}</a>`;
    },
    /** A metric (`count` | `sum` | `value`, `of`, `format`, `prefix`, `suffix`): its number, its denominator
     *  and its text. */
    metric(ctx, spec) {
      const n = spec.count ? h.count(ctx, spec.count) : spec.sum ? h.sum(ctx, spec.sum) : spec.value;
      const of = spec.of ? h.count(ctx, spec.of) : null;
      const text = [spec.prefix, typeof n === "number" ? h.fmt(n, spec.format) : String(n ?? "—"), spec.suffix].filter(x => x != null && x !== "").join(" ");
      return { n, of, text };
    },
  };

  window.MissionWidget = {
    h,
    define(name, render, opts) {
      registry.set(name, render);
      if (opts && opts.count) counts.set(name, opts.count);
      if (opts && opts.tools) tools.set(name, opts.tools);
    },
    get(name) { return registry.get(name); },
    count(name) { return counts.get(name); },
    tools(name) { return tools.get(name); },
  };
})();
