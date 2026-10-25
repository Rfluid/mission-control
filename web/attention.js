// @ts-check
// Attention signals (.design/components.md, "Attention signals"): the tab title counts open needs, the favicon
// is the beacon with its signal in the run's health tone (and a warn ring while needs are open), and, if this
// browser opted in with the bell, a notification for a new need or a newly blocked record: never for
// progress. Screen readers hear the same two things in a polite live region.
(function () {
  "use strict";
  const SIZE = 64;
  const STORE = "mission-control.notify"; // per browser, for every mission
  let lastIcon = "";
  const base = document.title.replace(/^\(\d+\)\s*/, "");

  /** Token values of a theme, read without repainting (toggle, read, restore in one task).
   *  @param {"light" | "dark"} theme @param {string[]} names */
  function tokens(theme, names) {
    const root = document.documentElement, was = root.dataset.theme;
    root.dataset.theme = theme;
    const cs = getComputedStyle(root), out = Object.fromEntries(names.map(n => [n, cs.getPropertyValue(`--${n}`).trim()]));
    if (was) root.dataset.theme = was; else delete root.dataset.theme;
    return out;
  }

  /** The beacon (as .design/logo/favicon.svg): ink tile, paper brackets, the signal in `tone` (the bright,
   *  dark-theme value, as on the static icon), plus a warn ring when `ring`. @param {string} tone @param {boolean} ring */
  function icon(tone, ring) {
    const key = `${tone}:${ring}`;
    if (key === lastIcon) return;
    lastIcon = key;
    const light = tokens("light", ["ink"]), dark = tokens("dark", ["ink", tone, "warn"]);
    const c = document.createElement("canvas");
    c.width = c.height = SIZE;
    const g = c.getContext("2d");
    if (!g) return;
    g.fillStyle = light.ink;
    g.beginPath(); g.roundRect(0, 0, SIZE, SIZE, 14); g.fill();
    g.strokeStyle = dark.ink; g.lineWidth = 6; g.lineCap = "round"; g.lineJoin = "round";
    g.beginPath(); g.moveTo(22, 17); g.lineTo(16, 17); g.lineTo(16, 47); g.lineTo(22, 47);
    g.moveTo(42, 17); g.lineTo(48, 17); g.lineTo(48, 47); g.lineTo(42, 47); g.stroke();
    if (ring) { g.strokeStyle = dark.warn; g.lineWidth = 3.5; g.beginPath(); g.arc(32, 32, 10.5, 0, Math.PI * 2); g.stroke(); }
    g.fillStyle = dark[tone] || dark.warn;
    g.beginPath(); g.arc(32, 32, ring ? 6 : 9, 0, Math.PI * 2); g.fill();
    let link = /** @type {HTMLLinkElement | null} */ (document.querySelector('link[rel="icon"]'));
    if (!link) { link = document.createElement("link"); link.rel = "icon"; document.head.appendChild(link); }
    link.type = "image/png";
    link.href = c.toDataURL("image/png");
    link.dataset.tone = tone;
    link.dataset.ring = String(ring);
  }

  /** Title and favicon from the current state. @param {Ctx} ctx */
  function update(ctx) {
    if (window.MISSION.snapshot) return;
    const n = window.MissionShell.needs(ctx).open.length, hl = window.MissionShell.health(ctx);
    document.title = n ? `(${n}) ${base}` : base;
    icon(hl.state ? ctx.h.tone(ctx, hl.state) : "warn", n > 0);
    bell();
  }

  const allowed = () => { try { return localStorage.getItem(STORE) === "on"; } catch { return false; } };
  /** The bell's look: on only when this browser opted in and the permission stands. */
  function bell() {
    const b = document.getElementById("bell");
    if (!b) return;
    const on = typeof Notification === "function" && Notification.permission === "granted" && allowed();
    b.setAttribute("aria-pressed", String(on));
    b.setAttribute("aria-label", on ? "Notifications on: new questions and blocks" : "Notify me about new questions and blocks");
    b.title = b.getAttribute("aria-label") || "";
  }

  /** @param {string} text */
  function announce(text) {
    const el = document.getElementById("announce");
    if (el) el.textContent = text;
  }

  /** Compares records before and after a change: new open needs and newly blocked records signal.
   *  @param {Ctx} ctx @param {Rec[]} before @param {Rec[]} after */
  function observe(ctx, before, after) {
    const coll = (ctx.config.strip || {}).needs || "needs";
    const was = new Map(before.map(r => [`${r.collection}/${r.key}`, r]));
    for (const r of after) {
      const old = was.get(`${r.collection}/${r.key}`);
      const isNeed = r.collection === coll;
      if (isNeed && r.state !== "answered" && (!old || old.state === "answered")) {
        const q = String((r.body || {}).question || r.key);
        announce(`New question: ${q}`);
        notify(`Question: ${q}`, ctx.mission.title, () => window.MissionPalette.go("overview", () => /** @type {HTMLElement | null} */ (document.querySelector(`[data-need-card="${CSS.escape(r.key)}"]`))?.focus()), `need-${r.key}`);
      } else if (!isNeed && ctx.h.tone(ctx, r.state) === "block" && (!old || ctx.h.tone(ctx, old.state) !== "block")) {
        const why = String((r.body || {}).reason || (r.body || {}).note || "");
        announce(`${r.key} is blocked${why ? `: ${why}` : ""}`);
        notify(`${r.key} ${ctx.h.title(r)} is blocked`, why || ctx.mission.title, () => window.MissionDrawer.show(`${r.collection}/${r.key}`), `block-${r.collection}-${r.key}`);
      }
    }
  }

  /** A notification, only if this browser opted in and the user isn't looking at the page.
   *  @param {string} title @param {string} body @param {() => void} open @param {string} tag */
  function notify(title, body, open, tag) {
    if (typeof Notification !== "function" || Notification.permission !== "granted" || !allowed()) return;
    if (document.visibilityState === "visible" && document.hasFocus()) return;
    const n = new Notification(title, { body, tag, icon: /** @type {HTMLLinkElement | null} */ (document.querySelector('link[rel="icon"]'))?.href });
    n.onclick = () => { window.focus(); open(); n.close(); };
  }

  // The bell asks once; after that it switches this browser's notifications on and off.
  document.addEventListener("click", async e => {
    if (!(e.target instanceof Element) || !e.target.closest("#bell") || typeof Notification !== "function") return;
    let on = !(Notification.permission === "granted" && allowed());
    if (on && Notification.permission !== "granted") on = (await Notification.requestPermission()) === "granted";
    try { localStorage.setItem(STORE, on ? "on" : "off"); } catch { /* not kept */ }
    bell();
  });

  window.MissionAttention = { update, observe };
})();
