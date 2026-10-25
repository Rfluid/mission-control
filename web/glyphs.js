// @ts-check
// State glyphs (.design/color.md, "Glyphs"): one 12 px SVG per tone, drawn in currentColor, knock-outs in
// --panel. Filled discs are the settled or loud states (block, done, warn); rings are open ones. Always
// aria-hidden: the state word goes next to the glyph, or in a legend.
(function () {
  "use strict";
  const RING = '<circle cx="6" cy="6" r="5.2" fill="none" stroke="currentColor" stroke-width="1.6"/>';
  const DISC = '<circle cx="6" cy="6" r="6" fill="currentColor"/>';
  const KO = 'fill="none" stroke="var(--panel)" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"';
  /** @type {Record<string, string>} */
  const SHAPES = {
    todo: RING,
    prog: `${RING}<path d="M4.8 3.6l3.4 2.4-3.4 2.4z" fill="currentColor"/>`,
    rev: `${RING}<path d="M6 .8a5.2 5.2 0 0 1 0 10.4z" fill="currentColor"/>`,
    block: `${DISC}<path d="M4 4l4 4M8 4l-4 4" ${KO}/>`,
    done: `${DISC}<path d="M3.4 6.2l1.8 1.8 3.4-3.8" ${KO}/>`,
    warn: `${DISC}<path d="M6 3v3.6" ${KO} stroke-width="1.7"/><circle cx="6" cy="8.7" r=".95" fill="var(--panel)"/>`,
  };
  /** Tone order for anything that counts states (phase mix bars, legends): settled first, open last. */
  const ORDER = ["done", "rev", "prog", "warn", "block", "todo"];

  /** The glyph of `tone` (unknown tones draw as todo).
   *  @param {string} tone */
  function glyph(tone) {
    return `<svg class="g" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">${SHAPES[tone] || SHAPES.todo}</svg>`;
  }

  /** A gate diamond: `done` passed, `warn` next, anything else later (--line2 on --panel).
   *  @param {string} tone @param {number} [size] */
  function diamond(tone, size = 16) {
    const [fill, stroke] = tone === "done" || tone === "warn" || tone === "block"
      ? [`var(--${tone}-bg)`, `var(--${tone})`] : ["var(--panel)", "var(--line2)"];
    return `<svg class="gd" width="${size}" height="${size}" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1l7 7-7 7-7-7z" style="fill:${fill};stroke:${stroke}" stroke-width="1.6"/></svg>`;
  }

  window.MissionGlyph = { glyph, diamond, ORDER };
})();
