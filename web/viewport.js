// @ts-check
// Diagram viewport (plan and system graphs): zoom, fit, fullscreen and pan, on any `[data-viewport]` scroller
// holding a `.vcanvas`. Zoom is CSS `zoom` on the canvas, so the scroller's own scrolling navigates it at any
// level. Fullscreen goes on the section (its markup is re-rendered on live updates; the section itself stays),
// and zoom and scroll position survive re-renders.
(function () {
  "use strict";
  const MIN = 0.4, MAX = 2, STEP = 1.2;
  /** @type {Map<string, number>} viewport id → zoom */
  const zooms = new Map();
  /** @type {Map<string, [number, number]>} viewport id → scroll position, across re-renders */
  const scrolls = new Map();

  const clamp = (/** @type {number} */ z) => Math.min(MAX, Math.max(MIN, Math.round(z * 100) / 100));
  /** @param {string} id */
  const zoomOf = id => zooms.get(id) ?? 1;
  /** @param {string} id */
  const scroller = id => /** @type {HTMLElement | null} */ (document.querySelector(`[data-viewport="${CSS.escape(id)}"]`));

  /** Inline style for a canvas: its zoom. @param {string} id */
  const style = id => `zoom:${zoomOf(id)}`;

  /** The controls (rendered by the widget, wired here). @param {string} id */
  function controls(id) {
    const e = (/** @type {string} */ s) => s.replace(/[&<>"]/g, c => `&#${c.charCodeAt(0)};`);
    const v = e(id);
    return `<div class="vctl" role="group" aria-label="Diagram view">
      <button class="btn icon" type="button" data-vz="out" data-v="${v}" aria-label="Zoom out" title="Zoom out (−)">−</button>
      <span class="vlevel mono" aria-live="polite">${Math.round(zoomOf(id) * 100)}%</span>
      <button class="btn icon" type="button" data-vz="in" data-v="${v}" aria-label="Zoom in" title="Zoom in (+)">+</button>
      <button class="btn icon" type="button" data-vz="fit" data-v="${v}" title="Fit (0)">Fit</button>
      <button class="btn icon" type="button" data-vz="full" data-v="${v}" aria-label="Fullscreen" title="Fullscreen (f)"><svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true"><path d="M2 6V2h4M10 2h4v4M14 10v4h-4M6 14H2v-4" fill="none" stroke="currentColor" stroke-width="1.6"/></svg></button></div>`;
  }

  /** Sets a zoom, keeping the point at (cx, cy) in the scroller (default its centre) where it was.
   *  @param {string} id @param {number} z @param {number} [cx] @param {number} [cy] */
  function zoomTo(id, z, cx, cy) {
    const sc = scroller(id), canvas = sc && /** @type {HTMLElement | null} */ (sc.querySelector(".vcanvas"));
    const old = zoomOf(id), next = clamp(z);
    zooms.set(id, next);
    if (!sc || !canvas) return;
    const px = cx ?? sc.clientWidth / 2, py = cy ?? sc.clientHeight / 2;
    const x = (sc.scrollLeft + px) / old, y = (sc.scrollTop + py) / old;
    canvas.style.zoom = String(next);
    sc.scrollLeft = x * next - px;
    sc.scrollTop = y * next - py;
    const level = sc.closest("section")?.querySelector(".vlevel");
    if (level) level.textContent = `${Math.round(next * 100)}%`;
  }

  /** Zoom so the whole canvas fits the scroller (never above 100% outside fullscreen). @param {string} id */
  function fit(id) {
    const sc = scroller(id), canvas = sc && /** @type {HTMLElement | null} */ (sc.querySelector(".vcanvas"));
    if (!sc || !canvas) return;
    const w = canvas.offsetWidth, hgt = canvas.offsetHeight;
    const full = !!document.fullscreenElement || !!sc.closest(".vfull");
    const z = Math.min(sc.clientWidth / w, full ? sc.clientHeight / hgt : Infinity, full ? MAX : 1);
    zoomTo(id, z);
    sc.scrollLeft = 0;
    sc.scrollTop = 0;
  }

  /** Fullscreen on the section; a fixed overlay where the API isn't there. @param {string} id */
  async function full(id) {
    const sec = scroller(id)?.closest("section");
    if (!sec) return;
    if (document.fullscreenElement === sec) return void document.exitFullscreen();
    if (sec.classList.contains("vfull")) return void sec.classList.remove("vfull");
    try { await sec.requestFullscreen(); } catch { sec.classList.add("vfull"); }
    requestAnimationFrame(() => fit(id));
  }

  document.addEventListener("click", e => {
    const b = e.target instanceof Element ? /** @type {HTMLElement | null} */ (e.target.closest("[data-vz]")) : null;
    if (!b) return;
    const id = b.dataset.v || "", what = b.dataset.vz;
    if (what === "in") zoomTo(id, zoomOf(id) * STEP);
    else if (what === "out") zoomTo(id, zoomOf(id) / STEP);
    else if (what === "fit") fit(id);
    else if (what === "full") void full(id);
  });
  document.addEventListener("fullscreenchange", () => {
    if (!document.fullscreenElement) for (const id of zooms.keys()) if (zoomOf(id) > 1) zoomTo(id, 1);
  });

  // Ctrl/⌘ + wheel zooms at the pointer; a plain wheel scrolls as usual.
  document.addEventListener("wheel", e => {
    const sc = e.target instanceof Element ? /** @type {HTMLElement | null} */ (e.target.closest("[data-viewport]")) : null;
    if (!sc || !(e.ctrlKey || e.metaKey)) return;
    e.preventDefault();
    const r = sc.getBoundingClientRect(), id = sc.dataset.viewport || "";
    zoomTo(id, zoomOf(id) * (e.deltaY < 0 ? STEP : 1 / STEP), e.clientX - r.left, e.clientY - r.top);
  }, { passive: false });

  // Drag the background to pan (nodes and buttons keep their clicks).
  /** @type {{sc: HTMLElement, x: number, y: number, l: number, t: number, moved: boolean} | null} */
  let drag = null;
  document.addEventListener("pointerdown", e => {
    const t = e.target instanceof Element ? e.target : null, sc = t && /** @type {HTMLElement | null} */ (t.closest("[data-viewport]"));
    if (!sc || !t || t.closest("a, button, input") || e.button !== 0) return;
    drag = { sc, x: e.clientX, y: e.clientY, l: sc.scrollLeft, t: sc.scrollTop, moved: false };
    sc.classList.add("grabbing");
  });
  document.addEventListener("pointermove", e => {
    if (!drag) return;
    drag.moved = true;
    drag.sc.scrollLeft = drag.l - (e.clientX - drag.x);
    drag.sc.scrollTop = drag.t - (e.clientY - drag.y);
  });
  document.addEventListener("pointerup", () => { if (drag) drag.sc.classList.remove("grabbing"); drag = null; });

  // With focus in a diagram: + / − zoom, 0 fits, f goes fullscreen (arrow keys scroll it natively).
  document.addEventListener("keydown", e => {
    const t = e.target instanceof Element ? e.target : null;
    const sec = t && t.closest("section"), sc = sec && /** @type {HTMLElement | null} */ (sec.querySelector("[data-viewport]"));
    if (!sc || e.metaKey || e.ctrlKey || e.altKey || (t && t.closest("input, textarea"))) return;
    const id = sc.dataset.viewport || "";
    const act = { "+": () => zoomTo(id, zoomOf(id) * STEP), "=": () => zoomTo(id, zoomOf(id) * STEP), "-": () => zoomTo(id, zoomOf(id) / STEP),
      "0": () => fit(id), f: () => void full(id) }[e.key];
    if (act) { e.preventDefault(); act(); }
  });

  /** Before a section re-renders: remember where its viewports were scrolled. @param {Element} el */
  function save(el) {
    for (const sc of el.querySelectorAll("[data-viewport]")) {
      const s = /** @type {HTMLElement} */ (sc);
      scrolls.set(s.dataset.viewport || "", [s.scrollLeft, s.scrollTop]);
    }
  }
  /** After it re-rendered: put them back. @param {Element} el */
  function restore(el) {
    for (const sc of el.querySelectorAll("[data-viewport]")) {
      const s = /** @type {HTMLElement} */ (sc), at = scrolls.get(s.dataset.viewport || "");
      if (at) [s.scrollLeft, s.scrollTop] = at;
    }
  }

  window.MissionViewport = { controls, style, save, restore, zoomTo, fit };
})();
