// @ts-check
// The missions home (`/`, .design/components.md "Missions home"): what needs you across missions, a card per
// mission (health, progress, stats, last activity) under an Active / Needs you / Finished filter, the recently
// finished ones, and ⌘K over missions and questions. One request (`/api/missions?summary=true&archived=true`)
// draws it; it refreshes every few seconds.
(function () {
  "use strict";
  const G = window.MissionGlyph;
  const REFRESH_MS = 5000, FINISHED_ROWS = 10, FILTER = "mission-control.home.filter";
  /** @param {unknown} s */
  const esc = s => String(s ?? "").replace(/[&<>"]/g, c => /** @type {Record<string, string>} */ ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
  const $ = (/** @type {string} */ id) => /** @type {HTMLElement} */ (document.getElementById(id));
  /** @param {number | null | undefined} ms */
  const ago = ms => {
    if (!ms) return "—";
    const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
    return s < 60 ? `${s}s ago` : s < 3600 ? `${Math.floor(s / 60)}m ago` : s < 86400 ? `${Math.floor(s / 3600)}h ago` : `${Math.floor(s / 86400)}d ago`;
  };
  /** @typedef {{label: string, value: Json, tone?: string}} Stat */
  /** @typedef {Mission & {summary: {open_needs: number, last_activity: number | null, health: string | null, health_tone?: string,
   *   reason?: string | null, template?: string, needs?: {key: string, question: Json, blocks: Json, since: number}[],
   *   progress?: {kind: string, segments: string[], caption: string} | null, stats?: Stat[]}}} Card */
  /** @type {Card[]} */
  let missions = [];
  let filter = (() => { try { return localStorage.getItem(FILTER) || "active"; } catch { return "active"; } })();

  /** @param {string} tone @param {string} word */
  const pill = (tone, word) => `<span class="pill c-${esc(tone)}">${G.glyph(tone)}${esc(word)}</span>`;
  /** @param {string | null | undefined} state */
  const word = state => String(state ?? "").replace(/_/g, " ");
  const home = (/** @type {string} */ path) => path.replace(/^~?/, "").replace(/^\/home\/[^/]+/, "~");

  /** @param {Card} m */
  function card(m) {
    const s = m.summary, n = (s.needs || []).length;
    const bar = s.progress ? `<div class="mprog"><span class="segs" aria-hidden="true">${s.progress.segments.map(t => `<i class="sg-${esc(t || "ahead")}"></i>`).join("")}</span><span class="small muted">${esc(s.progress.caption)}</span></div>` : "";
    const stats = (s.stats || []).map(x => `<span>${esc(x.label)} <b class="mono${x.tone ? ` t-${esc(x.tone)}` : ""}">${esc(x.value)}</b></span>`).join("");
    return `<a class="card mcard" href="/m/${encodeURIComponent(m.id)}">
      <span class="mtop"><b class="mt">${esc(m.title)}</b>${n ? pill("warn", `${n} needs you`) : ""}${m.archived ? pill("todo", "finished") : ""}</span>
      <span class="mono small muted mpath">${esc([m.project_dir ? home(m.project_dir) : "", s.template].filter(Boolean).join(" · "))}</span>
      <span class="mhl">${s.health ? pill(s.health_tone || "todo", word(s.health)) : `<span class="small muted">No health reported yet</span>`}${s.reason ? `<span class="small">${esc(s.reason)}</span>` : ""}</span>
      ${bar}<span class="mstat">${stats}<span class="grow"></span><span class="mono small muted">${esc(ago(s.last_activity))}</span></span></a>`;
  }

  function render() {
    const active = missions.filter(m => !m.archived), finished = missions.filter(m => m.archived);
    const asking = active.filter(m => (m.summary.needs || []).length);
    // Needs you, across missions.
    const rows = asking.flatMap(m => (m.summary.needs || []).map(nd => `<a class="nrow" href="/m/${encodeURIComponent(m.id)}#needs">
      <span class="small nm">${esc(m.title)}</span><span><b>${esc(nd.question || nd.key)}</b>${nd.blocks ? ` <span class="muted">· blocks ${esc([nd.blocks].flat().join(", "))}</span>` : ""}</span>
      <span class="mono small muted">${esc(ago(nd.since).replace(" ago", ""))}</span></a>`));
    $("needs").hidden = !rows.length;
    $("needs").innerHTML = rows.length ? `<h2 class="warnh">Needs you <span class="cnt">${rows.length} across ${asking.length} mission${asking.length === 1 ? "" : "s"}</span></h2><div class="card needsall">${rows.join("")}</div>` : "";
    // The grid, filtered.
    const sets = { active, needs: asking, finished };
    if (!(filter in sets)) filter = "active";
    const shown = /** @type {Record<string, Card[]>} */ (sets)[filter];
    const seg = [["active", "Active"], ["needs", "Needs you"], ["finished", "Finished"]].map(([k, l]) =>
      `<button type="button" data-filter="${k}" aria-pressed="${filter === k}">${l} <span class="mono">${/** @type {Record<string, Card[]>} */ (sets)[k].length}</span></button>`).join("");
    $("missions").innerHTML = `<h2>Missions<span class="grow"></span><span class="seg" role="group" aria-label="Filter">${seg}</span></h2>${shown.length
      ? `<div class="mgrid">${shown.map(card).join("")}</div>`
      : `<div class="card empty">${missions.length ? "None here." : `No missions yet. An agent starts one with <code>mission-control new "title"</code>.`}</div>`}`;
    // Recently finished.
    $("finished").hidden = !finished.length;
    $("finished").innerHTML = finished.length ? `<h2>Recently finished</h2><div class="card tw"><table><thead><tr><th>Mission</th><th>Template</th><th>Result</th><th>Ended</th></tr></thead><tbody>${
      finished.slice(0, FINISHED_ROWS).map(m => `<tr><td><a href="/m/${encodeURIComponent(m.id)}">${esc(m.title)}</a></td><td class="mono small muted">${esc(m.summary.template || "—")}</td>
        <td>${m.summary.health ? pill(m.summary.health_tone || "todo", word(m.summary.health)) : "—"}</td><td class="mono small muted">${esc(ago(m.summary.last_activity))}</td></tr>`).join("")}</tbody></table></div>` : "";
  }

  async function load() {
    try {
      const r = await fetch("/api/missions?summary=true&archived=true", { cache: "no-store" });
      if (!r.ok) throw new Error(await r.text());
      missions = await r.json();
      render();
    } catch (e) {
      $("missions").innerHTML = `<div class="card empty">mission-control isn't answering (${esc(e instanceof Error ? e.message : e)}). Run any <code>mission-control</code> command, then reload.</div>`;
    }
  }

  document.addEventListener("click", e => {
    const b = e.target instanceof Element ? /** @type {HTMLElement | null} */ (e.target.closest("[data-filter]")) : null;
    if (!b) return;
    filter = b.dataset.filter || "active";
    try { localStorage.setItem(FILTER, filter); } catch { /* not kept */ }
    render();
  });

  // ⌘K: find a mission or one of their open questions.
  const dlg = /** @type {HTMLDialogElement} */ ($("palette")), input = /** @type {HTMLInputElement} */ (dlg.querySelector("input"));
  /** @type {{label: string, hint: string, glyph: string, href: string}[]} */
  let found = [];
  let at = 0;
  function search() {
    const words = input.value.toLowerCase().split(/\s+/).filter(Boolean);
    const all = [
      ...missions.flatMap(m => (m.summary.needs || []).map(nd => ({ label: String(nd.question || nd.key), hint: m.title, glyph: G.glyph("warn"), href: `/m/${encodeURIComponent(m.id)}#needs` }))),
      ...missions.map(m => ({ label: m.title, hint: m.archived ? "finished" : word(m.summary.health) || m.summary.template || "", glyph: G.glyph(m.summary.health_tone || "todo"), href: `/m/${encodeURIComponent(m.id)}` })),
    ];
    found = all.filter(i => words.every(w => `${i.label} ${i.hint}`.toLowerCase().includes(w))).slice(0, 12);
    at = 0;
    draw();
  }
  function draw() {
    /** @type {HTMLElement} */ (dlg.querySelector(".pal-list")).innerHTML = found.length ? found.map((i, n) =>
      `<li role="option" id="pal-${n}" class="pal-i" aria-selected="${n === at}" data-n="${n}"><span class="pal-glyph">${i.glyph}</span><span class="pal-l">${esc(i.label)}</span><span class="pal-h">${esc(i.hint)}</span></li>`).join("")
      : `<li class="pal-empty">Nothing matches.</li>`;
    input.setAttribute("aria-activedescendant", found.length ? `pal-${at}` : "");
  }
  function openPalette() { input.value = ""; search(); dlg.showModal(); input.focus(); }
  input.addEventListener("input", search);
  dlg.addEventListener("keydown", e => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); at = found.length ? (at + (e.key === "ArrowDown" ? 1 : -1) + found.length) % found.length : 0; draw(); }
    else if (e.key === "Enter" && found[at]) { e.preventDefault(); location.href = found[at].href; }
  });
  dlg.addEventListener("click", e => {
    const li = e.target instanceof Element ? /** @type {HTMLElement | null} */ (e.target.closest(".pal-i")) : null;
    if (li) location.href = found[Number(li.dataset.n)].href;
    else if (e.target === dlg) dlg.close();
  });
  document.addEventListener("keydown", e => {
    const typing = e.target instanceof Element && !!e.target.closest("input, textarea, dialog[open]");
    if (((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") || (e.key === "/" && !typing)) { e.preventDefault(); openPalette(); }
  });
  /** @type {HTMLElement} */ (document.querySelector("[data-palette]")).onclick = openPalette;
  if (!/Mac|iPhone|iPad/.test(navigator.platform)) $("modk").textContent = "Ctrl K";

  $("theme").onclick = () => {
    const d = document.documentElement, dark = matchMedia("(prefers-color-scheme: dark)").matches;
    d.dataset.theme = (d.dataset.theme || (dark ? "dark" : "light")) === "dark" ? "light" : "dark";
  };

  void load();
  setInterval(load, REFRESH_MS);
})();
