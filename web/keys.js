// @ts-check
// Keyboard (.design/accessibility.md, "Keyboard"): ⌘K / Ctrl+K and `/` open the palette, `?` the shortcut
// sheet, `g` then a letter jumps to a page, `n` focuses the first open need, j/k walk table rows. Single keys
// never fire while typing, and `[ui] shortcuts = false` turns them off (WCAG 2.1.4); ⌘K / Ctrl+K stays.
(function () {
  "use strict";
  const P = window.MissionPages, esc = window.MissionWidget.h.esc;
  const PREFIX_MS = 1500; // how long `g` waits for its letter
  /** Pages with a letter fixed by the spec; the rest take their title's first free letter. */
  const FIXED = { overview: "o", plan: "p", activity: "a" };
  /** @type {() => Ctx | null} */
  let getCtx = () => null;
  let prefix = 0;

  /** Page id → its `g` letter. @param {Config} config */
  function letters(config) {
    const pages = P.list(config), out = new Map(), used = new Set();
    for (const p of pages) {
      const f = /** @type {Record<string, string>} */ (FIXED)[p.id];
      if (f && !used.has(f)) { out.set(p.id, f); used.add(f); }
    }
    for (const p of pages) {
      if (out.has(p.id)) continue;
      const c = [...p.title.toLowerCase()].find(ch => /[a-z]/.test(ch) && !used.has(ch));
      if (c) { out.set(p.id, c); used.add(c); }
    }
    return out;
  }

  const typing = (/** @type {EventTarget | null} */ t) => t instanceof Element && !!t.closest("input, textarea, select, [contenteditable], dialog[open]");
  const enabled = () => { const c = getCtx(); return !c || (c.config.ui || {}).shortcuts !== false; };

  /** The shortcut sheet. @param {Ctx} ctx */
  function sheet(ctx) {
    const d = /** @type {HTMLDialogElement} */ (document.getElementById("keys"));
    const row = (/** @type {string} */ k, /** @type {string} */ w) => `<tr><td>${k}</td><td>${esc(w)}</td></tr>`;
    const kbd = (/** @type {string} */ s) => s.split(" ").map(x => `<span class="kbd">${esc(x)}</span>`).join(" ");
    const mod = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl";
    const pages = [...letters(ctx.config)].map(([id, l]) => row(kbd(`g ${l}`), `Go to ${(P.list(ctx.config).find(p => p.id === id) || { title: id }).title}`)).join("");
    const on = enabled();
    /** @type {HTMLElement} */ (d.querySelector(".keys-body")).innerHTML = `<table class="keys">${row(kbd(`${mod} K`), "Command palette")}${on ? row(kbd("/"), "Command palette") + row(kbd("?"), "This sheet") + pages
      + row(kbd("n"), "First open question") + row(kbd("j") + " " + kbd("k"), "Next / previous record (graph, table, drawer)") : ""}
      ${row(kbd("1") + "–" + kbd("9"), "Pick that option of the focused question")}${row(kbd("Enter"), "Open the focused record; send an answer")}
      ${row(kbd("+") + " " + kbd("−") + " " + kbd("0") + " " + kbd("f"), "Zoom in, out, fit, fullscreen (focused diagram)")}${row(kbd("Esc"), "Close the drawer, palette or a dialog")}</table>
      ${on ? "" : `<p class="small muted">Single-key shortcuts are off (<code>[ui] shortcuts = false</code>).</p>`}`;
    d.showModal();
    /** @type {HTMLElement | null} */ (d.querySelector("button"))?.focus();
  }

  document.addEventListener("keydown", e => {
    const ctx = getCtx();
    if (!ctx) return;
    if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      return void window.MissionPalette.open();
    }
    // composedPath()[0], not e.target: a text box inside a shadow root (an overlay such as redline's)
    // retargets e.target to its host, which would read as "not typing".
    const src = e.composedPath()[0] ?? e.target;
    if (e.metaKey || e.ctrlKey || e.altKey || typing(src) || !enabled() || e.defaultPrevented) return;
    if (prefix && Date.now() - prefix < PREFIX_MS) {
      prefix = 0;
      const page = [...letters(ctx.config)].find(([, l]) => l === e.key.toLowerCase());
      if (page) { e.preventDefault(); window.MissionPalette.go(page[0]); }
      return;
    }
    prefix = 0;
    const t = src instanceof HTMLElement ? src : null;
    if (e.key === "/") { e.preventDefault(); window.MissionPalette.open(); }
    else if (e.key === "?") { e.preventDefault(); sheet(ctx); }
    else if (e.key === "g") { prefix = Date.now(); }
    else if (e.key === "n") {
      e.preventDefault();
      window.MissionPalette.go(P.OVERVIEW, () => /** @type {HTMLElement | null} */ (document.querySelector("[data-need-card]"))?.focus());
    } else if ((e.key === "j" || e.key === "k") && t && t.matches("tr[data-key]")) {
      e.preventDefault();
      const rows = /** @type {HTMLElement[]} */ ([...(t.closest("tbody") || document).querySelectorAll("tr[data-key]")]);
      rows[rows.indexOf(t) + (e.key === "j" ? 1 : -1)]?.focus();
    }
  });

  window.MissionKeys = { attach(/** @type {() => Ctx | null} */ g) { getCtx = g; }, letters };
})();
