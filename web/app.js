// @ts-check
// Mission page: loads the view, renders the open page's sections (pages.js), stays live (live.js), and hands
// clicks to actions.js. Only sections whose markup changed are re-rendered, so typing survives updates.
(function () {
  "use strict";
  const { id } = window.MISSION;
  /** A static snapshot (`mission-control export`): the view is embedded, nothing is fetched, buttons are inert. */
  const snapshot = window.MISSION.snapshot;
  const W = window.MissionWidget, P = window.MissionPages, L = window.MissionLive, esc = W.h.esc;
  /** @param {string} s */
  const $ = s => /** @type {HTMLElement} */ (document.getElementById(s));
  /** A dry-run config change to preview (`?draft=` in the preview URL `config patch --dry-run` returns). */
  const draft = new URLSearchParams(location.search).get("draft");
  const KEEP_EVENTS = 200; // newest events kept in memory for the feed
  /** @type {View | null} */
  let view = null;
  /** Page id + section ids last laid out; a change rebuilds the section slots. */
  let lastLayout = "";
  /** @type {Map<string, string>} section id → last rendered markup */
  const shown = new Map();
  /** The update notice's markup ("" when there's nothing to say). */
  let notice = "";
  /** Older events the user loaded into the feed. */
  let more = 0;
  /** The open page in a snapshot (a file has no page URLs). */
  let snapPage = P.OVERVIEW;

  /** @param {Rec[]} records */
  function byCollection(records) {
    /** @type {Ctx["records"]} */
    const out = {};
    for (const r of records) (out[r.collection] = out[r.collection] || []).push(r);
    return out;
  }

  /** Named widgets (`[widgets.x] base = ..., props = {...}`) resolve to a built-in with preset props.
   *  @param {Section} section @param {Config} config */
  function resolveWidget(section, config) {
    const named = (config.widgets || {})[section.widget];
    if (named && named.base) return { render: W.get(named.base), section: { ...(named.props || {}), ...section } };
    if (named && named.html) return { render: W.get("html"), section: { html: named.html, ...(named.props || {}), ...section } };
    return { render: W.get(section.widget), section };
  }

  /** The widget name a section renders with (a named widget's base). @param {Section} s @param {Config} config */
  const baseOf = (s, config) => (((config.widgets || {})[s.widget] || {}).base) || s.widget;

  /** Script widgets (`[widgets.x] script = "widgets/x.js"`, user/project config only) load once each; they
   *  register with `MissionWidget.define("x", render)`, and the page re-renders when one arrives.
   *  @param {Config} config */
  function loadScripts(config) {
    for (const [name, def] of Object.entries(config.widgets || {})) {
      if (!def || !def.script || W.get(name) || document.querySelector(`script[data-widget="${CSS.escape(name)}"]`)) continue;
      const el = document.createElement("script");
      el.src = `/m/${encodeURIComponent(id)}/widgets/${encodeURIComponent(name)}.js`;
      el.dataset.widget = name;
      el.onload = () => { shown.clear(); render(); };
      document.body.appendChild(el);
    }
  }

  /** @param {Config} config @returns {Ctx["buttons"]} */
  function buttons(config) {
    return (ids, row) => ids.map(a => {
      const act = (config.actions || {})[a];
      if (!act) return "";
      const on = row ? ` data-collection="${esc(row.collection)}" data-row="${esc(row.key)}"` : "";
      const off = snapshot ? ' disabled title="A snapshot: buttons work on the live page"' : "";
      return `<button class="btn" type="button" data-action="${esc(a)}"${on}${off}>${esc(act.label || a)}</button>`;
    }).join(" ");
  }

  /** @returns {Ctx} */
  function context() {
    const v = /** @type {View} */ (view);
    return { config: v.config, records: byCollection(v.records), events: v.events, eventCount: v.event_count,
      mission: v.mission, h: W.h, buttons: buttons(v.config), width: $("root").clientWidth || 800, more,
      page: snapshot ? (P.list(v.config).some(p => p.id === snapPage) ? snapPage : P.OVERVIEW) : P.current(v.config) };
  }

  /** @param {Section} s @param {Ctx} ctx */
  function sectionHtml(s, ctx) {
    const { render: fn, section } = resolveWidget(s, ctx.config);
    let body;
    try {
      body = fn ? fn(section, ctx) : `<div class="card empty">Widget <code>${esc(s.widget)}</code> is not available yet.</div>`;
    } catch (e) {
      body = `<div class="card empty">Widget <code>${esc(s.widget)}</code> failed: ${esc(e instanceof Error ? e.message : e)}</div>`;
    }
    const go = s.go && typeof s.go.page === "string" && P.list(ctx.config).some(p => p.id === s.go.page)
      ? `<a class="go" href="${esc(P.href(s.go.page))}" data-page="${esc(s.go.page)}">${esc(s.go.label || (P.list(ctx.config).find(p => p.id === s.go.page) || {}).title)}</a>` : "";
    const tool = W.tools(baseOf(s, ctx.config));
    let tl = "";
    try { tl = tool ? tool(section, ctx) : ""; } catch { /* the body shows the error */ }
    const acts = (s.actions && s.actions.length) || tl ? `<span class="acts">${tl}${ctx.buttons(s.actions || [])}</span>` : "";
    const counter = W.count(baseOf(s, ctx.config));
    let cnt = "";
    try { cnt = counter ? counter(section, ctx) : ""; } catch { /* the body shows the error */ }
    return `<h2>${esc(s.title || s.id)}${cnt ? ` <span class="cnt">${esc(cnt)}</span>` : ""}${go}${acts}</h2>${body}`;
  }

  /** Values typed into `[data-draft]` fields, so a re-rendered section keeps them. */
  function saveDrafts() {
    /** @type {Map<string, string>} */
    const m = new Map();
    for (const el of document.querySelectorAll("[data-draft]")) {
      const f = /** @type {HTMLInputElement} */ (el);
      if (f.value) m.set(f.dataset.draft || "", f.value);
    }
    const a = /** @type {HTMLElement | null} */ (document.activeElement);
    return { m, focus: a && a.dataset ? a.dataset.draft : undefined };
  }

  /** @param {ReturnType<typeof saveDrafts>} d */
  function restoreDrafts(d) {
    for (const el of document.querySelectorAll("[data-draft]")) {
      const f = /** @type {HTMLInputElement} */ (el), v = d.m.get(f.dataset.draft || "");
      if (v != null) f.value = v;
      if (d.focus && f.dataset.draft === d.focus) f.focus();
    }
  }

  /** Sets `el`'s markup only if it changed, so its buttons survive (a click mid-update still lands).
   *  @param {HTMLElement} el @param {string} html */
  function put(el, html) {
    if (el.dataset.html !== html) {
      el.innerHTML = html;
      el.dataset.html = html;
    }
  }

  function render() {
    if (!view) return;
    const ctx = context(), config = ctx.config, page = ctx.page || P.OVERVIEW;
    const sections = P.sectionsOn(config, page);
    const rail = sections.filter(s => s.column === "rail"), main = sections.filter(s => s.column !== "rail");
    const layout = [page, ...main.map(s => s.id), "|", ...rail.map(s => s.id)].join("\n");
    const drafts = saveDrafts();
    if (layout !== lastLayout) {
      // Two columns when the page has rail sections (layout.md: main, then the rail beside or below it).
      const slots = (/** @type {Section[]} */ list) => list.map(s => `<section id="${esc(s.id)}"></section>`).join("");
      $("root").innerHTML = rail.length
        ? `<div class="cols"><div class="col-main">${slots(main)}</div><aside class="col-rail">${slots(rail)}</aside></div>` : slots(sections);
      shown.clear();
      lastLayout = layout;
    }
    put($("nav"), P.tabs(ctx, page));
    window.MissionShell.render(ctx, view.template);
    put($("toolbar"), ctx.buttons(((config.toolbar || {}).actions) || []));
    // Widgets may report problems in the data (a dependency cycle); they join the config issues.
    /** @type {Issue[]} */
    const found = [];
    ctx.issue = (path, message) => { found.push({ severity: "warning", path, message }); };
    const htmls = new Map(sections.map(s => {
      const el = document.getElementById(s.id);
      // Charts draw to their column's width.
      return [s.id, sectionHtml(s, el && rail.length ? { ...ctx, width: el.clientWidth || ctx.width } : ctx)];
    }));
    const issues = [...view.issues, ...found];
    const preview = draft
      ? `<div class="card banner draft"><b>Preview:</b> a proposed config change, not applied yet. <a href="?">Back to the live page</a></div>` : "";
    put($("issues"), notice + preview + (issues.length
      ? `<div class="card banner ${issues.some(i => i.severity === "error") ? "" : "warn"}"><b>Config:</b> ${issues
          .map(i => `<code>${esc(i.path)}</code> ${esc(i.message)}`).join(" · ")}</div>` : ""));
    for (const s of sections) {
      const el = document.getElementById(s.id), html = /** @type {string} */ (htmls.get(s.id));
      if (el && shown.get(s.id) !== html) {
        window.MissionViewport.save(el);
        el.innerHTML = html;
        window.MissionViewport.restore(el);
        shown.set(s.id, html);
      }
    }
    restoreDrafts(drafts);
    fresh();
    window.MissionDrawer.refresh();
    window.MissionAttention.update(ctx);
    void window.MissionMermaid.run(document.body);
  }

  /** The live indicator: `live · 4s ago` while the stream is open, `reconnecting` when it dropped. */
  function fresh() {
    if (!snapshot) window.MissionShell.fresh(L.status());
  }

  const darkOS = matchMedia("(prefers-color-scheme: dark)");
  const isDark = () => (document.documentElement.dataset.theme || (darkOS.matches ? "dark" : "light")) === "dark";
  /** Theme token overrides, as custom properties (never CSS text): `[theme] accent = …` for both themes,
   *  `[theme.dark] …` on top in dark mode. Re-applied when the theme changes.
   *  @param {Config} config */
  function applyTheme(config) {
    const t = config.theme || {}, root = document.documentElement.style;
    for (const p of [...root]) if (p.startsWith("--")) root.removeProperty(p);
    // Colors only: a hex, a color name, or rgb()/hsl() with numbers. Anything else (url(), var() chains) is
    // ignored, so config can't make the page fetch from elsewhere.
    const COLOR = /^(#[0-9a-f]{3,8}|[a-z]{3,20}|(rgb|hsl)a?\([\d\s.,%/deg]+\))$/i;
    const set = (/** @type {Record<string, unknown>} */ o) => {
      for (const [k, v] of Object.entries(o)) if (typeof v === "string" && /^[a-z0-9-]+$/i.test(k) && COLOR.test(v.trim())) root.setProperty(`--${k}`, v);
    };
    set(t);
    if (isDark() && t.dark && typeof t.dark === "object") set(t.dark);
  }
  darkOS.addEventListener("change", () => { if (view) applyTheme(view.config); void window.MissionMermaid.run(document.body); });

  /** Re-reads the whole view (first load, config changes, after a gap in the stream). */
  async function load() {
    try {
      const q = draft ? `?draft=${encodeURIComponent(draft)}` : "";
      const r = await fetch(`/api/missions/${encodeURIComponent(id)}${q}`, { cache: "no-store" });
      if (!r.ok) throw new Error(await r.text());
      view = /** @type {View} */ (await r.json());
      L.touch();
      applyTheme(view.config);
      loadScripts(view.config);
      L.connect(view.config, { change: apply, reload: () => void load(), status: fresh });
      render();
    } catch (e) {
      $("fresh").dataset.state = "reconnecting";
      /** @type {HTMLElement} */ ($("fresh").querySelector(".lt")).textContent = `unreachable: ${e instanceof Error ? e.message : e}`;
    }
  }

  /** Applies one streamed change: records replaced or removed, events appended. Config changes re-read.
   *  @param {Change} c */
  function apply(c) {
    if (c.config || !view) return void load();
    const same = (/** @type {{collection: string, key: string}} */ a, /** @type {{collection: string, key: string}} */ b) =>
      a.collection === b.collection && a.key === b.key;
    const before = view.records;
    const gone = [...c.removed, ...c.records];
    view.records = view.records.filter(r => !gone.some(g => same(g, r))).concat(c.records);
    view.events = view.events.concat(c.events).slice(-(KEEP_EVENTS + more));
    view.event_count += c.events.length;
    window.MissionAttention.observe(context(), before, c.records);
    render();
  }

  // Widgets that draw to size (charts) re-render when the width changes.
  let lastWidth = 0, resizeTimer = 0;
  new ResizeObserver(() => {
    clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      const w = $("root").clientWidth;
      if (view && Math.abs(w - lastWidth) > 8) { lastWidth = w; render(); }
    }, 120);
  }).observe($("root"));

  $("theme").onclick = () => {
    const d = document.documentElement, dark = matchMedia("(prefers-color-scheme: dark)").matches;
    const cur = d.dataset.theme || (dark ? "dark" : "light");
    d.dataset.theme = cur === "dark" ? "light" : "dark";
    if (view) applyTheme(view.config);
    void window.MissionMermaid.run(document.body);
  };

  // Tabs switch pages in place (each page keeps its own URL); Back and Forward follow.
  document.addEventListener("click", e => {
    const a = e.target instanceof Element ? /** @type {HTMLAnchorElement | null} */ (e.target.closest("a[data-page]")) : null;
    if (!a || e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
    e.preventDefault();
    if (snapshot) snapPage = a.dataset.page || P.OVERVIEW;
    else if (a.href !== location.href) history.pushState(null, "", a.href);
    render();
    window.scrollTo(0, 0);
  });
  addEventListener("popstate", render);
  // The palette and shortcuts switch pages through here (a snapshot has no page URLs).
  document.addEventListener("mission:go", e => {
    const page = String(/** @type {CustomEvent} */ (e).detail || P.OVERVIEW);
    if (snapshot) snapPage = page;
    else if (location.pathname + location.search !== P.href(page)) history.pushState(null, "", P.href(page));
    render();
    window.scrollTo(0, 0);
  });
  // A widget changed its own view (Graph / Table, a toggle): draw again.
  document.addEventListener("mission:render", render);

  // "Older" in a feed: load a page of earlier events if they aren't here yet, then show a page more.
  document.addEventListener("click", async e => {
    if (!(e.target instanceof Element) || !e.target.closest("button[data-older]") || !view) return;
    const page = Number((view.config.retention || {}).page_size) || 50;
    const oldest = view.events.length ? view.events[0].seq : undefined;
    if (!snapshot && oldest !== undefined && view.event_count > view.events.length) {
      const r = await fetch(`/api/missions/${encodeURIComponent(id)}/events?before=${oldest}&limit=${page}`, { cache: "no-store" });
      if (r.ok) view.events = /** @type {Ev[]} */ (await r.json()).concat(view.events);
    }
    more += page;
    render();
  });

  /** Shows a notice when a newer mission-control release is out (checked daily by the daemon). */
  async function checkUpdate() {
    try {
      const u = await (await fetch("/api/update", { cache: "no-store" })).json();
      notice = u.notify ? `<div class="card banner draft"><b>Update:</b> mission-control ${esc(u.latest)} is out (this is ${esc(u.current)}).
        Ask your agent, or run <code>${esc(u.command)}</code>.${u.notes ? ` <a href="${esc(W.h.safeUrl(u.notes) || "#")}" target="_blank" rel="noopener">What's new</a>` : ""}
        <button class="btn" type="button" data-dismiss="${esc(u.latest)}">Skip this version</button>
        <button class="btn" type="button" data-dismiss="all">No update notices</button></div>` : "";
      render();
    } catch { /* no notice */ }
  }

  document.addEventListener("click", async e => {
    const b = e.target instanceof Element ? /** @type {HTMLElement | null} */ (e.target.closest("button[data-dismiss]")) : null;
    if (!b) return;
    const d = b.dataset.dismiss;
    await fetch(`/api/missions/${encodeURIComponent(id)}/update/dismiss`, {
      method: "POST", headers: { "content-type": "application/json", "x-mission-token": window.MISSION.token },
      body: JSON.stringify(d === "all" ? { all: true } : { version: d }),
    });
    notice = "";
    render();
  });

  if (!/Mac|iPhone|iPad/.test(navigator.platform)) $("modk").textContent = "Ctrl K";
  if (snapshot) {
    window.MissionDrawer.attach(() => (view ? context() : null));
    window.MissionPalette.attach(() => (view ? context() : null));
    window.MissionKeys.attach(() => (view ? context() : null));
    view = snapshot;
    applyTheme(view.config);
    render();
    $("fresh").dataset.state = "snapshot";
    /** @type {HTMLElement} */ ($("fresh").querySelector(".lt")).textContent = `snapshot of ${new Date(Math.max(0, ...view.events.map(e => e.at))).toLocaleString()}`;
  } else {
    window.MissionActions.attach(() => (view ? context() : null));
    window.MissionDrawer.attach(() => (view ? context() : null));
    window.MissionPalette.attach(() => (view ? context() : null));
    window.MissionKeys.attach(() => (view ? context() : null));
    void load().then(checkUpdate);
  }
})();
