// @ts-check
// Live updates: server-sent events (`refresh = "live"`) or polling (`poll:<s>`). Hands each change to the page
// and tracks the stream's state for the live indicator ("live · 4s ago", or "reconnecting").
(function () {
  "use strict";
  const POLL_FALLBACK_S = 5; // `live` without EventSource
  /** @type {{ mode: string, es?: EventSource, timer?: number, open: boolean, broken: boolean, last: number }} */
  const live = { mode: "", open: false, broken: false, last: 0 };

  /** `live` streams over SSE; `poll:<s>` polls. Reconnects only when the mode changes.
   *  @param {Config} config
   *  @param {{change: (c: Change) => void, reload: () => void, status: () => void}} on */
  function connect(config, on) {
    const m = /^poll:(\d+)$/.exec((config.mission || {}).refresh || "");
    const mode = m ? `poll:${m[1]}` : "live";
    if (mode === live.mode) return;
    if (live.es) live.es.close();
    if (live.timer) clearInterval(live.timer);
    Object.assign(live, { es: undefined, timer: undefined, mode, open: false, broken: false });
    if (mode === "live" && typeof EventSource === "function") {
      const es = new EventSource(`/api/missions/${encodeURIComponent(window.MISSION.id)}/stream`);
      es.addEventListener("change", e => { live.last = Date.now(); on.change(JSON.parse(/** @type {MessageEvent} */ (e).data)); });
      es.addEventListener("reset", () => on.reload());
      es.onerror = () => { live.open = false; live.broken = true; on.status(); };
      es.onopen = () => {
        live.open = true;
        live.last = Date.now();
        if (live.broken) { live.broken = false; on.reload(); } else on.status();
      };
      live.es = es;
    } else {
      live.timer = window.setInterval(() => { live.last = Date.now(); on.reload(); }, (m ? Number(m[1]) : POLL_FALLBACK_S) * 1000);
    }
  }

  /** `live` (stream open), `reconnecting` (stream dropped) or `poll`, and when the page last heard anything. */
  function status() {
    const state = live.mode.startsWith("poll") ? "poll" : live.open ? "live" : live.broken ? "reconnecting" : "connecting";
    return { state, last: live.last };
  }

  /** Marks the page as just refreshed (a full view load). */
  function touch() { live.last = Date.now(); }

  window.MissionLive = { connect, status, touch };
})();
