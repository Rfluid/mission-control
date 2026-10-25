// @ts-check
// Mermaid diagrams in markdown (```mermaid fences; markdown.js leaves a figure with the source). The renderer
// (web/vendor, compiled in) loads only when a page has one. It draws with the page's tokens, in strict mode
// (no HTML labels, no click handlers), and its SVG is sanitized again before it goes in: nothing in a block
// runs. A block that doesn't parse shows its source and the error. Re-drawn when the theme changes.
(function () {
  "use strict";
  /** Must match `MERMAID_URL` in src/daemon/assets.rs (a test checks). */
  const SRC = "/assets/vendor/mermaid-11.12.3.js";
  /** @type {Promise<any> | null} */
  let loading = null;
  /** @type {Map<string, {svg?: string, error?: string}>} theme + source → result */
  const cache = new Map();
  let theme = "", seq = 0;

  /** @param {unknown} s */
  const esc = s => String(s ?? "").replace(/[&<>"]/g, c => `&#${c.charCodeAt(0)};`);

  function load() {
    if (!loading) {
      loading = new Promise((resolve, reject) => {
        const el = document.createElement("script");
        el.src = SRC;
        el.onload = () => resolve(/** @type {any} */ (window).mermaid);
        el.onerror = () => { loading = null; reject(new Error("the diagram renderer didn't load")); };
        document.head.appendChild(el);
      });
    }
    return loading;
  }

  /** The page's tokens as Mermaid theme variables (resolved values: Mermaid can't read CSS variables). */
  function vars() {
    const cs = getComputedStyle(document.documentElement), t = (/** @type {string} */ n) => cs.getPropertyValue(`--${n}`).trim();
    return {
      darkMode: isDark(), background: t("panel"), fontFamily: "Inter, system-ui, sans-serif", fontSize: "12px",
      primaryColor: t("sunk"), primaryBorderColor: t("line2"), primaryTextColor: t("ink"), secondaryColor: t("bg"),
      tertiaryColor: t("panel"), lineColor: t("ink3"), textColor: t("ink"), mainBkg: t("sunk"), nodeBorder: t("line2"),
      clusterBkg: t("bg"), clusterBorder: t("line"), titleColor: t("ink"), edgeLabelBackground: t("panel"),
      actorBkg: t("sunk"), actorBorder: t("line2"), actorTextColor: t("ink"), actorLineColor: t("line2"),
      signalColor: t("ink2"), signalTextColor: t("ink2"), labelBoxBkgColor: t("sunk"), labelBoxBorderColor: t("line2"),
      labelTextColor: t("ink"), loopTextColor: t("ink2"), noteBkgColor: t("sunk"), noteBorderColor: t("line2"),
      noteTextColor: t("ink2"), sequenceNumberColor: t("panel"),
    };
  }
  const isDark = () => (document.documentElement.dataset.theme || (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")) === "dark";

  /** Strips anything that could run or fetch from an SVG string. @param {string} svg */
  function clean(svg) {
    const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
    const root = doc.documentElement;
    if (root.nodeName !== "svg") return "";
    for (const el of root.querySelectorAll("script, foreignObject, iframe, object, embed, image, use[href^='http'], a")) el.remove();
    for (const el of [root, ...root.querySelectorAll("*")]) {
      for (const a of [...el.attributes]) {
        const v = a.value.trim().toLowerCase();
        if (a.name.startsWith("on") || ((a.name === "href" || a.name === "xlink:href") && !v.startsWith("#")) || v.startsWith("javascript:")) el.removeAttribute(a.name);
      }
    }
    root.setAttribute("role", "img");
    return new XMLSerializer().serializeToString(root);
  }

  /** @param {any} mermaid @param {string} src */
  async function draw(mermaid, src) {
    const key = `${theme}\n${src}`, hit = cache.get(key);
    if (hit) return hit;
    const id = `mmd-${++seq}`;
    let out;
    try {
      const { svg } = await mermaid.render(id, src);
      out = { svg: clean(svg) };
    } catch (e) {
      out = { error: String(e instanceof Error ? e.message : e).split("\n").slice(0, 4).join(" ") };
    } finally {
      document.getElementById(id)?.remove();
      document.getElementById(`d${id}`)?.remove();
    }
    cache.set(key, out);
    return out;
  }

  /** Draws every figure under `root` not drawn yet (all of them when the theme changed). @param {ParentNode} root */
  async function run(root) {
    const now = `${isDark() ? "dark" : "light"}:${getComputedStyle(document.documentElement).getPropertyValue("--panel").trim()}`;
    const figs = /** @type {HTMLElement[]} */ ([...root.querySelectorAll("figure.mmd")]).filter(f => f.dataset.theme !== now);
    if (!figs.length) return;
    let mermaid;
    try { mermaid = await load(); } catch (e) {
      for (const f of figs) f.dataset.error = String(e instanceof Error ? e.message : e);
      return;
    }
    if (theme !== now) {
      theme = now;
      mermaid.initialize({ startOnLoad: false, securityLevel: "strict", theme: "base", themeVariables: vars(),
        flowchart: { htmlLabels: false, useMaxWidth: true },
        // Compact sequence diagrams: small actor boxes, no mirrored actors under the lifelines.
        sequence: { useMaxWidth: true, mirrorActors: false, width: 110, height: 34, actorMargin: 30, messageMargin: 26, boxMargin: 6,
          actorFontSize: 12, messageFontSize: 12, noteFontSize: 12 } });
    }
    for (const f of figs) {
      const pre = f.querySelector("pre.mmd-src");
      const src = f.dataset.src ?? (pre ? pre.textContent || "" : "");
      f.dataset.src = src;
      const r = await draw(mermaid, src);
      f.innerHTML = r.svg ? r.svg : `<pre class="mmd-src"><code>${esc(src)}</code></pre><div class="mmd-err">Diagram error: ${esc(r.error)}</div>`;
      f.classList.toggle("bad", !r.svg);
      f.dataset.theme = now;
    }
  }

  window.MissionMermaid = { run };
})();
